import { ApiError } from '@/api/error'
import type { ResumesApi } from '@/api/endpoints/resume'
import { RESUME_LIMIT } from '@/api/types'
import type { ResumeDocument, ResumeInput } from '@/api/types'

import { seedResumes } from './data/resumes'

// 简历接口尚未接入后端：内存里维护用户的多份简历。
let resumes: ResumeDocument[] = structuredClone(seedResumes)

function find(id: string): ResumeDocument {
  const resume = resumes.find((item) => item.id === id)
  if (!resume) {
    throw new ApiError('简历不存在', 404, 'RESUME_NOT_FOUND')
  }
  return resume
}

/** 投递时由投递接口解析简历标题，模拟服务端关联。 */
export function resumeTitleFor(id: string): string | undefined {
  return resumes.find((resume) => resume.id === id)?.title
}

function build(input: ResumeInput, id: string, isDefault: boolean): ResumeDocument {
  const meta = {
    id,
    title: input.title.trim() || '未命名简历',
    updatedAt: new Date().toISOString(),
    isDefault,
  }
  return input.kind === 'online'
    ? { ...meta, kind: 'online', payload: { ...input.payload } }
    : { ...meta, kind: 'attachment', attachment: { ...input.attachment } }
}

export const mockResumeApi: ResumesApi = {
  async list(): Promise<ResumeDocument[]> {
    return structuredClone(resumes)
  },

  async create(input): Promise<ResumeDocument> {
    if (resumes.length >= RESUME_LIMIT) {
      throw new ApiError(`最多保存 ${RESUME_LIMIT} 份简历`, 409, 'RESUME_LIMIT_REACHED')
    }
    const created = build(input, `resume-${Date.now().toString(36)}`, resumes.length === 0)
    resumes = [created, ...resumes]
    return structuredClone(created)
  },

  async update(id, input): Promise<ResumeDocument> {
    const current = find(id)
    const updated = build(input, id, current.isDefault)
    resumes = resumes.map((resume) => (resume.id === id ? updated : resume))
    return structuredClone(updated)
  },

  async remove(id): Promise<void> {
    const current = find(id)
    resumes = resumes.filter((resume) => resume.id !== id)
    if (current.isDefault && resumes.length > 0) {
      resumes[0]!.isDefault = true
    }
  },

  async setDefault(id): Promise<ResumeDocument> {
    find(id)
    resumes = resumes.map((resume) => ({ ...resume, isDefault: resume.id === id }))
    return structuredClone(resumes.find((resume) => resume.id === id)!)
  },
}
