ALTER TABLE sessions
    ALTER COLUMN stage_position TYPE integer;

ALTER TABLE applications
    DROP CONSTRAINT applications_stage_position_nonnegative,
    DROP COLUMN stage_position;

DROP INDEX applications_session_idx;
