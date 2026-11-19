CREATE INDEX applications_job_idx
    ON applications (job_id);

CREATE INDEX jobs_pipeline_idx
    ON jobs (pipeline_id)
    WHERE deleted_at IS NULL;
