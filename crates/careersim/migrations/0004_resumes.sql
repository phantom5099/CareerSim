CREATE TABLE resumes (
    id                  uuid PRIMARY KEY,   -- 简历 ID
    user_id             uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, -- 归属用户；最多 3 份有效
    title               text NOT NULL,      -- 展示标题，1..=80
    is_default          boolean NOT NULL DEFAULT false, -- 默认简历；每用户 ≤1（部分唯一索引）
    content             jsonb NOT NULL,     -- 结构化在线内容（必有）；附件解析结果回填于此
    attachment_key      text,               -- 服务端存储键；仅附件简历非空，全表唯一
    attachment_name     text,               -- 原始文件名；仅展示/下载头；后缀即 MIME（仅 .pdf/.docx）
    attachment_size     bigint,             -- 附件字节数，1..=10485760
    parse_status        text NOT NULL,      -- not_applicable/pending/ready/failed
    parse_error_code    text,               -- 解析失败错误码；固定枚举字符串（如 unsupported_format）
    created_at          timestamptz NOT NULL, -- 创建时间
    updated_at          timestamptz NOT NULL, -- 最近更新时间；删除默认简历时按它提升
    deleted_at          timestamptz,        -- 软删除时间；NULL=未删除
    -- 附件字段组成组可空：无附件 ⇔ 全 NULL 且 parse_status='not_applicable'；
    -- 有附件 ⇔ 全非空且解析状态为 pending/ready/failed（parse_error_code 仅 failed 非空）。
    CONSTRAINT resumes_title_len CHECK (char_length(title) BETWEEN 1 AND 80),
    CONSTRAINT resumes_content_object CHECK (jsonb_typeof(content) = 'object'),
    CONSTRAINT resumes_content_size CHECK (octet_length(content::text) <= 262144),
    CONSTRAINT resumes_parse_status CHECK (
        parse_status IN ('not_applicable', 'pending', 'ready', 'failed')
    ),
    CONSTRAINT resumes_attachment_group CHECK (
        (
            attachment_key IS NULL AND attachment_name IS NULL
            AND attachment_size IS NULL
            AND parse_status = 'not_applicable' AND parse_error_code IS NULL
        ) OR (
            attachment_key IS NOT NULL AND attachment_name IS NOT NULL
            AND attachment_name ~* '\.(pdf|docx)$'
            AND attachment_size BETWEEN 1 AND 10485760
            AND parse_status IN ('pending', 'ready', 'failed')
        )
    )
);

CREATE UNIQUE INDEX resumes_one_default_uq
    ON resumes (user_id) WHERE is_default AND deleted_at IS NULL;
CREATE UNIQUE INDEX resumes_attachment_key_uq
    ON resumes (attachment_key) WHERE attachment_key IS NOT NULL;
CREATE INDEX resumes_user_updated_idx
    ON resumes (user_id, updated_at DESC) WHERE deleted_at IS NULL;
