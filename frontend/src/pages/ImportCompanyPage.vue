<script setup lang="ts">
import { ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import { companiesApi } from '@/api/endpoints/companies'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'
import {
  ACCENT_OPTIONS,
  COMPANY_SIZE_OPTIONS,
  DEFAULT_ACCENT,
  TEMPLATE_LIMITS,
} from '@/domain/types'

const router = useRouter()
const catalog = useCompanyCatalogStore()

const preview = ref(false)
const errors = ref<string[]>([])
const notice = ref('')
const validating = ref(false)

// 智能生成卡片（弹层）
const showAiCard = ref(false)
const prompt = ref('')
const generating = ref(false)
const generateError = ref('')

const form = ref({
  name: '',
  englishName: '',
  industry: '',
  city: '',
  size: '100–499 人',
  tagline: '',
  description: '',
  highlights: [] as string[],
  logoText: '',
  accent: DEFAULT_ACCENT as string,
})

function addHighlight() {
  if (form.value.highlights.length >= TEMPLATE_LIMITS.highlightItems) {
    return
  }
  form.value.highlights.push('')
}

function selectAccent(value: string) {
  form.value.accent = value
  resetResult()
}

function removeHighlight(index: number) {
  form.value.highlights.splice(index, 1)
}

function resetResult() {
  preview.value = false
  errors.value = []
  notice.value = ''
}

// 表单 → CompanyTemplateV1；招聘项目、流程和岗位在公司保存后再单独维护。
function buildTemplate() {
  return {
    schemaVersion: 1,
    company: {
      name: form.value.name.trim(),
      englishName: form.value.englishName.trim(),
      industry: form.value.industry.trim(),
      city: form.value.city.trim(),
      size: form.value.size.trim(),
      tagline: form.value.tagline.trim(),
      description: form.value.description.trim(),
      highlights: form.value.highlights.map((line) => line.trim()).filter(Boolean),
      logoText: form.value.logoText.trim(),
      accent: form.value.accent.trim(),
    },
    projects: [],
    pipelines: [],
    jobs: [],
  }
}

function clampText(value: unknown, max: number): string {
  return [...String(value ?? '')].slice(0, max).join('')
}

// 模板对象 → 表单；智能生成草稿与导入模板都从这里进表单，超长值与非法枚举一律收敛。
function loadIntoForm(template: Record<string, unknown>) {
  const company = (template.company ?? {}) as Record<string, unknown>
  const size = String(company.size ?? '')
  const accent = String(company.accent ?? '')
  form.value = {
    name: clampText(company.name, TEMPLATE_LIMITS.name),
    englishName: clampText(company.englishName, TEMPLATE_LIMITS.englishName),
    industry: clampText(company.industry, TEMPLATE_LIMITS.industry),
    city: clampText(company.city, TEMPLATE_LIMITS.city),
    size: COMPANY_SIZE_OPTIONS.some((option) => option === size) ? size : '100–499 人',
    tagline: clampText(company.tagline, TEMPLATE_LIMITS.tagline),
    description: clampText(company.description, TEMPLATE_LIMITS.description),
    highlights: Array.isArray(company.highlights)
      ? (company.highlights as unknown[])
          .slice(0, TEMPLATE_LIMITS.highlightItems)
          .map((item) => clampText(item, TEMPLATE_LIMITS.highlightItem))
      : [],
    logoText: clampText(company.logoText, TEMPLATE_LIMITS.logoText),
    accent: ACCENT_OPTIONS.some((option) => option.value === accent)
      ? accent
      : (DEFAULT_ACCENT as string),
  }
}

async function validate() {
  resetResult()
  if (validating.value) {
    return
  }
  validating.value = true
  try {
    const result = await companiesApi.validateImport(buildTemplate())
    if (result.ok) {
      preview.value = true
      notice.value = '模板校验通过，可以保存到公司列表。'
    } else {
      errors.value = result.errors
    }
  } catch (error) {
    errors.value = [error instanceof Error ? error.message : '模板校验失败，请稍后重试。']
  } finally {
    validating.value = false
  }
}

async function generate() {
  if (!prompt.value.trim() || generating.value) {
    return
  }
  generating.value = true
  generateError.value = ''
  try {
    const draft = await companiesApi.generateTemplate(prompt.value)
    loadIntoForm(draft)
    showAiCard.value = false
    await validate()
    if (!errors.value.length) {
      notice.value = '已生成模板草稿，请检查字段后保存。'
    }
  } catch (error) {
    generateError.value =
      error instanceof Error ? error.message : '生成失败，请调整提示词后重试。'
  } finally {
    generating.value = false
  }
}

async function save() {
  if (!preview.value) {
    await validate()
    return
  }
  const saved = await companiesApi.importTemplate(buildTemplate())
  // 刷新目录缓存后再跳转，详情页立即可见，无需手动刷新。
  await catalog.refresh()
  router.push({ name: 'companyDetail', params: { id: saved.id } })
}
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

    <h1 class="sr-only">录入公司</h1>

    <div class="mt-7">
      <div>
        <section>
          <div class="flex flex-wrap items-center justify-between gap-2 border-b border-ink pb-2">
            <h2 class="font-display text-2xl">模板内容</h2>
          </div>

          <!-- 公司信息 -->
          <div class="mt-5">
            <h3 class="text-[0.7rem] font-medium text-ink">公司信息</h3>
            <div class="mt-3 grid gap-3 sm:grid-cols-2">
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">公司名称 *</span>
                <input v-model="form.name" type="text" :maxlength="TEMPLATE_LIMITS.name" class="field mt-1 w-full text-[0.78rem]" @input="resetResult" />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">英文名 / 副标题 *</span>
                <input v-model="form.englishName" type="text" :maxlength="TEMPLATE_LIMITS.englishName" class="field mt-1 w-full text-[0.78rem]" @input="resetResult" />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">行业 *</span>
                <input v-model="form.industry" type="text" :maxlength="TEMPLATE_LIMITS.industry" class="field mt-1 w-full text-[0.78rem]" placeholder="如：数据服务" @input="resetResult" />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">城市 *</span>
                <input v-model="form.city" type="text" :maxlength="TEMPLATE_LIMITS.city" class="field mt-1 w-full text-[0.78rem]" @input="resetResult" />
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">团队规模 *</span>
                <select v-model="form.size" class="field mt-1 w-full text-[0.78rem]">
                  <option v-for="size in COMPANY_SIZE_OPTIONS" :key="size" :value="size">
                    {{ size }}
                  </option>
                </select>
              </label>
              <label class="block">
                <span class="text-[0.62rem] text-ink-faint">标志文字（建议 2 字）*</span>
                <input v-model="form.logoText" type="text" :maxlength="TEMPLATE_LIMITS.logoText" class="field mt-1 w-full text-[0.78rem]" @input="resetResult" />
              </label>
              <label class="block sm:col-span-2">
                <span class="text-[0.62rem] text-ink-faint">一句话主张 *</span>
                <input v-model="form.tagline" type="text" :maxlength="TEMPLATE_LIMITS.tagline" class="field mt-1 w-full text-[0.78rem]" @input="resetResult" />
              </label>
              <label class="block sm:col-span-2">
                <span class="text-[0.62rem] text-ink-faint">公司简介 *</span>
                <textarea v-model="form.description" rows="3" :maxlength="TEMPLATE_LIMITS.description" class="field mt-1 w-full text-[0.78rem] leading-6" @input="resetResult" />
              </label>
              <div class="block sm:col-span-2">
                <span class="text-[0.62rem] text-ink-faint">亮点标签 *（最多 {{ TEMPLATE_LIMITS.highlightItems }} 条，每条 {{ TEMPLATE_LIMITS.highlightItem }} 字以内）</span>
                <div class="mt-2 space-y-2">
                  <div
                    v-for="(_highlight, index) in form.highlights"
                    :key="index"
                    class="flex items-center gap-2"
                  >
                    <input
                      v-model="form.highlights[index]"
                      type="text"
                      :maxlength="TEMPLATE_LIMITS.highlightItem"
                      class="field w-full text-[0.78rem]"
                      :placeholder="`亮点 ${index + 1}`"
                      @input="resetResult"
                    />
                    <button
                      type="button"
                      class="shrink-0 text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      :aria-label="`删除亮点 ${index + 1}`"
                      @click="removeHighlight(index)"
                    >
                      删除
                    </button>
                  </div>
                </div>
                <button
                  type="button"
                  class="btn-line mt-2 py-1 text-[0.64rem]"
                  :disabled="form.highlights.length >= TEMPLATE_LIMITS.highlightItems"
                  @click="addHighlight"
                >
                  ＋ 添加标签
                </button>
              </div>
              <div class="block sm:col-span-2">
                <span class="text-[0.62rem] text-ink-faint">品牌色 *（点色块选择）</span>
                <div class="mt-2 flex flex-wrap items-center gap-2">
                  <button
                    v-for="option in ACCENT_OPTIONS"
                    :key="option.value"
                    type="button"
                    class="relative h-8 w-8 border border-line transition hover:border-ink"
                    :class="form.accent === option.value ? 'ring-2 ring-ink' : ''"
                    :style="{ backgroundColor: option.value }"
                    :aria-label="`${option.label} ${option.value}`"
                    :aria-pressed="form.accent === option.value"
                    :title="`${option.label} ${option.value}`"
                    @click="selectAccent(option.value)"
                  >
                    <span
                      v-if="form.accent === option.value"
                      class="absolute inset-0 flex items-center justify-center text-[0.7rem] text-white"
                    >✓</span>
                  </button>
                  <span class="font-mono text-[0.66rem] text-ink-soft">{{ form.accent }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- 保存公司模板后，再从公司详情页添加招聘项目和岗位。 -->

          <!-- 表单底部操作 -->
          <div class="mt-6 flex flex-wrap items-center justify-between gap-2 border-t border-line pt-4">
            <button
              type="button"
              class="btn-line py-1.5 text-[0.7rem]"
              @click="showAiCard = true"
            >
              ✦ 智能生成
            </button>
            <div class="flex flex-wrap gap-2">
              <button
                type="button"
                class="btn-line py-1.5 text-[0.7rem]"
                :disabled="validating"
                @click="validate"
              >
                {{ validating ? '校验中…' : '校验并预览' }}
              </button>
              <button
                type="button"
                class="btn-solid py-1.5 text-[0.7rem]"
                :disabled="!preview || validating"
                @click="save"
              >
                保存到公司列表 →
              </button>
            </div>
          </div>
        </section>

        <p
          v-if="notice"
          role="status"
          class="mt-4 border border-moss/40 bg-moss/5 px-3 py-2 text-[0.7rem] text-moss"
        >
          {{ notice }}
        </p>

        <ul
          v-if="errors.length"
          role="alert"
          class="mt-4 space-y-1.5 border border-vermilion/40 bg-vermilion/5 px-4 py-3"
        >
          <li
            v-for="error in errors"
            :key="error"
            class="flex gap-2 text-[0.7rem] leading-5 text-vermilion-deep"
          >
            <span class="font-mono text-[0.56rem]">!</span>
            <span>{{ error }}</span>
          </li>
        </ul>
      </div>
    </div>

    <!-- 智能生成弹卡 -->
    <div
      v-if="showAiCard"
      class="fixed inset-0 z-50 flex items-center justify-center bg-ink/30 p-4"
      @click.self="showAiCard = false"
    >
      <div class="w-full max-w-xl border border-ink bg-paper p-5" role="dialog" aria-modal="true" aria-label="智能生成模板">
        <div class="flex items-baseline justify-between border-b border-line pb-2">
          <h2 class="font-display text-xl">智能生成</h2>
          <button
            type="button"
            class="text-[0.7rem] text-ink-soft transition hover:text-vermilion"
            aria-label="关闭智能生成"
            @click="showAiCard = false"
          >
            ✕
          </button>
        </div>
        <label for="template-prompt" class="mt-4 block text-[0.62rem] text-ink-faint">提示词</label>
        <textarea
          id="template-prompt"
          v-model="prompt"
          rows="3"
          class="field mt-1.5 w-full text-[0.78rem] leading-6"
          placeholder="例如：一家在深圳做消费级机器人的公司，2026 秋招，招嵌入式工程师、机器人算法和硬件测试实习生。"
          @input="generateError = ''"
        />
        <div class="mt-3 flex flex-wrap items-center justify-between gap-2">
          <p class="text-[0.62rem] text-ink-faint">
            描述公司、行业、城市与想招的岗位，生成后填入上方表单，仍可手动修改。
          </p>
          <button
            type="button"
            class="btn-solid py-1.5 text-[0.7rem]"
            :disabled="generating || !prompt.trim()"
            @click="generate"
          >
            {{ generating ? '生成中…' : '生成草稿 →' }}
          </button>
        </div>
        <p v-if="generateError" role="alert" class="mt-2 text-[0.66rem] text-vermilion">
          {{ generateError }}
        </p>
      </div>
    </div>
  </div>
</template>
