/// Matches `contracts::API_VERSION` (rust_core/contracts/src/lib.rs).
/// Compared exactly against `/health`'s `api_version` — any mismatch is a
/// hard incompatibility, not something to warn-and-continue past (U3
/// requirement 5).
const String kSupportedApiVersion = 'v1';
