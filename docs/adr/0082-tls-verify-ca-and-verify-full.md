# 0082 — TLS `verify-ca`/`verify-full` Modes

## Status

Accepted (unit U77).

## Context

ADR 0009's own `TlsMode` doc comment named this gap explicitly rather
than silently accepting it: *"`verify-ca`/`verify-full` need a CA
bundle path plus a certificate-verifying TLS connector wired in on top
of this ... real, separate work with its own config surface ... not
something to fake by adding variants this code can't actually honor."*
Confirmed directly: `TlsMode` had exactly three variants
(`Disable`/`Prefer`/`Require`), and `pool::build_pool`'s `Prefer`/
`Require` branch used `EncryptOnlyVerifier` — real TLS 1.2/1.3
handshake-signature verification, but chain/hostname validation
explicitly skipped (`sslmode=require`'s own documented, weaker
guarantee).

Confirmed with the user before starting: unlike the last several units
this session, this one is genuine security-sensitive crypto code (real
certificate-chain and hostname verification, a new dependency), not a
mechanical extraction — flagged as such and built carefully, with a
real self-signed-cert/real-TLS-handshake test for every claim rather
than any mocked verifier behavior.

## Decision

**`TlsMode::VerifyCa`/`VerifyFull`, both requiring `ConnectionMetadata::
ca_bundle_path`** (a new `Option<PathBuf>` field, mirroring `libpq`'s
own `sslrootcert`). `pool::build_pool` refuses to even attempt a
connection — a real, explicit `DbmsError::Other`, never a silent
downgrade — when either mode is requested without a bundle path, or
when the bundle path is unreadable, unparsable, or contains zero
certificates.

**`VerifyFull`** uses `rustls::ClientConfig::builder().
with_root_certificates(roots)` directly — the same standard, safe
webpki-based verification path any correct TLS client uses: chain
trust *and* hostname, both real.

**`VerifyCa`** wraps the same underlying `WebPkiServerVerifier` in a
new `ChainOnlyVerifier` that runs full chain validation unmodified,
but converts *only* a `CertificateError::NotValidForName`/
`NotValidForNameContext` result to success — matching `libpq`'s own
documented `verify-ca` semantics exactly ("verify the server
certificate is trusted, but don't verify its name matches the host").
Every other verification failure (expired, unknown issuer, revoked,
malformed, wrong CA) still propagates as a real, unmodified failure —
this is a narrow, specific tolerance, not "accept anything with a
valid-looking chain."

**Dependency: `rustls-pki-types` directly, not `rustls-pemfile`.**
`rustls-pemfile` was the obvious first choice (the conventional
PEM-parsing complement to `rustls`) and was added first — but `cargo
deny check` caught it as unmaintained (RUSTSEC-2025-0134, archived
August 2025) before this unit shipped. That advisory's own stated fix
is exactly what this unit does instead: depend on `rustls-pki-types`
directly and use its `PemObject::pem_file_iter`, which
`rustls-pemfile` had become a thin wrapper around anyway. No
functional difference in `load_root_cert_store`'s own behavior, just a
dependency swap caught by the CI gate doing its job.

**A real, empirically-discovered gotcha, not assumed correct from
reading rustls's docs alone**: `openssl req -x509`'s own default
self-signed-cert generation sets `basicConstraints=CA:TRUE` (a legacy
compatibility default) — `rustls-webpki`'s real chain validation
unconditionally rejects any end-entity/leaf certificate claiming to be
a CA (`Error::CaUsedAsEndEntity`), which broke every new test against
`TestPostgres::start_with_tls`'s existing self-signed cert. Fixed by
adding `-addext "basicConstraints=critical,CA:FALSE"` to the harness's
own cert generation, making it a genuine leaf cert — matching what a
real deployment's server certificate would actually look like. A
second, related discovery: `rustls-webpki` never falls back to the
legacy `CN` field for hostname matching (only real `subjectAltName`
entries), so a `-addext "subjectAltName=DNS:localhost"` was also
needed for any positive (both-sides-match) verify-full test to be
possible at all.

## Verification (real, not simulated)

- `core/tests/dbms_tls_require_test.rs` (8 new tests, all against a
  real PostgreSQL instance with a real self-signed cert, none mocked):
  missing/nonexistent/empty CA bundle paths are real, explicit errors
  before any connection is attempted; `verify-full` succeeds when CA
  and hostname genuinely both match (`localhost` against a cert with a
  real `DNS:localhost` SAN) and fails on a genuine hostname mismatch
  (`127.0.0.1` against that same cert); `verify-ca` succeeds despite
  that exact same mismatch (the central distinguishing behavior this
  unit exists for); `verify-ca` still rejects a real server certificate
  that doesn't chain to a genuinely unrelated CA (proving it isn't
  simply "accept anything"). Reran 3× to rule out TLS-handshake timing
  flakiness.
- `service/src/dbms_config.rs` (6 new unit tests): `verify-ca`/
  `verify-full` sslmode strings parse correctly; `ca_bundle_path`
  resolves from the new 9th config parameter and treats an empty
  string as unset, matching every other field's own convention.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `cargo deny check` clean (advisories/bans/licenses/sources all
  `ok` — the `rustls-pemfile` advisory this unit itself triggered and
  then fixed). Both grep gates and the AI-reachability gate pass
  (unaffected).
- `docs/traceability-matrix.md` regenerated: no FR-ID drift (TRS §17/18
  aren't tracked FR-IDs).

## Consequences

- ADR 0009's own named gap is closed — `TlsMode` now covers all five
  `libpq`-meaningful modes this codebase's `tokio_postgres`/
  `deadpool_postgres` dependency versions can express (the full
  six-mode set also has `allow`, which neither of those crates'
  `SslMode` enums support either).
- `$ASTEROPS_DB_SSLMODE=verify-ca`/`verify-full` plus
  `$ASTEROPS_DB_CA_BUNDLE_PATH` are now real, usable configuration for
  the `service` binary's own DB connection, not just `core`-level
  plumbing nothing surfaces.
- `TestPostgres::start_with_tls`'s cert now has a real `CA:FALSE`
  basicConstraints and a `DNS:localhost` SAN — a more realistic
  fixture for any future TLS-related test in this harness, not just
  this unit's own.
