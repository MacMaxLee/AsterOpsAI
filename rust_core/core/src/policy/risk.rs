//! Risk classification (SRS FR-POL-001/002). SRS pins only the two end
//! poles ("INFORMATIONAL through PROHIBITED") — the three levels in
//! between are a documented judgment call, same spirit as ADR 0006/0010's
//! own threshold judgment calls. FR-POL-001's exhaustiveness requirement
//! ("an action type with no assigned risk cannot exist in a shipped
//! build") is enforced structurally: `ActionKind::risk_level` is a
//! required, non-defaulted trait method, so omitting it is a compile
//! error, not a runtime gap.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Informational,
    Low,
    Medium,
    High,
    /// Closed-enum guarantee: `decide` never returns anything but `Deny`
    /// for this level, in any environment — not a runtime check a future
    /// edit could accidentally loosen.
    Prohibited,
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Informational => "INFORMATIONAL",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Prohibited => "PROHIBITED",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "INFORMATIONAL" => Some(Self::Informational),
            "LOW" => Some(Self::Low),
            "MEDIUM" => Some(Self::Medium),
            "HIGH" => Some(Self::High),
            "PROHIBITED" => Some(Self::Prohibited),
            _ => None,
        }
    }
}

/// Distinct from, and structurally unrelated to, `dbms::connection_metadata
/// ::Environment` — that one is a field of `ConnectionMetadata` (a single
/// PostgreSQL connection's own setting); this one governs host-level
/// action policy generally and has no PG-connection meaning at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

/// Ordered by permissiveness — `AutoAllow > RequireApproval > Deny` — so
/// FR-POL-004's "production is never more permissive than development for
/// the same action type" can be checked as a plain `<=` comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyDecision {
    Deny,
    RequireApproval,
    AutoAllow,
}

/// Pure function over `(risk, environment)` (SRS FR-POL-002). `Prohibited`
/// is always `Deny`, in every environment. `Informational` is always
/// `AutoAllow`. `Low` auto-allows in Development/Staging but requires
/// approval in Production — the one place environment changes the outcome
/// for a fixed risk level, and it only ever makes Production *less*
/// permissive, never more (FR-POL-004, exhaustively tested in
/// `policy_risk_decision_test.rs`). `Medium`/`High` always require
/// approval regardless of environment.
pub fn decide(risk: RiskLevel, environment: Environment) -> PolicyDecision {
    match risk {
        RiskLevel::Prohibited => PolicyDecision::Deny,
        RiskLevel::Informational => PolicyDecision::AutoAllow,
        RiskLevel::Low => match environment {
            Environment::Production => PolicyDecision::RequireApproval,
            Environment::Development | Environment::Staging => PolicyDecision::AutoAllow,
        },
        RiskLevel::Medium | RiskLevel::High => PolicyDecision::RequireApproval,
    }
}
