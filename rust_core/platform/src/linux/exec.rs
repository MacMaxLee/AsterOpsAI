//! The only module (with its `windows`/`macos` siblings) permitted to call
//! `std::process::Command` — enforced by
//! `scripts/check-no-command-outside-exec.sh`. Empty until unit U8 adds a
//! narrow, typed action-execution surface; established now so later units
//! have a pre-gated location to add to rather than needing a CI change.
