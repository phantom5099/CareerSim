<script setup lang="ts">
import { RouterLink } from 'vue-router'

import { STAGE_LABELS } from '@/domain/types'
import type { ApplicationStage, StageKey, StageState } from '@/domain/types'

const props = withDefaults(
  defineProps<{
    stages: ApplicationStage[]
    dense?: boolean
    applicationId?: string
    activeStage?: StageKey
  }>(),
  { dense: false, applicationId: undefined, activeStage: undefined },
)

const stateClass: Record<StageState, string> = {
  done: 'border-ink text-ink',
  current: 'border-vermilion text-vermilion',
  pending: 'border-line-strong text-ink-soft',
  failed: 'border-line-strong text-ink-faint line-through',
}

function stageClass(stage: ApplicationStage): string {
  if (stage.state === 'current' && stage.key === props.activeStage) {
    return 'border-moss-bright text-moss-bright'
  }
  return stateClass[stage.state]
}

function isReviewable(stage: ApplicationStage): boolean {
  return Boolean(props.applicationId) && (stage.state === 'done' || stage.state === 'failed')
}
</script>

<template>
  <ol class="flex gap-1" aria-label="招聘阶段">
    <template v-for="(stage, index) in stages" :key="stage.key">
      <RouterLink
        v-if="isReviewable(stage)"
        :to="{
          name: 'interview',
          params: { applicationId },
          query: { review: '1', stage: stage.key },
        }"
        class="min-w-0 flex-1 border-t-2 pt-1.5 transition hover:border-vermilion hover:text-vermilion"
        :class="stageClass(stage)"
        title="查看本轮复盘"
      >
        <span class="block font-mono text-[0.55rem] leading-none">
          {{ String(index + 1).padStart(2, '0') }}
        </span>
        <span class="mt-1 block truncate" :class="dense ? 'text-[0.62rem]' : 'text-xs'">
          {{ STAGE_LABELS[stage.key] }}
        </span>
      </RouterLink>
      <li
        v-else
        class="min-w-0 flex-1 border-t-2 pt-1.5"
        :class="stageClass(stage)"
      >
        <span class="block font-mono text-[0.55rem] leading-none">
          {{ String(index + 1).padStart(2, '0') }}
        </span>
        <span class="mt-1 block truncate" :class="dense ? 'text-[0.62rem]' : 'text-xs'">
          {{ STAGE_LABELS[stage.key] }}
        </span>
      </li>
    </template>
  </ol>
</template>
