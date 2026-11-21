-- 将已有会话事件中的阶段键转换为流程位置。
-- 应用通过 session_id 关联岗位，再由岗位关联流程模板，确保转换使用权威的 pipeline_stages.position。
WITH application_pipelines AS (
    SELECT a.session_id, j.pipeline_id
    FROM applications a
    JOIN jobs j ON j.id = a.job_id
), meta_positions AS (
    SELECT e.session_id, e.seq, ps.position
    FROM session_events e
    JOIN application_pipelines ap ON ap.session_id = e.session_id
    JOIN pipeline_stages ps
      ON ps.pipeline_id = ap.pipeline_id
     AND ps.stage_key = e.kind->'initial_stage'->>'key'
    WHERE e.kind->>'type' = 'session_meta'
), completed_positions AS (
    SELECT e.session_id,
           e.seq,
           current_stage.position AS stage_position,
           next_stage.position AS next_stage_position
    FROM session_events e
    JOIN application_pipelines ap ON ap.session_id = e.session_id
    JOIN pipeline_stages current_stage
      ON current_stage.pipeline_id = ap.pipeline_id
     AND current_stage.stage_key = e.kind->>'stage_key'
    LEFT JOIN pipeline_stages next_stage
      ON next_stage.pipeline_id = ap.pipeline_id
     AND next_stage.stage_key = e.kind->'outcome'->>'next_stage_key'
    WHERE e.kind->>'type' = 'stage_completed'
)
UPDATE session_events e
SET kind = jsonb_set(
    e.kind,
    ARRAY['initial_stage']::text[],
    jsonb_build_object(
        'position', m.position,
        'kind', e.kind->'initial_stage'->'kind'
    )
),
    v = 3
FROM meta_positions m
WHERE e.session_id = m.session_id
  AND e.seq = m.seq;

WITH application_pipelines AS (
    SELECT a.session_id, j.pipeline_id
    FROM applications a
    JOIN jobs j ON j.id = a.job_id
), completed_positions AS (
    SELECT e.session_id,
           e.seq,
           current_stage.position AS stage_position,
           next_stage.position AS next_stage_position
    FROM session_events e
    JOIN application_pipelines ap ON ap.session_id = e.session_id
    JOIN pipeline_stages current_stage
      ON current_stage.pipeline_id = ap.pipeline_id
     AND current_stage.stage_key = e.kind->>'stage_key'
    LEFT JOIN pipeline_stages next_stage
      ON next_stage.pipeline_id = ap.pipeline_id
     AND next_stage.stage_key = e.kind->'outcome'->>'next_stage_key'
    WHERE e.kind->>'type' = 'stage_completed'
)
UPDATE session_events e
SET kind = jsonb_set(
    jsonb_set(
        e.kind - 'stage_key'::text,
        ARRAY['stage_position']::text[],
        to_jsonb(c.stage_position)
    ),
    ARRAY['outcome']::text[],
    CASE
        WHEN e.kind->'outcome'->>'type' = 'advance'
        THEN jsonb_set(
            (e.kind->'outcome') - 'next_stage_key'::text,
            ARRAY['next_stage_position']::text[],
            to_jsonb(c.next_stage_position)
        )
        ELSE e.kind->'outcome'
    END
),
    v = 3
FROM completed_positions c
WHERE e.session_id = c.session_id
  AND e.seq = c.seq;

UPDATE session_events e
SET v = 3
WHERE e.v = 2
  AND EXISTS (
      SELECT 1
      FROM applications a
      WHERE a.session_id = e.session_id
  );

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM session_events WHERE v = 2) THEN
        RAISE EXCEPTION 'cannot migrate legacy session events without an application-to-pipeline mapping';
    END IF;
END;
$$;