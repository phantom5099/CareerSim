<script setup lang="ts">
import { reactive, ref } from 'vue'

import { companiesApi } from '@/api/endpoints/companies'
import type { PipelinePayload, StageDefinitionPayload } from '@/api/types'
import { STAGE_LABELS } from '@/domain/types'
import type { PipelineInfo, StageKind } from '@/domain/types'

const props = defineProps<{
  companyId: string
  projectId: string
  pipeline: PipelineInfo | null
}>()

const emit = defineEmits<{ close: []; saved: [] }>()

const stageKinds: StageKind[] = [
  'hr_screen',
  'exam',
  'tech_interview',
  'manager_interview',
  'hrbp_interview',
  'offer',
]

function defaultStage(kind: StageKind = 'hr_screen'): StageDefinitionPayload {
  return {
    position: 0,
    kind,
    title: STAGE_LABELS[kind],
    minRounds: 1,
    maxRounds: 3,
    rubric: {
      passScore: 60,
      dimensions: [{ key: 'overall', label: '综合表现', weight: 100, criteria: '' }],
    },
  }
}

function cloneStages(pipeline: PipelineInfo | null): StageDefinitionPayload[] {
  if (!pipeline) {
    return [defaultStage(), defaultStage('offer')]
  }
  return pipeline.stages.map((stage) => ({
    position: stage.position,
    kind: stage.kind,
    title: stage.title,
    minRounds: stage.minRounds,
    maxRounds: stage.maxRounds,
    rubric: {
      passScore: stage.rubric.passScore,
      dimensions: stage.rubric.dimensions.map((dimension) => ({ ...dimension })),
    },
  }))
}

const form = reactive({
  name: props.pipeline?.name ?? '新流程',
  stages: cloneStages(props.pipeline),
})
const saving = ref(false)
const error = ref('')

function moveStage(index: number, direction: -1 | 1) {
  const target = index + direction
  if (target < 0 || target >= form.stages.length) {
    return
  }
  const [stage] = form.stages.splice(index, 1)
  form.stages.splice(target, 0, stage)
}

function addStage() {
  form.stages.push(defaultStage())
}

function addDimension(stage: StageDefinitionPayload) {
  stage.rubric ??= { passScore: 60, dimensions: [] }
  stage.rubric.dimensions.push({
    key: `dimension_${stage.rubric.dimensions.length + 1}`,
    label: '新维度',
    weight: 0,
    criteria: '',
  })
}

function removeDimension(stage: StageDefinitionPayload, index: number) {
  if (stage.rubric && stage.rubric.dimensions.length > 1) {
    stage.rubric.dimensions.splice(index, 1)
  }
}

function removeStage(index: number) {
  if (form.stages.length > 1) {
    form.stages.splice(index, 1)
  }
}

function stageWeightTotal(stage: StageDefinitionPayload): number {
  return stage.rubric.dimensions.reduce(
    (total, dimension) => total + (Number.isFinite(dimension.weight) ? dimension.weight : 0),
    0,
  )
}

function stageLabel(stage: StageDefinitionPayload, index: number): string {
  return `第 ${index + 1} 阶段「${stage.title.trim() || STAGE_LABELS[stage.kind as StageKind] || stage.kind}」`
}

function validateStages(): string | undefined {
  const offerIndexes = form.stages
    .map((stage, index) => (stage.kind === 'offer' ? index : -1))
    .filter((index) => index >= 0)
  if (offerIndexes.length === 0) {
    return '流程必须包含 1 个 Offer 阶段，当前没有 Offer 阶段。'
  }
  if (offerIndexes.length > 1) {
    const stages = offerIndexes.map((index) => `第 ${index + 1} 阶段`).join('、')
    return `流程只能包含 1 个 Offer 阶段，当前有 ${offerIndexes.length} 个（${stages}）。`
  }
  if (offerIndexes[0] !== form.stages.length - 1) {
    return `${stageLabel(form.stages[offerIndexes[0]], offerIndexes[0])}必须放在最后。`
  }

  for (const [index, stage] of form.stages.entries()) {
    const label = stageLabel(stage, index)
    if (!stage.title.trim()) {
      return `${label}的阶段标题不能为空。`
    }
    if ([...stage.title.trim()].length > 40) {
      return `${label}的阶段标题超过 40 个字符。`
    }
    if (
      !Number.isInteger(stage.minRounds) ||
      !Number.isInteger(stage.maxRounds) ||
      stage.minRounds < 1 ||
      stage.maxRounds < stage.minRounds ||
      stage.maxRounds > 20
    ) {
      return `${label}的轮次设置无效，必须满足 1 ≤ 最少轮次 ≤ 最多轮次 ≤ 20。`
    }
    if (!Number.isInteger(stage.rubric.passScore) || stage.rubric.passScore < 0 || stage.rubric.passScore > 100) {
      return `${label}的通过分数必须是 0 到 100 的整数。`
    }
    if (!stage.rubric.dimensions.length) {
      return `${label}至少需要一个评分维度。`
    }
    if (stage.rubric.dimensions.length > 10) {
      return `${label}最多只能有 10 个评分维度。`
    }

    let total = 0
    let hasZero = false
    const dimensionKeys = new Set<string>()
    for (const [dimensionIndex, dimension] of stage.rubric.dimensions.entries()) {
      const dimensionLabel = dimension.label.trim() || `第 ${dimensionIndex + 1} 个维度`
      const dimensionKey = dimension.key.trim()
      if (!dimensionKey) {
        return `${label}的${dimensionLabel}缺少维度键。`
      }
      if (dimensionKeys.has(dimensionKey)) {
        return `${label}存在重复的评分维度键「${dimensionKey}」。`
      }
      dimensionKeys.add(dimensionKey)
      if (!Number.isInteger(dimension.weight)) {
        return `${label}的评分维度「${dimensionLabel}」权重必须是整数。`
      }
      if (dimension.weight < 0 || dimension.weight > 100) {
        return `${label}的评分维度「${dimensionLabel}」权重为 ${dimension.weight}，必须在 0 到 100 之间。`
      }
      total += dimension.weight
      hasZero ||= dimension.weight === 0
    }
    if (total !== 0 && hasZero) {
      return `${label}包含 0 权重维度。只有所有维度权重都为 0 时才表示等权，否则每个维度都必须大于 0。`
    }
    if (total !== 0 && total !== 100) {
      return `${label}的评分权重合计为 ${total}，必须等于 100。`
    }
  }
  return undefined
}

async function save() {
  error.value = ''
  if (saving.value) {
    return
  }
  if (!form.name.trim()) {
    error.value = '流程名称不能为空。'
    return
  }
  if (!form.stages.length) {
    error.value = '流程至少需要一个阶段。'
    return
  }
  const stageError = validateStages()
  if (stageError) {
    error.value = stageError
    return
  }
  saving.value = true
  try {
    const input: PipelinePayload = {
      name: form.name.trim(),
      stages: form.stages.map((stage, position) => ({
        ...stage,
        position,
        title: stage.title.trim(),
        rubric: {
          ...stage.rubric,
          dimensions: stage.rubric.dimensions.map((dimension) => ({
            ...dimension,
            key: dimension.key.trim(),
            label: dimension.label.trim(),
            criteria: dimension.criteria.trim(),
          })),
        },
      })),
    }
    if (props.pipeline) {
      await companiesApi.updatePipeline(props.companyId, props.pipeline.id, input)
    } else {
      await companiesApi.createPipeline(props.companyId, props.projectId, input)
    }
    emit('saved')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存失败，请稍后重试。'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div
    class="fixed inset-0 z-50 flex items-end justify-center bg-ink/40 p-4 sm:items-center"
    role="presentation"
    @click.self="emit('close')"
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="pipeline-dialog-title"
      class="max-h-[92vh] w-full max-w-4xl overflow-y-auto border border-ink bg-paper-light shadow-xl"
    >
      <div class="flex items-baseline justify-between border-b border-ink px-5 py-3">
        <h2 id="pipeline-dialog-title" class="font-display text-xl">
          {{ pipeline ? '编辑流程模板' : '添加流程模板' }}
        </h2>
        <button type="button" class="text-lg" aria-label="关闭流程编辑" @click="emit('close')">
          ×
        </button>
      </div>

      <div class="space-y-4 px-5 py-4">
        <label class="block max-w-md">
          <span class="text-[0.62rem] text-ink-faint">流程名称 *</span>
          <input v-model="form.name" class="field mt-1 w-full text-[0.78rem]" maxlength="60" />
        </label>

        <div class="flex items-center justify-between border-b border-line pb-2">
          <div>
            <h3 class="font-display text-lg">阶段顺序</h3>
            <p class="text-[0.64rem] text-ink-faint">第一项是流程起点，Offer 阶段应放在末尾。</p>
          </div>
          <button type="button" class="btn-line py-1 text-[0.64rem]" @click="addStage">
            ＋ 添加阶段
          </button>
        </div>

        <div class="space-y-4">
          <section
            v-for="(stage, index) in form.stages"
            :key="`${index}-${stage.kind}`"
            class="border border-line p-4"
          >
            <div class="flex items-center justify-between gap-2">
              <h4 class="font-medium">第 {{ index + 1 }} 阶段</h4>
              <div class="flex gap-1.5">
                <button type="button" class="btn-line px-2 py-0.5 text-[0.62rem]" @click="moveStage(index, -1)">
                  上移
                </button>
                <button type="button" class="btn-line px-2 py-0.5 text-[0.62rem]" @click="moveStage(index, 1)">
                  下移
                </button>
                <button type="button" class="btn-line px-2 py-0.5 text-[0.62rem]" @click="removeStage(index)">
                  删除
                </button>
              </div>
            </div>

            <div class="mt-3 grid gap-3 sm:grid-cols-4">
              <label class="block">
                <span class="text-[0.6rem] text-ink-faint">阶段类型</span>
                <select v-model="stage.kind" class="field mt-1 w-full text-[0.7rem]">
                  <option v-for="kind in stageKinds" :key="kind" :value="kind">
                    {{ STAGE_LABELS[kind] }}
                  </option>
                </select>
              </label>
              <label class="block sm:col-span-2">
                <span class="text-[0.6rem] text-ink-faint">阶段标题</span>
                <input v-model="stage.title" class="field mt-1 w-full text-[0.7rem]" maxlength="40" />
              </label>
              <label class="block">
                <span class="text-[0.6rem] text-ink-faint">最少轮次</span>
                <input v-model.number="stage.minRounds" type="number" min="1" max="20" class="field mt-1 w-full text-[0.7rem]" />
              </label>
              <label class="block">
                <span class="text-[0.6rem] text-ink-faint">最多轮次</span>
                <input v-model.number="stage.maxRounds" type="number" min="1" max="20" class="field mt-1 w-full text-[0.7rem]" />
              </label>
              <label class="block">
                <span class="text-[0.6rem] text-ink-faint">通过分数</span>
                <input v-model.number="stage.rubric.passScore" type="number" min="0" max="100" class="field mt-1 w-full text-[0.7rem]" />
              </label>
            </div>

            <div class="mt-4 border-t border-line pt-3">
              <div class="flex items-center justify-between">
                <h5 class="text-[0.7rem] font-medium">评分维度</h5>
                <div class="flex items-center gap-3">
                  <span
                    class="text-[0.62rem]"
                    :class="stageWeightTotal(stage) === 0 || stageWeightTotal(stage) === 100 ? 'text-moss' : 'text-vermilion'"
                  >
                    权重合计 {{ stageWeightTotal(stage) }} / 100<span v-if="stageWeightTotal(stage) === 0">（等权）</span>
                  </span>
                  <button type="button" class="text-[0.64rem] text-vermilion" @click="addDimension(stage)">
                    ＋ 添加维度
                  </button>
                </div>
              </div>
              <div class="mt-2 space-y-2">
                <div
                  v-for="(dimension, dimensionIndex) in stage.rubric.dimensions"
                  :key="`${stage.position}-${dimensionIndex}`"
                  class="grid gap-2 sm:grid-cols-[1fr_1fr_5rem_2fr_auto]"
                >
                  <input v-model="dimension.key" class="field text-[0.68rem]" placeholder="键" />
                  <input v-model="dimension.label" class="field text-[0.68rem]" placeholder="名称" />
                  <input v-model.number="dimension.weight" type="number" min="0" max="100" class="field text-[0.68rem]" placeholder="权重" />
                  <input v-model="dimension.criteria" class="field text-[0.68rem]" placeholder="评分标准" />
                  <button type="button" class="btn-line px-2 text-[0.62rem]" @click="removeDimension(stage, dimensionIndex)">
                    删除
                  </button>
                </div>
              </div>
            </div>
          </section>
        </div>

        <p v-if="error" role="alert" class="border border-vermilion/40 px-3 py-2 text-[0.68rem] text-vermilion">
          {{ error }}
        </p>
      </div>

      <div class="flex justify-end gap-2 border-t border-line px-5 py-3">
        <button type="button" class="btn-line py-1.5 text-[0.7rem]" :disabled="saving" @click="emit('close')">
          取消
        </button>
        <button type="button" class="btn-solid py-1.5 text-[0.7rem]" :disabled="saving" @click="save">
          {{ saving ? '保存中…' : '保存流程' }}
        </button>
      </div>
    </div>
  </div>
</template>
