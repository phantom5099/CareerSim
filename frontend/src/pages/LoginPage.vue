<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { demoConfig } from '@/config/demo'
import { brand, stages } from '@/content/brand'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

const form = reactive({
  username: demoConfig.enabled ? demoConfig.username : '',
  password: demoConfig.enabled ? demoConfig.password : '',
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

async function useDemoAccount() {
  form.username = demoConfig.username
  form.password = demoConfig.password
  touched.value = true
  await submit()
}

const year = new Date().getFullYear()
const dateLabel = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
}).format(new Date())
</script>

<template>
  <main class="min-h-dvh px-5 py-5 sm:px-8 lg:px-12 lg:py-7">
    <header class="border-b-[3px] border-double border-ink pb-4">
      <div
        class="flex flex-wrap items-center justify-between gap-2 text-[0.62rem] tracking-[0.08em] text-ink-faint"
      >
        <span>{{ brand.name }} · {{ brand.eyebrow }}</span>
        <span class="hidden sm:inline">{{ brand.issue }} · {{ brand.issueNote }}</span>
        <span>{{ dateLabel }}</span>
      </div>
      <div class="mt-4 flex items-end justify-between gap-4">
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
        <p class="hidden text-right text-[0.6rem] leading-5 text-ink-faint sm:block">
          模拟招聘会<br />
          所有公司与岗位均为虚构
        </p>
      </div>
    </header>

    <div class="grid gap-9 pt-8 lg:grid-cols-[1.1fr_0.9fr] lg:gap-16 lg:pt-12">
      <section class="animate-rise">
        <p class="eyebrow flex items-center gap-3">
          <span class="h-px w-8 bg-vermilion" aria-hidden="true" />
          卷首语 · 先练一遍，再上场
        </p>

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

        <div class="mt-9 hidden sm:block">
          <div
            class="flex items-baseline justify-between border-b border-ink pb-2 text-[0.64rem] text-ink-faint"
          >
            <span>本期流程 / PROGRAMME</span>
            <span>共 6 站</span>
          </div>
          <ol class="grid grid-cols-2 gap-x-9">
            <li
              v-for="(stage, index) in stages"
              :key="stage.label"
              class="flex gap-3 border-b border-line py-3"
            >
              <span class="font-mono text-[0.6rem] text-vermilion">
                {{ String(index + 1).padStart(2, '0') }}
              </span>
              <span>
                <span class="block font-display text-[0.95rem] leading-tight">{{ stage.label }}</span>
                <span class="mt-0.5 block text-[0.62rem] tracking-wide text-ink-faint">
                  {{ stage.note }}
                </span>
              </span>
            </li>
          </ol>
        </div>

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
                  <span class="font-mono text-[0.54rem] tracking-[0.16em] text-ink-faint">01</span>
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
                  <span class="font-mono text-[0.54rem] tracking-[0.16em] text-ink-faint">02</span>
                </label>
                <div class="relative">
                  <input
                    id="password"
                    v-model="form.password"
                    class="field pr-16"
                    :type="showPassword ? 'text' : 'password'"
                    name="password"
                    autocomplete="current-password"
                    placeholder="请输入密码"
                    :aria-invalid="Boolean(passwordError)"
                    :aria-describedby="passwordError ? 'password-error' : undefined"
                  />
                  <button
                    type="button"
                    class="absolute inset-y-0 right-0 px-3 text-[0.68rem] text-ink-faint transition hover:text-ink"
                    :aria-label="showPassword ? '隐藏密码' : '显示密码'"
                    @click="showPassword = !showPassword"
                  >
                    {{ showPassword ? '隐藏' : '显示' }}
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
            </form>

            <template v-if="demoConfig.enabled">
              <div
                class="my-5 flex items-center gap-3 text-[0.64rem] tracking-[0.1em] text-ink-faint"
              >
                <span class="h-px flex-1 bg-line" aria-hidden="true" />
                或 / OR
                <span class="h-px flex-1 bg-line" aria-hidden="true" />
              </div>

              <div class="border border-dashed border-line-strong px-3 py-3">
                <div class="flex items-baseline justify-between gap-3">
                  <p class="text-[0.64rem] text-ink-faint">
                    体验账号 / DEMO PASS
                  </p>
                  <p class="font-mono text-[0.68rem] text-ink-soft">
                    {{ demoConfig.username }} / {{ demoConfig.password }}
                  </p>
                </div>
                <button
                  type="button"
                  class="btn-line mt-3 w-full"
                  :disabled="auth.loading"
                  @click="useDemoAccount"
                >
                  一键入场
                </button>
              </div>
            </template>
          </div>

          <p class="border-t border-line px-5 py-3 text-[0.58rem] text-ink-faint sm:px-7">
            登录即进入本地模拟环境，不会产生真实投递。
          </p>
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
      <span v-if="demoConfig.enabled">DEMO · {{ demoConfig.username }}</span>
    </footer>
  </main>
</template>
