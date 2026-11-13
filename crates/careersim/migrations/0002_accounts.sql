CREATE TABLE users (
    id                uuid PRIMARY KEY,             -- 用户 ID
    username          text NOT NULL,                -- 登录名展示原文（保留大小写）
    username_key      text NOT NULL,                -- 归一化登录名（小写 [a-z0-9_]）；唯一约束对象
    password_hash     text NOT NULL,                -- Argon2id PHC 字符串；不存明文
    display_name      text NOT NULL,                -- 展示昵称，1..=40 字符
    school            text,                         -- 学校，可空，≤80 字符
    major             text,                         -- 专业，可空，≤80 字符
    graduation_year   smallint,                     -- 毕业年份，1980..=2100，可空
    target_roles      text[] NOT NULL DEFAULT '{}', -- 目标岗位，≤10 项
    target_cities     text[] NOT NULL DEFAULT '{}', -- 目标城市，≤10 项
    created_at        timestamptz NOT NULL,         -- 注册时间
    updated_at        timestamptz NOT NULL,         -- 资料最近更新时间
    CONSTRAINT users_username_key_uq UNIQUE (username_key),
    CONSTRAINT users_username_key_fmt CHECK (username_key ~ '^[a-z0-9_]{3,32}$'),
    CONSTRAINT users_username_len CHECK (char_length(username) BETWEEN 3 AND 32),
    CONSTRAINT users_display_name_len CHECK (char_length(display_name) BETWEEN 1 AND 40),
    CONSTRAINT users_school_len CHECK (school IS NULL OR char_length(school) <= 80),
    CONSTRAINT users_major_len CHECK (major IS NULL OR char_length(major) <= 80),
    CONSTRAINT users_grad_year CHECK (graduation_year IS NULL
        OR graduation_year BETWEEN 1980 AND 2100),
    CONSTRAINT users_target_roles_len CHECK (cardinality(target_roles) <= 10),
    CONSTRAINT users_target_cities_len CHECK (cardinality(target_cities) <= 10)
);

CREATE TABLE login_sessions (
    id                  uuid PRIMARY KEY,           -- 会话 ID
    user_id             uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, -- 归属用户
    token_hash          bytea NOT NULL,             -- 令牌 SHA-256（32 字节）；不存明文令牌
    created_at          timestamptz NOT NULL,       -- 创建时间
    last_seen_at        timestamptz NOT NULL,       -- 最近活跃时间；节流更新（60 秒）
    idle_expires_at     timestamptz NOT NULL,       -- 空闲过期时间；随活跃顺延（7 天）
    absolute_expires_at timestamptz NOT NULL,       -- 绝对过期时间；不随活跃顺延（30 天）
    revoked_at          timestamptz,                -- 撤销时间；NULL=会话有效
    CONSTRAINT login_sessions_token_uq UNIQUE (token_hash),
    CONSTRAINT login_sessions_token_len CHECK (octet_length(token_hash) = 32),
    CONSTRAINT login_sessions_time_order CHECK (
        created_at <= last_seen_at
        AND last_seen_at < idle_expires_at
        AND idle_expires_at <= absolute_expires_at
    )
);

CREATE INDEX login_sessions_user_active_idx
    ON login_sessions (user_id) WHERE revoked_at IS NULL;
