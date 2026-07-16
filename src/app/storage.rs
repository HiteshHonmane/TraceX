use chrono::{DateTime, Local};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

pub(super) fn timestamped_log_path(
    base: &Path,
    timestamp: DateTime<Local>,
    sequence: u32,
) -> PathBuf {
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let stem = base
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("HI-Logger");
    let extension = base
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("log");
    let sequence_suffix = if sequence == 0 {
        String::new()
    } else {
        format!("_{sequence:03}")
    };
    parent.join(format!(
        "{stem}_{}{sequence_suffix}.{extension}",
        timestamp.format("%Y-%m-%d_%H-%M-%S")
    ))
}

pub(super) fn create_timestamped_log_file(
    base: &Path,
    timestamp: DateTime<Local>,
    starting_sequence: u32,
) -> std::io::Result<(std::fs::File, PathBuf, u32)> {
    let mut sequence = starting_sequence;
    loop {
        let path = timestamped_log_path(base, timestamp, sequence);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path, sequence.saturating_add(1))),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                sequence = sequence.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn should_rotate_log(current_bytes: u64, incoming_bytes: usize, limit_mb: u64) -> bool {
    let limit_bytes = limit_mb.saturating_mul(1024 * 1024);
    limit_bytes > 0
        && current_bytes > 0
        && current_bytes.saturating_add(incoming_bytes as u64) > limit_bytes
}
