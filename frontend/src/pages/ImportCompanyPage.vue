<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import { companiesApi } from '@/api/endpoints/companies'
import CompanyMark from '@/components/CompanyMark.vue'
import { PROJECT_STATUS_LABELS, formatDate } from '@/domain/types'
import type { Company } from '@/domain/types'
const router = useRouter()

const jsonText = ref('')
const sampleJson = ref('')
const loadingSample = ref(true)
const preview = ref<Company | null>(null)
const errors = ref<string[]>([])
const notice = ref('')
const prompt = ref('')
const generating = ref(false)
const validating = ref(false)
const generateError = ref('')
const fileInput = ref<HTMLInputElement | null>(null)

const fieldRows = [
  { field: 'name', desc: '公司名称', required: true },
  { field: 'englishName', desc: '英文名 / 副标题', required: true },
  { field: 'industry', desc: '公司行业，如「人工智能」', required: true },
  { field: 'city', desc: '所在城市', required: true },
  { field: 'size', desc: '团队规模', required: true },
  { field: 'tagline', desc: '一句话主张', required: true },
  { field: 'description', desc: '公司简介', required: true },
  { field: 'highlights', desc: '亮点，字符串数组', required: true },
  { field: 'logoText', desc: '标志文字，建议 2 字', required: true },
  { field: 'accent', desc: '品牌色 #RRGGBB', required: false },
  { field: 'projects[]', desc: '招聘项目：名称、季节、状态、截止日、描述', required: true },
  { field: 'jobs[]', desc: '岗位：标题、城市、类型、薪资、标签、描述', required: true },
  { field: 'jobs[].project', desc: '所属项目名称，需与 projects[].name 一致', required: false },
  { field: 'jobs[].industry', desc: '岗位方向，缺省沿用公司行业', required: false },
]

function resetResult() {
  preview.value = null
  errors.value = []
  notice.value = ''
}

async function validate() {
  resetResult()
  if (validating.value) {
    return
  }
  let parsed: unknown
  try {
    parsed = JSON.parse(jsonText.value)
  } catch {
    errors.value = ['JSON 解析失败，请检查逗号、引号与括号是否完整。']
    return
  }
  validating.value = true
  try {
    const result = await companiesApi.validateImport(parsed)
    if (result.ok) {
      preview.value = result.company
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

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) {
    return
  }
  jsonText.value = await file.text()
  await validate()
  input.value = ''
}

async function loadSample() {
  if (!sampleJson.value) {
    return
  }
  jsonText.value = sampleJson.value
  await validate()
}

async function copySample() {
  if (!sampleJson.value) {
    return
  }
  try {
    await navigator.clipboard.writeText(sampleJson.value)
    notice.value = '示例模板已复制到剪贴板。'
  } catch {
    notice.value = '复制失败，可以手动选择示例文本。'
  }
}

function downloadSample() {
  if (!sampleJson.value) {
    return
  }
  const blob = new Blob([sampleJson.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = 'careersim-company-template.json'
  anchor.click()
  URL.revokeObjectURL(url)
}

async function generate() {
  if (!prompt.value.trim() || generating.value) {
    return
  }
  generating.value = true
  generateError.value = ''
  try {
    const draft = await companiesApi.generateTemplate(prompt.value)
    jsonText.value = JSON.stringify(draft, null, 2)
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

onMounted(async () => {
  try {
    sampleJson.value = await companiesApi.templateSample()
    jsonText.value = sampleJson.value
  } catch {
    errors.value = ['示例模板加载失败，请刷新页面重试。']
  } finally {
    loadingSample.value = false
  }
})

async function save() {
  if (!preview.value) {
    await validate()
    return
  }
  let parsed: unknown
  try {
    parsed = JSON.parse(jsonText.value)
  } catch {
    errors.value = ['JSON 解析失败，请检查逗号、引号与括号是否完整。']
    return
  }
  const saved = await companiesApi.importTemplate(parsed)
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

    <header class="mt-4 border-b-[3px] border-double border-ink pb-5">
      <h1 class="sr-only">录入公司</h1>
      <p class="max-w-2xl text-[0.8rem] leading-6 text-ink-soft">
        用一句提示词或一份 JSON 模板描述公司、招聘项目与岗位。保存后会出现在公司列表，并可直接投递。
      </p>
    </header>

    <div class="mt-7 grid gap-8 lg:grid-cols-[minmax(0,1fr)_19rem]">
      <div>
        <section class="mb-9">
          <div class="flex items-baseline justify-between border-b border-ink pb-2">
            <h2 class="font-display text-2xl">智能生成</h2>
            <span class="font-mono text-[0.56rem] tracking-[0.14em] text-ink-faint">
              AI DRAFT
            </span>
          </div>
          <div class="mt-4 border border-line bg-paper-light p-4">
            <label for="template-prompt" class="text-[0.62rem] text-ink-faint">提示词</label>
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
                描述公司、行业、城市与想招的岗位，生成后仍可手动修改。
              </p>
              <button
                type="button"
                class="btn-solid py-1.5 text-[0.7rem]"
                :disabled="generating || !prompt.trim()"
                @click="generate"
              >
                {{ generating ? '生成中…' : '智能生成 →' }}
              </button>
            </div>
            <p v-if="generateError" role="alert" class="mt-2 text-[0.66rem] text-vermilion">
              {{ generateError }}
            </p>
          </div>
        </section>

        <section>
          <div class="flex flex-wrap items-center justify-between gap-2 border-b border-ink pb-2">
            <h2 class="font-display text-2xl">模板内容</h2>
            <div class="flex flex-wrap gap-1.5">
              <button
                type="button"
                class="btn-line py-1.5 text-[0.66rem]"
                :disabled="!sampleJson"
                @click="loadSample"
              >
                载入示例
              </button>
              <button
                type="button"
                class="btn-line py-1.5 text-[0.66rem]"
                @click="fileInput?.click()"
              >
                选择 JSON 文件
              </button>
              <input
                ref="fileInput"
                type="file"
                accept=".json,application/json"
                class="hidden"
                @change="onFile"
              />
            </div>
          </div>

          <textarea
            v-model="jsonText"
            class="field mt-4 w-full font-mono text-[0.7rem] leading-5"
            rows="24"
            spellcheck="false"
            aria-label="公司模板 JSON"
            :placeholder="loadingSample ? '正在加载示例模板…' : '粘贴公司模板 JSON'"
            @input="resetResult"
          />
          <div class="mt-2 flex flex-wrap items-center justify-between gap-2">
            <p class="font-mono text-[0.56rem] tracking-[0.1em] text-ink-faint">
              COMPANY.JSON · UTF-8
            </p>
            <button
              type="button"
              class="btn-solid py-1.5 text-[0.7rem]"
              :disabled="validating"
              @click="validate"
            >
              {{ validating ? '校验中…' : '校验并预览 →' }}
            </button>
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

        <section class="mt-8">
          <div class="flex items-baseline justify-between border-b border-ink pb-2">
            <h2 class="font-display text-2xl">模板预览</h2>
            <span class="font-mono text-[0.56rem] tracking-[0.14em] text-ink-faint">
              {{ preview ? 'READY' : 'WAITING' }}
            </span>
          </div>

          <article v-if="preview" class="mt-4 border border-line bg-paper-light p-5">
            <div class="flex items-start gap-4">
              <CompanyMark :text="preview.logoText" :accent="preview.accent" size="lg" />
              <div class="min-w-0">
                <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                  <h3 class="font-display text-2xl leading-tight">{{ preview.name }}</h3>
                  <span class="border border-vermilion/40 px-1.5 py-0.5 text-[0.6rem] text-vermilion">
                    我的模板
                  </span>
                </div>
                <p class="mt-1 font-mono text-[0.56rem] tracking-[0.18em] text-ink-faint">
                  {{ preview.englishName }}
                </p>
                <p class="mt-1 text-[0.68rem] text-ink-soft">
                  {{ preview.industry }} · {{ preview.city }} · {{ preview.size }}
                </p>
              </div>
            </div>

            <p class="mt-4 font-display text-lg">「{{ preview.tagline }}」</p>
            <p class="mt-2 text-[0.76rem] leading-6 text-ink-soft">{{ preview.description }}</p>

            <div class="mt-4 grid gap-4 border-t border-line pt-4 sm:grid-cols-2">
              <div>
                <p class="text-[0.64rem] text-ink-faint">
                  招聘项目 · {{ preview.projects.length }}
                </p>
                <ul class="mt-2 space-y-1.5">
                  <li
                    v-for="project in preview.projects"
                    :key="project.id"
                    class="flex items-baseline justify-between gap-2 text-[0.68rem]"
                  >
                    <span class="truncate">{{ project.name }}</span>
                    <span class="shrink-0 text-[0.62rem] text-ink-faint">
                      {{ PROJECT_STATUS_LABELS[project.status] }} · {{ formatDate(project.deadline) }}
                    </span>
                  </li>
                </ul>
              </div>
              <div>
                <p class="text-[0.64rem] text-ink-faint">
                  开放岗位 · {{ preview.jobs.length }}
                </p>
                <ul class="mt-2 space-y-1.5">
                  <li
                    v-for="job in preview.jobs"
                    :key="job.id"
                    class="flex items-baseline justify-between gap-2 text-[0.68rem]"
                  >
                    <span class="truncate">{{ job.title }}</span>
                    <span class="shrink-0 text-[0.62rem] text-ink-faint">
                      {{ job.type }} · {{ job.salary }}
                    </span>
                  </li>
                </ul>
              </div>
            </div>

            <div class="mt-5 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-4">
              <p class="text-[0.6rem] leading-5 text-ink-faint">
                保存后数据仅写入当前浏览器，可随时重新录入。
              </p>
              <button type="button" class="btn-solid" @click="save">保存到公司列表 →</button>
            </div>
          </article>

          <div
            v-else
            class="mt-4 border border-dashed border-line-strong px-6 py-12 text-center"
          >
            <p class="font-display text-lg">校验通过后在这里预览</p>
            <p class="mt-2 text-[0.68rem] leading-5 text-ink-faint">
              至少需要 1 个招聘项目与 1 个岗位。
            </p>
          </div>
        </section>
      </div>

      <aside class="space-y-6">
        <section class="border border-line bg-paper-light p-4">
          <p class="text-[0.64rem] text-ink-faint">填写步骤</p>
          <ol class="mt-3 space-y-2.5">
            <li class="flex gap-2 text-[0.68rem] leading-5 text-ink-soft">
              <span class="font-mono text-[0.54rem] text-vermilion">01</span>
              <span>输入提示词智能生成，或复制示例、选择本地 JSON 文件。</span>
            </li>
            <li class="flex gap-2 text-[0.68rem] leading-5 text-ink-soft">
              <span class="font-mono text-[0.54rem] text-vermilion">02</span>
              <span>替换公司信息、招聘项目与岗位。</span>
            </li>
            <li class="flex gap-2 text-[0.68rem] leading-5 text-ink-soft">
              <span class="font-mono text-[0.54rem] text-vermilion">03</span>
              <span>校验并预览，确认后保存到公司列表。</span>
            </li>
          </ol>
          <div class="mt-4 flex flex-col gap-1.5">
            <button
              type="button"
              class="btn-line py-1.5 text-[0.66rem]"
              :disabled="!sampleJson"
              @click="copySample"
            >
              复制示例模板
            </button>
            <button
              type="button"
              class="btn-line py-1.5 text-[0.66rem]"
              :disabled="!sampleJson"
              @click="downloadSample"
            >
              下载示例 JSON
            </button>
          </div>
        </section>

        <section class="border border-line p-4">
          <p class="text-[0.64rem] text-ink-faint">字段说明</p>
          <dl class="mt-3 space-y-2">
            <div
              v-for="row in fieldRows"
              :key="row.field"
              class="flex items-start justify-between gap-2 border-b border-line/60 pb-1.5 text-[0.64rem] last:border-b-0 last:pb-0"
            >
              <dt class="shrink-0 font-mono text-[0.58rem] text-ink">
                {{ row.field }}
                <span v-if="row.required" class="text-vermilion">*</span>
              </dt>
              <dd class="text-right text-[0.6rem] leading-4 text-ink-faint">{{ row.desc }}</dd>
            </div>
          </dl>
        </section>

        <p class="text-[0.6rem] leading-5 text-ink-faint">
          模板中的公司与岗位会被视为虚构内容。请勿填写真实企业的敏感招聘信息。
        </p>
      </aside>
    </div>
  </div>
</template>
