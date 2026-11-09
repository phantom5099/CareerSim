import { demoConfig } from '@/config/demo'
import { ApiError } from '@/api/error'
import type { AuthApi } from '@/api/endpoints/auth'
import type { AuthSession, LoginPayload, User } from '@/api/types'

// 后端认证接口尚未接入：这里按 /auth/* 的契约返回演示账号。
const demoUser: User = {
  id: 'user-demo',
  username: demoConfig.username,
  displayName: '林同学',
  school: '南山大学',
  major: '计算机科学',
  graduationYear: 2027,
  targetRoles: ['前端工程师', '全栈工程师'],
  targetCities: ['杭州', '上海'],
  avatarText: '林',
  avatarColor: '#3D5A45',
}

let activeToken: string | null = null

function issueToken(): string {
  return `mock-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
}

export const mockAuthApi: AuthApi = {
  async login(payload: LoginPayload): Promise<AuthSession> {
    if (payload.username !== demoConfig.username || payload.password !== demoConfig.password) {
      throw new ApiError('用户名或密码不正确', 401, 'INVALID_CREDENTIALS')
    }
    activeToken = issueToken()
    return {
      token: activeToken,
      expiresAt: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
      user: demoUser,
    }
  },

  async me(token: string): Promise<User> {
    if (!activeToken || token !== activeToken) {
      throw new ApiError('登录已失效，请重新登录', 401, 'UNAUTHORIZED')
    }
    return demoUser
  },

  async logout(token: string): Promise<void> {
    if (token === activeToken) {
      activeToken = null
    }
  },
}
