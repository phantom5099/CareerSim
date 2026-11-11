import { ApiError } from '@/api/error'
import type { ApplicationsApi } from '@/api/endpoints/applications'
import type { Application, Company, Job } from '@/domain/types'
import { STAGE_LABELS, STAGE_ORDER, stageIndex } from '@/domain/types'

import { seedApplications } from './data/applications'

// 后端投递接口尚未接入：内存里维护投递与阶段推进。
let applications: Application[] = structuredClone(seedApplications)

export const mockApplicationsApi: ApplicationsApi = {
  async list(): Promise<Application[]> {
    return structuredClone(applications)
  },

  async apply(
    company: Company,
    job: Job,
    resumeId: string,
    resumeTitle: string,
  ): Promise<Application> {
    const existing = applications.find(
      (application) => application.companyId === company.id && application.jobId === job.id,
    )
    if (existing) {
      return structuredClone(existing)
    }

    if (!resumeId || !resumeTitle) {
      throw new ApiError('请选择一份有效的简历', 404, 'RESUME_NOT_FOUND')
    }

    const application: Application = {
      id: `app-${Date.now().toString(36)}`,
      companyId: company.id,
      jobId: job.id,
      jobTitle: job.title,
      resumeId,
      resumeTitle,
      appliedAt: new Date().toISOString(),
      status: 'active',
      currentStage: 'hr_screen',
      nextAction: '等待 HR 初筛 · 预计 2 个工作日',
      stages: STAGE_ORDER.map((key, index) => ({
        key,
        state: index === 0 ? 'current' : 'pending',
      })),
    }
    applications = [application, ...applications]
    return structuredClone(application)
  },

  async advance(id: string): Promise<Application> {
    const application = applications.find((item) => item.id === id)
    if (!application) {
      throw new ApiError('投递记录不存在', 404, 'APPLICATION_NOT_FOUND')
    }
    if (application.status !== 'active') {
      return structuredClone(application)
    }

    const index = stageIndex(application.currentStage)
    const stages = application.stages.map((stage) =>
      stage.key === application.currentStage
        ? { ...stage, state: 'done' as const, note: stage.note ?? '本轮已完成 · 模拟记录' }
        : stage,
    )

    if (index >= STAGE_ORDER.length - 1) {
      application.stages = stages
      application.status = 'offer'
      application.nextAction = 'Offer 已发出 · 请在截止日前答复'
      return structuredClone(application)
    }

    const nextStage = STAGE_ORDER[index + 1]
    application.currentStage = nextStage
    application.stages = stages.map((stage) =>
      stage.key === nextStage ? { ...stage, state: 'current' as const } : stage,
    )
    application.nextAction =
      nextStage === 'offer'
        ? '等待 Offer 发放 · 请留意邮件'
        : `进入${STAGE_LABELS[nextStage]} · 等待安排`
    return structuredClone(application)
  },
}
