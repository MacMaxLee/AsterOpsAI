-- Unit U28's real, deliberate gap (ADR 0033), closed for real by unit
-- U30: `rollback_by_row_id` needs the real previous_state a real
-- rollback restores to, but V4/V9 never gave it anywhere durable to
-- live. Nullable: existing rows, and any row executed before this
-- migration, simply won't have one.
ALTER TABLE actions ADD COLUMN previous_state_json TEXT;
