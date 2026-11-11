CREATE TABLE resumes (
    id                  uuid PRIMARY KEY,   -- 简历 ID
    user_id             uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, -- 归属用户；最多 3 份有效
    title               text NOT NULL,      -- 展示标题，1..=80
    kind                text NOT NULL,      -- online/attachment
    is_default          boolean NOT NULL DEFAULT false, -- 默认简历；每用户 ≤1（部分唯一索引）
    content             jsonb NOT NULL,     -- 结构化内容；附件简历存解析草稿
    attachment_key      text,               -- 服务端存储键；仅附件简历非空，全表唯一
    attachment_name     text,               -- 原始文件名；仅展示/下载头
    attachment_mime     text,               -- 校验后 MIME；仅 PDF/DOCX 两种
    attachment_size     bigint,             -- 附件字节数，1..=10485760
    attachment_sha256   bytea,              -- 文件内容摘要（32 字节）
    parse_status        text NOT NULL,      -- not_applicable/pending/ready/failed
    parse_error_code    text,               -- 解析失败错误码；固定枚举字符串（如 unsupported_format）
    created_at          timestamptz NOT NULL, -- 创建时间
    updated_at          timestamptz NOT NULL, -- 最近更新时间；删除默认简历时按它提升
    deleted_at          timestamptz,        -- 软删除时间；NULL=未删除
    CONSTRAINT resumes_kind CHECK (kind IN ('online', 'attachment')),
    CONSTRAINT resumes_title_len CHECK (char_length(title) BETWEEN 1 AND 80),
    CONSTRAINT resumes_content_object CHECK (jsonb_typeof(content) = 'object'),
    CONSTRAINT resumes_content_size CHECK (octet_length(content::text) <= 262144),
    CONSTRAINT resumes_parse_status CHECK (
        parse_status IN ('not_applicable', 'pending', 'ready', 'failed')
    ),
    CONSTRAINT resumes_online_fields CHECK (
        kind <> 'online' OR (
            attachment_key IS NULL AND attachment_name IS NULL
            AND attachment_mime IS NULL AND attachment_size IS NULL
            AND attachment_sha256 IS NULL
            AND parse_status = 'not_applicable' AND parse_error_code IS NULL
        )
    ),
    CONSTRAINT resumes_attachment_fields CHECK (
        kind <> 'attachment' OR (
            attachment_key IS NOT NULL AND attachment_name IS NOT NULL
            AND attachment_mime IN ('application/pdf',
                'application/vnd.openxmlformats-officedocument.wordprocessingml.document')
            AND attachment_size BETWEEN 1 AND 10485760
            AND octet_length(attachment_sha256) = 32
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
