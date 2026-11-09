<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

import CompanyMark from '@/components/CompanyMark.vue'
import StageRail from '@/components/StageRail.vue'
import { screeningFor, scriptFor } from '@/data/interviews'
import { STAGE_LABELS, STAGE_ORDER } from '@/domain/types'
import type { StageKey } from '@/domain/types'
import { useApplicationsStore } from '@/stores/applications'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'

const route = useRoute()
const router = useRouter()
const applications = useApplicationsStore()
const catalog = useCompanyCatalogStore()

const applicationId = computed(() => String(route.params.applicationId ?? ''))
const application = computed(() => applications.byId(applicationId.value))
const company = computed(() =>
  application.value ? catalog.findCompany(application.value.companyId) : undefined,
)
const requestedStage = computed<StageKey | undefined>(() => {
  const value = route.query.stage
  if (typeof value !== 'string') {
    return undefined
  }
  return (STAGE_ORDER as readonly string[]).includes(value) ? (value as StageKey) : undefined
})
const stage = computed<StageKey | undefined>(
  () => requestedStage.value ?? application.value?.currentStage,
)
const script = computed(() =>
  application.value && stage.value
    ? scriptFor(application.value, stage.value, company.value)
    : undefined,
)
const isScreening = computed(() => stage.value === 'hr_screen')
const screening = computed(() =>
  application.value && isScreening.value
    ? screeningFor(application.value, company.value)
    : undefined,
)

const answers = ref<string[]>([])
const step = ref(0)
const finished = ref(false)
const elapsed = ref('00:00')
const startedAt = ref(Date.now())
let timer: number | undefined

const reviewRequested = computed(() => route.query.review === '1')
const isReview = computed(
  () =>
    reviewRequested.value ||
    application.value?.status === 'rejected' ||
    application.value?.status === 'withdrawn',
)

const turns = computed(() => script.value?.turns ?? [])
const visibleTurns = computed(() =>
  isReview.value || finished.value ? turns.value : turns.value.slice(0, step.value + 1),
)
const currentAnswer = computed({
  get: () => answers.value[step.value] ?? '',
  set: (value: string) => {
    answers.value = answers.value.map((item, index) => (index === step.value ? value : item))
  },
})
const answeredCount = computed(() => answers.value.filter((item) => item.trim().length > 0).length)
const canSubmit = computed(() => currentAnswer.value.trim().length >= 8)
const progress = computed(() => {
  if (!turns.value.length) {
    return 0
  }
  const reached = finished.value ? turns.value.length : step.value
  return Math.round((reached / turns.value.length) * 100)
})
const score = computed(() => {
  const lengths = answers.value
    .filter((answer) => answer.trim().length > 0)
    .map((answer) => answer.trim().length)
  if (!lengths.length) {
    return 0
  }
  const average = lengths.reduce((sum, length) => sum + length, 0) / lengths.length
  return Math.min(96, 68 + Math.round(average / 6))
})

watch(
  script,
  (value) => {
    answers.value = value ? value.turns.map(() => '') : []
    step.value = 0
    finished.value = false
    startedAt.value = Date.now()
    elapsed.value = '00:00'
  },
  { immediate: true },
)

function formatElapsed(seconds: number): string {
  const minutes = Math.floor(seconds / 60)
  const rest = seconds % 60
  return `${String(minutes).padStart(2, '0')}:${String(rest).padStart(2, '0')}`
}

onMounted(() => {
  timer = window.setInterval(() => {
    elapsed.value = formatElapsed(Math.floor((Date.now() - startedAt.value) / 1000))
  }, 1000)
})

onBeforeUnmount(() => {
  if (timer !== undefined) {
    window.clearInterval(timer)
  }
})

function submit() {
  if (!canSubmit.value || isReview.value) {
    return
  }
  if (step.value >= turns.value.length - 1) {
    finished.value = true
    return
  }
  step.value += 1
}

async function complete() {
  if (application.value && !isReview.value) {
    await applications.advanceStage(application.value.id)
  }
  router.push({ name: 'applications' })
}
</script>

<template>
  <div>
    <div class="text-[0.88rem]">
      <RouterLink
        :to="{ name: 'applications' }"
        class="inline-flex items-center gap-1.5 font-medium text-ink-soft transition hover:text-vermilion"
      >
        <span class="text-[1.05rem] leading-none" aria-hidden="true">←</span>
        返回面试进度
      </RouterLink>
    </div>

    <div
      v-if="!application || !company || !script"
      class="mt-8 border border-line bg-paper-light px-6 py-12 text-center"
    >
      <p class="font-display text-2xl">这场面试还没有安排</p>
      <p class="mt-2 text-sm text-ink-soft">
        该投递尚未生成面试脚本，或公司模板已被移除。
      </p>
      <RouterLink :to="{ name: 'applications' }" class="btn-line mt-6">返回面试进度</RouterLink>
    </div>

    <template v-else>
      <header class="mt-4 border-b border-line pb-6">
        <div class="grid gap-5 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-start">
          <CompanyMark :text="company.logoText" :accent="company.accent" size="lg" />
          <div class="min-w-0">
            <p class="text-[0.66rem] text-ink-faint">
              {{ company.name }} · {{ company.englishName }}
            </p>
            <h1 class="mt-1 font-display text-3xl leading-tight tracking-tight sm:text-4xl">
              {{ application.jobTitle }}
            </h1>
            <p class="mt-2 text-[0.72rem] text-ink-faint">
              <template v-if="screening">初筛官 {{ screening.reviewer }} · {{ screening.reviewerTitle }}</template>
              <template v-else>面试官 {{ script.interviewer }} · {{ script.interviewerTitle }}</template>
            </p>
          </div>
          <span
            class="stamp hidden sm:inline-flex"
            :class="
              isReview
                ? 'text-ink-faint'
                : finished
                  ? 'text-moss'
                  : 'text-vermilion'
            "
            aria-hidden="true"
          >
            {{ isReview ? '复盘' : finished ? '已结束' : '进行中' }}
          </span>
        </div>

        <dl class="mt-5 grid grid-cols-2 gap-y-3 border-t border-line pt-4 sm:grid-cols-4">
          <div>
            <dt class="text-[0.62rem] text-ink-faint">
              {{ reviewRequested ? '复盘阶段' : '当前阶段' }}
            </dt>
            <dd class="mt-1 text-[0.76rem]">
              {{ STAGE_LABELS[script.stage] }}
            </dd>
          </div>
          <template v-if="screening">
            <div>
              <dt class="text-[0.62rem] text-ink-faint">简历评分</dt>
              <dd class="mt-1 font-mono text-[0.76rem] text-vermilion">
                {{ screening.score }} / 100
              </dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">评估维度</dt>
              <dd class="mt-1 text-[0.76rem]">
                {{ screening.dimensions.map((item) => item.label).join(' / ') }}
              </dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">初筛结论</dt>
              <dd class="mt-1 text-[0.76rem]">{{ screening.verdict }}</dd>
            </div>
          </template>
          <template v-else>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">面试重点</dt>
              <dd class="mt-1 text-[0.76rem]">{{ script.focus.join(' / ') }}</dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">预计时长</dt>
              <dd class="mt-1 text-[0.76rem]">{{ script.estimatedMinutes }} 分钟</dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">已用时间</dt>
              <dd class="mt-1 font-mono text-[0.76rem]">{{ isReview ? '—' : elapsed }}</dd>
            </div>
          </template>
        </dl>
      </header>

      <div class="mt-7 grid gap-9 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div>
          <div v-if="isReview" class="mb-4 border border-line bg-paper-deep/60 px-4 py-3">
            <p class="text-[0.64rem] text-ink-faint">
              {{ reviewRequested ? '阶段复盘 / STAGE REVIEW' : '复盘模式 / REVIEW' }}
            </p>
            <p class="mt-1 text-[0.7rem] leading-5 text-ink-soft">
              {{
                screening
                  ? '这是该投递的简历初筛解析记录，包含评分与评估结论，仅供回顾。'
                  : reviewRequested
                    ? '这是本轮已完成阶段的面试记录，仅供回顾，不能继续作答。'
                    : '这场流程已经结束，只能查看面试问题，不能继续作答。'
              }}
            </p>
          </div>

          <div v-if="screening" class="border border-line bg-paper-light">
            <div
              class="flex flex-wrap items-center justify-between gap-2 border-b border-line px-4 py-3 text-[0.66rem] text-ink-faint sm:px-6"
            >
              <span>简历解析 / RESUME SCREENING</span>
              <span class="font-mono text-vermilion">{{ screening.score }} / 100</span>
            </div>

            <div class="px-4 py-5 sm:px-6 sm:py-6">
              <p class="text-[0.8rem] leading-7 text-ink-soft">{{ screening.summary }}</p>

              <dl class="mt-6 grid gap-x-6 gap-y-4 sm:grid-cols-2">
                <div
                  v-for="dimension in screening.dimensions"
                  :key="dimension.label"
                  class="border-t border-line pt-2.5"
                >
                  <div class="flex items-baseline justify-between gap-2">
                    <dt class="text-[0.7rem] text-ink">{{ dimension.label }}</dt>
                    <dd class="font-mono text-[0.62rem] text-vermilion">{{ dimension.score }}</dd>
                  </div>
                  <div class="mt-1.5 h-0.5 bg-paper-deep">
                    <div class="h-0.5 bg-ink" :style="{ width: `${dimension.score}%` }" />
                  </div>
                  <dd class="mt-1.5 text-[0.64rem] leading-5 text-ink-faint">
                    {{ dimension.note }}
                  </dd>
                </div>
              </dl>

              <div class="mt-6 grid gap-5 border-t border-line pt-5 sm:grid-cols-2">
                <div>
                  <p class="eyebrow">简历亮点</p>
                  <ul class="mt-2 space-y-1.5">
                    <li
                      v-for="highlight in screening.highlights"
                      :key="highlight"
                      class="flex gap-2 text-[0.72rem] leading-5 text-ink-soft"
                    >
                      <span class="pt-0.5 text-[0.6rem] text-moss" aria-hidden="true">—</span>
                      <span>{{ highlight }}</span>
                    </li>
                  </ul>
                </div>
                <div>
                  <p class="eyebrow">待确认</p>
                  <ul class="mt-2 space-y-1.5">
                    <li
                      v-for="concern in screening.concerns"
                      :key="concern"
                      class="flex gap-2 text-[0.72rem] leading-5 text-ink-soft"
                    >
                      <span class="pt-0.5 text-[0.6rem] text-vermilion" aria-hidden="true">—</span>
                      <span>{{ concern }}</span>
                    </li>
                  </ul>
                </div>
              </div>

              <div
                class="mt-6 flex flex-wrap items-end justify-between gap-4 border border-ink bg-ink px-4 py-4 text-paper-light"
              >
                <div>
                  <p class="text-[0.62rem] text-paper-light/50">初筛结论 / SCREENING RESULT</p>
                  <p class="mt-1.5 font-display text-xl leading-tight">
                    {{ screening.verdict }}
                  </p>
                </div>
                <p class="font-mono text-2xl leading-none">
                  {{ screening.score }}
                  <span class="text-[0.66rem] text-paper-light/60">/ 100</span>
                </p>
              </div>
            </div>
          </div>

          <div v-else class="border border-line bg-paper-light">
            <div
              class="flex flex-wrap items-center justify-between gap-2 border-b border-line px-4 py-3 text-[0.66rem] text-ink-faint sm:px-6"
            >
              <span>面试记录 / INTERVIEW TRANSCRIPT</span>
              <span>{{ answeredCount }} / {{ turns.length }} 已答</span>
            </div>
            <div class="h-0.5 bg-paper-deep">
              <div class="h-0.5 bg-vermilion transition-all" :style="{ width: `${progress}%` }" />
            </div>

            <ol class="divide-y divide-line">
              <li
                v-for="(turn, index) in visibleTurns"
                :key="index"
                class="px-4 py-5 sm:px-6 sm:py-6"
              >
                <div class="flex gap-3 sm:gap-5">
                  <span class="shrink-0 text-[0.66rem] text-vermilion">
                    问 {{ String(index + 1).padStart(2, '0') }}
                  </span>
                  <div class="min-w-0 flex-1">
                    <p class="font-display text-lg leading-7">{{ turn.question }}</p>
                    <p v-if="turn.followUp" class="mt-1.5 text-[0.68rem] leading-5 text-ink-faint">
                      追问 · {{ turn.followUp }}
                    </p>

                    <div
                      v-if="index === step && !finished && !isReview"
                      class="mt-4 border-t border-dashed border-line pt-4"
                    >
                      <label
                        :for="`answer-${index}`"
                        class="flex items-baseline justify-between gap-2 text-[0.64rem] text-ink-faint"
                      >
                        <span>写下你的回答 / 至少 8 字</span>
                        <span>{{ currentAnswer.trim().length }} 字</span>
                      </label>
                      <textarea
                        :id="`answer-${index}`"
                        v-model="currentAnswer"
                        class="field mt-2 text-sm leading-6"
                        rows="5"
                        placeholder="先给结论，再讲过程和结果。像真正面试一样组织语言。"
                      />
                      <div class="mt-3 flex flex-wrap items-center justify-between gap-3">
                        <p class="max-w-xs text-[0.6rem] leading-4 text-ink-faint">
                          提交后进入下一问，无法回退修改。
                        </p>
                        <button
                          type="button"
                          class="btn-solid"
                          :disabled="!canSubmit"
                          @click="submit"
                        >
                          {{ step === turns.length - 1 ? '提交并结束面试' : '提交，进入下一问' }}
                        </button>
                      </div>
                    </div>

                    <div v-else class="mt-4 border-l-2 border-line pl-3">
                      <p class="text-[0.62rem] text-ink-faint">
                        答 {{ String(index + 1).padStart(2, '0') }}
                      </p>
                      <p class="mt-1 whitespace-pre-line text-sm leading-6 text-ink-soft">
                        {{
                          answers[index] ||
                          (isReview ? '复盘模式：此处仅保留问题。' : '（未作答）')
                        }}
                      </p>
                    </div>
                  </div>
                </div>
              </li>
            </ol>
          </div>

          <div
            v-if="finished && !isReview"
            class="mt-5 border border-ink bg-ink px-5 py-5 text-paper-light"
          >
            <div class="flex flex-wrap items-end justify-between gap-4">
              <div>
                <p class="text-[0.64rem] text-paper-light/50">
                  本轮记录完成 / SESSION COMPLETE
                </p>
                <p class="mt-2 font-display text-3xl leading-none">
                  {{ score }}
                  <span class="text-sm text-paper-light/60">/ 100 · 模拟评估</span>
                </p>
                <p class="mt-1.5 text-[0.66rem] text-paper-light/60">
                  共回答 {{ answeredCount }} 题 · 用时 {{ elapsed }}
                </p>
              </div>
              <button type="button" class="btn-paper" @click="complete">
                完成本轮，推进流程 →
              </button>
            </div>
          </div>
        </div>

        <aside class="space-y-6">
          <section class="border border-line bg-paper-light p-4">
            <div class="flex items-baseline justify-between border-b border-line pb-2">
              <h2 class="font-display text-lg">流程位置</h2>
              <span class="text-[0.64rem] text-ink-faint">
                {{ STAGE_LABELS[script.stage] }}
              </span>
            </div>
            <div class="mt-3">
              <StageRail
                :stages="application.stages"
                :application-id="application.id"
                :active-stage="isReview ? undefined : application.currentStage"
                dense
              />
            </div>
            <p class="mt-3 text-[0.66rem] leading-5 text-ink-soft">{{ application.nextAction }}</p>
          </section>

          <section class="border border-line p-4">
            <p class="text-[0.64rem] text-ink-faint">{{ screening ? '初筛官' : '面试官' }}</p>
            <p class="mt-2 font-display text-lg">
              {{ screening ? screening.reviewer : script.interviewer }}
            </p>
            <p class="text-[0.64rem] text-ink-faint">
              {{ screening ? screening.reviewerTitle : script.interviewerTitle }}
            </p>
            <div class="mt-3 flex flex-wrap gap-1.5">
              <span
                v-for="focus in screening ? screening.dimensions.map((item) => item.label) : script.focus"
                :key="focus"
                class="chip"
              >
                {{ focus }}
              </span>
            </div>
          </section>

          <section v-if="!screening" class="border border-line p-4">
            <p class="text-[0.64rem] text-ink-faint">作答提示</p>
            <ul class="mt-3 space-y-2 text-[0.66rem] leading-5 text-ink-soft">
              <li>· 用「情境—行动—结果」组织项目经历。</li>
              <li>· 数字比形容词有说服力，尽量给出指标。</li>
              <li>· 不确定的问题，说出你的假设和验证方式。</li>
            </ul>
          </section>

          <RouterLink
            v-if="isReview"
            :to="{ name: 'applications' }"
            class="btn-line w-full"
          >
            返回面试进度
          </RouterLink>
        </aside>
      </div>
    </template>
  </div>
</template>
