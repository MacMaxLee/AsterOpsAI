//! What a real `ActionTypeEntry::construct` needs and a bare `fn` pointer
//! can't capture (unit U8) — a shared handle to the live `PlatformAdapter`.
//! Extensible: a later unit adding DB-side actions would add a pool/
//! adapter handle here too, not invent a second context type.

use std::sync::Arc;

use platform::PlatformAdapter;

#[derive(Clone)]
pub struct ActionContext {
    pub platform: Arc<dyn PlatformAdapter>,
}
