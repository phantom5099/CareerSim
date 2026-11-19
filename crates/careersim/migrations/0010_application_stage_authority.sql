ALTER TABLE applications
    ADD COLUMN stage_position integer NOT NULL DEFAULT 0,
    ADD CONSTRAINT applications_stage_position_nonnegative CHECK (stage_position >= 0);

ALTER TABLE sessions
    DROP CONSTRAINT sessions_stage_position_nonnegative,
    DROP COLUMN stage_position,
    DROP COLUMN stage;
