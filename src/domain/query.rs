use super::log::{LogEntry, LogLevel};

/// Severity selection shared by every query consumer. Other levels stay visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeveritySelection {
    pub(crate) info: bool,
    pub(crate) warn: bool,
    pub(crate) error: bool,
}

/// Secondary results can invert text matching without changing the main view.
#[derive(Clone, Copy)]
pub(crate) enum QueryScope {
    Main,
    Include,
    Exclude,
}

/// A trimmed, Unicode-lowercased substring query against raw or parsed text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LogQuery {
    text: String,
    severity: SeveritySelection,
}

impl LogQuery {
    pub(crate) fn new(text: &str, severity: SeveritySelection) -> Self {
        Self {
            text: text.trim().to_lowercase(),
            severity,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(crate) fn matches(&self, entry: &LogEntry, scope: QueryScope) -> bool {
        let visible = match entry.level {
            LogLevel::Info => self.severity.info,
            LogLevel::Warn => self.severity.warn,
            LogLevel::Error => self.severity.error,
            _ => true,
        };
        if !visible {
            return false;
        }
        if self.is_empty() {
            return matches!(scope, QueryScope::Main);
        }
        let contains = entry.raw.to_lowercase().contains(&self.text)
            || entry.message.to_lowercase().contains(&self.text);
        match scope {
            QueryScope::Main | QueryScope::Include => contains,
            QueryScope::Exclude => !contains,
        }
    }

    pub(crate) fn indices<'a>(
        &self,
        entries: impl IntoIterator<Item = &'a LogEntry>,
        scope: QueryScope,
    ) -> Vec<usize> {
        entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| self.matches(entry, scope).then_some(index))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str) -> LogQuery {
        LogQuery::new(
            text,
            SeveritySelection {
                info: true,
                warn: false,
                error: true,
            },
        )
    }

    #[test]
    fn combines_trimmed_raw_or_message_matching_with_severity() {
        let mut raw = LogEntry::new(1, "RAW-TOKEN".into(), "now");
        raw.message = "parsed body".into();
        raw.level = LogLevel::Info;
        let mut hidden = raw.clone();
        hidden.level = LogLevel::Warn;
        let mut parsed = raw.clone();
        parsed.raw = "other".into();
        parsed.message = "raw-token".into();
        let entries = [raw, hidden, parsed];
        assert_eq!(
            query("  rAw-ToKeN \n").indices(&entries, QueryScope::Main),
            [0, 2]
        );
        assert_eq!(
            query("  rAw-ToKeN \n").indices(&entries, QueryScope::Include),
            [0, 2]
        );
        assert!(
            query("raw-token")
                .indices(&entries, QueryScope::Exclude)
                .is_empty()
        );
    }

    #[test]
    fn secondary_exclusion_preserves_severity_and_empty_query_is_empty() {
        let mut entry = LogEntry::new(1, "unrelated".into(), "now");
        entry.level = LogLevel::Error;
        assert!(query("needle").matches(&entry, QueryScope::Exclude));
        assert!(!query("needle").matches(&entry, QueryScope::Main));
        assert!(query(" \t").matches(&entry, QueryScope::Main));
        assert!(!query(" \t").matches(&entry, QueryScope::Include));
        assert!(!query(" \t").matches(&entry, QueryScope::Exclude));
        entry.level = LogLevel::Warn;
        assert!(!query("needle").matches(&entry, QueryScope::Exclude));
    }
}
