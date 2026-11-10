import type { CompanyTemplateResult } from '@/api/endpoints/companies'
import type { Company, Job, ProjectStatus, RecruitmentProject } from '@/domain/types'

import {
  companySizes,
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

const PROJECT_STATUSES: ProjectStatus[] = ['open', 'closing', 'closed']
const JOB_TYPES: Job['type'][] = ['校招', '实习', '社招']

type Raw = Record<string, unknown>

function isRecord(value: unknown): value is Raw {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function readText(source: Raw, key: string, label: string, errors: string[]): string {
  const value = source[key]
  if (typeof value !== 'string' || value.trim().length === 0) {
    errors.push(`${label} 不能为空`)
    return ''
  }
  return value.trim()
}

function readTextList(source: Raw, key: string, label: string, errors: string[]): string[] {
  const value = source[key]
  if (!Array.isArray(value)) {
    errors.push(`${label} 必须是字符串数组`)
    return []
  }
  const items = value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
  if (items.length === 0) {
    errors.push(`${label} 至少需要一项`)
  }
  return items
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`
}

function parseProjects(value: unknown, errors: string[]): RecruitmentProject[] {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push('projects 至少需要 1 个招聘项目')
    return []
  }

  const projects: RecruitmentProject[] = []
  value.forEach((item, index) => {
    if (!isRecord(item)) {
      errors.push(`projects[${index}] 必须是对象`)
      return
    }
    const rawStatus = item.status
    const status =
      typeof rawStatus === 'string' && PROJECT_STATUSES.includes(rawStatus as ProjectStatus)
        ? (rawStatus as ProjectStatus)
        : 'open'
    if (status !== rawStatus) {
      errors.push(`projects[${index}].status 只能是 open / closing / closed`)
    }
    projects.push({
      id: makeId('project'),
      name: readText(item, 'name', `projects[${index}].name`, errors),
      season: readText(item, 'season', `projects[${index}].season`, errors),
      status,
      deadline: readText(item, 'deadline', `projects[${index}].deadline`, errors),
      description: readText(item, 'description', `projects[${index}].description`, errors),
    })
  })
  return projects
}

function parseJobs(
  value: unknown,
  projects: RecruitmentProject[],
  companyIndustry: string,
  errors: string[],
): Job[] {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push('jobs 至少需要 1 个岗位')
    return []
  }

  const projectIds = new Map(projects.map((project) => [project.name, project.id]))
  const defaultProjectId = projects[0]?.id

  const jobs: Job[] = []
  value.forEach((item, index) => {
    if (!isRecord(item)) {
      errors.push(`jobs[${index}] 必须是对象`)
      return
    }
    const rawType = item.type
    const type =
      typeof rawType === 'string' && JOB_TYPES.includes(rawType as Job['type'])
        ? (rawType as Job['type'])
        : '校招'
    if (type !== rawType) {
      errors.push(`jobs[${index}].type 只能是 校招 / 实习 / 社招`)
    }
    let projectId = defaultProjectId
    const rawProject = item.project
    if (rawProject !== undefined) {
      if (typeof rawProject !== 'string' || rawProject.trim().length === 0) {
        errors.push(`jobs[${index}].project 必须是招聘项目名称`)
      } else if (projectIds.has(rawProject.trim())) {
        projectId = projectIds.get(rawProject.trim())!
      } else {
        errors.push(`jobs[${index}].project 未匹配到同名招聘项目`)
      }
    }
    let industry = companyIndustry
    const rawIndustry = item.industry
    if (rawIndustry !== undefined) {
      if (typeof rawIndustry !== 'string' || rawIndustry.trim().length === 0) {
        errors.push(`jobs[${index}].industry 必须是非空字符串`)
      } else {
        industry = rawIndustry.trim()
      }
    }
    jobs.push({
      id: makeId('job'),
      projectId,
      industry,
      title: readText(item, 'title', `jobs[${index}].title`, errors),
      city: readText(item, 'city', `jobs[${index}].city`, errors),
      type,
      salary: readText(item, 'salary', `jobs[${index}].salary`, errors),
      tags: readTextList(item, 'tags', `jobs[${index}].tags`, errors),
      description: readText(item, 'description', `jobs[${index}].description`, errors),
    })
  })
  return jobs
}

export function parseCompanyTemplate(raw: unknown): CompanyTemplateResult {
  if (!isRecord(raw)) {
    return { ok: false, errors: ['模板必须是一个 JSON 对象'] }
  }

  const errors: string[] = []
  const rawAccent = raw.accent
  const accent =
    typeof rawAccent === 'string' && /^#[0-9a-fA-F]{6}$/.test(rawAccent) ? rawAccent : '#3D5A45'
  if (rawAccent !== undefined && accent !== rawAccent) {
    errors.push('accent 必须是 #RRGGBB 格式的十六进制颜色')
  }

  const industry = readText(raw, 'industry', 'industry', errors)
  const projects = parseProjects(raw.projects, errors)
  const jobs = parseJobs(raw.jobs, projects, industry, errors)

  const company: Company = {
    id: makeId('local'),
    name: readText(raw, 'name', 'name', errors),
    englishName: readText(raw, 'englishName', 'englishName', errors),
    industry,
    city: readText(raw, 'city', 'city', errors),
    size: readText(raw, 'size', 'size', errors),
    tagline: readText(raw, 'tagline', 'tagline', errors),
    description: readText(raw, 'description', 'description', errors),
    highlights: readTextList(raw, 'highlights', 'highlights', errors),
    logoText: readText(raw, 'logoText', 'logoText', errors),
    accent,
    featured: false,
    imported: true,
    projects,
    jobs,
  }

  return errors.length > 0 ? { ok: false, errors } : { ok: true, company }
}

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

export function generateCompanyTemplate(prompt: string): Record<string, unknown> {
  const trimmed = prompt.trim()
  const seed = hash(trimmed)
  const profile = detectProfile(trimmed)
  const city = detectCity(trimmed)
  const name = detectName(trimmed, seed, profile)
  const size = pick(companySizes, seed >>> 3)
  const track = profile.type.split(' · ')[0]!
  const campusProject = `${new Date().getFullYear()} 秋季校园招聘`
  const internProject = '日常实习计划'

  const campusJobs = profile.jobs.filter((job) => job.type !== '实习')
  const internJobs = profile.jobs.filter((job) => job.type === '实习')

  return {
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
    projects: [
      {
        name: campusProject,
        season: `${new Date().getFullYear()} 秋招`,
        status: 'open',
        deadline: futureDate(45),
        description: `面向应届生，开放${campusJobs.map((job) => job.title).join('、')}方向。`,
      },
      {
        name: internProject,
        season: '日常实习',
        status: 'closing',
        deadline: futureDate(20),
        description: `每周至少到岗 3 天，${internJobs.map((job) => job.title).join('、')}方向可转正。`,
      },
    ],
    jobs: profile.jobs.map((job) => ({
      title: job.title,
      project: job.type === '实习' ? internProject : campusProject,
      industry: track,
      city,
      type: job.type,
      salary: job.salary,
      tags: job.tags,
      description: job.description,
    })),
  }
}
