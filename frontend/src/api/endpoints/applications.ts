import { apiFetch } from '@/api/http'

import type { Application, Company, Job } from '@/domain/types'

export interface ApplicationsApi {
  list(): Promise<Application[]>
  /** resumeId + resumeTitle 一起传入：mock 与未来 HTTP 实现都不再回查简历模块。 */
  apply(company: Company, job: Job, resumeId: string, resumeTitle: string): Promise<Application>
  advance(id: string): Promise<Application>
}

interface ApplicationStageDto {
  position: number
  kind: string
  title: string
  state: 'done' | 'current' | 'pending' | 'failed'
}

interface ApplicationDto {
  id: string
  companyId: string
  jobId: string
  jobTitle: string
  resumeId: string
  resumeTitle: string
  appliedAt: string
  status: Application['status']
  currentStage: string
  stages: ApplicationStageDto[]
  nextAction: string
}

function toApplication(dto: ApplicationDto): Application {
  return {
    id: dto.id,
    companyId: dto.companyId,
    jobId: dto.jobId,
    jobTitle: dto.jobTitle,
    resumeId: dto.resumeId,
    resumeTitle: dto.resumeTitle,
    appliedAt: dto.appliedAt,
    status: dto.status,
    currentStage: dto.currentStage as Application['currentStage'],
    stages: dto.stages.map((stage) => ({
      position: stage.position,
      key: stage.kind as Application['currentStage'],
      state: stage.state,
      note: stage.title,
    })),
    nextAction: dto.nextAction,
  }
}

export const applicationsApi: ApplicationsApi = {
  async list(): Promise<Application[]> {
    const payload = await apiFetch<ApplicationDto[]>('/api/v1/applications')
    return payload.map(toApplication)
  },

  async apply(
    _company: Company,
    job: Job,
    resumeId: string,
    _resumeTitle: string,
  ): Promise<Application> {
    const payload = await apiFetch<ApplicationDto>('/api/v1/applications', {
      method: 'POST',
      body: JSON.stringify({ jobId: job.id, resumeId }),
    })
    return toApplication(payload)
  },

  async advance(id: string): Promise<Application> {
    const payload = await apiFetch<ApplicationDto>(`/api/v1/applications/${id}/advance`, {
      method: 'POST',
    })
    return toApplication(payload)
  },
}
