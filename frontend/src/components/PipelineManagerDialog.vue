<script setup lang="ts">
import type { PipelineInfo } from '@/domain/types'

const props = defineProps<{
  projectName: string
  pipelines: PipelineInfo[]
}>()

const emit = defineEmits<{
  close: []
  create: []
  edit: [pipeline: PipelineInfo]
  delete: [pipeline: PipelineInfo]
}>()
</script>

<template>
  <div class="fixed inset-0 z-40 flex items-end justify-center bg-ink/40 p-4 sm:items-center" role="presentation" @click.self="emit('close')">
    <div class="w-full max-w-xl border border-ink bg-paper-light shadow-xl" role="dialog" aria-modal="true" aria-labelledby="pipeline-manager-title">
      <div class="flex items-baseline justify-between border-b border-ink px-5 py-3">
        <div>
          <p class="text-[0.62rem] text-ink-faint">{{ projectName }}</p>
          <h2 id="pipeline-manager-title" class="font-display text-xl">流程管理</h2>
        </div>
        <button type="button" class="text-lg" aria-label="关闭流程管理" @click="emit('close')">×</button>
      </div>

      <div class="space-y-3 px-5 py-4">
        <div v-if="props.pipelines.length" class="space-y-2">
          <article v-for="pipeline in props.pipelines" :key="pipeline.id" class="border border-line px-3 py-3">
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0">
                <h3 class="truncate text-[0.8rem] font-medium">{{ pipeline.name }}</h3>
                <p class="mt-1 text-[0.64rem] text-ink-faint">{{ pipeline.stages.length }} 个阶段</p>
              </div>
              <div class="flex shrink-0 gap-2">
                <button type="button" class="btn-line px-2 py-1 text-[0.62rem]" @click="emit('edit', pipeline)">编辑</button>
                <button type="button" class="btn-line px-2 py-1 text-[0.62rem]" @click="emit('delete', pipeline)">删除</button>
              </div>
            </div>
          </article>
        </div>
        <p v-else class="border border-dashed border-line px-3 py-6 text-center text-[0.7rem] text-ink-faint">暂无流程模板。</p>
      </div>

      <div class="flex justify-end gap-2 border-t border-line px-5 py-3">
        <button type="button" class="btn-line py-1.5 text-[0.7rem]" @click="emit('close')">关闭</button>
        <button type="button" class="btn-solid py-1.5 text-[0.7rem]" @click="emit('create')">添加流程</button>
      </div>
    </div>
  </div>
</template>
