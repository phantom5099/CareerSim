import { mockResumeApi } from '@/mock/resume'

import type { ResumePayload } from '../types'

export interface ResumeApi {
  load(): Promise<ResumePayload | null>
  save(payload: ResumePayload): Promise<void>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const resumeApi: ResumeApi = mockResumeApi
