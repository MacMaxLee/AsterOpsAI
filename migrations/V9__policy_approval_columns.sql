-- Unit U7's policy/approval lifecycle reuses V4's `actions` table rather
-- than inventing a parallel "approvals" table: one row is one full action
-- lifecycle instance, from proposal through approval/denial/expiry to
-- execution/rollback, with `status` transitioning forward-only. These five
-- columns are what TRS §25's approval binding (action_id, target_identity,
-- parameters_hash) plus a full audit trail need that V4 didn't yet have.
ALTER TABLE actions ADD COLUMN requested_by TEXT NOT NULL DEFAULT '';
ALTER TABLE actions ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE actions ADD COLUMN parameters_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE actions ADD COLUMN resource_descriptor_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE actions ADD COLUMN approval_expires_at TEXT;
