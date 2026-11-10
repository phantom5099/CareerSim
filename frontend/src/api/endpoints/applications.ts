import { mockApplicationsApi } from '@/mock/applications'

import type { Application, Company, Job } from '@/domain/types'

export interface ApplicationsApi {
  list(): Promise<Application[]>
  apply(company: Company, job: Job, resumeId: string): Promise<Application>
  advance(id: string): Promise<Application>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const applicationsApi: ApplicationsApi = mockApplicationsApi
