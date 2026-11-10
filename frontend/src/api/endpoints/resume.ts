import { mockResumeApi } from '@/mock/resume'

import type { ResumeDocument, ResumeInput } from '../types'

export interface ResumesApi {
  list(): Promise<ResumeDocument[]>
  create(input: ResumeInput): Promise<ResumeDocument>
  update(id: string, input: ResumeInput): Promise<ResumeDocument>
  remove(id: string): Promise<void>
  setDefault(id: string): Promise<ResumeDocument>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const resumeApi: ResumesApi = mockResumeApi
