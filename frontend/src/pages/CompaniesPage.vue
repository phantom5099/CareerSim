<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import CompanyMark from '@/components/CompanyMark.vue'
import MultiSelect from '@/components/MultiSelect.vue'
import { companiesApi } from '@/api/endpoints/companies'
import { PROJECT_STATUS_LABELS, formatDate } from '@/domain/types'
import type { Company, ProjectStatus } from '@/domain/types'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'

const catalog = useCompanyCatalogStore()

const query = ref('')
const searchInput = ref<HTMLInputElement | null>(null)
const industries = ref<string[]>([])
const activeIndustries = ref<string[]>([])

onMounted(async () => {
  try {
    industries.value = (await companiesApi.industries()).industries
  } catch {
    industries.value = []
  }
})

const visibleCompanies = computed(() => {
  const keyword = query.value.trim().toLowerCase()
  return catalog.companies.filter((company) => {
    if (activeIndustries.value.length && !activeIndustries.value.includes(company.industry)) {
      return false
    }
    if (!keyword) {
      return true
    }
    return [company.name, company.englishName, company.industry, company.city, company.tagline]
      .join(' ')
      .toLowerCase()
      .includes(keyword)
  })
})

const HEADLINE_COUNT = 3
const headlineIndex = ref(0)
const headlineDirection = ref<'next' | 'prev'>('next')
const headlineHovered = ref(false)
const headlineFocused = ref(false)
let headlineTimer: number | undefined

const headlinePool = computed(() =>
  query.value.trim() || activeIndustries.value.length
    ? []
    : visibleCompanies.value.slice(0, HEADLINE_COUNT),
)

const headlineCompany = computed(() => {
  const pool = headlinePool.value
  return pool.length ? pool[headlineIndex.value % pool.length] : undefined
})

const headlineActive = computed(() => {
  const pool = headlinePool.value
  return pool.length ? headlineIndex.value % pool.length : 0
})

const rest = computed(() => {
  const headlines = headlinePool.value
  if (!headlines.length) {
    return visibleCompanies.value
  }
  return visibleCompanies.value.filter(
    (company) => !headlines.some((item) => item.id === company.id),
  )
})

function stepHeadline(step: number) {
  const pool = headlinePool.value
  if (pool.length <= 1) {
    return
  }
  headlineDirection.value = step > 0 ? 'next' : 'prev'
  headlineIndex.value = ((headlineIndex.value + step) % pool.length + pool.length) % pool.length
  if (!headlineHovered.value && !headlineFocused.value) {
    startHeadlineTimer()
  }
}

function goToHeadline(index: number) {
  if (index === headlineActive.value) {
    return
  }
  headlineDirection.value = index > headlineActive.value ? 'next' : 'prev'
  headlineIndex.value = index
  if (!headlineHovered.value && !headlineFocused.value) {
    startHeadlineTimer()
  }
}

function startHeadlineTimer() {
  stopHeadlineTimer()
  const pool = headlinePool.value
  if (pool.length <= 1) {
    return
  }
  headlineTimer = window.setInterval(() => {
    headlineDirection.value = 'next'
    headlineIndex.value = (headlineIndex.value + 1) % pool.length
  }, 6000)
}

function stopHeadlineTimer() {
  if (headlineTimer !== undefined) {
    window.clearInterval(headlineTimer)
    headlineTimer = undefined
  }
}

function pauseHeadline() {
  headlineHovered.value = true
  stopHeadlineTimer()
}

function resumeHeadline() {
  headlineHovered.value = false
  if (!headlineFocused.value) {
    startHeadlineTimer()
  }
}

function focusHeadline() {
  headlineFocused.value = true
  stopHeadlineTimer()
}

function blurHeadline() {
  headlineFocused.value = false
  if (!headlineHovered.value) {
    startHeadlineTimer()
  }
}

watch(visibleCompanies, () => {
  headlineIndex.value = 0
  if (!headlineHovered.value && !headlineFocused.value) {
    startHeadlineTimer()
  }
})

onMounted(startHeadlineTimer)
onBeforeUnmount(stopHeadlineTimer)

const projectTone: Record<ProjectStatus, string> = {
  open: 'border-moss/60 text-moss',
  closing: 'border-vermilion/50 text-vermilion',
  closed: 'border-line text-ink-faint',
}

function activeProject(company: Company) {
  return company.projects.find((project) => project.status !== 'closed') ?? company.projects[0]
}

function projectStatus(company: Company): ProjectStatus | undefined {
  return activeProject(company)?.status
}

function projectToneOf(company: Company): string {
  const status = projectStatus(company)
  return status ? projectTone[status] : ''
}

function projectLabel(company: Company): string {
  const status = projectStatus(company)
  return status ? PROJECT_STATUS_LABELS[status] : ''
}

function clearFilters() {
  query.value = ''
  activeIndustries.value = []
}
</script>

<template>
  <div>
    <header>
      <h1 class="sr-only">在招公司</h1>
      <div
        class="flex flex-col gap-3 md:flex-row md:items-stretch md:gap-4 xl:grid xl:grid-cols-[minmax(10rem,1fr)_minmax(0,42rem)_minmax(10rem,1fr)] xl:items-stretch xl:gap-4"
      >
        <span class="hidden xl:block" aria-hidden="true"></span>
        <div
          class="flex min-w-0 items-stretch border border-line bg-paper-light transition focus-within:border-ink md:flex-1"
        >
          <span class="hidden shrink-0 items-center pl-4 font-mono text-[0.6rem] tracking-[0.18em] text-ink-faint sm:flex">
            SEARCH
          </span>
          <input
            ref="searchInput"
            v-model="query"
            type="text"
            class="min-w-0 flex-1 bg-transparent py-2.5 pl-4 pr-3 text-[0.86rem] text-ink outline-none placeholder:text-ink-faint sm:pl-3"
            placeholder="搜索公司名称、行业或城市"
            aria-label="搜索公司"
            autocomplete="off"
          />
          <button
            v-if="query"
            type="button"
            class="shrink-0 px-2 text-[0.66rem] text-ink-soft transition hover:text-vermilion"
            @click="query = ''"
          >
            清除
          </button>
          <button
            type="button"
            class="flex shrink-0 items-center gap-1.5 border-l border-line px-4 text-[0.74rem] text-ink-soft transition hover:bg-ink hover:text-paper-light"
            @click="searchInput?.focus()"
          >
            <svg viewBox="0 0 16 16" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
              <circle cx="7" cy="7" r="4.5" />
              <path d="M10.5 10.5 14 14" stroke-linecap="square" />
            </svg>
            搜索
          </button>
        </div>
        <div class="flex justify-end md:shrink-0 xl:justify-start">
          <RouterLink
            :to="{ name: 'companyImport' }"
            class="btn-line whitespace-nowrap py-1.5 text-[0.7rem]"
          >
            ＋ 上传公司模板
          </RouterLink>
        </div>
      </div>
    </header>

    <article
      v-if="headlineCompany"
      class="mt-6 border-y border-line"
      aria-roledescription="carousel"
      aria-label="本期头条"
      @mouseenter="pauseHeadline"
      @mouseleave="resumeHeadline"
      @focusin="focusHeadline"
      @focusout="blurHeadline"
    >
      <div class="py-7">
        <p class="eyebrow px-6 sm:px-10">本期头条</p>

        <div class="relative mt-5">
          <button
            type="button"
            class="carousel-arrow carousel-arrow--prev"
            aria-label="上一条本期头条"
            @click="stepHeadline(-1)"
          >
            <svg viewBox="0 0 16 16" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
              <path d="M10 3 5 8l5 5" stroke-linecap="square" />
            </svg>
          </button>
          <button
            type="button"
            class="carousel-arrow carousel-arrow--next"
            aria-label="下一条本期头条"
            @click="stepHeadline(1)"
          >
            <svg viewBox="0 0 16 16" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
              <path d="M6 3l5 5-5 5" stroke-linecap="square" />
            </svg>
          </button>

          <div class="min-h-[13rem]">
            <Transition
              :name="headlineDirection === 'next' ? 'carousel-next' : 'carousel-prev'"
              mode="out-in"
            >
              <div
                :key="headlineCompany.id"
                class="grid gap-6 px-6 sm:px-10 lg:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]"
              >
                <div>
                  <div class="flex items-start gap-4">
                    <CompanyMark
                      :text="headlineCompany.logoText"
                      :accent="headlineCompany.accent"
                      size="lg"
                    />
                    <div class="min-w-0">
                      <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                        <h2 class="font-display text-3xl leading-none tracking-tight sm:text-4xl">
                          {{ headlineCompany.name }}
                        </h2>
                        <span
                          v-if="headlineCompany.imported"
                          class="border border-vermilion/40 px-1.5 py-0.5 text-[0.62rem] text-vermilion"
                        >
                          我的模板
                        </span>
                      </div>
                      <p class="mt-2 font-mono text-[0.56rem] tracking-[0.22em] text-ink-faint">
                        {{ headlineCompany.englishName }}
                      </p>
                      <p class="mt-1.5 text-[0.72rem] text-ink-soft">
                        {{ headlineCompany.industry }} · {{ headlineCompany.city }} ·
                        {{ headlineCompany.size }}
                      </p>
                    </div>
                  </div>
  
                  <p class="mt-5 font-display text-xl leading-snug text-ink">
                    「{{ headlineCompany.tagline }}」
                  </p>
                  <p class="mt-3 max-w-xl text-[0.82rem] leading-7 text-ink-soft">
                    {{ headlineCompany.description }}
                  </p>
                </div>
  
                <div
                  class="flex flex-col justify-between gap-5 border-t border-line pt-5 lg:border-l lg:border-t-0 lg:pl-8 lg:pt-0"
                >
                  <ul class="grid gap-x-6 gap-y-1.5 sm:grid-cols-2 lg:grid-cols-1">
                    <li
                      v-for="highlight in headlineCompany.highlights"
                      :key="highlight"
                      class="flex gap-2 text-[0.72rem] leading-5 text-ink-soft"
                    >
                      <span class="pt-0.5 font-mono text-[0.6rem] text-vermilion">—</span>
                      <span>{{ highlight }}</span>
                    </li>
                  </ul>
  
                  <div class="flex flex-wrap items-center justify-between gap-3 border-t border-line pt-4">
                    <div class="flex flex-wrap items-center gap-2">
                      <span
                        v-if="activeProject(headlineCompany)"
                        class="border px-2 py-0.5 text-[0.62rem]"
                        :class="projectToneOf(headlineCompany)"
                      >
                        {{ projectLabel(headlineCompany) }} ·
                        {{ activeProject(headlineCompany)?.name }}
                      </span>
                      <span class="chip">{{ headlineCompany.jobs.length }} 个开放岗位</span>
                    </div>
                    <RouterLink
                      :to="{ name: 'companyDetail', params: { id: headlineCompany.id } }"
                      class="border-b border-ink pb-0.5 text-[0.7rem] transition hover:border-vermilion hover:text-vermilion"
                    >
                      查阅完整档案 →
                    </RouterLink>
                  </div>
                </div>
              </div>
            </Transition>
          </div>
        </div>

        <div
          v-if="headlinePool.length > 1"
          class="mt-6 flex items-center justify-center gap-2"
          role="group"
          aria-label="本期头条切换"
        >
          <button
            v-for="(item, index) in headlinePool"
            :key="item.id"
            type="button"
            class="h-2 w-2 rounded-full transition"
            :class="
              index === headlineActive
                ? 'bg-vermilion'
                : 'bg-line-strong hover:bg-ink-faint'
            "
            :aria-label="`查看第 ${index + 1} 条本期头条`"
            :aria-current="index === headlineActive ? 'true' : undefined"
            @click="goToHeadline(index)"
          />
        </div>
      </div>
    </article>

    <section class="mt-9">
      <div class="flex flex-wrap items-center justify-between gap-3 border-b border-ink pb-2">
        <h2 class="font-display text-2xl">公司条目</h2>
        <div class="flex items-center gap-3">
          <MultiSelect
            v-model="activeIndustries"
            class="w-36"
            label="按行业筛选公司条目"
            placeholder="全部行业"
            :options="industries"
          />
          <span class="text-[0.66rem] text-ink-faint">
            {{ rest.length }} 条{{ query.trim() ? ' · 搜索结果' : '' }}
          </span>
        </div>
      </div>

      <div
        v-if="!rest.length"
        class="mt-6 border border-dashed border-line-strong px-6 py-12 text-center"
      >
        <p class="font-display text-xl">没有找到匹配的公司</p>
        <p class="mt-2 text-[0.78rem] leading-6 text-ink-faint">换个行业或关键词试试。</p>
        <button type="button" class="btn-line mt-5 py-1.5 text-[0.72rem]" @click="clearFilters">
          清除筛选
        </button>
      </div>

      <RouterLink
        v-for="company in rest"
        :key="company.id"
        :to="{ name: 'companyDetail', params: { id: company.id } }"
        class="group -mx-3 grid gap-4 border-b border-line px-3 py-5 transition hover:bg-paper-light/70 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start"
      >
        <div class="flex items-start gap-4">
          <CompanyMark :text="company.logoText" :accent="company.accent" size="sm" />

          <div class="min-w-0">
            <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <h3 class="font-display text-xl leading-tight transition group-hover:text-vermilion">
                {{ company.name }}
              </h3>
              <span class="font-mono text-[0.56rem] tracking-[0.18em] text-ink-faint">
                {{ company.englishName }}
              </span>
              <span
                v-if="company.imported"
                class="border border-vermilion/40 px-1.5 py-0.5 text-[0.6rem] text-vermilion"
              >
                我的模板
              </span>
            </div>
            <p class="mt-1.5 max-w-2xl text-[0.78rem] leading-6 text-ink-soft">
              {{ company.tagline }}。{{ company.description }}
            </p>
            <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[0.66rem] text-ink-faint">
              <span>{{ company.industry }}</span>
              <span aria-hidden="true">/</span>
              <span>{{ company.city }}</span>
              <span aria-hidden="true">/</span>
              <span>{{ company.size }}</span>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-between gap-4 pl-14 sm:flex-col sm:items-end sm:justify-start sm:pl-0">
          <p
            v-if="activeProject(company)"
            class="border px-2 py-0.5 text-[0.6rem]"
            :class="projectToneOf(company)"
          >
            {{ projectLabel(company) }}
          </p>
          <p v-if="activeProject(company)" class="text-[0.66rem] text-ink-faint">
            截止 {{ formatDate(activeProject(company)?.deadline ?? '') }}
          </p>
        </div>
      </RouterLink>
    </section>
  </div>
</template>

<style scoped>
.carousel-next-enter-active,
.carousel-next-leave-active,
.carousel-prev-enter-active,
.carousel-prev-leave-active {
  transition:
    opacity 160ms ease,
    transform 160ms ease;
}
.carousel-next-enter-from {
  opacity: 0;
  transform: translateX(12px);
}
.carousel-next-leave-to {
  opacity: 0;
  transform: translateX(-12px);
}
.carousel-prev-enter-from {
  opacity: 0;
  transform: translateX(-12px);
}
.carousel-prev-leave-to {
  opacity: 0;
  transform: translateX(12px);
}
.carousel-arrow {
  position: absolute;
  top: 50%;
  z-index: 10;
  display: flex;
  height: 2.25rem;
  width: 2.25rem;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-line-strong);
  border-radius: 9999px;
  background: var(--color-paper-light);
  color: var(--color-ink-soft);
  box-shadow: 0 1px 0 rgb(23 21 15 / 0.06);
  transition:
    color 160ms ease,
    background-color 160ms ease,
    border-color 160ms ease;
}
.carousel-arrow--prev {
  left: 0;
  transform: translate(-50%, -50%);
}
.carousel-arrow--next {
  right: 0;
  transform: translate(50%, -50%);
}
.carousel-arrow:hover {
  border-color: var(--color-ink);
  background: var(--color-ink);
  color: var(--color-paper-light);
}
.carousel-arrow:focus-visible {
  outline: 2px solid var(--color-vermilion);
  outline-offset: 2px;
}
@media (max-width: 639px) {
  .carousel-arrow {
    height: 2rem;
    width: 2rem;
  }
}
</style>
