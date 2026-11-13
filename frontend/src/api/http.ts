import { ApiError } from './error'

/**
 * 统一 fetch 封装：
 * - 所有请求携带 `credentials: 'include'`，会话经 HttpOnly Cookie 传递；
 * - S-H02 错误体 `{error:{code,message,fields?}}` 映射为用户可读中文文案；
 * - 401 时通知订阅者（跳登录页），认证接口自身除外。
 */

const JSON_HEADERS = { 'Content-Type': 'application/json' } as const

/** 认证接口自身的 401 交给调用方展示，不触发全局跳转。 */
const AUTH_PATHS = [
  '/api/v1/auth/register',
  '/api/v1/auth/login',
  '/api/v1/auth/logout',
  '/api/v1/auth/me',
] as const

type UnauthorizedListener = () => void

let unauthorizedListener: UnauthorizedListener | null = null

/** 由应用入口注册：收到非认证接口的 401 时清空本地会话并跳登录。 */
export function onUnauthorized(listener: UnauthorizedListener): void {
  unauthorizedListener = listener
}

interface ApiErrorBody {
  error?: { code?: string; message?: string; fields?: string[] }
}

/** 按 code 生成用户可读文案；后端英文 message 不直接透出。 */
function readableMessage(status: number, code: string): string {
  switch (code) {
    case 'validation_failed':
      return '输入内容不符合要求，请检查后重试'
    case 'current_password_mismatch':
      return '当前密码不正确'
    case 'invalid_credentials':
      return '账号或密码不正确'
    case 'unauthenticated':
      return '登录已失效，请重新登录'
    case 'forbidden':
      return '没有权限执行该操作'
    case 'not_found':
      return '资源不存在或已被删除'
    case 'conflict':
      return '与现有数据冲突，请调整后重试'
    case 'payload_too_large':
      return '提交内容过大，请精简后重试'
    case 'unsupported_media_type':
      return '不支持的文件类型'
    case 'rate_limited':
      return '操作过于频繁，请稍后再试'
    case 'internal_error':
      return '服务暂时不可用，请稍后再试'
    default:
      return status >= 500 ? '服务暂时不可用，请稍后再试' : '请求失败，请稍后再试'
  }
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  let response: Response
  try {
    response = await fetch(path, {
      credentials: 'include',
      ...init,
      headers: { ...JSON_HEADERS, ...(init.headers ?? {}) },
    })
  } catch {
    // 网络层失败（断网、超时、代理中断）。
    throw new ApiError('网络异常，请检查连接后重试', 0, 'NETWORK_ERROR')
  }

  // 非 2xx：解析 S-H02 错误体并映射文案。
  if (!response.ok) {
    const code = await readErrorCode(response)
    if (response.status === 401 && unauthorizedListener && !isAuthPath(path)) {
      unauthorizedListener()
    }
    throw new ApiError(readableMessage(response.status, code), response.status, code)
  }

  // 204 无响应体；其余尝试解析 JSON。
  if (response.status === 204) {
    return undefined as T
  }
  try {
    return (await response.json()) as T
  } catch {
    throw new ApiError('服务响应格式异常，请稍后再试', response.status, 'BAD_RESPONSE')
  }
}

async function readErrorCode(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as ApiErrorBody
    return body.error?.code ?? 'API_ERROR'
  } catch {
    return 'API_ERROR'
  }
}

function isAuthPath(path: string): boolean {
  return (AUTH_PATHS as readonly string[]).includes(path)
}
