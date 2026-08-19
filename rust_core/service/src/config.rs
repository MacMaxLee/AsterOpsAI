//! Intentionally minimal in unit U0 — no configuration surface exists yet
//! beyond the transport's socket/pipe path, which is resolved directly in
//! `transport::unix`/`transport::windows`. Real configuration (poll
//! intervals, DBMS connections, etc.) is added by the units that need it.
