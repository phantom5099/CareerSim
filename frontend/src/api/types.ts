export interface User {
  id: string
  username: string
  displayName: string
  school: string | null
  major: string | null
  graduationYear: number | null
  targetRoles: string[]
  targetCities: string[]
  avatarText: string
  avatarColor: string
}

export interface LoginPayload {
  username: string
  password: string
}

export interface RegisterPayload {
  username: string
  password: string
  displayName: string
}

/** 资料补丁；省略 = 不修改，null = 清空。 */
export interface ProfilePatchPayload {
  displayName?: string
  school?: string | null
  major?: string | null
  graduationYear?: number | null
  targetRoles?: string[]
  targetCities?: string[]
}

export interface ChangePasswordPayload {
  currentPassword: string
  newPassword: string
}

export interface IndustriesResponse {
  industries: string[]
}

// ---------- catalog（公司模板目录）payload ----------

/** 公司补丁；省略 = 不修改。 */
export interface CompanyPatchPayload {
  name?: string
  englishName?: string
  industry?: string
  city?: string
  size?: string
  tagline?: string
  description?: string
  highlights?: string[]
  logoText?: string
  accent?: string
  isPrivate?: boolean
}

export interface ProjectPayload {
  name: string
  season?: string
  status?: string
  deadline?: string
  description?: string
}

/** 截止日期省略 = 不修改，`null` = 清空。 */
export interface ProjectPatchPayload {
  name?: string
  season?: string
  status?: string
  deadline?: string | null
  description?: string
}

/** 阶段定义与后端 `StageDefinition` 同构；rubric 必填。 */
export interface StageDefinitionPayload {
  key: string
  kind: string
  title: string
  minRounds?: number
  maxRounds?: number
  rubric?: {
    passScore?: number
    dimensions: { key: string; label: string; weight?: number; criteria?: string }[]
  }
}

export interface PipelinePayload {
  name: string
  stages: StageDefinitionPayload[]
}

export interface PipelinePatchPayload {
  name?: string
  stages?: StageDefinitionPayload[]
}

export interface JobPayload {
  pipelineId: string
  title: string
  city?: string
  jobType?: string
  salaryMin?: number
  salaryMax?: number
  salaryMonths?: number
  tags?: string[]
  description?: string
  status?: string
}

/** 薪资字段省略 = 不修改，`null` = 清空。 */
export interface JobPatchPayload {
  pipelineId?: string
  title?: string
  city?: string
  jobType?: string
  salaryMin?: number | null
  salaryMax?: number | null
  salaryMonths?: number | null
  tags?: string[]
  description?: string
  status?: string
}

export interface ResumeEducation {
  school: string
  degree: string
  major: string
  startDate: string
  endDate: string
  ranking: string
}

export interface ResumeExperience {
  company: string
  role: string
  type: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumeProject {
  name: string
  role: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumePractice {
  name: string
  role: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumeSkill {
  name: string
  level: string
}

export interface ResumeCertificate {
  name: string
  date: string
}

export interface ResumeHonor {
  name: string
  level: string
  date: string
}

export interface ResumePayload {
  name: string
  phone: string
  email: string
  city: string
  targetRole: string
  targetCity: string
  jobType: string
  availableDate: string
  educations: ResumeEducation[]
  experiences: ResumeExperience[]
  projects: ResumeProject[]
  practices: ResumePractice[]
  skills: ResumeSkill[]
  certificates: ResumeCertificate[]
  honors: ResumeHonor[]
  summary: string
}

export interface ResumeAttachment {
  fileName: string
}

interface ResumeMeta {
  id: string
  title: string
  updatedAt: string
  isDefault: boolean
}

export type ResumeParseStatus = 'not_applicable' | 'pending' | 'ready' | 'failed'

/** 简历文档：结构化在线内容必有；附件可选（解析结果回填 payload）。 */
export interface ResumeDocument extends ResumeMeta {
  payload: ResumePayload
  attachment?: ResumeAttachment
  /** 无附件时为 not_applicable；有附件时为解析状态。 */
  parseStatus: ResumeParseStatus
  /** 解析失败时的固定错误码；存储键与哈希不对外返回。 */
  parseErrorCode?: string
}

export interface ResumeCreateInput {
  title: string
  payload: ResumePayload
}

export interface ResumeUpdateInput {
  title?: string
  payload?: ResumePayload
}

export const RESUME_LIMIT = 3

