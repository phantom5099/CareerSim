import { mockInterviewsApi } from '@/mock/interviews'

import type {
  Application,
  Company,
  InterviewScript,
  ResumeScreening,
  StageKey,
} from '@/domain/types'

export interface InterviewSession {
  script: InterviewScript
  screening?: ResumeScreening
}

export interface InterviewsApi {
  load(application: Application, stage: StageKey, company?: Company): Promise<InterviewSession>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const interviewsApi: InterviewsApi = mockInterviewsApi
