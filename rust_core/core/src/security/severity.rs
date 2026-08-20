//! Severity (SRS FR-SEC-002: "INFO..CRITICAL"). SRS pins only the two end
//! poles — the three levels in between are a documented judgment call,
//! same precedent `policy::RiskLevel` already established for its own
//! SRS-pinned "INFORMATIONAL through PROHIBITED" endpoints.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "INFO" => Some(Self::Info),
            "LOW" => Some(Self::Low),
            "MEDIUM" => Some(Self::Medium),
            "HIGH" => Some(Self::High),
            "CRITICAL" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_round_trips_through_its_string() {
        for severity in [
            Severity::Info,
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ] {
            assert_eq!(Severity::parse(severity.as_str()), Some(severity));
        }
    }

    #[test]
    fn unknown_string_parses_to_none() {
        assert_eq!(Severity::parse("WAT"), None);
    }
}
