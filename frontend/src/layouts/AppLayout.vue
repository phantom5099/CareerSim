<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, RouterView, useRoute, useRouter } from 'vue-router'

import { brand } from '@/content/brand'
import { useAuthStore } from '@/stores/auth'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()

const navItems = [
  {
    label: '在招公司',
    hint: '本期公司与岗位',
    to: { name: 'companies' },
    matches: ['companies', 'companyDetail', 'companyImport'],
  },
  {
    label: '面试进度',
    hint: '按公司查看进度',
    to: { name: 'applications' },
    matches: ['applications', 'interview'],
  },
  {
    label: 'Offer 清单',
    hint: '比较与答复',
    to: { name: 'offers' },
    matches: ['offers'],
  },
  {
    label: '简历管理',
    hint: '多份简历与优化',
    to: { name: 'resume' },
    matches: ['resume'],
  },
] as const

const displayName = computed(() => auth.user?.displayName ?? '同学')
const avatarText = computed(() => auth.user?.avatarText ?? '?')
const avatarColor = computed(() => auth.user?.avatarColor ?? '#C13D24')
const subtitle = computed(() => {
  if (!auth.user) {
    return '体验账号'
  }
  return [auth.user.school, auth.user.major].filter(Boolean).join(' · ') || '体验账号'
})

function isActive(item: (typeof navItems)[number]): boolean {
  return item.matches.some((name) => route.name === name)
}

async function signOut() {
  await auth.logout()
  await router.replace({ name: 'login' })
}
</script>

<template>
  <div
    class="min-h-dvh lg:grid lg:grid-cols-[16.5rem_minmax(0,1fr)] xl:grid-cols-[18.5rem_minmax(0,1fr)]"
  >
    <aside
      class="hidden border-r border-line bg-paper-deep/55 lg:sticky lg:top-0 lg:flex lg:h-dvh lg:flex-col"
    >
      <div class="border-b border-line px-6 pb-5 pt-7">
        <RouterLink :to="{ name: 'companies' }" class="block">
          <p class="font-mono text-[0.56rem] tracking-[0.3em] text-ink-faint">
            {{ brand.eyebrow }}
          </p>
          <div class="mt-2 flex items-end justify-between gap-3">
            <p class="font-display text-[1.7rem] leading-none tracking-wide">
              {{ brand.chineseName }}
            </p>
            <p class="pb-0.5 text-[0.62rem] text-ink-faint">
              {{ brand.issue }}
            </p>
          </div>
        </RouterLink>
        <p class="mt-3 text-[0.66rem] leading-5 text-ink-soft">
          一份关于投递、面试与 Offer 的模拟特刊。
        </p>
      </div>

      <nav class="flex-1 overflow-y-auto px-3 py-4" aria-label="主导航">
        <RouterLink
          v-for="item in navItems"
          :key="item.label"
          :to="item.to"
          class="block border-b border-l-2 border-line/70 py-3 pl-3 pr-2 transition"
          :class="
            isActive(item)
              ? 'border-l-vermilion bg-paper-light'
              : 'border-l-transparent hover:bg-paper-light/60'
          "
        >
          <span
            class="block font-display text-[1.02rem] leading-tight"
            :class="isActive(item) ? 'text-ink' : 'text-ink-soft'"
          >
            {{ item.label }}
          </span>
          <span class="mt-0.5 block text-[0.62rem] tracking-wide text-ink-faint">
            {{ item.hint }}
          </span>
        </RouterLink>
      </nav>

      <div class="border-t border-line px-5 py-4">
        <div class="flex items-center gap-3">
          <RouterLink
            :to="{ name: 'profile' }"
            class="flex min-w-0 flex-1 items-center gap-3 transition hover:opacity-80"
            aria-label="个人资料"
          >
            <span
              class="flex h-9 w-9 shrink-0 items-center justify-center font-display text-xs text-paper-light"
              :style="{ backgroundColor: avatarColor }"
            >
              {{ avatarText }}
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-[0.82rem] font-medium text-ink">{{ displayName }}</p>
              <p class="truncate text-[0.64rem] text-ink-faint">
                {{ subtitle }}
              </p>
            </div>
          </RouterLink>
          <RouterLink
            :to="{ name: 'profile' }"
            class="shrink-0 text-ink-soft transition hover:text-vermilion"
            aria-label="个人资料"
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
              <circle cx="8" cy="5.2" r="2.6" />
              <path d="M2.8 13.6c.7-2.6 2.8-4 5.2-4s4.5 1.4 5.2 4" />
            </svg>
          </RouterLink>
          <button
            type="button"
            class="shrink-0 text-ink-soft transition hover:text-vermilion"
            aria-label="退出登录"
            @click="signOut"
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
              <path d="M6 2H3.5A1.5 1.5 0 0 0 2 3.5v9A1.5 1.5 0 0 0 3.5 14H6" />
              <path d="M10 4.5 13.5 8 10 11.5" />
              <path d="M13.5 8H6.5" />
            </svg>
          </button>
        </div>
      </div>
    </aside>

    <div class="flex min-w-0 flex-col">
      <header class="sticky top-0 z-30 border-b border-line bg-paper/95 backdrop-blur lg:hidden">
        <div class="flex items-center justify-between gap-3 px-4 py-3">
          <RouterLink :to="{ name: 'companies' }" class="flex items-baseline gap-2">
            <span class="font-display text-lg leading-none">{{ brand.chineseName }}</span>
            <span class="font-mono text-[0.52rem] tracking-[0.18em] text-ink-faint">CAREERSIM</span>
          </RouterLink>
          <div class="flex items-center gap-3">
            <RouterLink :to="{ name: 'profile' }" aria-label="个人资料">
              <span
                class="flex h-7 w-7 items-center justify-center font-display text-[0.62rem] text-paper-light"
                :style="{ backgroundColor: avatarColor }"
              >
                {{ avatarText }}
              </span>
            </RouterLink>
            <button
              type="button"
              class="text-ink-soft transition hover:text-vermilion"
              aria-label="退出登录"
              @click="signOut"
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
                <path d="M6 2H3.5A1.5 1.5 0 0 0 2 3.5v9A1.5 1.5 0 0 0 3.5 14H6" />
                <path d="M10 4.5 13.5 8 10 11.5" />
                <path d="M13.5 8H6.5" />
              </svg>
            </button>
          </div>
        </div>
        <nav
          class="no-scrollbar flex gap-0.5 overflow-x-auto border-t border-line/70 px-2 py-2"
          aria-label="主导航"
        >
          <RouterLink
            v-for="item in navItems"
            :key="item.label"
            :to="item.to"
            class="flex shrink-0 items-center border px-2 py-1.5"
            :class="
              isActive(item) ? 'border-ink bg-ink text-paper-light' : 'border-line text-ink-soft'
            "
          >
            <span class="text-[0.64rem]">{{ item.label }}</span>
          </RouterLink>
        </nav>
      </header>

      <main class="flex-1 px-5 pb-7 pt-3.5 sm:px-8 lg:px-12 lg:pb-10 lg:pt-5 xl:px-14">
        <div class="mx-auto w-full max-w-6xl">
          <RouterView />
        </div>
      </main>

      <footer
        class="flex flex-wrap items-center justify-between gap-2 border-t border-line px-5 py-4 text-[0.62rem] text-ink-faint sm:px-8 lg:px-12"
      >
        <span>© {{ new Date().getFullYear() }} {{ brand.name }} · {{ brand.slogan }}</span>
        <span class="hidden sm:inline">{{ brand.disclaimer }}</span>
      </footer>
    </div>
  </div>
</template>
