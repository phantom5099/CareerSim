import type { ResumeApi } from '@/api/endpoints/resume'
import type { ResumePayload } from '@/api/types'

// 后端简历接口尚未接入：草稿只保存在内存里。
let draft: ResumePayload | null = null

export const mockResumeApi: ResumeApi = {
  async load(): Promise<ResumePayload | null> {
    return draft ? { ...draft } : null
  },

  async save(payload: ResumePayload): Promise<void> {
    draft = { ...payload }
  },
}
