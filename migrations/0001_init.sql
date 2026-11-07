CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    stage text NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    last_seq bigint NOT NULL DEFAULT 0,
    last_turn_id bigint NOT NULL DEFAULT 0
);

CREATE TABLE session_events (
    session_id uuid NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq bigint NOT NULL,
    v integer NOT NULL,
    at timestamptz NOT NULL,
    kind jsonb NOT NULL,
    PRIMARY KEY (session_id, seq)
);

CREATE INDEX sessions_updated_at_idx ON sessions (updated_at DESC, id);
