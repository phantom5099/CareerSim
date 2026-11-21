ALTER TABLE pipeline_stages
    DROP CONSTRAINT pipeline_stages_pkey,
    DROP CONSTRAINT pipeline_stages_key_fmt,
    DROP CONSTRAINT pipeline_stages_position_uq,
    DROP COLUMN stage_key,
    ADD PRIMARY KEY (pipeline_id, position);
