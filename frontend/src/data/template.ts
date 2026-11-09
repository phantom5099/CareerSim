import type { Company, Job, ProjectStatus, RecruitmentProject } from '@/domain/types'

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

export type CompanyTemplateResult =
  | { ok: true; company: Company }
  | { ok: false; errors: string[] }

export function validateCompanyTemplate(raw: unknown): CompanyTemplateResult {
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

export const companyTemplateSample = {
  name: '南风数据',
  englishName: 'SOUTHWIND DATA',
  industry: '数据服务',
  city: '深圳',
  size: '100–499 人',
  tagline: '把报表做成一次真正的决策',
  description: '南风数据为零售与消费品牌提供经营分析平台，团队由分析师、工程师与行业顾问组成。',
  highlights: ['服务 200+ 消费品牌', '每周五下午固定研究时间', '深圳南山 · 地铁直达'],
  logoText: '南风',
  accent: '#3D5A45',
  projects: [
    {
      name: '2026 秋季校园招聘',
      season: '2026 秋招',
      status: 'open',
      deadline: '2026-12-20',
      description: '面向 2026 届毕业生，开放数据分析与前端方向。',
    },
  ],
  jobs: [
    {
      title: '数据分析师',
      project: '2026 秋季校园招聘',
      industry: '商业分析',
      city: '深圳',
      type: '校招',
      salary: '16–24K · 14 薪',
      tags: ['SQL', 'BI', '零售'],
      description: '负责经营指标体系搭建与专题分析，和业务团队一起做决策。',
    },
  ],
}

export const companyTemplateJson = JSON.stringify(companyTemplateSample, null, 2)
