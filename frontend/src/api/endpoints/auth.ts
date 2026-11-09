import { request } from '@/api/http'
import type { AuthSession, LoginPayload, User } from '@/api/types'

export const authApi = {
  login(payload: LoginPayload): Promise<AuthSession> {
    return request<AuthSession>('/auth/login', {
      method: 'POST',
      body: payload,
      token: null,
    })
  },

  me(token: string): Promise<User> {
    return request<User>('/auth/me', { token })
  },

  logout(token: string): Promise<void> {
    return request<void>('/auth/logout', { method: 'POST', token })
  },
}
