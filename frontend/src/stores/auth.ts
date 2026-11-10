import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { authApi } from '@/api/endpoints/auth'
import { ApiError } from '@/api/error'
import type { User } from '@/api/types'

export const useAuthStore = defineStore('auth', () => {
  // 会话只存在内存里：刷新后需重新登录，真实 token 续期由后端接口负责。
  const token = ref<string | null>(null)
  const user = ref<User | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  let bootstrapPromise: Promise<boolean> | null = null

  const isAuthenticated = computed(() => token.value !== null && user.value !== null)

  function applySession(nextToken: string, nextUser: User) {
    token.value = nextToken
    user.value = nextUser
  }

  function clearSession() {
    token.value = null
    user.value = null
  }

  async function login(username: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      const session = await authApi.login({ username, password })
      applySession(session.token, session.user)
      return true
    } catch (cause) {
      error.value = cause instanceof ApiError ? cause.message : '登录失败，请稍后再试'
      return false
    } finally {
      loading.value = false
    }
  }

  async function bootstrap(): Promise<boolean> {
    if (!token.value) {
      return false
    }
    if (user.value) {
      return true
    }
    if (bootstrapPromise) {
      return bootstrapPromise
    }

    const currentToken = token.value
    loading.value = true
    bootstrapPromise = authApi
      .me(currentToken)
      .then((profile) => {
        if (token.value === currentToken) {
          user.value = profile
        }
        return token.value !== null
      })
      .catch((cause) => {
        if (cause instanceof ApiError && cause.status === 401) {
          clearSession()
        }
        return false
      })
      .finally(() => {
        loading.value = false
        bootstrapPromise = null
      })

    return bootstrapPromise
  }

  async function logout(): Promise<void> {
    const currentToken = token.value
    clearSession()
    if (currentToken) {
      try {
        await authApi.logout(currentToken)
      } catch {
        // 本地会话已经清除，服务端 token 会自行过期。
      }
    }
  }

  return {
    token,
    user,
    loading,
    error,
    isAuthenticated,
    login,
    bootstrap,
    logout,
  }
})
