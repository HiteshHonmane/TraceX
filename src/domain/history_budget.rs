//! Conservative accounting for retained records, not a resident-memory estimate.
use super::log::LogEntry;

/// Interim safety limits. Reaching either limit stops capture without eviction.
#[derive(Clone, Debug)]
pub(crate) struct HistoryBudget {
    max_records: usize,
    max_bytes: usize,
    records: usize,
    bytes: usize,
}

impl Default for HistoryBudget {
    fn default() -> Self {
        Self::new(100_000, 64 * 1024 * 1024)
    }
}

impl HistoryBudget {
    pub(crate) fn new(max_records: usize, max_bytes: usize) -> Self {
        Self {
            max_records,
            max_bytes,
            records: 0,
            bytes: 0,
        }
    }

    fn entry_overhead() -> usize {
        // Cover Vec's initial minimum allocation (four entries) and later
        // geometric capacity growth, without depending on its exact capacity.
        4 * std::mem::size_of::<LogEntry>()
    }

    fn entry_bytes(entry: &LogEntry) -> usize {
        Self::entry_overhead()
            .saturating_add(entry.raw.capacity())
            .saturating_add(entry.message.capacity())
            .saturating_add(entry.timestamp.capacity())
            .saturating_add(entry.tag.as_ref().map_or(0, String::capacity))
    }

    fn fits(&self, additional: usize) -> bool {
        self.records < self.max_records
            && self
                .bytes
                .checked_add(additional)
                .is_some_and(|total| total <= self.max_bytes)
    }

    /// Preflight the parser's maximum retained strings without consuming input.
    pub(crate) fn can_retain_line_parts(&self, length: usize, capacity: usize) -> bool {
        // Parsed message and tag each fit within the raw input length. Current
        // application timestamps use 12 ASCII bytes; reserve 64 for formatting.
        let upper = capacity
            .saturating_add(length.saturating_mul(2))
            .saturating_add(64)
            .saturating_add(Self::entry_overhead());
        self.fits(upper)
    }

    pub(crate) fn can_retain_entry(&self, entry: &LogEntry) -> bool {
        self.fits(Self::entry_bytes(entry))
    }

    /// Call after admission; accounting never wraps even for invalid callers.
    pub(crate) fn record_entry(&mut self, entry: &LogEntry) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(Self::entry_bytes(entry));
    }

    pub(crate) fn reset(&mut self) {
        self.records = 0;
        self.bytes = 0;
    }

    pub(crate) fn rebuild(&mut self, entries: &[LogEntry]) {
        self.reset();
        for entry in entries {
            self.record_entry(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_byte_limit_accepts_one_entry_and_rejects_the_next() {
        let entry = LogEntry::new(0, "INFO test".into(), "12:00:00.000");
        let mut budget = HistoryBudget::new(10, HistoryBudget::entry_bytes(&entry));
        assert!(budget.can_retain_entry(&entry));
        budget.record_entry(&entry);
        assert!(!budget.can_retain_entry(&entry));
        budget.reset();
        assert!(budget.can_retain_entry(&entry));
    }

    #[test]
    fn record_limit_is_independent_of_available_bytes() {
        let entry = LogEntry::new(0, String::new(), "");
        let mut budget = HistoryBudget::new(1, usize::MAX);
        budget.record_entry(&entry);
        assert!(!budget.can_retain_line_parts(0, 0));
    }

    #[test]
    fn parser_preflight_covers_unicode_ansi_and_tags() {
        for line in [
            "",
            "hello 世界",
            "\u{1b}[31mERROR\u{1b}[0m",
            "[00][MODEM] INFO connected",
        ] {
            let raw = line.to_owned();
            let upper = raw.capacity() + raw.len() * 2 + 64 + HistoryBudget::entry_overhead();
            let budget = HistoryBudget::new(1, upper);
            assert!(budget.can_retain_line_parts(raw.len(), raw.capacity()));
            let entry = LogEntry::new(0, raw, "12:00:00.000");
            assert!(budget.can_retain_entry(&entry));
        }
    }

    #[test]
    fn rebuild_matches_incremental_accounting_and_overflow_is_refused() {
        let entries = vec![
            LogEntry::new(0, "a".into(), "t"),
            LogEntry::new(1, "b".into(), "t"),
        ];
        let mut budget = HistoryBudget::new(2, 10_000);
        budget.rebuild(&entries);
        assert!(!budget.can_retain_entry(&entries[0]));
        assert!(!HistoryBudget::default().can_retain_line_parts(usize::MAX, usize::MAX));
    }
}
