ALTER TABLE sessions
    ADD COLUMN stage_position integer NOT NULL DEFAULT 0,
    ADD CONSTRAINT sessions_stage_position_nonnegative CHECK (stage_position >= 0);

UPDATE sessions s
SET stage_position = COALESCE(
    (
        SELECT COUNT(*)::integer
        FROM session_events e
        WHERE e.session_id = s.id
          AND e.kind->>'type' = 'stage_completed'
          AND e.kind->'outcome'->>'type' = 'advance'
    ),
    0
);

CREATE TABLE applications (
    id             uuid PRIMARY KEY,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    job_id         uuid NOT NULL REFERENCES jobs(id) ON DELETE RESTRICT,
    resume_id      uuid NOT NULL REFERENCES resumes(id) ON DELETE RESTRICT,
    session_id     uuid NOT NULL UNIQUE REFERENCES sessions(id) ON DELETE CASCADE,
    status         text NOT NULL DEFAULT 'active',
    stage_position smallint NOT NULL DEFAULT 0,
    created_at     timestamptz NOT NULL,
    updated_at     timestamptz NOT NULL,
    CONSTRAINT applications_status CHECK (
        status IN ('active', 'rejected', 'offer', 'withdrawn')
    ),
    CONSTRAINT applications_stage_position_nonnegative CHECK (stage_position >= 0)
);

CREATE UNIQUE INDEX applications_user_job_uq
    ON applications (user_id, job_id);
CREATE INDEX applications_user_updated_idx
    ON applications (user_id, updated_at DESC, id DESC);
CREATE INDEX applications_session_idx
    ON applications (session_id);
