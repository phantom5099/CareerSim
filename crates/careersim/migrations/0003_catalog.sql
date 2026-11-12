CREATE TABLE companies (
    id             uuid PRIMARY KEY,                 -- 公司模板 ID
    owner_user_id  uuid REFERENCES users(id) ON DELETE CASCADE, -- 归属用户；种子模板为 NULL
    is_seed        boolean NOT NULL DEFAULT false,   -- 种子模板标记；服务层拒绝一切写操作
    is_private     boolean NOT NULL DEFAULT false,   -- 私有标记；种子恒 false；true 时仅 owner 可见
    name           text NOT NULL,                    -- 公司名，1..=80
    name_key       text NOT NULL,                    -- 公司名归一化键；owner 内大小写不敏感唯一
    english_name   text NOT NULL DEFAULT '',         -- 英文名
    industry       text NOT NULL,                    -- 所属行业
    city           text NOT NULL DEFAULT '',         -- 所在城市
    size           text NOT NULL DEFAULT '',         -- 公司规模
    tagline        text NOT NULL DEFAULT '',         -- 标语
    description    text NOT NULL DEFAULT '',         -- 公司介绍
    highlights     text[] NOT NULL DEFAULT '{}',     -- 亮点，≤12 项
    logo_text      text NOT NULL DEFAULT '',         -- 头像文字，≤8 字符
    accent         text NOT NULL DEFAULT '',         -- 主题色，≤16 字符
    created_at     timestamptz NOT NULL,             -- 创建时间
    updated_at     timestamptz NOT NULL,             -- 最近更新时间
    deleted_at     timestamptz,                      -- 软删除时间；NULL=未删除
    CONSTRAINT companies_owner_seed CHECK (
        (is_seed AND owner_user_id IS NULL)
        OR (NOT is_seed AND owner_user_id IS NOT NULL)
    ),
    CONSTRAINT companies_seed_public CHECK (NOT is_seed OR NOT is_private),
    CONSTRAINT companies_name_len CHECK (char_length(name) BETWEEN 1 AND 80),
    CONSTRAINT companies_highlights_len CHECK (cardinality(highlights) <= 12)
);

CREATE UNIQUE INDEX companies_seed_name_uq
    ON companies (name_key) WHERE is_seed AND deleted_at IS NULL;
CREATE UNIQUE INDEX companies_owner_name_uq
    ON companies (owner_user_id, name_key)
    WHERE (NOT is_seed) AND deleted_at IS NULL;
CREATE INDEX companies_owner_idx ON companies (owner_user_id)
    WHERE deleted_at IS NULL;
CREATE INDEX companies_public_idx ON companies (updated_at DESC)
    WHERE deleted_at IS NULL AND NOT is_private;

CREATE TABLE recruitment_projects (
    id          uuid PRIMARY KEY,          -- 项目 ID
    company_id  uuid NOT NULL REFERENCES companies(id) ON DELETE CASCADE, -- 所属公司
    name        text NOT NULL,             -- 项目名，1..=60，公司内唯一
    name_key    text NOT NULL,             -- 项目名归一化键
    season      text NOT NULL DEFAULT '',  -- 招聘季，如 "2027"
    status      text NOT NULL,             -- open/closing/closed
    deadline    date,                      -- 报名截止日期，可空
    description text NOT NULL DEFAULT '',  -- 项目说明
    created_at  timestamptz NOT NULL,      -- 创建时间
    updated_at  timestamptz NOT NULL,      -- 最近更新时间
    deleted_at  timestamptz,               -- 软删除时间；NULL=未删除
    CONSTRAINT projects_status CHECK (status IN ('open', 'closing', 'closed')),
    CONSTRAINT projects_name_len CHECK (char_length(name) BETWEEN 1 AND 60),
    UNIQUE (company_id, id)
);

CREATE UNIQUE INDEX projects_company_name_uq
    ON recruitment_projects (company_id, name_key)
    WHERE deleted_at IS NULL;

CREATE TABLE pipelines (
    id          uuid PRIMARY KEY,          -- 流水线 ID
    project_id  uuid NOT NULL REFERENCES recruitment_projects(id) ON DELETE CASCADE, -- 所属项目
    name        text NOT NULL,             -- 流水线名，1..=60，项目内唯一
    name_key    text NOT NULL,             -- 流水线名归一化键
    created_at  timestamptz NOT NULL,      -- 创建时间
    updated_at  timestamptz NOT NULL,      -- 最近更新时间
    deleted_at  timestamptz,               -- 软删除时间；NULL=未删除
    CONSTRAINT pipelines_name_len CHECK (char_length(name) BETWEEN 1 AND 60),
    UNIQUE (project_id, id)
);

CREATE UNIQUE INDEX pipelines_project_name_uq
    ON pipelines (project_id, name_key) WHERE deleted_at IS NULL;

CREATE TABLE pipeline_stages (
    pipeline_id  uuid NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE, -- 所属流水线（联合主键）
    stage_key    text NOT NULL,            -- 阶段键；流水线内唯一（联合主键）
    position     smallint NOT NULL CHECK (position >= 0), -- 执行顺序号；流水线内唯一
    kind         text NOT NULL,            -- 阶段类型（CHECK 枚举，6 类）
    title        text NOT NULL,            -- 展示标题，1..=40
    rubric       jsonb NOT NULL,           -- 评分规则；无固定形状，整体读写
    min_rounds   smallint NOT NULL DEFAULT 1, -- 最少轮次，1..=10
    max_rounds   smallint NOT NULL,        -- 最大轮次，min..=20
    PRIMARY KEY (pipeline_id, stage_key),
    CONSTRAINT pipeline_stages_position_uq UNIQUE (pipeline_id, position),
    CONSTRAINT pipeline_stages_key_fmt CHECK (stage_key ~ '^[a-z][a-z0-9_]{1,31}$'),
    CONSTRAINT pipeline_stages_kind CHECK (kind IN (
        'hr_screen', 'exam', 'tech_interview', 'manager_interview',
        'hrbp_interview', 'offer'
    )),
    CONSTRAINT pipeline_stages_title_len CHECK (char_length(title) BETWEEN 1 AND 40),
    CONSTRAINT pipeline_stages_rounds CHECK (
        min_rounds BETWEEN 1 AND 10 AND max_rounds BETWEEN min_rounds AND 20
    ),
    CONSTRAINT pipeline_stages_rubric_object CHECK (jsonb_typeof(rubric) = 'object')
);

CREATE TABLE jobs (
    id            uuid PRIMARY KEY,        -- 岗位 ID
    project_id    uuid NOT NULL REFERENCES recruitment_projects(id) ON DELETE CASCADE, -- 所属项目
    pipeline_id   uuid NOT NULL,           -- 绑定的流水线；复合外键保证与岗位同项目
    title         text NOT NULL,           -- 岗位名，项目内唯一
    title_key     text NOT NULL,           -- 岗位名归一化键
    city          text NOT NULL DEFAULT '',-- 工作城市
    job_type      text NOT NULL,           -- campus/internship/experienced
    salary_min    integer,                 -- 月薪下限（整数元），可空
    salary_max    integer,                 -- 月薪上限（整数元），可空；有值时 >= salary_min
    salary_months smallint,                -- 年薪折算月数，1..=24，可空
    tags          text[] NOT NULL DEFAULT '{}', -- 标签，≤12 项
    description   text NOT NULL DEFAULT '',-- 岗位描述
    status        text NOT NULL DEFAULT 'draft', -- draft/open/closed；非 owner 仅见 open
    created_at    timestamptz NOT NULL,    -- 创建时间
    updated_at    timestamptz NOT NULL,    -- 最近更新时间
    deleted_at    timestamptz,             -- 软删除时间；NULL=未删除
    CONSTRAINT jobs_type CHECK (job_type IN ('campus', 'internship', 'experienced')),
    CONSTRAINT jobs_status CHECK (status IN ('draft', 'open', 'closed')),
    CONSTRAINT jobs_salary_nonneg CHECK (
        (salary_min IS NULL OR salary_min >= 0)
        AND (salary_max IS NULL OR salary_max >= 0)
        AND (salary_min IS NULL OR salary_max IS NULL OR salary_min <= salary_max)
    ),
    CONSTRAINT jobs_salary_months CHECK (salary_months IS NULL OR salary_months BETWEEN 1 AND 24),
    CONSTRAINT jobs_tags_len CHECK (cardinality(tags) <= 12),
    CONSTRAINT jobs_pipeline_same_project
        FOREIGN KEY (project_id, pipeline_id)
        REFERENCES pipelines (project_id, id)
);

CREATE UNIQUE INDEX jobs_project_title_uq
    ON jobs (project_id, title_key) WHERE deleted_at IS NULL;
CREATE INDEX jobs_project_status_idx ON jobs (project_id, status)
    WHERE deleted_at IS NULL;
