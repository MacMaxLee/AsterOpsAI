//! Confirms every schema-bearing contract type actually produces a schema.
//! This does not replace the CI schema-drift gate (`scripts/check-schema-drift.sh`),
//! which additionally checks the emitted files match what's committed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use contracts::{ApiError, Capability, Envelope, HealthResponse};
use schemars::schema_for;

#[test]
fn every_contract_type_produces_a_schema() {
    let health = schema_for!(HealthResponse);
    let enveloped = schema_for!(Envelope<HealthResponse>);
    let error = schema_for!(ApiError);
    let capability = schema_for!(Capability);

    assert!(health.schema.object.is_some());
    assert!(enveloped.schema.object.is_some());
    assert!(error.schema.subschemas.is_some() || error.schema.object.is_some());
    assert!(capability.schema.subschemas.is_some() || capability.schema.object.is_some());
}
