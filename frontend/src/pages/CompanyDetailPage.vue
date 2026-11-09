<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import CompanyMark from '@/components/CompanyMark.vue'
import MultiSelect from '@/components/MultiSelect.vue'
import StageRail from '@/components/StageRail.vue'
import { actionableStage } from '@/data/interviews'
import { APPLICATION_STATUS_LABELS, PROJECT_STATUS_LABELS, formatDate } from '@/domain/types'
import type { ApplicationStatus, Job, ProjectStatus } from '@/domain/types'
import { useApplicationsStore } from '@/stores/applications'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'

const route = useRoute()
const catalog = useCompanyCatalogStore()
const applications = useApplicationsStore()

const companyId = computed(() => String(route.params.id ?? ''))
const company = computed(() => catalog.findCompany(companyId.value))
const companyApplications = computed(() => applications.forCompany(companyId.value))

const flash = ref('')
let flashTimer: number | undefined

const projectTone: Record<ProjectStatus, string> = {
  open: 'border-moss/60 text-moss',
  closing: 'border-vermilion/50 text-vermilion',
  closed: 'border-line text-ink-faint',
}

const statusTone: Record<ApplicationStatus, string> = {
  active: 'border-moss/60 text-moss',
  rejected: 'border-line text-ink-faint',
  offer: 'border-vermilion/50 text-vermilion',
  withdrawn: 'border-line text-ink-faint',
}

function applicationFor(jobId: string) {
  return companyApplications.value.find((application) => application.jobId === jobId)
}

const jobTypeOptions: string[] = ['校招', '实习', '社招']

const activeProjectId = ref<string | null>(null)
const activeIndustries = ref<string[]>([])
const activeCities = ref<string[]>([])
const activeJobTypes = ref<string[]>([])
const jobQuery = ref('')

const activeProject = computed(() =>
  company.value?.projects.find((project) => project.id === activeProjectId.value),
)

function projectIdFor(job: Job): string | undefined {
  return job.projectId ?? company.value?.projects[0]?.id
}

function projectNameFor(job: Job): string | undefined {
  const projectId = projectIdFor(job)
  return company.value?.projects.find((project) => project.id === projectId)?.name
}

function jobsInProject(projectId: string): Job[] {
  return company.value?.jobs.filter((job) => projectIdFor(job) === projectId) ?? []
}

function jobIndustry(job: Job): string {
  return job.industry ?? company.value?.industry ?? '其他'
}

function buildFacet(read: (job: Job) => string): string[] {
  const counts = new Map<string, number>()
  for (const job of company.value?.jobs ?? []) {
    const value = read(job)
    counts.set(value, (counts.get(value) ?? 0) + 1)
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], 'zh-CN'))
    .map(([value]) => value)
}

const industryOptions = computed(() => buildFacet(jobIndustry))
const cityOptions = computed(() => buildFacet((job) => job.city))

const filteredJobs = computed(() => {
  if (!company.value) {
    return []
  }
  const query = jobQuery.value.trim().toLowerCase()
  return company.value.jobs.filter((job) => {
    if (activeProjectId.value && projectIdFor(job) !== activeProjectId.value) {
      return false
    }
    if (activeIndustries.value.length && !activeIndustries.value.includes(jobIndustry(job))) {
      return false
    }
    if (activeCities.value.length && !activeCities.value.includes(job.city)) {
      return false
    }
    if (activeJobTypes.value.length && !activeJobTypes.value.includes(job.type)) {
      return false
    }
    if (!query) {
      return true
    }
    return [job.title, job.description, job.city, ...job.tags].some((text) =>
      text.toLowerCase().includes(query),
    )
  })
})

const hasJobFilter = computed(
  () =>
    Boolean(activeProjectId.value) ||
    activeIndustries.value.length > 0 ||
    activeCities.value.length > 0 ||
    activeJobTypes.value.length > 0 ||
    jobQuery.value.trim().length > 0,
)

const jobsCountLabel = computed(() => {
  if (!company.value) {
    return ''
  }
  if (!hasJobFilter.value) {
    return `${company.value.jobs.length} 个岗位`
  }
  return `${filteredJobs.value.length} / ${company.value.jobs.length} 个岗位`
})

function selectProject(projectId: string) {
  activeProjectId.value = activeProjectId.value === projectId ? null : projectId
  document.getElementById('open-jobs')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

function clearJobFilters() {
  activeProjectId.value = null
  activeIndustries.value = []
  activeCities.value = []
  activeJobTypes.value = []
  jobQuery.value = ''
}

async function apply(job: Job) {
  if (!company.value) {
    return
  }
  await applications.applyToJob(company.value, job)
  flash.value = `已投递「${job.title}」· 流程从 HR 初筛开始`
  if (flashTimer !== undefined) {
    window.clearTimeout(flashTimer)
  }
  flashTimer = window.setTimeout(() => {
    flash.value = ''
  }, 3200)
}

onBeforeUnmount(() => {
  if (flashTimer !== undefined) {
    window.clearTimeout(flashTimer)
  }
})
</script>

<template>
  <div>
    <div class="-ml-1 text-[0.88rem] sm:-ml-4">
      <RouterLink
        :to="{ name: 'companies' }"
        class="inline-flex items-center gap-1.5 font-medium text-ink-soft transition hover:text-vermilion"
      >
        <span class="text-[1.05rem] leading-none" aria-hidden="true">←</span>
        返回在招公司
      </RouterLink>
    </div>

    <div
      v-if="!company"
      class="mt-6 border border-line bg-paper-light px-6 py-12 text-center"
    >
      <p class="font-display text-2xl">未找到这家公司</p>
      <p class="mt-2 text-sm text-ink-soft">它可能来自另一份模板，或已被移除。</p>
      <RouterLink :to="{ name: 'companies' }" class="btn-line mt-6">返回在招公司</RouterLink>
    </div>

    <template v-else>
      <p
        v-if="flash"
        role="status"
        class="mt-4 border border-moss/40 bg-moss/5 px-3 py-2 text-[0.7rem] text-moss"
      >
        {{ flash }}
      </p>

      <header class="mt-4 border-b border-line pb-7">
        <div class="grid gap-5 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-start">
          <CompanyMark :text="company.logoText" :accent="company.accent" size="lg" />
          <div class="min-w-0">
            <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <h1 class="font-display text-4xl leading-none tracking-tight sm:text-5xl">
                {{ company.name }}
              </h1>
              <span
                v-if="company.imported"
                class="border border-vermilion/40 px-1.5 py-0.5 text-[0.62rem] text-vermilion"
              >
                我的模板
              </span>
            </div>
            <p class="mt-2 font-mono text-[0.58rem] tracking-[0.24em] text-ink-faint">
              {{ company.englishName }}
            </p>
            <p class="mt-2 font-display text-lg text-ink-soft">「{{ company.tagline }}」</p>
          </div>
          <span class="stamp hidden text-vermilion/70 sm:inline-flex" aria-hidden="true">档案</span>
        </div>

        <dl class="mt-6 grid grid-cols-2 gap-y-3 border-t border-line pt-4 sm:grid-cols-4">
          <div>
            <dt class="text-[0.62rem] text-ink-faint">行业</dt>
            <dd class="mt-1 text-[0.76rem]">{{ company.industry }}</dd>
          </div>
          <div>
            <dt class="text-[0.62rem] text-ink-faint">城市</dt>
            <dd class="mt-1 text-[0.76rem]">{{ company.city }}</dd>
          </div>
          <div>
            <dt class="text-[0.62rem] text-ink-faint">规模</dt>
            <dd class="mt-1 text-[0.76rem]">{{ company.size }}</dd>
          </div>
          <div>
            <dt class="text-[0.62rem] text-ink-faint">在招岗位</dt>
            <dd class="mt-1 text-[0.76rem]">{{ company.jobs.length }} 个</dd>
          </div>
        </dl>
      </header>

      <div class="mt-7 grid gap-9 xl:grid-cols-[minmax(0,1fr)_17rem]">
        <div>
          <section>
            <div class="flex items-baseline justify-between border-b border-ink pb-2">
              <h2 class="font-display text-2xl">公司简介</h2>
              <span class="font-mono text-[0.56rem] tracking-[0.14em] text-ink-faint">ABOUT</span>
            </div>
            <p class="mt-4 text-[0.84rem] leading-7 text-ink-soft">{{ company.description }}</p>
            <ul class="mt-4 grid gap-2 sm:grid-cols-2">
              <li
                v-for="highlight in company.highlights"
                :key="highlight"
                class="flex gap-2 border-t border-line pt-2 text-[0.72rem] leading-5 text-ink-soft"
              >
                <span class="pt-0.5 text-[0.66rem] text-vermilion">—</span>
                <span>{{ highlight }}</span>
              </li>
            </ul>
          </section>

          <section class="mt-9">
            <div class="flex items-baseline justify-between border-b border-ink pb-2">
              <h2 class="font-display text-2xl">招聘项目</h2>
              <span class="text-[0.66rem] text-ink-faint">
                {{ company.projects.length }} 个项目
              </span>
            </div>
            <button
              v-for="project in company.projects"
              :key="project.id"
              type="button"
              class="-mx-3 block w-[calc(100%+1.5rem)] border-b border-line px-3 py-4 text-left transition"
              :class="
                activeProjectId === project.id ? 'bg-paper-light' : 'hover:bg-paper-light/50'
              "
              :aria-pressed="activeProjectId === project.id"
              @click="selectProject(project.id)"
            >
              <div class="flex flex-wrap items-baseline justify-between gap-2">
                <h3
                  class="font-display text-lg leading-tight"
                  :class="activeProjectId === project.id ? 'text-vermilion' : ''"
                >
                  {{ project.name }}
                </h3>
                <span
                  class="border px-2 py-0.5 text-[0.62rem]"
                  :class="projectTone[project.status]"
                >
                  {{ PROJECT_STATUS_LABELS[project.status] }}
                </span>
              </div>
              <p class="mt-1.5 text-[0.76rem] leading-6 text-ink-soft">{{ project.description }}</p>
              <div
                class="mt-2 flex flex-wrap items-center justify-between gap-2 text-[0.68rem] text-ink-faint"
              >
                <span>{{ project.season }} · 截止 {{ formatDate(project.deadline) }}</span>
                <span
                  class="text-[0.62rem]"
                  :class="activeProjectId === project.id ? 'text-vermilion' : ''"
                >
                  {{
                    activeProjectId === project.id
                      ? '已筛选 · 再点取消'
                      : `查看 ${jobsInProject(project.id).length} 个岗位 →`
                  }}
                </span>
              </div>
            </button>
          </section>

          <section id="open-jobs" class="mt-9 scroll-mt-20">
            <div class="flex items-baseline justify-between border-b border-ink pb-2">
              <h2 class="font-display text-2xl">开放岗位</h2>
              <span class="text-[0.66rem] text-ink-faint">{{ jobsCountLabel }}</span>
            </div>

            <div class="mt-4 grid gap-6 md:grid-cols-[minmax(0,1fr)_12rem] md:items-start">
              <aside
                class="border border-line bg-paper-light p-4 md:sticky md:top-24 md:order-2 lg:top-8"
                aria-label="筛选岗位"
              >
                <div class="flex items-baseline justify-between gap-2 border-b border-ink pb-2">
                  <h3 class="font-display text-lg">筛选</h3>
                  <button
                    v-if="hasJobFilter"
                    type="button"
                    class="text-[0.64rem] text-vermilion transition hover:underline"
                    @click="clearJobFilters"
                  >
                    清除
                  </button>
                </div>

                <div
                  v-if="activeProject"
                  class="mt-3 flex items-start justify-between gap-2 border border-vermilion/40 px-2 py-1 text-[0.64rem] text-vermilion"
                >
                  <span class="min-w-0">项目 · {{ activeProject.name }}</span>
                  <button
                    type="button"
                    class="shrink-0"
                    aria-label="取消项目筛选"
                    @click="activeProjectId = null"
                  >
                    ×
                  </button>
                </div>

                <div class="mt-3 space-y-3">
                  <div>
                    <label for="filter-industry" class="text-[0.6rem] text-ink-faint">
                      岗位方向
                    </label>
                    <MultiSelect
                      v-model="activeIndustries"
                      class="mt-1"
                      button-id="filter-industry"
                      label="按岗位方向筛选"
                      placeholder="全部方向"
                      :options="industryOptions"
                    />
                  </div>

                  <div>
                    <label for="filter-city" class="text-[0.6rem] text-ink-faint">地点</label>
                    <MultiSelect
                      v-model="activeCities"
                      class="mt-1"
                      button-id="filter-city"
                      label="按地点筛选"
                      placeholder="全部地点"
                      :options="cityOptions"
                    />
                  </div>

                  <div>
                    <label for="filter-job-type" class="text-[0.6rem] text-ink-faint">
                      招聘类型
                    </label>
                    <MultiSelect
                      v-model="activeJobTypes"
                      class="mt-1"
                      button-id="filter-job-type"
                      label="按招聘类型筛选"
                      placeholder="全部类型"
                      :options="jobTypeOptions"
                    />
                  </div>

                  <div>
                    <label for="job-query" class="text-[0.6rem] text-ink-faint">关键词</label>
                    <input
                      id="job-query"
                      v-model="jobQuery"
                      type="search"
                      class="field mt-1 w-full py-1.5 text-[0.7rem]"
                      placeholder="岗位或标签"
                    />
                  </div>
                </div>
              </aside>

              <div class="min-w-0 md:order-1">
                <article
                  v-for="job in filteredJobs"
                  :key="job.id"
                  class="grid gap-3 border-b border-line py-5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start"
                >
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                      <h3 class="font-display text-xl leading-tight">{{ job.title }}</h3>
                      <span class="chip">{{ job.type }}</span>
                      <span class="font-mono text-[0.62rem] text-vermilion">{{ job.salary }}</span>
                    </div>
                    <p class="mt-2 text-[0.78rem] leading-6 text-ink-soft">{{ job.description }}</p>
                    <div class="mt-2 flex flex-wrap gap-1.5">
                      <span class="chip">{{ job.city }}</span>
                      <span class="chip">{{ jobIndustry(job) }}</span>
                      <span v-for="tag in job.tags" :key="tag" class="chip">{{ tag }}</span>
                    </div>
                    <p
                      v-if="!activeProjectId && projectNameFor(job)"
                      class="mt-1.5 text-[0.62rem] text-ink-faint"
                    >
                      所属项目 · {{ projectNameFor(job) }}
                    </p>
                  </div>
                  <div class="flex items-center gap-2 sm:flex-col sm:items-end sm:justify-center">
                    <RouterLink
                      v-if="applicationFor(job.id)"
                      :to="{ name: 'applications' }"
                      class="btn-line py-1.5 text-[0.7rem]"
                    >
                      已投递 · 查看流程
                    </RouterLink>
                    <button
                      v-else
                      type="button"
                      class="btn-solid py-1.5 text-[0.7rem]"
                      @click="apply(job)"
                    >
                      投递这份岗位
                    </button>
                  </div>
                </article>

                <div
                  v-if="!filteredJobs.length"
                  class="border border-dashed border-line-strong px-6 py-10 text-center"
                >
                  <p class="font-display text-lg">没有匹配的岗位</p>
                  <p class="mt-1 text-[0.7rem] text-ink-faint">
                    换个项目、岗位方向、地点、招聘类型或关键词试试。
                  </p>
                  <button
                    v-if="hasJobFilter"
                    type="button"
                    class="btn-line mt-4 py-1.5 text-[0.68rem]"
                    @click="clearJobFilters"
                  >
                    清除筛选
                  </button>
                </div>
              </div>
            </div>
          </section>
        </div>

        <aside class="space-y-6">
          <section class="border border-line bg-paper-light p-4">
            <div class="flex items-baseline justify-between border-b border-line pb-2">
              <h2 class="font-display text-lg">你的投递</h2>
              <span class="text-[0.62rem] text-ink-faint">
                {{ companyApplications.length }} 条
              </span>
            </div>
            <div v-if="companyApplications.length" class="mt-1">
              <div
                v-for="application in companyApplications"
                :key="application.id"
                class="border-b border-line/70 py-3 last:border-b-0"
              >
                <div class="flex items-baseline justify-between gap-2">
                  <p class="text-[0.78rem] font-medium">{{ application.jobTitle }}</p>
                  <span
                    class="border px-1.5 py-0.5 text-[0.6rem]"
                    :class="statusTone[application.status]"
                  >
                    {{ APPLICATION_STATUS_LABELS[application.status] }}
                  </span>
                </div>
                <div class="mt-2">
                  <StageRail
                    :stages="application.stages"
                    :application-id="application.id"
                    :active-stage="actionableStage(application)"
                    dense
                  />
                </div>
                <p class="mt-2 text-[0.62rem] leading-4 text-ink-faint">
                  {{ application.nextAction }}
                </p>
              </div>
            </div>
            <p v-else class="py-4 text-[0.72rem] leading-6 text-ink-faint">
              还没有投递这家公司。选中一个岗位，流程会从 HR 初筛开始。
            </p>
            <RouterLink
              v-if="companyApplications.length"
              :to="{ name: 'applications' }"
              class="mt-2 inline-block border-b border-line-strong pb-0.5 text-[0.68rem] transition hover:border-vermilion hover:text-vermilion"
            >
              查看全部投递 →
            </RouterLink>
          </section>
        </aside>
      </div>
    </template>
  </div>
</template>
