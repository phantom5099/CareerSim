<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

import { brand } from '@/content/brand'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

const form = reactive({
  username: '',
  password: '',
})
const showPassword = ref(false)
const touched = ref(false)

const usernameError = computed(() =>
  touched.value && form.username.trim().length === 0 ? '请输入账号' : '',
)
const passwordError = computed(() =>
  touched.value && form.password.length === 0 ? '请输入密码' : '',
)
const canSubmit = computed(
  () => form.username.trim().length > 0 && form.password.length > 0 && !auth.loading,
)

function redirectTarget(): string {
  const target = route.query.redirect
  return typeof target === 'string' && target.startsWith('/') && !target.startsWith('//')
    ? target
    : '/'
}

async function submit() {
  touched.value = true
  if (!canSubmit.value) {
    return
  }

  if (await auth.login(form.username.trim(), form.password)) {
    await router.replace(redirectTarget())
  }
}

const year = new Date().getFullYear()
</script>

<template>
  <main class="min-h-dvh px-5 py-5 sm:px-8 lg:px-12 lg:py-7">
    <header class="border-b-[3px] border-double border-ink pb-4">
      <div class="flex items-end justify-between gap-4">
        <div class="flex items-center gap-3">
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
        </div>
      </div>
    </header>

    <div class="grid gap-9 pt-8 lg:grid-cols-[1.1fr_0.9fr] lg:gap-16 lg:pt-12">
      <section class="animate-rise">
        <h1
          class="mt-5 font-display text-[clamp(3rem,7vw,5.3rem)] leading-[1.02] tracking-tight text-ink"
          aria-label="体验求职之乐"
        >
          体验求职<span class="text-vermilion">之乐</span>
        </h1>
        <div class="mt-5 h-[3px] w-24 bg-vermilion" aria-hidden="true" />

        <p class="mt-6 max-w-md text-sm leading-7 text-ink-soft">
          {{ brand.subcopy }}
        </p>
        <p class="mt-2 max-w-md text-sm leading-7 text-ink-soft">
          {{ brand.description }}
        </p>

        <div class="mt-9 hidden items-end justify-between gap-6 sm:flex">
          <p class="max-w-xs text-[0.64rem] leading-5 text-ink-faint">
            {{ brand.disclaimer }}
          </p>
          <span class="stamp rotate-[-7deg] text-vermilion/70" aria-hidden="true">模拟</span>
        </div>
      </section>

      <section class="animate-rise delay-1">
        <div class="border border-ink bg-paper-light">
          <div
            class="flex items-center justify-between border-b border-ink px-5 py-3 text-[0.64rem] text-ink-faint sm:px-7"
          >
            <span>入场登记 / ENTRY</span>
            <span>NO. 0001</span>
          </div>

          <div class="px-5 py-6 sm:px-7 sm:py-8">
            <div class="flex items-start justify-between gap-4">
              <div>
                <h2 class="font-display text-[1.8rem] leading-tight">回到模拟现场</h2>
                <p class="mt-2 text-xs leading-5 text-ink-soft">
                  使用体验账号登录，继续你的求职演练。
                </p>
              </div>
              <span class="stamp hidden shrink-0 text-vermilion/75 sm:inline-flex">模拟</span>
            </div>

            <form class="mt-7 space-y-5" novalidate @submit.prevent="submit">
              <div>
                <label
                  for="username"
                  class="mb-1.5 flex items-baseline justify-between text-[0.8rem] text-ink"
                >
                  <span>账号</span>
                </label>
                <input
                  id="username"
                  v-model="form.username"
                  class="field"
                  type="text"
                  name="username"
                  autocomplete="username"
                  placeholder="请输入账号"
                  :aria-invalid="Boolean(usernameError)"
                  :aria-describedby="usernameError ? 'username-error' : undefined"
                />
                <p v-if="usernameError" id="username-error" class="mt-1.5 text-xs text-vermilion">
                  {{ usernameError }}
                </p>
              </div>

              <div>
                <label
                  for="password"
                  class="mb-1.5 flex items-baseline justify-between text-[0.8rem] text-ink"
                >
                  <span>密码</span>
                </label>
                <div class="relative">
                  <input
                    id="password"
                    v-model="form.password"
                    class="field pr-11"
                    :type="showPassword ? 'text' : 'password'"
                    name="password"
                    autocomplete="current-password"
                    placeholder="请输入密码"
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

              <p
                v-if="auth.error"
                role="alert"
                class="border border-vermilion/40 bg-vermilion/5 px-3 py-2.5 text-sm text-vermilion-deep"
              >
                {{ auth.error }}
              </p>

              <button type="submit" class="btn-solid w-full" :disabled="!canSubmit">
                {{ auth.loading ? '正在入场…' : '入场 →' }}
              </button>

              <p class="text-center text-xs text-ink-soft">
                还没有账号？
                <RouterLink
                  :to="{ name: 'register' }"
                  class="text-vermilion-deep underline-offset-2 hover:underline"
                >
                  注册一个
                </RouterLink>
              </p>
            </form>
          </div>
        </div>

        <p class="mt-4 text-center text-[0.6rem] leading-5 text-ink-faint sm:hidden">
          {{ brand.disclaimer }}
        </p>
      </section>
    </div>

    <footer
      class="mt-10 flex flex-wrap items-center justify-between gap-2 border-t border-line pb-2 pt-4 text-[0.62rem] text-ink-faint"
    >
      <span>© {{ year }} {{ brand.name }} · {{ brand.slogan }}</span>
    </footer>
  </main>
</template>
