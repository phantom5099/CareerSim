<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { resumeApi } from '@/api/endpoints/resume'
import type { ResumePayload } from '@/api/types'
import { useAuthStore } from '@/stores/auth'

type ResumeDraft = ResumePayload

const auth = useAuthStore()

function blankDraft(): ResumeDraft {
  return {
    name: auth.user?.displayName ?? '林同学',
    school: auth.user?.school ?? '南山大学',
    major: auth.user?.major ?? '计算机科学',
    targetRole: auth.user?.targetRoles[0] ?? '前端工程师',
    city: auth.user?.targetCities[0] ?? '杭州',
    contact: 'lin@example.com',
    experiences:
      '负责公司官网改版，首屏加载从 1.2s 降到 0.8s，转化率提升 12%\n搭建内部组件库，覆盖 30+ 业务页面，减少重复开发约 40%',
    skills: 'Vue 3, TypeScript, Vite, Pinia, Tailwind CSS',
  }
}

const draft = ref<ResumeDraft>(blankDraft())
const preview = ref<ResumeDraft>({ ...draft.value })
const notice = ref<{ tone: 'ok' | 'warn'; text: string } | null>(null)

onMounted(async () => {
  const saved = await resumeApi.load()
  if (saved) {
    draft.value = { ...saved }
    preview.value = { ...saved }
  }
})

watch(
  draft,
  (value) => {
    void resumeApi.save({ ...value })
  },
  { deep: true },
)

function lines(value: string): string[] {
  return value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
}

function keywords(value: string): string[] {
  return value
    .split(/[,，、/]/)
    .map((item) => item.trim())
    .filter(Boolean)
}

const previewExperiences = computed(() => lines(preview.value.experiences))
const previewSkills = computed(() => keywords(preview.value.skills))

function generate() {
  const missing: string[] = []
  if (!draft.value.name.trim()) {
    missing.push('姓名')
  }
  if (!draft.value.school.trim()) {
    missing.push('学校')
  }
  if (!draft.value.targetRole.trim()) {
    missing.push('求职方向')
  }
  if (!lines(draft.value.experiences).length) {
    missing.push('项目或实习经历')
  }
  if (missing.length) {
    notice.value = { tone: 'warn', text: `请先补全：${missing.join('、')}。` }
    return
  }
  preview.value = { ...draft.value }
  notice.value = { tone: 'ok', text: '简历已生成，可复制文本或直接对照修改。' }
}

function resumeText(value: ResumeDraft): string {
  const target = [value.targetRole, value.city].filter(Boolean).join(' · ')
  const sections = [
    [value.name, [value.school, value.major].filter(Boolean).join(' · ')].join('\n'),
    target ? `求职方向：${target}` : '',
    value.contact ? `联系方式：${value.contact}` : '',
    ['项目与实习经历', ...lines(value.experiences).map((line) => `- ${line}`)].join('\n'),
    keywords(value.skills).length ? ['技能', ...keywords(value.skills)].join(' · ') : '',
  ]
  return sections.filter(Boolean).join('\n\n')
}

async function copyResume() {
  try {
    await navigator.clipboard.writeText(resumeText(preview.value))
    notice.value = { tone: 'ok', text: '简历文本已复制到剪贴板。' }
  } catch {
    notice.value = { tone: 'warn', text: '复制失败，可直接选中预览内容手动复制。' }
  }
}

function resetDraft() {
  draft.value = blankDraft()
  preview.value = { ...draft.value }
  notice.value = null
}

const tips = computed(() => {
  const items: { tone: 'ok' | 'warn'; text: string }[] = []
  const experienceLines = lines(draft.value.experiences)
  if (experienceLines.length < 2) {
    items.push({ tone: 'warn', text: '项目与实习经历至少写两条，每条一句背景、一句结果。' })
  } else if (!experienceLines.every((line) => /\d/.test(line))) {
    items.push({ tone: 'warn', text: '部分经历缺少数字，补上人数、耗时或提升比例。' })
  } else {
    items.push({ tone: 'ok', text: '经历描述已包含可量化的结果。' })
  }
  if (keywords(draft.value.skills).length < 3) {
    items.push({ tone: 'warn', text: '技能关键词建议补到 3 个以上，覆盖语言、框架与工具。' })
  } else {
    items.push({ tone: 'ok', text: '技能关键词数量足够，注意与目标岗位对齐。' })
  }
  if (!draft.value.targetRole.trim()) {
    items.push({ tone: 'warn', text: '填写求职方向，招聘方才能快速判断匹配度。' })
  }
  if (!draft.value.contact.trim()) {
    items.push({ tone: 'warn', text: '补上邮箱或手机号，方便招聘方联系。' })
  }
  return items
})
</script>

<template>
  <div>
    <h1 class="sr-only">简历优化</h1>

    <div class="mt-1 grid gap-9 lg:grid-cols-[minmax(0,1fr)_21rem]">
      <div class="space-y-9">
        <section>
          <div class="flex items-baseline justify-between border-b border-ink pb-2">
            <h2 class="font-display text-2xl">简历信息</h2>
          </div>

          <form class="mt-4" @submit.prevent="generate">
            <div class="grid gap-4 sm:grid-cols-2">
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">姓名</span>
                <input
                  v-model="draft.name"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="你的姓名"
                  autocomplete="off"
                />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">学校</span>
                <input
                  v-model="draft.school"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="学校名称"
                  autocomplete="off"
                />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">专业</span>
                <input
                  v-model="draft.major"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="专业方向"
                  autocomplete="off"
                />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">求职方向</span>
                <input
                  v-model="draft.targetRole"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="如：前端工程师"
                  autocomplete="off"
                />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">意向城市</span>
                <input
                  v-model="draft.city"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="如：杭州"
                  autocomplete="off"
                />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">联系方式</span>
                <input
                  v-model="draft.contact"
                  type="text"
                  class="field mt-1.5 text-sm"
                  placeholder="邮箱或手机号"
                  autocomplete="off"
                />
              </label>
            </div>

            <label class="mt-4 block">
              <span class="text-[0.62rem] text-ink-faint">项目与实习经历 · 一行一条</span>
              <textarea
                v-model="draft.experiences"
                class="field mt-1.5 text-sm leading-6"
                rows="6"
                placeholder="一句话写清背景、动作和结果，尽量带上数字。"
              />
            </label>

            <label class="mt-4 block">
              <span class="text-[0.62rem] text-ink-faint">技能与证书 · 用逗号分隔</span>
              <input
                v-model="draft.skills"
                type="text"
                class="field mt-1.5 text-sm"
                placeholder="如：Vue 3, TypeScript, Vite"
                autocomplete="off"
              />
            </label>

            <div class="mt-5 flex flex-wrap items-center gap-3">
              <button type="submit" class="btn-solid py-2 text-[0.78rem]">生成简历 →</button>
              <button type="button" class="btn-line py-2 text-[0.76rem]" @click="copyResume">
                复制文本
              </button>
              <button
                type="button"
                class="border-b border-line-strong pb-0.5 text-[0.68rem] text-ink-soft transition hover:border-vermilion hover:text-vermilion"
                @click="resetDraft"
              >
                重置
              </button>
            </div>

            <p
              v-if="notice"
              role="status"
              class="mt-4 border px-3 py-2 text-[0.7rem]"
              :class="
                notice.tone === 'ok'
                  ? 'border-moss/40 bg-moss/5 text-moss'
                  : 'border-vermilion/40 bg-vermilion/5 text-vermilion'
              "
            >
              {{ notice.text }}
            </p>
          </form>
        </section>

        <section>
          <div class="border-b border-ink pb-2">
            <h2 class="font-display text-2xl">优化提示</h2>
          </div>
          <ul class="mt-3 space-y-2.5">
            <li
              v-for="tip in tips"
              :key="tip.text"
              class="flex gap-2 text-[0.72rem] leading-6 text-ink-soft"
            >
              <span :class="tip.tone === 'ok' ? 'text-moss' : 'text-vermilion'" aria-hidden="true">
                —
              </span>
              <span>{{ tip.text }}</span>
            </li>
          </ul>
        </section>
      </div>

      <aside class="lg:sticky lg:top-6 lg:self-start">
        <div class="flex items-baseline justify-between border-b border-ink pb-2">
          <h2 class="font-display text-lg">简历预览</h2>
          <span class="text-[0.62rem] text-ink-faint">单页 · A4</span>
        </div>

        <div class="mt-3 border border-line bg-paper-light p-5">
          <p class="font-mono text-[0.54rem] tracking-[0.22em] text-ink-faint">RESUME / 2026</p>
          <p class="mt-2 font-display text-3xl leading-none">{{ preview.name || '姓名' }}</p>
          <p class="mt-2 text-[0.74rem] text-ink-soft">
            {{ [preview.school, preview.major].filter(Boolean).join(' · ') || '学校 · 专业' }}
          </p>
          <p class="mt-1 text-[0.68rem] text-ink-faint">
            {{ [preview.targetRole, preview.city].filter(Boolean).join(' · ') || '求职方向' }}
          </p>
          <p v-if="preview.contact" class="mt-1 font-mono text-[0.58rem] text-ink-faint">
            {{ preview.contact }}
          </p>

          <div class="mt-4 border-t border-ink pt-2">
            <p class="text-[0.62rem] text-ink-faint">项目与实习经历</p>
            <ul v-if="previewExperiences.length" class="mt-2 space-y-2">
              <li
                v-for="line in previewExperiences"
                :key="line"
                class="flex gap-2 text-[0.72rem] leading-5"
              >
                <span class="text-vermilion" aria-hidden="true">—</span>
                <span>{{ line }}</span>
              </li>
            </ul>
            <p v-else class="mt-2 text-[0.66rem] text-ink-faint">暂无经历。</p>
          </div>

          <div class="mt-4 border-t border-line pt-2">
            <p class="text-[0.62rem] text-ink-faint">技能</p>
            <div class="mt-2 flex flex-wrap gap-1.5">
              <span v-for="skill in previewSkills" :key="skill" class="chip">{{ skill }}</span>
              <span v-if="!previewSkills.length" class="text-[0.62rem] text-ink-faint">暂无</span>
            </div>
          </div>
        </div>

      </aside>
    </div>
  </div>
</template>
