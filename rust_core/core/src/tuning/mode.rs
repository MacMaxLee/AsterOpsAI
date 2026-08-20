/// SRS FR-TUNE-001. Governs how far `plan::start_plan` goes on its own
/// once candidates are built (docs/adr/0015 has the full scope note for
/// each variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationMode {
    /// Computes candidates and stops — never touches `policy::evaluate`,
    /// so nothing is proposed or audited. A pure suggestion.
    RecommendOnly,
    /// Calls `policy::evaluate` for real (a real proposed/audited row
    /// exists per candidate) but never auto-authorizes any of them,
    /// regardless of what the policy decision would have allowed — the
    /// actual "ask" is a future console/service unit's job.
    AskBeforeChanges,
    /// `AskBeforeChanges`'s behavior, except a candidate that is
    /// `AutoAllowed` *and* whose action type is `reversible` *and* has a
    /// recorded improved-outcome history (FR-TUNE-003) is authorized and
    /// executed automatically instead of being left pending.
    AutoLowRisk,
}

pub fn mode_label(mode: AutomationMode) -> &'static str {
    match mode {
        AutomationMode::RecommendOnly => "RECOMMEND_ONLY",
        AutomationMode::AskBeforeChanges => "ASK_BEFORE_CHANGES",
        AutomationMode::AutoLowRisk => "AUTO_LOW_RISK",
    }
}
