<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { companiesApi } from '@/api/endpoints/companies'
import { defaultTemplateStages } from '@/mock/data/companyTemplates'
import {
  JOB_STATUS_LABELS,
  JOB_TYPE_LABELS,
  TEMPLATE_LIMITS,
} from '@/domain/types'
import type {
  Job,
  JobStatus,
  JobType,
  PipelineInfo,
  RecruitmentProject,
} from '@/domain/types'

const props = defineProps<{
  companyId: string
  projects: RecruitmentProject[]
  pipelines: PipelineInfo[]
  /** null = 新建岗位。 */
  job: Job | null
  /** 新建时默认选中的项目。 */
  defaultProjectId?: string
}>()

const emit = defineEmits<{ close: []; saved: [] }>()

const saving = ref(false)
const error = ref('')

// 领域 Job.type 存展示标签（校招/实习/社招），保存前还原原始枚举。
const initialJobType = (Object.entries(JOB_TYPE_LABELS).find(
  ([, label]) => label === props.job?.type,
)?.[0] ?? 'campus') as JobType

const initialProjectKey = computed(() => {
  if (props.job?.projectId) {
    return props.job.projectId
  }
  return props.defaultProjectId ?? props.projects[0]?.id ?? ''
})

const form = ref({
  projectKey: initialProjectKey.value,
  pipelineId: props.job?.pipelineId ?? '',
  title: props.job?.title ?? '',
  city: props.job?.city ?? '',
  jobType: initialJobType,
  status: (props.job?.status ?? 'draft') as JobStatus,
  salaryMin: props.job?.salaryMin != null ? String(props.job.salaryMin) : '',
  salaryMax: props.job?.salaryMax != null ? String(props.job.salaryMax) : '',
  salaryMonths: props.job?.salaryMonths != null ? String(props.job.salaryMonths) : '',
  tags: props.job?.tags.join(', ') ?? '',
  description: props.job?.description ?? '',
})

const jobTypeOptions = Object.entries(JOB_TYPE_LABELS) as [JobType, string][]
const jobStatusOptions = Object.entries(JOB_STATUS_LABELS) as [JobStatus, string][]

const pipelineOptions = computed(() =>
  props.pipelines.filter((pipeline) => pipeline.projectId === form.value.projectKey),
)

// 切换所属项目后原流水线不再有效；无流水线时用 '' 表示保存时自动创建。
watch(
  () => form.value.projectKey,
  () => {
    form.value.pipelineId = pipelineOptions.value[0]?.id ?? ''
  },
)

// type="number" 的 v-model 会把输入转成 number，这里统一按字符串处理。
function optionalNumber(value: string | number): number | undefined {
  const text = String(value).trim()
  const parsed = Number(text)
  return text !== '' && Number.isFinite(parsed) ? parsed : undefined
}

/** 编辑时留空表示清空（PATCH 语义：null = 清空）。 */
function nullableNumber(value: string | number): number | null | undefined {
  const text = String(value).trim()
  if (text === '') {
    return null
  }
  const parsed = Number(text)
  return Number.isFinite(parsed) ? parsed : undefined
}

function splitTags(value: string): string[] {
  return value
    .split(/[,，、]/)
    .map((tag) => tag.trim())
    .filter(Boolean)
}

async function save() {
  error.value = ''
  if (saving.value) {
    return
  }
  if (!form.value.title.trim()) {
    error.value = '岗位名称不能为空。'
    return
  }
  if (!form.value.projectKey) {
    error.value = '请先为这家公司添加招聘项目。'
    return
  }
  saving.value = true
  try {
    const common = {
      title: form.value.title.trim(),
      city: form.value.city.trim(),
      jobType: form.value.jobType,
      status: form.value.status,
      tags: splitTags(form.value.tags),
      description: form.value.description.trim(),
    }
    if (props.job) {
      await companiesApi.updateJob(props.companyId, props.job.id, {
        ...common,
        pipelineId: form.value.pipelineId,
        salaryMin: nullableNumber(form.value.salaryMin),
        salaryMax: nullableNumber(form.value.salaryMax),
        salaryMonths: nullableNumber(form.value.salaryMonths),
      })
    } else {
      let pipelineId = form.value.pipelineId
      if (!pipelineId) {
        const pipeline = await companiesApi.createPipeline(props.companyId, form.value.projectKey, {
          name: '标准面试流程',
          stages: defaultTemplateStages(),
        })
        pipelineId = pipeline.id
      }
      await companiesApi.createJob(props.companyId, form.value.projectKey, {
        ...common,
        pipelineId,
        salaryMin: optionalNumber(form.value.salaryMin),
        salaryMax: optionalNumber(form.value.salaryMax),
        salaryMonths: optionalNumber(form.value.salaryMonths),
      })
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
      aria-labelledby="job-dialog-title"
      class="max-h-[90vh] w-full max-w-xl overflow-y-auto border border-ink bg-paper-light shadow-xl"
    >
      <div class="flex items-baseline justify-between border-b border-ink px-5 py-3">
        <h2 id="job-dialog-title" class="font-display text-xl">
          {{ job ? '编辑岗位' : '添加岗位' }}
        </h2>
        <button
          type="button"
          class="text-lg leading-none text-ink-faint transition hover:text-vermilion"
          aria-label="关闭岗位编辑"
          @click="emit('close')"
        >
          ×
        </button>
      </div>

      <div class="grid gap-3 px-5 py-4 sm:grid-cols-2">
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">所属项目 *</span>
          <select
            v-model="form.projectKey"
            class="field mt-1 w-full text-[0.78rem]"
            :disabled="Boolean(job)"
            :title="job ? '已有岗位不能更换项目' : ''"
          >
            <option v-for="project in projects" :key="project.id" :value="project.id">
              {{ project.name }}
            </option>
          </select>
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">岗位名称 *</span>
          <input v-model="form.title" type="text" :maxlength="TEMPLATE_LIMITS.jobTitle" class="field mt-1 w-full text-[0.78rem]" />
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">工作城市</span>
          <input v-model="form.city" type="text" :maxlength="TEMPLATE_LIMITS.city" class="field mt-1 w-full text-[0.78rem]" />
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">岗位类型</span>
          <select v-model="form.jobType" class="field mt-1 w-full text-[0.78rem]">
            <option v-for="[value, label] in jobTypeOptions" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">发布状态</span>
          <select v-model="form.status" class="field mt-1 w-full text-[0.78rem]">
            <option v-for="[value, label] in jobStatusOptions" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">面试流水线</span>
          <select
            v-model="form.pipelineId"
            class="field mt-1 w-full text-[0.78rem]"
            :disabled="pipelineOptions.length === 0"
          >
            <option v-if="!pipelineOptions.length" value="">保存时自动创建标准流程</option>
            <option v-for="pipeline in pipelineOptions" :key="pipeline.id" :value="pipeline.id">
              {{ pipeline.name }}
            </option>
          </select>
        </label>
        <div class="grid grid-cols-3 gap-2 sm:col-span-2">
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">月薪下限</span>
            <input v-model="form.salaryMin" type="number" min="0" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">月薪上限</span>
            <input v-model="form.salaryMax" type="number" min="0" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">薪资月数</span>
            <input v-model="form.salaryMonths" type="number" min="1" max="24" class="field mt-1 w-full text-[0.78rem]" />
          </label>
        </div>
        <label class="block sm:col-span-2">
          <span class="text-[0.62rem] text-ink-faint">技能标签（用逗号分隔）</span>
          <input v-model="form.tags" type="text" class="field mt-1 w-full text-[0.78rem]" placeholder="如：SQL, BI, 零售" />
        </label>
        <label class="block sm:col-span-2">
          <span class="text-[0.62rem] text-ink-faint">岗位描述</span>
          <textarea v-model="form.description" rows="2" :maxlength="TEMPLATE_LIMITS.jobDescription" class="field mt-1 w-full text-[0.78rem] leading-6" />
        </label>
      </div>

      <p
        v-if="error"
        role="alert"
        class="mx-5 border border-vermilion/40 bg-vermilion/5 px-3 py-2 text-[0.68rem] text-vermilion"
      >
        {{ error }}
      </p>

      <div class="flex flex-wrap items-center justify-end gap-2 border-t border-line px-5 py-3">
        <button
          type="button"
          class="btn-line py-1.5 text-[0.7rem]"
          :disabled="saving"
          @click="emit('close')"
        >
          取消
        </button>
        <button
          type="button"
          class="btn-solid py-1.5 text-[0.7rem]"
          :disabled="saving"
          @click="save"
        >
          {{ saving ? '保存中…' : job ? '保存修改 →' : '创建岗位 →' }}
        </button>
      </div>
    </div>
  </div>
</template>
