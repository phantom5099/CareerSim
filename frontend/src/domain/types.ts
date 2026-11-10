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

export type ProjectStatus = 'open' | 'closing' | 'closed'

export interface RecruitmentProject {
  id: string
  name: string
  season: string
  status: ProjectStatus
  deadline: string
  description: string
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
  featured?: boolean
  imported?: boolean
  projects: RecruitmentProject[]
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
  closing: '即将截止',
  closed: '已结束',
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
