-- 0007 以前的会话没有岗位与流程关联，无法可靠映射到 position。
-- 这些旧会话及事件按本次升级策略清理，新的投递流程从 Application 开始。
DELETE FROM session_events;
DELETE FROM sessions;

ALTER TABLE sessions DROP COLUMN stage;

ALTER TABLE pipeline_stages
    DROP CONSTRAINT pipeline_stages_pkey,
    DROP CONSTRAINT pipeline_stages_key_fmt,
    DROP CONSTRAINT pipeline_stages_position_uq,
    DROP COLUMN stage_key,
    ADD PRIMARY KEY (pipeline_id, position);

CREATE TABLE applications (
    id             uuid PRIMARY KEY,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    job_id         uuid NOT NULL REFERENCES jobs(id) ON DELETE RESTRICT,
    resume_id      uuid NOT NULL REFERENCES resumes(id) ON DELETE RESTRICT,
    session_id     uuid NOT NULL UNIQUE REFERENCES sessions(id) ON DELETE CASCADE,
    status         text NOT NULL DEFAULT 'active',
    stage_position integer NOT NULL DEFAULT 0,
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

CREATE INDEX applications_job_idx
    ON applications (job_id);

CREATE INDEX jobs_pipeline_idx
    ON jobs (pipeline_id)
    WHERE deleted_at IS NULL;
