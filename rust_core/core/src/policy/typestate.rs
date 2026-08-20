//! TRS §24's typestate chain: `ActionRequest -> Validated<ActionRequest> ->
//! PolicyApproved -> Executed` (the last stage lives in `core::actions`,
//! since the executor produces it). Each stage's inner state is private,
//! constructible only from inside `core::policy` (`pub(in crate::policy)`
//! constructors) — a policy bypass is a compile error: nothing outside
//! this module can build a `Validated<T>` or a `PolicyApproved` by hand,
//! only by actually calling `validate()`/`approval::authorize()`.

use std::sync::Arc;

use crate::actions::ActionKind;

use super::resource::ResourceDescriptor;
use super::target::TargetIdentity;

#[derive(Debug, Clone)]
pub struct Validated<T> {
    inner: T,
}

impl<T> Validated<T> {
    pub(in crate::policy) fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn get(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// The executor's only public entry point accepts this type (TRS §24) —
/// there is no public way to build one outside `core::policy`.
pub struct PolicyApproved {
    action: Arc<dyn ActionKind>,
    row_id: i64,
    target: TargetIdentity,
    resource: ResourceDescriptor,
}

impl PolicyApproved {
    pub(in crate::policy) fn new(
        action: Arc<dyn ActionKind>,
        row_id: i64,
        target: TargetIdentity,
        resource: ResourceDescriptor,
    ) -> Self {
        Self {
            action,
            row_id,
            target,
            resource,
        }
    }

    pub fn action(&self) -> &dyn ActionKind {
        self.action.as_ref()
    }

    /// A cheap, cloned handle to the same underlying action — lets the
    /// executor own an `Arc<dyn ActionKind>` (to carry into its own
    /// `Executed` result type) without consuming `self` by value.
    pub fn action_arc(&self) -> Arc<dyn ActionKind> {
        Arc::clone(&self.action)
    }

    pub fn row_id(&self) -> i64 {
        self.row_id
    }

    pub fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub fn resource(&self) -> &ResourceDescriptor {
        &self.resource
    }
}
