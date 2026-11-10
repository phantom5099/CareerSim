import { mockAuthApi } from '@/mock/auth'

import type { AuthSession, LoginPayload, User } from '../types'

export interface AuthApi {
  login(payload: LoginPayload): Promise<AuthSession>
  me(token: string): Promise<User>
  logout(token: string): Promise<void>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const authApi: AuthApi = mockAuthApi
