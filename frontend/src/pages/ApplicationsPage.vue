<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'

import CompanyMark from '@/components/CompanyMark.vue'
import StageRail from '@/components/StageRail.vue'
import { APPLICATION_STATUS_LABELS, actionableStage, formatDate } from '@/domain/types'
import type { Application, ApplicationStatus, Company } from '@/domain/types'
import { useApplicationsStore } from '@/stores/applications'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'

interface ApplicationGroup {
  company: Company | undefined
  items: Application[]
}

const applications = useApplicationsStore()
const catalog = useCompanyCatalogStore()

const statusTone: Record<ApplicationStatus, string> = {
  active: 'border-moss/60 text-moss',
  rejected: 'border-line text-ink-faint',
  offer: 'border-vermilion/50 text-vermilion',
  withdrawn: 'border-line text-ink-faint',
}

type ProgressFilter = 'all' | 'exam' | 'interview' | 'offer'

const filters: { key: ProgressFilter; label: string }[] = [
  { key: 'all', label: '全部' },
  { key: 'exam', label: '待笔试' },
  { key: 'interview', label: '待面试' },
  { key: 'offer', label: '已发 Offer' },
]

const activeFilter = ref<ProgressFilter>('all')

function matchesFilter(application: Application, filter: ProgressFilter): boolean {
  if (filter === 'all') {
    return true
  }
  if (filter === 'offer') {
    return application.status === 'offer'
  }
  if (application.status !== 'active') {
    return false
  }
  if (filter === 'exam') {
    return application.currentStage === 'exam'
  }
  return (
    application.currentStage === 'tech_interview' ||
    application.currentStage === 'manager_interview' ||
    application.currentStage === 'hrbp_interview'
  )
}

const filterCounts = computed<Record<ProgressFilter, number>>(() => {
  const counts: Record<ProgressFilter, number> = { all: 0, exam: 0, interview: 0, offer: 0 }
  for (const filter of filters) {
    counts[filter.key] = applications.applications.filter((application) =>
      matchesFilter(application, filter.key),
    ).length
  }
  return counts
})

const visibleApplications = computed(() =>
  applications.applications.filter((application) =>
    matchesFilter(application, activeFilter.value),
  ),
)

const activeFilterLabel = computed(
  () => filters.find((filter) => filter.key === activeFilter.value)?.label ?? '',
)

const groups = computed<ApplicationGroup[]>(() => {
  const map = new Map<string, ApplicationGroup>()
  for (const application of visibleApplications.value) {
    const group = map.get(application.companyId) ?? {
      company: catalog.findCompany(application.companyId),
      items: [],
    }
    group.items.push(application)
    map.set(application.companyId, group)
  }
  return Array.from(map.values()).sort(
    (a, b) =>
      Number(b.items.some((item) => item.status === 'active')) -
      Number(a.items.some((item) => item.status === 'active')),
  )
})

function enterLabel(application: Application): string {
  if (application.currentStage === 'exam') {
    return '进入笔试 →'
  }
  return '进入面试 →'
}

function reviewLink(application: Application) {
  const stage =
    application.stages.find((item) => item.state === 'failed')?.key ??
    application.currentStage
  return {
    name: 'interview',
    params: { applicationId: application.id },
    query: { review: '1', stage },
  }
}
</script>

<template>
  <div>
    <h1 class="sr-only">面试进度</h1>

    <div
      class="mt-1 flex flex-wrap items-center justify-between gap-3 border-b border-line pb-3"
    >
      <div class="no-scrollbar flex items-center gap-1.5 overflow-x-auto" role="tablist" aria-label="面试进度筛选">
        <button
          v-for="filter in filters"
          :key="filter.key"
          type="button"
          role="tab"
          :aria-selected="activeFilter === filter.key"
          class="flex shrink-0 items-baseline gap-1.5 border px-3 py-1.5 text-[0.7rem] transition"
          :class="
            activeFilter === filter.key
              ? 'border-ink bg-ink text-paper-light'
              : 'border-line text-ink-soft hover:border-ink hover:text-ink'
          "
          @click="activeFilter = filter.key"
        >
          {{ filter.label }}
          <span
            class="font-mono text-[0.58rem]"
            :class="activeFilter === filter.key ? 'text-paper-light/70' : 'text-ink-faint'"
          >
            {{ filterCounts[filter.key] }}
          </span>
        </button>
      </div>
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[0.62rem] text-ink-soft">
        <span class="flex items-center gap-1.5">
          <span class="h-0.5 w-4 bg-ink" aria-hidden="true" />已完成
        </span>
        <span class="flex items-center gap-1.5">
          <span class="h-0.5 w-4 bg-moss-bright" aria-hidden="true" />进行中
        </span>
        <span class="flex items-center gap-1.5">
          <span class="h-0.5 w-4 bg-vermilion" aria-hidden="true" />等待结果
        </span>
        <span class="flex items-center gap-1.5">
          <span class="h-0.5 w-4 bg-line-strong" aria-hidden="true" />待进行
        </span>
      </div>
    </div>

    <div v-if="!applications.applications.length" class="mt-8 border border-dashed border-line-strong px-6 py-12 text-center">
      <p class="font-display text-2xl">还没有投递记录</p>
      <p class="mt-2 text-sm text-ink-soft">去在招公司挑一家公司，投出第一份简历。</p>
      <RouterLink :to="{ name: 'companies' }" class="btn-line mt-6">前往在招公司</RouterLink>
    </div>

    <div v-else-if="!groups.length" class="mt-8 border border-dashed border-line-strong px-6 py-12 text-center">
      <p class="font-display text-2xl">没有{{ activeFilterLabel }}的投递</p>
      <p class="mt-2 text-sm text-ink-soft">当前筛选下没有匹配的投递记录。</p>
      <button type="button" class="btn-line mt-6" @click="activeFilter = 'all'">查看全部投递</button>
    </div>

    <template v-else>
      <section
        v-for="(group, groupIndex) in groups"
        :key="group.company?.id ?? `group-${groupIndex}`"
        class="border-b border-line pt-6 pb-4"
      >
      <header class="flex flex-wrap items-start justify-between gap-3">
        <div class="flex min-w-0 items-start gap-3">
          <CompanyMark
            v-if="group.company"
            :text="group.company.logoText"
            :accent="group.company.accent"
            size="sm"
          />
          <div class="min-w-0">
            <RouterLink
              v-if="group.company"
              :to="{ name: 'companyDetail', params: { id: group.company.id } }"
              class="font-display text-2xl leading-tight transition hover:text-vermilion"
            >
              {{ group.company.name }}
            </RouterLink>
            <p v-else class="font-display text-2xl leading-tight">已移除的公司</p>
            <p v-if="group.company" class="mt-1 text-[0.68rem] text-ink-faint">
              {{ group.company.industry }} · {{ group.company.city }}
            </p>
          </div>
        </div>
        <span class="chip">
          {{ group.items.filter((item) => item.status === 'active').length }} 份进行中
        </span>
      </header>

      <div
        v-for="application in group.items"
        :key="application.id"
        class="mt-4 grid gap-4 border-t border-line/70 pt-4 lg:grid-cols-[minmax(0,1fr)_14rem] lg:items-start"
      >
        <div class="min-w-0">
          <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <h3 class="font-display text-xl leading-tight">{{ application.jobTitle }}</h3>
            <span class="text-[0.66rem] text-ink-faint">
              投递于 {{ formatDate(application.appliedAt) }}
            </span>
            <span class="text-[0.66rem] text-ink-faint">简历 · {{ application.resumeTitle }}</span>
          </div>
          <div class="mt-4">
            <StageRail
              :stages="application.stages"
              :application-id="application.id"
              :active-stage="actionableStage(application)"
            />
          </div>
          <p class="mt-3 text-[0.68rem] leading-5 text-ink-faint">
            {{ application.nextAction }}
          </p>
        </div>

        <div class="flex flex-col gap-2 border border-line p-3">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[0.64rem] text-ink-faint">当前状态</span>
            <span
              class="border px-1.5 py-0.5 text-[0.6rem]"
              :class="statusTone[application.status]"
            >
              {{ APPLICATION_STATUS_LABELS[application.status] }}
            </span>
          </div>

          <div class="flex flex-wrap gap-1.5">
            <RouterLink
              v-if="application.status === 'offer'"
              :to="{ name: 'offers' }"
              class="btn-solid py-1.5 text-[0.68rem]"
            >
              查看 Offer →
            </RouterLink>
            <RouterLink
              v-else-if="
                application.status === 'active' && application.currentStage !== 'hr_screen'
              "
              :to="{ name: 'interview', params: { applicationId: application.id } }"
              class="btn-solid py-1.5 text-[0.68rem]"
            >
              {{ enterLabel(application) }}
            </RouterLink>
            <RouterLink
              v-else-if="application.status === 'rejected'"
              :to="reviewLink(application)"
              class="btn-line py-1.5 text-[0.68rem]"
            >
              查看复盘
            </RouterLink>
            <span
              v-else-if="application.status === 'active'"
              class="border border-line px-2 py-1 text-[0.62rem] text-ink-faint"
            >
              {{ application.currentStage === 'hr_screen' ? '等待初筛结果' : '等待面试安排' }}
            </span>
          </div>
        </div>
      </div>
      </section>
    </template>
  </div>
</template>
