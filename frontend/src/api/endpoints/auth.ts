import { apiFetch } from '../http'
import type {
  ChangePasswordPayload,
  LoginPayload,
  ProfilePatchPayload,
  RegisterPayload,
  User,
} from '../types'

export interface AuthApi {
  /** 注册即登录：201 返回用户，会话经 Set-Cookie 建立。 */
  register(payload: RegisterPayload): Promise<User>
  login(payload: LoginPayload): Promise<User>
  me(): Promise<User>
  logout(): Promise<void>
  updateProfile(patch: ProfilePatchPayload): Promise<User>
  changePassword(payload: ChangePasswordPayload): Promise<void>
}

export const authApi: AuthApi = {
  async register(payload: RegisterPayload): Promise<User> {
    return apiFetch<User>('/api/v1/auth/register', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  async login(payload: LoginPayload): Promise<User> {
    return apiFetch<User>('/api/v1/auth/login', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  async me(): Promise<User> {
    return apiFetch<User>('/api/v1/auth/me')
  },

  async logout(): Promise<void> {
    await apiFetch<void>('/api/v1/auth/logout', { method: 'POST' })
  },

  async updateProfile(patch: ProfilePatchPayload): Promise<User> {
    return apiFetch<User>('/api/v1/me', {
      method: 'PATCH',
      body: JSON.stringify(patch),
    })
  },

  async changePassword(payload: ChangePasswordPayload): Promise<void> {
    await apiFetch<void>('/api/v1/me/password', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },
}
