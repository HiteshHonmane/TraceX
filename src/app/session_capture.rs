//! Connection and ordered capture polling for one session runtime.
use super::capture;
use super::session::SessionRuntime;
use super::workspace::ConnectionId;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

impl SessionRuntime {
    pub(super) fn connection_ready(&mut self) -> bool {
        if self.capture.is_some()
            || self.auto_save.stopping
            || self.shutdown.active()
            || self.session_archive_busy()
            || self.history_limit_reached
        {
            self.operation_error =
                Some("Wait for the previous connection to finish disconnecting.".to_owned());
            return false;
        }
        if self.selected_port.is_empty() {
            self.operation_error = Some("Select a serial port before connecting.".to_owned());
            return false;
        }
        if self.next_connection_id.is_none() {
            self.operation_error = Some(
                "Connection identifiers exhausted; save this session and restart TraceX.".into(),
            );
            return false;
        }
        true
    }

    pub(super) fn connect(&mut self) -> bool {
        if !self.connection_ready() {
            return false;
        }

        let port = match serialport::new(&self.selected_port, self.baud_rate)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(p) => p,
            Err(error) => {
                self.operation_error = Some(format!("Could not connect to serial port: {error}"));
                return false;
            }
        };
        self.start_capture(port)
    }

    /// Publish identity/endpoint only after the real capture worker starts.
    pub(super) fn start_capture(&mut self, port: impl Read + Write + Send + 'static) -> bool {
        if !self.connection_ready() {
            return false;
        }
        let Some(generation) = self.next_connection_id else {
            return false;
        };
        let stop = Arc::new(Mutex::new(false));
        let (inbox, commands) = match capture::start_reader(
            port,
            capture::CaptureBudget::default(),
            Arc::clone(&stop),
            self.next_capture_batch,
        ) {
            Ok(reader) => reader,
            Err(error) => {
                self.operation_error = Some(format!("Could not start capture: {error}"));
                return false;
            }
        };
        self.capture = Some(inbox);
        self.serial_tx = Some(commands);
        self.stop_flag = Some(stop);
        self.capture_error = None;
        self.connection_id = Some(ConnectionId(generation));
        self.next_connection_id = generation.checked_add(1);
        self.occupied_endpoint = Some(self.selected_port.clone());

        self.connected = true;
        self.operation_error = None;
        self.last_connected_port = Some(self.selected_port.clone());

        if self.auto_save_enabled && !self.auto_save.active {
            self.open_new_auto_save_file();
        }
        true
    }

    pub(super) fn disconnect(&mut self) {
        if let Some(flag) = &self.stop_flag
            && let Ok(mut f) = flag.lock()
        {
            *f = true;
        }
        self.connected = false;
        // Retain incoming logs and autosave until the worker closes its channel.
        self.serial_tx = None;
        // Clear last_connected_port on manual disconnect to prevent auto-reconnect
        self.last_connected_port = None;
    }

    #[cfg(test)]
    pub(super) fn poll(&mut self) {
        self.poll_with_budget(super::workspace::FRAME_RECORD_BUDGET);
    }

    pub(super) fn poll_with_budget(&mut self, record_budget: usize) -> usize {
        let mut admitted = 0;
        self.poll_session_archive();
        self.poll_auto_save();
        if self.auto_save_enabled && self.auto_save_error.is_some() && self.capture.is_some() {
            self.disconnect();
        }
        self.poll_capture_status();
        // A record remains in its bounded batch until history and storage accept it.
        for _ in 0..record_budget {
            if !self.can_ingest_log() || self.history_limit_reached || self.session_archive_busy() {
                break;
            }
            let shape = self.capture.as_mut().and_then(|capture| {
                capture
                    .peek_record()
                    .map(|line| (line.len(), line.capacity()))
            });
            if let Some((length, capacity)) = shape {
                if self.next_id == usize::MAX {
                    self.capture_error = Some("Capture stopped because record identifiers were exhausted. Pending received data remains retained; save this session before closing.".to_owned());
                    self.history_limit_reached = true;
                    self.disconnect();
                    break;
                }
                if !self.can_retain_line_parts(length, capacity) {
                    self.history_limit_reached = true;
                    self.capture_error = Some("Capture stopped because retained history reached its safety limit. Use Save Session to preserve all retained records, then clear to process pending received data. Save the newly drained records separately. Filtered text export may omit records. Additional device output may be missing.".to_owned());
                    self.disconnect();
                    break;
                }
                if let Some(record) = self
                    .capture
                    .as_mut()
                    .and_then(capture::CaptureInbox::pop_record)
                {
                    self.record_log_line(record);
                    admitted += 1;
                }
            } else {
                // Filling an empty inbox can discover sender loss or publish a
                // terminal tail. Observe that status before dropping ownership.
                self.poll_capture_status();
                if self
                    .capture
                    .as_ref()
                    .is_some_and(capture::CaptureInbox::is_drained)
                {
                    self.handle_disconnection();
                }
                break;
            }
        }
        self.poll_capture_status();

        if self.rate_timer.elapsed() >= Duration::from_secs(1) {
            self.rate = self.rate_count as f32;
            self.rate_count = 0;
            self.rate_timer = Instant::now();
        }
        admitted
    }

    pub(super) fn handle_disconnection(&mut self) {
        // Clean up connection state but keep last_connected_port for auto-reconnect
        self.connected = false;
        self.capture = None;
        self.serial_tx = None;
        self.stop_flag = None;
        self.occupied_endpoint = None;
        self.connection_id = None;
        self.stop_auto_save();
        // Reset reconnect cooldown
        self.reconnect_cooldown = Instant::now();
    }

    pub(super) fn reconnect_eligible(
        &self,
        now: Instant,
        available_ports: &[(String, String)],
    ) -> bool {
        // Auto-reconnect if we were connected and the same port is available again
        // Add a small cooldown to avoid rapid reconnection attempts
        if !self.connected
            && self.capture.is_none()
            && !self.shutdown.active()
            && !self.history_limit_reached
            && !self.session_archive_busy()
            && self.auto_save_error.is_none()
            && !self.auto_save.stopping
            && now.saturating_duration_since(self.reconnect_cooldown) >= Duration::from_millis(500)
            && let Some(ref last_port) = self.last_connected_port
        {
            // Check if the port is available
            let port_available = available_ports
                .iter()
                .any(|(_, port_name)| port_name == last_port);

            if port_available && self.selected_port == *last_port {
                return true;
            }
        }
        false
    }

    pub(super) fn poll_capture_status(&mut self) {
        let Some(inbox) = &mut self.capture else {
            return;
        };
        inbox.poll_status();
        if let Some(sequence) = inbox.current_batch_id() {
            self.next_capture_batch = self.next_capture_batch.max(sequence.saturating_add(1));
        }
        if let Some(reason) = inbox.terminal_reason() {
            self.next_capture_batch = self.next_capture_batch.max(inbox.next_batch_id());
            if reason.requires_attention() && self.capture_error.is_none() {
                let usage = inbox.usage();
                self.capture_error = Some(format!(
                    "{} Received {} bytes; {} records processed; {} accounted backlog bytes; {} host batches rejected from the inbox and retained separately.",
                    reason.message(),
                    inbox
                        .received_bytes()
                        .map_or_else(|| "unknown".to_owned(), |bytes| bytes.to_string()),
                    inbox.processed_records(),
                    usage.bytes,
                    inbox.rejected_batches()
                ));
            }
            if matches!(
                reason,
                capture::CaptureStopReason::Saturated
                    | capture::CaptureStopReason::WorkerEndedUnexpectedly
                    | capture::CaptureStopReason::SequenceExhausted
                    | capture::CaptureStopReason::ConsumerGone
            ) {
                self.last_connected_port = None;
            }
            self.connected = false;
            self.serial_tx = None;
        }
    }
}
