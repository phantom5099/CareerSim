import { ApiError } from '../error'
import { apiFetch } from '../http'
import { RESUME_LIMIT } from '../types'
import type { ResumeCreateInput, ResumeDocument, ResumeUpdateInput } from '../types'

export interface ResumesApi {
  list(): Promise<ResumeDocument[]>
  create(input: ResumeCreateInput): Promise<ResumeDocument>
  /** multipart 上传：`title` + `file`；同步解析，返回含 parseStatus 的简历。 */
  uploadAttachment(title: string, file: File): Promise<ResumeDocument>
  update(id: string, input: ResumeUpdateInput): Promise<ResumeDocument>
  remove(id: string): Promise<void>
  setDefault(id: string): Promise<ResumeDocument>
  /** 附件下载地址；鉴权由会话 Cookie 承担，不暴露存储键。 */
  attachmentDownloadUrl(id: string): string
}

interface ApiErrorBody {
  error?: { code?: string }
}

async function readErrorCode(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as ApiErrorBody
    return body.error?.code ?? 'API_ERROR'
  } catch {
    return 'API_ERROR'
  }
}

/** 上传错误的文案映射；与 http.ts 同一口径，但更具体。 */
function uploadMessage(status: number, code: string): string {
  switch (code) {
    case 'validation_failed':
      return '简历标题或文件不符合要求，请修改后重试'
    case 'payload_too_large':
      return '附件超过 10MB 上限，请精简后重试'
    case 'unsupported_media_type':
      return '仅支持 PDF / DOCX 文件，且文件内容需与扩展名一致'
    case 'conflict':
      return `最多保存 ${RESUME_LIMIT} 份简历，请先删除一份`
    case 'unauthenticated':
      return '登录已失效，请重新登录'
    case 'internal_error':
      return '解析服务暂时不可用，请稍后再试'
    default:
      return status >= 500 ? '服务暂时不可用，请稍后再试' : '上传失败，请稍后再试'
  }
}

/** FormData 上传不能走 apiFetch（它强制 JSON Content-Type），这里用原始 fetch。 */
async function upload(title: string, file: File): Promise<ResumeDocument> {
  const form = new FormData()
  form.append('title', title)
  form.append('file', file)

  let response: Response
  try {
    response = await fetch('/api/v1/resumes/attachment', {
      method: 'POST',
      credentials: 'include',
      body: form,
    })
  } catch {
    throw new ApiError('网络异常，请检查连接后重试', 0, 'NETWORK_ERROR')
  }

  if (!response.ok) {
    const code = await readErrorCode(response)
    throw new ApiError(uploadMessage(response.status, code), response.status, code)
  }
  return (await response.json()) as ResumeDocument
}

export const resumeApi: ResumesApi = {
  list(): Promise<ResumeDocument[]> {
    return apiFetch<ResumeDocument[]>('/api/v1/resumes')
  },

  create(input: ResumeCreateInput): Promise<ResumeDocument> {
    return apiFetch<ResumeDocument>('/api/v1/resumes', {
      method: 'POST',
      body: JSON.stringify(input),
    })
  },

  uploadAttachment(title: string, file: File): Promise<ResumeDocument> {
    return upload(title, file)
  },

  update(id: string, input: ResumeUpdateInput): Promise<ResumeDocument> {
    return apiFetch<ResumeDocument>(`/api/v1/resumes/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(input),
    })
  },

  async remove(id: string): Promise<void> {
    await apiFetch<void>(`/api/v1/resumes/${id}`, { method: 'DELETE' })
  },

  setDefault(id: string): Promise<ResumeDocument> {
    return apiFetch<ResumeDocument>(`/api/v1/resumes/${id}/default`, { method: 'PUT' })
  },

  attachmentDownloadUrl(id: string): string {
    return `/api/v1/resumes/${id}/attachment`
  },
}
