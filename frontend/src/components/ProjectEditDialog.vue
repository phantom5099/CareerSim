<script setup lang="ts">
import { ref } from 'vue'

import { companiesApi } from '@/api/endpoints/companies'
import { defaultTemplateStages } from '@/mock/data/companyTemplates'
import {
  PROJECT_STATUS_LABELS,
  TEMPLATE_LIMITS,
} from '@/domain/types'
import type { ProjectStatus, RecruitmentProject } from '@/domain/types'

const props = defineProps<{
  companyId: string
  /** null = 新建项目。 */
  project: RecruitmentProject | null
}>()

const emit = defineEmits<{ close: []; saved: [] }>()

const saving = ref(false)
const error = ref('')

const form = ref({
  name: props.project?.name ?? '',
  season: props.project?.season ?? '',
  status: (props.project?.status ?? 'open') as ProjectStatus,
  deadline: props.project?.deadline ?? '',
  description: props.project?.description ?? '',
})

const statusOptions = Object.entries(PROJECT_STATUS_LABELS) as [ProjectStatus, string][]

async function save() {
  error.value = ''
  if (saving.value) {
    return
  }
  if (!form.value.name.trim()) {
    error.value = '项目名称不能为空。'
    return
  }
  saving.value = true
  try {
    if (props.project) {
      // PATCH 语义：deadline 为 null 表示清空截止日期。
      await companiesApi.updateProject(props.companyId, props.project.id, {
        name: form.value.name.trim(),
        season: form.value.season.trim(),
        status: form.value.status,
        deadline: form.value.deadline || null,
        description: form.value.description.trim(),
      })
    } else {
      const created = await companiesApi.createProject(props.companyId, {
        name: form.value.name.trim(),
        season: form.value.season.trim(),
        status: form.value.status,
        deadline: form.value.deadline || undefined,
        description: form.value.description.trim(),
      })
      // 与导入流程一致：新项目默认挂一条标准面试流程，岗位才能立即创建。
      await companiesApi.createPipeline(props.companyId, created.id, {
        name: '标准面试流程',
        stages: defaultTemplateStages(),
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
      aria-labelledby="project-dialog-title"
      class="w-full max-w-lg border border-ink bg-paper-light shadow-xl"
    >
      <div class="flex items-baseline justify-between border-b border-ink px-5 py-3">
        <h2 id="project-dialog-title" class="font-display text-xl">
          {{ project ? '编辑招聘项目' : '添加招聘项目' }}
        </h2>
        <button
          type="button"
          class="text-lg leading-none text-ink-faint transition hover:text-vermilion"
          aria-label="关闭项目编辑"
          @click="emit('close')"
        >
          ×
        </button>
      </div>

      <div class="grid gap-3 px-5 py-4 sm:grid-cols-2">
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">项目名称 *</span>
          <input v-model="form.name" type="text" :maxlength="TEMPLATE_LIMITS.projectName" class="field mt-1 w-full text-[0.78rem]" placeholder="如：2027 校园招聘" />
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">季节标注</span>
          <input v-model="form.season" type="text" :maxlength="TEMPLATE_LIMITS.season" class="field mt-1 w-full text-[0.78rem]" placeholder="如：2027 秋招" />
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">状态（手动关闭开关）</span>
          <select v-model="form.status" class="field mt-1 w-full text-[0.78rem]">
            <option v-for="[value, label] in statusOptions" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>
        <label class="block">
          <span class="text-[0.62rem] text-ink-faint">截止日期（已过则对外显示已结束）</span>
          <input v-model="form.deadline" type="date" class="field mt-1 w-full text-[0.78rem]" />
        </label>
        <label class="block sm:col-span-2">
          <span class="text-[0.62rem] text-ink-faint">项目描述</span>
          <textarea v-model="form.description" rows="2" :maxlength="TEMPLATE_LIMITS.projectDescription" class="field mt-1 w-full text-[0.78rem] leading-6" />
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
          {{ saving ? '保存中…' : project ? '保存修改 →' : '创建项目 →' }}
        </button>
      </div>
    </div>
  </div>
</template>
