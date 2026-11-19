import { ApiError } from '../error'
import { apiFetch } from '../http'
import type {
  CompanyPatchPayload,
  JobPatchPayload,
  JobPayload,
  PipelinePayload,
  ProjectPatchPayload,
  ProjectPayload,
} from '../types'
import {
  JOB_TYPE_LABELS,
  salaryLabel,
} from '@/domain/types'
import type {
  Company,
  Job,
  JobStatus,
  JobType,
  PipelineInfo,
  ProjectStatus,
  RecruitmentProject,
} from '@/domain/types'
import {
  generateCompanyTemplate,
  normalizeTemplate,
  templatePreviewFromV1,
} from '@/mock/companyImports'
import { companyTemplateSample } from '@/mock/data/companyTemplates'

/**
 * 公司模板目录 API（S-B04 /catalog）：
 * - 列表按概要返回，详情再逐个拉取补全项目、流水线与岗位；
 * - 模板导入支持 CompanyTemplateV1，旧版扁平格式由 `normalizeTemplate` 升级；
 * - 校验走服务端 dry-run，保存走 import，字段错误透出为中文提示列表。
 */

export interface TemplateStats {
  name: string
  industry: string
  projectCount: number
  pipelineCount: number
  jobCount: number
}

export type CompanyTemplateResult =
  | { ok: true; company: Company; stats: TemplateStats }
  | { ok: false; errors: string[] }

export interface CompaniesApi {
  list(): Promise<Company[]>
  get(id: string): Promise<Company | undefined>
  templateSample(): Promise<string>
  validateImport(raw: unknown): Promise<CompanyTemplateResult>
  importTemplate(raw: unknown): Promise<Company>
  generateTemplate(prompt: string): Promise<Record<string, unknown>>
  updateCompany(id: string, patch: CompanyPatchPayload): Promise<Company>
  deleteCompany(id: string): Promise<void>
  createProject(companyId: string, input: ProjectPayload): Promise<RecruitmentProject>
  updateProject(
    companyId: string,
    projectId: string,
    patch: ProjectPatchPayload,
  ): Promise<void>
  deleteProject(companyId: string, projectId: string): Promise<void>
  createPipeline(
    companyId: string,
    projectId: string,
    input: PipelinePayload,
  ): Promise<PipelineInfo>
  updatePipeline(
    companyId: string,
    pipelineId: string,
    input: PipelinePayload,
  ): Promise<void>
  deletePipeline(companyId: string, pipelineId: string): Promise<void>
  createJob(
    companyId: string,
    projectId: string,
    input: JobPayload,
  ): Promise<Job>
  updateJob(companyId: string, jobId: string, patch: JobPatchPayload): Promise<void>
  deleteJob(companyId: string, jobId: string): Promise<void>
}

// ---------- 响应 DTO ----------

interface CompanyDto {
  id: string
  source: 'seed' | 'user'
  ownerId: string | null
  isPrivate: boolean
  isOwner: boolean
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
  createdAt: string
  updatedAt: string
}

interface ProjectDto {
  id: string
  companyId: string
  name: string
  season: string
  status: string
  deadline: string | null
  description: string
}

interface PipelineDto {
  id: string
  projectId: string
  name: string
  stages: {
    key: string
    kind: string
    title: string
    minRounds: number
    maxRounds: number
    rubric: {
      passScore: number
      dimensions: { key: string; label: string; weight: number; criteria: string }[]
    }
  }[]
}

interface JobDto {
  id: string
  projectId: string
  pipelineId: string
  title: string
  city: string
  jobType: string
  salaryMin: number | null
  salaryMax: number | null
  salaryMonths: number | null
  tags: string[]
  description: string
  status: string
}

interface CompanyDetailResponse extends CompanyDto {
  projects: ProjectDto[]
  pipelines: PipelineDto[]
  jobs: JobDto[]
}

interface CompanyListResponse {
  items: (CompanyDto & { projectCount: number; visibleJobCount: number })[]
  nextCursor?: string
}

// ---------- DTO → 领域对象 ----------

function projectStatusOf(raw: string): ProjectStatus {
  return raw === 'open' || raw === 'closed' ? raw : 'open'
}

function toPipeline(dto: PipelineDto): PipelineInfo {
  return {
    id: dto.id,
    projectId: dto.projectId,
    name: dto.name,
    stages: dto.stages.map((stage) => ({
      key: stage.key,
      kind: stage.kind as PipelineInfo['stages'][number]['kind'],
      title: stage.title,
      minRounds: stage.minRounds,
      maxRounds: stage.maxRounds,
      rubric: {
        passScore: stage.rubric.passScore,
        dimensions: stage.rubric.dimensions.map((dimension) => ({ ...dimension })),
      },
    })),
  }
}

function toProject(dto: ProjectDto): RecruitmentProject {
  return {
    id: dto.id,
    name: dto.name,
    season: dto.season,
    status: projectStatusOf(dto.status),
    deadline: dto.deadline ?? '',
    description: dto.description,
  }
}

function toJob(dto: JobDto): Job {
  return {
    id: dto.id,
    projectId: dto.projectId,
    pipelineId: dto.pipelineId,
    title: dto.title,
    city: dto.city,
    type: (JOB_TYPE_LABELS[dto.jobType as JobType] ?? JOB_TYPE_LABELS.campus) as Job['type'],
    salary: salaryLabel(dto.salaryMin, dto.salaryMax, dto.salaryMonths),
    tags: dto.tags,
    description: dto.description,
    status: (dto.status === 'draft' || dto.status === 'closed'
      ? dto.status
      : 'open') as JobStatus,
    salaryMin: dto.salaryMin,
    salaryMax: dto.salaryMax,
    salaryMonths: dto.salaryMonths,
  }
}

function toCompany(dto: CompanyDetailResponse): Company {
  return {
    id: dto.id,
    name: dto.name,
    englishName: dto.englishName,
    industry: dto.industry,
    city: dto.city,
    size: dto.size,
    tagline: dto.tagline,
    description: dto.description,
    highlights: dto.highlights,
    logoText: dto.logoText,
    accent: dto.accent,
    source: dto.source,
    ownerId: dto.ownerId,
    isPrivate: dto.isPrivate,
    isOwner: dto.isOwner,
    projects: dto.projects.map(toProject),
    pipelines: dto.pipelines.map(toPipeline),
    jobs: dto.jobs.map(toJob),
  }
}

// ---------- 错误处理 ----------

function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404
}

/** 校验失败字段列表 → 用户可读中文提示（保留模板路径）。 */
function templateErrorMessages(body: { errors?: { path: string; message: string }[] }): string[] {
  return (body.errors ?? []).map((issue) =>
    issue.path ? `${issue.path}: ${issue.message}` : issue.message,
  )
}

// ---------- HTTP 实现 ----------

async function fetchDetail(id: string): Promise<Company> {
  const dto = await apiFetch<CompanyDetailResponse>(`/api/v1/companies/${id}`)
  return toCompany(dto)
}

export const companiesApi: CompaniesApi = {
  async list(): Promise<Company[]> {
    const list = await apiFetch<CompanyListResponse>(
      '/api/v1/companies?scope=all&limit=50',
    )
    // 概要列表不含项目与岗位；并行拉取详情补全（数量上限 50）。
    const details = await Promise.allSettled(list.items.map((item) => fetchDetail(item.id)))
    const companies = details
      .filter((entry): entry is PromiseFulfilledResult<Company> => entry.status === 'fulfilled')
      .map((entry) => entry.value)
    // 有条目但详情全部失败时按失败处理，避免被误渲染成"筛选无结果"。
    if (list.items.length > 0 && companies.length === 0) {
      const firstError = details.find(
        (entry): entry is PromiseRejectedResult => entry.status === 'rejected',
      )
      throw firstError?.reason ?? new Error('公司详情加载失败')
    }
    return companies
  },

  async get(id: string): Promise<Company | undefined> {
    try {
      return await fetchDetail(id)
    } catch (error) {
      if (isNotFound(error)) {
        return undefined
      }
      throw error
    }
  },

  async templateSample(): Promise<string> {
    // 示例模板为本地静态资源；智能生成为客户端启发式草稿工具。
    return JSON.stringify(companyTemplateSample, null, 2)
  },

  async validateImport(raw): Promise<CompanyTemplateResult> {
    const template = normalizeTemplate(raw)
    const body = await apiFetch<{
      ok: boolean
      preview?: TemplateStats
      errors?: { path: string; message: string }[]
    }>('/api/v1/companies/import/validate', {
      method: 'POST',
      body: JSON.stringify(template),
    })
    if (!body.ok || !body.preview) {
      return { ok: false, errors: templateErrorMessages(body) }
    }
    return {
      ok: true,
      company: templatePreviewFromV1(template),
      stats: body.preview,
    }
  },

  async importTemplate(raw): Promise<Company> {
    const template = normalizeTemplate(raw)
    const created = await apiFetch<{ companyId: string }>('/api/v1/companies/import', {
      method: 'POST',
      body: JSON.stringify(template),
    })
    return fetchDetail(created.companyId)
  },

  async generateTemplate(prompt: string): Promise<Record<string, unknown>> {
    await new Promise((resolve) => setTimeout(resolve, 700))
    return generateCompanyTemplate(prompt)
  },

  async updateCompany(id: string, patch: CompanyPatchPayload): Promise<Company> {
    await apiFetch(`/api/v1/companies/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(patch),
    })
    return fetchDetail(id)
  },

  async deleteCompany(id: string): Promise<void> {
    await apiFetch<void>(`/api/v1/companies/${id}`, { method: 'DELETE' })
  },

  async createProject(companyId: string, input: ProjectPayload): Promise<RecruitmentProject> {
    const dto = await apiFetch<ProjectDto>(`/api/v1/companies/${companyId}/projects`, {
      method: 'POST',
      body: JSON.stringify(input),
    })
    return toProject(dto)
  },

  async updateProject(
    _companyId: string,
    projectId: string,
    patch: ProjectPatchPayload,
  ): Promise<void> {
    await apiFetch(`/api/v1/projects/${projectId}`, {
      method: 'PATCH',
      body: JSON.stringify(patch),
    })
  },

  async deleteProject(_companyId: string, projectId: string): Promise<void> {
    await apiFetch<void>(`/api/v1/projects/${projectId}`, { method: 'DELETE' })
  },

  async createPipeline(
    _companyId: string,
    projectId: string,
    input: PipelinePayload,
  ): Promise<PipelineInfo> {
    const dto = await apiFetch<PipelineDto>(`/api/v1/projects/${projectId}/pipelines`, {
      method: 'POST',
      body: JSON.stringify(input),
    })
    return toPipeline(dto)
  },

  async updatePipeline(
    _companyId: string,
    pipelineId: string,
    patch: PipelinePayload,
  ): Promise<void> {
    await apiFetch(`/api/v1/pipelines/${pipelineId}`, {
      method: 'PUT',
      body: JSON.stringify(patch),
    })
  },

  async deletePipeline(_companyId: string, pipelineId: string): Promise<void> {
    await apiFetch<void>(`/api/v1/pipelines/${pipelineId}`, { method: 'DELETE' })
  },

  async createJob(
    _companyId: string,
    projectId: string,
    input: JobPayload,
  ): Promise<Job> {
    const dto = await apiFetch<JobDto>(`/api/v1/projects/${projectId}/jobs`, {
      method: 'POST',
      body: JSON.stringify(input),
    })
    return toJob(dto)
  },

  async updateJob(_companyId: string, jobId: string, patch: JobPatchPayload): Promise<void> {
    await apiFetch(`/api/v1/jobs/${jobId}`, {
      method: 'PATCH',
      body: JSON.stringify(patch),
    })
  },

  async deleteJob(_companyId: string, jobId: string): Promise<void> {
    await apiFetch<void>(`/api/v1/jobs/${jobId}`, { method: 'DELETE' })
  },
}
