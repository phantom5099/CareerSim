<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import { brand } from '@/content/brand'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const auth = useAuthStore()

const form = reactive({
  username: '',
  displayName: '',
  password: '',
  confirmPassword: '',
})
const showPassword = ref(false)
const touched = ref(false)

const USERNAME_PATTERN = /^[a-z0-9_]{3,32}$/

const usernameError = computed(() => {
  if (!touched.value) {
    return ''
  }
  const value = form.username.trim()
  if (value.length === 0) {
    return '请输入账号'
  }
  if (!USERNAME_PATTERN.test(value)) {
    return '账号需 3-32 位小写字母、数字或下划线'
  }
  return ''
})
const displayNameError = computed(() => {
  if (!touched.value) {
    return ''
  }
  const length = [...form.displayName.trim()].length
  if (length === 0) {
    return '请输入昵称'
  }
  if (length > 40) {
    return '昵称不能超过 40 个字符'
  }
  return ''
})
const passwordError = computed(() => {
  if (!touched.value) {
    return ''
  }
  if (form.password.length === 0) {
    return '请输入密码'
  }
  if (form.password.length < 8) {
    return '密码至少 8 位'
  }
  if (form.password.length > 128) {
    return '密码最长 128 位'
  }
  return ''
})
const confirmPasswordError = computed(() => {
  if (!touched.value) {
    return ''
  }
  if (form.confirmPassword.length === 0) {
    return '请再次输入密码'
  }
  if (form.confirmPassword !== form.password) {
    return '两次输入的密码不一致'
  }
  return ''
})
const canSubmit = computed(
  () =>
    USERNAME_PATTERN.test(form.username.trim()) &&
    form.displayName.trim().length > 0 &&
    form.password.length >= 8 &&
    form.password === form.confirmPassword &&
    !auth.loading,
)

async function submit() {
  touched.value = true
  if (!canSubmit.value) {
    return
  }

  if (await auth.register(form.username.trim(), form.password, form.displayName.trim())) {
    await router.replace('/')
  }
}

const year = new Date().getFullYear()
</script>

<template>
  <main class="min-h-dvh px-5 py-5 sm:px-8 lg:px-12 lg:py-7">
    <header class="border-b-[3px] border-double border-ink pb-4">
      <RouterLink :to="{ name: 'login' }" class="flex items-center gap-3">
        <span
          class="flex h-10 w-10 items-center justify-center bg-ink font-display text-xl leading-none text-paper-light"
        >
          C
        </span>
        <div>
          <p class="font-display text-2xl leading-none tracking-wide">{{ brand.chineseName }}</p>
          <p class="mt-1 text-[0.62rem] tracking-[0.16em] text-ink-faint">
            CAREERSIM · 求职模拟器
          </p>
        </div>
      </RouterLink>
    </header>

    <div class="mx-auto grid max-w-xl gap-9 pt-8 lg:pt-12">
      <section class="animate-rise">
        <div class="border border-ink bg-paper-light">
          <div
            class="flex items-center justify-between border-b border-ink px-5 py-3 text-[0.64rem] text-ink-faint sm:px-7"
          >
            <span>新用户登记 / REGISTER</span>
            <span>NO. 0002</span>
          </div>

          <div class="px-5 py-6 sm:px-7 sm:py-8">
            <h1 class="font-display text-[1.8rem] leading-tight">创建你的模拟档案</h1>
            <p class="mt-2 text-xs leading-5 text-ink-soft">
              注册后即可开始演练；账号仅用于本模拟器。
            </p>

            <form class="mt-7 space-y-5" novalidate @submit.prevent="submit">
              <div>
                <label for="username" class="mb-1.5 block text-[0.8rem] text-ink">账号</label>
                <input
                  id="username"
                  v-model="form.username"
                  class="field"
                  type="text"
                  name="username"
                  autocomplete="username"
                  placeholder="小写字母、数字或下划线"
                  :aria-invalid="Boolean(usernameError)"
                  :aria-describedby="usernameError ? 'username-error' : undefined"
                />
                <p v-if="usernameError" id="username-error" class="mt-1.5 text-xs text-vermilion">
                  {{ usernameError }}
                </p>
              </div>

              <div>
                <label for="displayName" class="mb-1.5 block text-[0.8rem] text-ink">昵称</label>
                <input
                  id="displayName"
                  v-model="form.displayName"
                  class="field"
                  type="text"
                  name="displayName"
                  placeholder="面试官会看到的称呼"
                  :aria-invalid="Boolean(displayNameError)"
                  :aria-describedby="displayNameError ? 'displayName-error' : undefined"
                />
                <p
                  v-if="displayNameError"
                  id="displayName-error"
                  class="mt-1.5 text-xs text-vermilion"
                >
                  {{ displayNameError }}
                </p>
              </div>

              <div>
                <label for="password" class="mb-1.5 block text-[0.8rem] text-ink">密码</label>
                <div class="relative">
                  <input
                    id="password"
                    v-model="form.password"
                    class="field pr-11"
                    :type="showPassword ? 'text' : 'password'"
                    name="password"
                    autocomplete="new-password"
                    placeholder="至少 8 位"
                    :aria-invalid="Boolean(passwordError)"
                    :aria-describedby="passwordError ? 'password-error' : undefined"
                  />
                  <button
                    type="button"
                    class="absolute inset-y-0 right-0 flex items-center px-3 text-ink-faint transition hover:text-ink"
                    :aria-label="showPassword ? '隐藏密码' : '显示密码'"
                    @click="showPassword = !showPassword"
                  >
                    <svg
                      viewBox="0 0 16 16"
                      class="h-4 w-4"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="1.4"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      aria-hidden="true"
                    >
                      <path d="M1.5 8S4 3.9 8 3.9 14.5 8 14.5 8 12 12.1 8 12.1 1.5 8 1.5 8Z" />
                      <circle cx="8" cy="8" r="2.1" />
                      <path v-if="showPassword" d="M2.6 2.6l10.8 10.8" />
                    </svg>
                  </button>
                </div>
                <p v-if="passwordError" id="password-error" class="mt-1.5 text-xs text-vermilion">
                  {{ passwordError }}
                </p>
              </div>

              <div>
                <label for="confirmPassword" class="mb-1.5 block text-[0.8rem] text-ink">
                  确认密码
                </label>
                <input
                  id="confirmPassword"
                  v-model="form.confirmPassword"
                  class="field"
                  type="password"
                  name="confirmPassword"
                  autocomplete="new-password"
                  placeholder="再输入一次"
                  :aria-invalid="Boolean(confirmPasswordError)"
                  :aria-describedby="confirmPasswordError ? 'confirmPassword-error' : undefined"
                />
                <p
                  v-if="confirmPasswordError"
                  id="confirmPassword-error"
                  class="mt-1.5 text-xs text-vermilion"
                >
                  {{ confirmPasswordError }}
                </p>
              </div>

              <p
                v-if="auth.error"
                role="alert"
                class="border border-vermilion/40 bg-vermilion/5 px-3 py-2.5 text-sm text-vermilion-deep"
              >
                {{ auth.error }}
              </p>

              <button type="submit" class="btn-solid w-full" :disabled="!canSubmit">
                {{ auth.loading ? '正在登记…' : '登记入场 →' }}
              </button>

              <p class="text-center text-xs text-ink-soft">
                已有账号？
                <RouterLink
                  :to="{ name: 'login' }"
                  class="text-vermilion-deep underline-offset-2 hover:underline"
                >
                  直接登录
                </RouterLink>
              </p>
            </form>
          </div>
        </div>
      </section>
    </div>

    <footer
      class="mt-10 flex flex-wrap items-center justify-between gap-2 border-t border-line pb-2 pt-4 text-[0.62rem] text-ink-faint"
    >
      <span>© {{ year }} {{ brand.name }} · {{ brand.slogan }}</span>
    </footer>
  </main>
</template>
