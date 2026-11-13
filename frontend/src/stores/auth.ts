import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { authApi } from '@/api/endpoints/auth'
import { ApiError } from '@/api/error'
import type { ProfilePatchPayload, User } from '@/api/types'

export const useAuthStore = defineStore('auth', () => {
  // 会话令牌存于 HttpOnly Cookie，前端不接触令牌；只维护当前用户。
  const user = ref<User | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  let bootstrapPromise: Promise<boolean> | null = null

  const isAuthenticated = computed(() => user.value !== null)

  function applyUser(nextUser: User) {
    user.value = nextUser
  }

  function clearUser() {
    user.value = null
  }

  async function register(
    username: string,
    password: string,
    displayName: string,
  ): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      applyUser(await authApi.register({ username, password, displayName }))
      return true
    } catch (cause) {
      error.value = describeAuthError(cause, '注册失败，请稍后再试', {
        conflict: '用户名已被占用，请换一个',
      })
      return false
    } finally {
      loading.value = false
    }
  }

  async function login(username: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      applyUser(await authApi.login({ username, password }))
      return true
    } catch (cause) {
      error.value = describeAuthError(cause, '登录失败，请稍后再试')
      return false
    } finally {
      loading.value = false
    }
  }

  /** 刷新后用 Cookie 恢复登录态；并发调用共享同一次请求。 */
  async function bootstrap(): Promise<boolean> {
    if (user.value) {
      return true
    }
    if (bootstrapPromise) {
      return bootstrapPromise
    }

    bootstrapPromise = authApi
      .me()
      .then((profile) => {
        applyUser(profile)
        return true
      })
      .catch(() => {
        clearUser()
        return false
      })
      .finally(() => {
        bootstrapPromise = null
      })

    return bootstrapPromise
  }

  async function logout(): Promise<void> {
    const hadUser = user.value !== null
    clearUser()
    if (hadUser) {
      try {
        await authApi.logout()
      } catch {
        // 本地会话已清除；服务端会话会按空闲/绝对超时失效。
      }
    }
  }

  async function updateProfile(patch: ProfilePatchPayload): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      applyUser(await authApi.updateProfile(patch))
      return true
    } catch (cause) {
      error.value = describeAuthError(cause, '保存失败，请稍后再试')
      return false
    } finally {
      loading.value = false
    }
  }

  /** 改密成功后后端会轮换 Cookie，前端无需额外处理。 */
  async function changePassword(currentPassword: string, newPassword: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      await authApi.changePassword({ currentPassword, newPassword })
      return true
    } catch (cause) {
      error.value = describeAuthError(cause, '修改失败，请稍后再试')
      return false
    } finally {
      loading.value = false
    }
  }

  return {
    user,
    loading,
    error,
    isAuthenticated,
    register,
    login,
    bootstrap,
    logout,
    updateProfile,
    changePassword,
    /** 供 401 全局处理使用：只清本地状态，不再调接口。 */
    clearUser,
  }
})

/** 按错误码细化文案；未识别的错误退回通用文案。 */
function describeAuthError(
  cause: unknown,
  fallback: string,
  overrides: Record<string, string> = {},
): string {
  if (cause instanceof ApiError) {
    const override = overrides[cause.code]
    if (override) {
      return override
    }
    return cause.message || fallback
  }
  return fallback
}
