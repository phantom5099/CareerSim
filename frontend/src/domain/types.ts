export const STAGE_ORDER = [
  'hr_screen',
  'exam',
  'tech_interview',
  'manager_interview',
  'hrbp_interview',
  'offer',
] as const

export type StageKey = (typeof STAGE_ORDER)[number]

export const STAGE_LABELS: Record<StageKey, string> = {
  hr_screen: '简历初筛',
  exam: '在线笔试',
  tech_interview: '技术一面',
  manager_interview: '主管面',
  hrbp_interview: 'HRBP 面',
  offer: 'Offer',
}

export const STAGE_NOTES: Record<StageKey, string> = {
  hr_screen: 'HR 视角',
  exam: '限时作答',
  tech_interview: '项目深挖',
  manager_interview: '业务判断',
  hrbp_interview: '动机与匹配',
  offer: '谈薪与选择',
}

export type ProjectStatus = 'open' | 'closed'

export interface RecruitmentProject {
  id: string
  name: string
  season: string
  status: ProjectStatus
  deadline: string
  description: string
}

export type StageKind =
  | 'hr_screen'
  | 'exam'
  | 'tech_interview'
  | 'manager_interview'
  | 'hrbp_interview'
  | 'offer'

export interface PipelineRubricDimension {
  key: string
  label: string
  weight: number
  criteria: string
}

export interface PipelineRubric {
  passScore: number
  dimensions: PipelineRubricDimension[]
}

export interface PipelineStage {
  key: string
  kind: StageKind
  title: string
  minRounds: number
  maxRounds: number
  rubric: PipelineRubric
}

export interface PipelineInfo {
  id: string
  projectId: string
  name: string
  stages: PipelineStage[]
}

export type JobType = 'campus' | 'internship' | 'experienced'

export type JobStatus = 'draft' | 'open' | 'closed'

export const JOB_TYPE_LABELS: Record<JobType, string> = {
  campus: '校招',
  internship: '实习',
  experienced: '社招',
}

export const JOB_STATUS_LABELS: Record<JobStatus, string> = {
  draft: '草稿',
  open: '开放中',
  closed: '已关闭',
}

export function jobTypeLabel(value: string): string {
  return JOB_TYPE_LABELS[value as JobType] ?? value
}

/** 由月薪区间与薪数生成展示文案；日薪等非常规数字按 ¥ 原值展示。 */
export function salaryLabel(
  min?: number | null,
  max?: number | null,
  months?: number | null,
): string {
  const fmt = (value: number): string =>
    value % 1000 === 0 ? `${value / 1000}K` : `¥${value}`
  let base = ''
  if (min != null && max != null) {
    base = min === max ? fmt(min) : `${fmt(min)}–${fmt(max)}`
  } else if (min != null) {
    base = fmt(min)
  } else if (max != null) {
    base = `≤ ${fmt(max)}`
  } else {
    return '面议'
  }
  if (months != null && months !== 12) {
    return `${base} · ${months} 薪`
  }
  return base
}

export interface Job {
  id: string
  title: string
  city: string
  type: '校招' | '实习' | '社招'
  salary: string
  tags: string[]
  description: string
  projectId?: string
  industry?: string
  pipelineId?: string
  status?: JobStatus
  salaryMin?: number | null
  salaryMax?: number | null
  salaryMonths?: number | null
}

export interface Company {
  id: string
  name: string
  englishName: string
  industry: string
  city: string
  size: string
  tagline: string
  description: string
  highlights: string[]
  logoText: string
  accent: string
  source?: 'seed' | 'user'
  ownerId?: string | null
  isPrivate?: boolean
  isOwner?: boolean
  projects: RecruitmentProject[]
  pipelines?: PipelineInfo[]
  jobs: Job[]
}

export type ApplicationStatus = 'active' | 'rejected' | 'offer' | 'withdrawn'

export type StageState = 'done' | 'current' | 'pending' | 'failed'

export interface ApplicationStage {
  key: StageKey
  state: StageState
  note?: string
  at?: string
}

export interface Application {
  id: string
  companyId: string
  jobId: string
  jobTitle: string
  resumeId: string
  resumeTitle: string
  appliedAt: string
  status: ApplicationStatus
  currentStage: StageKey
  stages: ApplicationStage[]
  nextAction: string
  score?: number
}

export type OfferStatus = 'pending' | 'accepted' | 'declined'

export interface Offer {
  id: string
  companyId: string
  jobTitle: string
  baseSalary: number
  months: number
  bonus: string
  city: string
  issuedAt: string
  deadline: string
  status: OfferStatus
}

export interface InterviewTurn {
  question: string
  followUp?: string
}

export interface InterviewScript {
  applicationId: string
  stage: StageKey
  interviewer: string
  interviewerTitle: string
  focus: string[]
  estimatedMinutes: number
  turns: InterviewTurn[]
}

export interface ResumeDimension {
  label: string
  score: number
  note: string
}

export interface ResumeScreening {
  reviewer: string
  reviewerTitle: string
  score: number
  verdict: string
  summary: string
  dimensions: ResumeDimension[]
  highlights: string[]
  concerns: string[]
}

export const APPLICATION_STATUS_LABELS: Record<ApplicationStatus, string> = {
  active: '进行中',
  rejected: '已结束',
  offer: '已录用',
  withdrawn: '已撤回',
}

export const PROJECT_STATUS_LABELS: Record<ProjectStatus, string> = {
  open: '开放中',
  closed: '已结束',
}

/** 状态只表示是否手动关闭；deadline 已过（按本地日期比较）对外视为已结束。 */
export function effectiveProjectStatus(project: {
  status: ProjectStatus
  deadline: string
}): ProjectStatus {
  if (project.status === 'closed') return 'closed'
  if (!project.deadline) return 'open'
  const today = new Date()
  const todayKey = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`
  return project.deadline < todayKey ? 'closed' : 'open'
}

export function stageIndex(stage: StageKey): number {
  return STAGE_ORDER.indexOf(stage)
}

/** 当前阶段是否由候选人操作（进行中），否则视为等待对方结果。 */
export function actionableStage(application: Application): StageKey | undefined {
  if (application.status !== 'active') {
    return undefined
  }
  if (application.currentStage === 'hr_screen' || application.currentStage === 'offer') {
    return undefined
  }
  return application.currentStage
}

export function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
  }).format(date)
}

export function formatDateLong(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(date)
}

/** 团队规模枚举；与后端 COMPANY_SIZES 严格一致，表单下拉与草稿生成共用。 */
export const COMPANY_SIZE_OPTIONS = [
  '50 人以下',
  '50–200 人',
  '100–499 人',
  '500–2000 人',
  '2000 人以上',
] as const

/** 品牌色枚举；与后端 COMPANY_ACCENTS 严格一致，只允许点选，不允许自由输入。 */
export const ACCENT_OPTIONS = [
  { value: '#3D5A45', label: '苔绿' },
  { value: '#2F5D50', label: '松绿' },
  { value: '#4F6042', label: '橄榄' },
  { value: '#A4482F', label: '朱砂' },
  { value: '#8A5A2B', label: '赭石' },
  { value: '#5B4A7A', label: '黛紫' },
  { value: '#29405E', label: '靛青' },
  { value: '#2C4A7C', label: '藏蓝' },
] as const

export const DEFAULT_ACCENT = ACCENT_OPTIONS[0].value

/** 模板文本字段上限；与后端 catalog::model 的 MAX_CHARS 常量严格一致。 */
export const TEMPLATE_LIMITS = {
  name: 80,
  englishName: 40,
  industry: 24,
  city: 24,
  tagline: 60,
  description: 600,
  logoText: 8,
  highlightItem: 60,
  highlightItems: 12,
  projectName: 60,
  season: 24,
  projectDescription: 300,
  jobTitle: 60,
  jobDescription: 600,
} as const

/** 简历枚举选项；与后端 resume::content 的常量严格一致。 */
export const RESUME_JOB_TYPES = ['校招', '实习', '社招'] as const
export const DEGREE_OPTIONS = ['大专', '本科', '硕士', '博士', '其他'] as const
export const EXPERIENCE_TYPE_OPTIONS = ['实习', '全职', '兼职'] as const
export const SKILL_LEVEL_OPTIONS = ['了解', '掌握', '熟练'] as const
export const HONOR_LEVEL_OPTIONS = ['国家级', '省级', '市级', '校级', '院级', '其他'] as const
