import { JOB_TYPE_LABELS, salaryLabel } from '@/domain/types'
import type {
  Company,
  Job,
  JobType,
  PipelineInfo,
  ProjectStatus,
  StageKind,
} from '@/domain/types'

import {
  companySizes,
  defaultTemplateStages,
  englishPrefixes,
  englishSuffixes,
  fallbackProfile,
  industryProfiles,
  namePrefixes,
  nameSuffixes,
  templateCities,
  workStyles,
} from './data/companyTemplates'
import type { IndustryProfile } from './data/companyTemplates'

const STAGE_KINDS: readonly StageKind[] = [
  'hr_screen',
  'exam',
  'tech_interview',
  'manager_interview',
  'hrbp_interview',
  'offer',
]

/**
 * 模板工具集（纯函数，无 IO）：
 * - `normalizeTemplate`：把旧版扁平格式升级为 CompanyTemplateV1，V1 原样透传；
 *   字段缺失不在此报错——统一交给服务端校验，返回带路径的完整错误列表；
 * - `templatePreviewFromV1`：校验通过后生成本地预览 Company（合成临时 id）；
 * - `generateCompanyTemplate`：按提示词生成 V1 草稿（客户端启发式）。
 */

type Raw = Record<string, unknown>

const PROJECT_STATUSES: readonly ProjectStatus[] = ['open', 'closing', 'closed']
const DEFAULT_PIPELINE_KEY = 'pl1'

function isRecord(value: unknown): value is Raw {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function textOr(value: unknown, fallback = ''): string {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : fallback
}

function listOr(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return []
  }
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
}

function isAccent(value: unknown): value is string {
  return typeof value === 'string' && /^#[0-9a-fA-F]{6}$/.test(value)
}

function legacyJobType(text: string): JobType {
  if (text === '实习') {
    return 'internship'
  }
  if (text === '社招') {
    return 'experienced'
  }
  return 'campus'
}

function clampNumber(value: number, max: number): number {
  return Number.isFinite(value) && value >= 0 && value <= max ? Math.round(value) : Number.NaN
}

/** 旧版薪资文案 → 结构化数字；日薪按每月 22 个工作日折算。 */
function parseSalaryText(text: string): {
  salaryMin?: number
  salaryMax?: number
  salaryMonths?: number
} {
  const daily = /\/\s*天/.test(text)
  const scaled = text.includes('K') || text.includes('k')
    ? (value: number) => value * 1000
    : daily
      ? (value: number) => value * 22
      : (value: number) => value
  const range = text.match(/(\d+(?:\.\d+)?)\s*[–—-]\s*(\d+(?:\.\d+)?)/)
  const single = text.match(/(\d+(?:\.\d+)?)/)
  const months = text.match(/(\d+)\s*薪/)
  const salaryMonths = months ? clampNumber(Number(months[1]), 24) : NaN
  let salaryMin: number | undefined
  let salaryMax: number | undefined
  if (range) {
    salaryMin = clampNumber(scaled(Number(range[1])), 1_000_000)
    salaryMax = clampNumber(scaled(Number(range[2])), 1_000_000)
  } else if (single) {
    salaryMin = clampNumber(scaled(Number(single[1])), 1_000_000)
  }
  const result: { salaryMin?: number; salaryMax?: number; salaryMonths?: number } = {}
  if (Number.isFinite(salaryMin ?? NaN)) {
    result.salaryMin = salaryMin
  }
  if (Number.isFinite(salaryMax ?? NaN)) {
    result.salaryMax = salaryMax
  }
  if (Number.isFinite(salaryMonths)) {
    result.salaryMonths = salaryMonths
  }
  return result
}

/** 旧版扁平格式 → CompanyTemplateV1；缺字段时保留空槽位，交给服务端报错。 */
export function upgradeLegacyTemplate(raw: Raw): Raw {
  const projectsRaw = Array.isArray(raw.projects) ? raw.projects : []
  const projectKeys = new Map<string, string>()
  const projects: Raw[] = projectsRaw.map((item, index) => {
    const source = isRecord(item) ? item : {}
    const name = textOr(source.name, `项目 ${index + 1}`)
    const key = `p${index + 1}`
    projectKeys.set(name, key)
    const status = textOr(source.status)
    const project: Raw = {
      key,
      name,
      season: textOr(source.season),
      description: textOr(source.description),
    }
    if ((PROJECT_STATUSES as readonly string[]).includes(status)) {
      project.status = status
    }
    const deadline = textOr(source.deadline)
    if (deadline) {
      project.deadline = deadline
    }
    return project
  })

  const jobsRaw = Array.isArray(raw.jobs) ? raw.jobs : []
  const jobs: Raw[] = jobsRaw.map((item, index) => {
    const source = isRecord(item) ? item : {}
    const projectName = textOr(source.project)
    const job: Raw = {
      key: `j${index + 1}`,
      projectKey: projectKeys.get(projectName) ?? projects[0]?.key ?? 'p1',
      pipelineKey: DEFAULT_PIPELINE_KEY,
      title: textOr(source.title),
      city: textOr(source.city),
      jobType: legacyJobType(textOr(source.type, '校招')),
      tags: listOr(source.tags),
      description: textOr(source.description),
    }
    const salary = parseSalaryText(textOr(source.salary))
    if (salary.salaryMin !== undefined) {
      job.salaryMin = salary.salaryMin
    }
    if (salary.salaryMax !== undefined) {
      job.salaryMax = salary.salaryMax
    }
    if (salary.salaryMonths !== undefined) {
      job.salaryMonths = salary.salaryMonths
    }
    return job
  })

  return {
    schemaVersion: 1,
    company: {
      name: textOr(raw.name),
      englishName: textOr(raw.englishName),
      industry: textOr(raw.industry),
      city: textOr(raw.city),
      size: textOr(raw.size),
      tagline: textOr(raw.tagline),
      description: textOr(raw.description),
      highlights: listOr(raw.highlights),
      logoText: textOr(raw.logoText),
      accent: isAccent(raw.accent) ? raw.accent : '#3D5A45',
    },
    projects,
    pipelines: [
      {
        key: DEFAULT_PIPELINE_KEY,
        projectKey: projects[0]?.key ?? 'p1',
        name: '标准面试流程',
        stages: defaultTemplateStages(),
      },
    ],
    jobs,
  }
}

/** 导入归一化入口：V1 原样透传，其余一律按旧版扁平格式升级。 */
export function normalizeTemplate(raw: unknown): Raw {
  if (!isRecord(raw)) {
    return { schemaVersion: 1 }
  }
  if (raw.schemaVersion === undefined) {
    return upgradeLegacyTemplate(raw)
  }
  return raw
}

// ---------- V1 → 预览 Company ----------

function projectPreviewId(projectKey: unknown, projects: Raw[]): string {
  const key = textOr(projectKey)
  const index = projects.findIndex((project) => project.key === key)
  return `preview-p${(index >= 0 ? index : 0) + 1}`
}

function pipelineInfo(index: number, source: Raw, projects: Raw[]): PipelineInfo {
  const stagesRaw = Array.isArray(source.stages) ? source.stages : []
  return {
    id: `preview-pl${index + 1}`,
    projectId: projectPreviewId(source.projectKey, projects),
    name: textOr(source.name, `流程 ${index + 1}`),
    stages: stagesRaw
      .filter(isRecord)
      .map((stage) => ({
        key: textOr(stage.key),
        kind: (STAGE_KINDS as readonly string[]).includes(textOr(stage.kind))
          ? (textOr(stage.kind) as StageKind)
          : 'hr_screen',
        title: textOr(stage.title, '面试阶段'),
      })),
  }
}

/** 校验通过后的本地预览；id 为临时合成值，不映射服务端标识。 */
export function templatePreviewFromV1(template: Raw): Company {
  const company = isRecord(template.company) ? template.company : {}
  const projects = Array.isArray(template.projects) ? template.projects.filter(isRecord) : []
  const pipelines = Array.isArray(template.pipelines) ? template.pipelines.filter(isRecord) : []
  const jobs = Array.isArray(template.jobs) ? template.jobs.filter(isRecord) : []

  return {
    id: 'preview-company',
    name: textOr(company.name, '未命名公司'),
    englishName: textOr(company.englishName),
    industry: textOr(company.industry),
    city: textOr(company.city),
    size: textOr(company.size),
    tagline: textOr(company.tagline),
    description: textOr(company.description),
    highlights: listOr(company.highlights),
    logoText: textOr(company.logoText, '预览'),
    accent: isAccent(company.accent) ? company.accent : '#3D5A45',
    source: 'user',
    isPrivate: false,
    isOwner: true,
    projects: projects.map((project, index) => ({
      id: projectPreviewId(project.key, projects),
      name: textOr(project.name, `项目 ${index + 1}`),
      season: textOr(project.season),
      status: (PROJECT_STATUSES as readonly string[]).includes(textOr(project.status))
        ? (textOr(project.status) as ProjectStatus)
        : 'open',
      deadline: textOr(project.deadline),
      description: textOr(project.description),
    })),
    pipelines: pipelines.map((pipeline, index) => pipelineInfo(index, pipeline, projects)),
    jobs: jobs.map((job, index) => {
      const salaryMin = typeof job.salaryMin === 'number' ? job.salaryMin : undefined
      const salaryMax = typeof job.salaryMax === 'number' ? job.salaryMax : undefined
      const salaryMonths = typeof job.salaryMonths === 'number' ? job.salaryMonths : undefined
      const jobType = textOr(job.jobType, 'campus')
      return {
        id: `preview-job${index + 1}`,
        projectId: projectPreviewId(job.projectKey, projects),
        pipelineId: textOr(job.pipelineKey, DEFAULT_PIPELINE_KEY),
        title: textOr(job.title),
        city: textOr(job.city),
        type: (JOB_TYPE_LABELS[jobType as JobType] ?? JOB_TYPE_LABELS.campus) as Job['type'],
        salary: salaryLabel(salaryMin, salaryMax, salaryMonths),
        tags: listOr(job.tags),
        description: textOr(job.description),
        status: 'open' as const,
        salaryMin: salaryMin ?? null,
        salaryMax: salaryMax ?? null,
        salaryMonths: salaryMonths ?? null,
      }
    }),
  }
}

// ---------- 提示词草稿生成 ----------

function hash(text: string): number {
  let value = 0
  for (const char of text) {
    value = (value * 31 + char.codePointAt(0)!) >>> 0
  }
  return value
}

function pick<T>(items: readonly T[], seed: number): T {
  return items[seed % items.length]!
}

function detectProfile(prompt: string): IndustryProfile {
  const lowered = prompt.toLowerCase()
  let best: IndustryProfile | undefined
  let bestHits = 0
  for (const profile of industryProfiles) {
    const hits = profile.keywords.filter((keyword) => lowered.includes(keyword)).length
    if (hits > bestHits) {
      best = profile
      bestHits = hits
    }
  }
  return best ?? fallbackProfile
}

function detectCity(prompt: string): string {
  return templateCities.find((city) => prompt.includes(city)) ?? '杭州'
}

function detectName(prompt: string, seed: number, profile: IndustryProfile): string {
  const quoted = prompt.match(/[「“"]([^」”"]{2,12})[」”"]/)
  if (quoted?.[1]) {
    return quoted[1].trim()
  }
  const suffix =
    profile === fallbackProfile
      ? pick(nameSuffixes, seed)
      : profile.type.split(' · ')[0]!.replace(/(与|和).*$/, '')
  return `${pick(namePrefixes, seed)}${suffix}`
}

function futureDate(days: number): string {
  const date = new Date()
  date.setDate(date.getDate() + days)
  return date.toISOString().slice(0, 10)
}

/** 生成 CompanyTemplateV1 草稿；校招与实习岗拆分为两个项目。 */
export function generateCompanyTemplate(prompt: string): Record<string, unknown> {
  const trimmed = prompt.trim()
  const seed = hash(trimmed)
  const profile = detectProfile(trimmed)
  const city = detectCity(trimmed)
  const name = detectName(trimmed, seed, profile)
  const size = pick(companySizes, seed >>> 3)
  const campusProject = `${new Date().getFullYear()} 秋季校园招聘`
  const internProject = '日常实习计划'

  const campusJobs = profile.jobs.filter((job) => job.type !== '实习')
  const internJobs = profile.jobs.filter((job) => job.type === '实习')

  const projects: Raw[] = [
    {
      key: 'p1',
      name: campusProject,
      season: `${new Date().getFullYear()} 秋招`,
      status: 'open',
      deadline: futureDate(45),
      description: `面向应届生，开放${campusJobs.map((job) => job.title).join('、')}方向。`,
    },
  ]
  if (internJobs.length > 0) {
    projects.push({
      key: 'p2',
      name: internProject,
      season: '日常实习',
      status: 'closing',
      deadline: futureDate(20),
      description: `每周至少到岗 3 天，${internJobs.map((job) => job.title).join('、')}方向可转正。`,
    })
  }

  const jobs: Raw[] = profile.jobs.map((job, index) => {
    const salary = parseSalaryText(job.salary)
    return {
      key: `j${index + 1}`,
      projectKey: job.type === '实习' ? 'p2' : 'p1',
      pipelineKey: DEFAULT_PIPELINE_KEY,
      title: job.title,
      city,
      jobType: legacyJobType(job.type),
      ...salary,
      tags: job.tags,
      description: job.description,
    }
  })

  return {
    schemaVersion: 1,
    company: {
      name,
      englishName: `${pick(englishPrefixes, seed >>> 5)} ${pick(englishSuffixes, seed >>> 7)}`,
      industry: profile.industry,
      city,
      size,
      tagline: profile.tagline,
      description: `${name}是一家位于${city}的${profile.type}团队，专注${profile.field}。${profile.blurb}`,
      highlights: [
        `${city} · ${pick(workStyles, seed >>> 9)}`,
        `专注${profile.field}`,
        `${pick(['服务 200+ 客户', '产品用户 8 万+', '连续三年稳定增长', '核心团队共事 5 年'], seed >>> 11)}`,
      ],
      logoText: [...name].slice(0, 2).join(''),
      accent: profile.accent,
    },
    projects,
    pipelines: [
      {
        key: DEFAULT_PIPELINE_KEY,
        projectKey: 'p1',
        name: '标准面试流程',
        stages: defaultTemplateStages(),
      },
    ],
    jobs,
  }
}
