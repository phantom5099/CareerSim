<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

import { companiesApi } from '@/api/endpoints/companies'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'
import {
  ACCENT_OPTIONS,
  COMPANY_SIZE_OPTIONS,
  DEFAULT_ACCENT,
  TEMPLATE_LIMITS,
} from '@/domain/types'

const route = useRoute()
const router = useRouter()
const catalog = useCompanyCatalogStore()

const companyId = computed(() => String(route.params.id ?? ''))

const loading = ref(true)
const notFound = ref(false)
const loadError = ref('')
const saving = ref(false)
const errors = ref<string[]>([])

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

// 载入值不在预设枚举时补进选项，避免直接改库的数据被下拉静默改写。
const sizeOptions = computed(() =>
  COMPANY_SIZE_OPTIONS.includes(form.value.size as (typeof COMPANY_SIZE_OPTIONS)[number])
    ? [...COMPANY_SIZE_OPTIONS]
    : [form.value.size, ...COMPANY_SIZE_OPTIONS],
)

function addHighlight() {
  if (form.value.highlights.length >= TEMPLATE_LIMITS.highlightItems) {
    return
  }
  form.value.highlights.push('')
}

function removeHighlight(index: number) {
  form.value.highlights.splice(index, 1)
}

onMounted(async () => {
  try {
    const company = await companiesApi.get(companyId.value)
    if (!company) {
      notFound.value = true
      return
    }
    if (!company.isOwner) {
      loadError.value = '只有公司创建者可以编辑这份模板。'
      return
    }
    form.value = {
      name: company.name,
      englishName: company.englishName,
      industry: company.industry,
      city: company.city,
      size: company.size || COMPANY_SIZE_OPTIONS[0],
      tagline: company.tagline,
      description: company.description,
      highlights: [...company.highlights],
      logoText: company.logoText,
      accent: company.accent || (DEFAULT_ACCENT as string),
    }
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : '公司加载失败，请稍后重试。'
  } finally {
    loading.value = false
  }
})

async function save() {
  errors.value = []
  if (saving.value) {
    return
  }
  if (!form.value.name.trim()) {
    errors.value = ['公司名称不能为空。']
    return
  }
  saving.value = true
  try {
    await companiesApi.updateCompany(companyId.value, {
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
    })
    await catalog.refresh()
    router.push({ name: 'companyDetail', params: { id: companyId.value } })
  } catch (error) {
    errors.value = [error instanceof Error ? error.message : '保存失败，请稍后重试。']
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div>
    <div class="-ml-1 text-[0.88rem] sm:-ml-4">
      <RouterLink
        :to="{ name: 'companyDetail', params: { id: companyId } }"
        class="inline-flex items-center gap-1.5 font-medium text-ink-soft transition hover:text-vermilion"
      >
        <span class="text-[1.05rem] leading-none" aria-hidden="true">←</span>
        返回公司档案
      </RouterLink>
    </div>

    <h1 class="sr-only">编辑公司</h1>

    <div v-if="loading" class="mt-7 border border-line bg-paper-light px-6 py-12 text-center">
      <p class="text-sm text-ink-soft">正在加载公司资料…</p>
    </div>

    <div
      v-else-if="notFound"
      class="mt-6 border border-line bg-paper-light px-6 py-12 text-center"
    >
      <p class="font-display text-2xl">未找到这家公司</p>
      <p class="mt-2 text-sm text-ink-soft">它可能已被移除。</p>
      <RouterLink :to="{ name: 'companies' }" class="btn-line mt-6">返回在招公司</RouterLink>
    </div>

    <div v-else-if="loadError" class="mt-6 border border-line bg-paper-light px-6 py-12 text-center">
      <p class="font-display text-2xl">无法编辑</p>
      <p class="mt-2 text-sm text-ink-soft">{{ loadError }}</p>
      <RouterLink :to="{ name: 'companies' }" class="btn-line mt-6">返回在招公司</RouterLink>
    </div>

    <div v-else class="mt-7">
      <section>
        <div class="flex flex-wrap items-center justify-between gap-2 border-b border-ink pb-2">
          <h2 class="font-display text-2xl">公司信息</h2>
          <span class="font-mono text-[0.56rem] tracking-[0.14em] text-ink-faint">EDIT</span>
        </div>

        <div class="mt-5 grid gap-3 sm:grid-cols-2">
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">公司名称 *</span>
            <input v-model="form.name" type="text" :maxlength="TEMPLATE_LIMITS.name" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">英文名 / 副标题 *</span>
            <input v-model="form.englishName" type="text" :maxlength="TEMPLATE_LIMITS.englishName" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">行业 *</span>
            <input v-model="form.industry" type="text" :maxlength="TEMPLATE_LIMITS.industry" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">城市 *</span>
            <input v-model="form.city" type="text" :maxlength="TEMPLATE_LIMITS.city" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">团队规模 *</span>
            <select v-model="form.size" class="field mt-1 w-full text-[0.78rem]">
              <option v-for="size in sizeOptions" :key="size" :value="size">
                {{ size }}
              </option>
            </select>
          </label>
          <label class="block">
            <span class="text-[0.62rem] text-ink-faint">标志文字（建议 2 字）*</span>
            <input v-model="form.logoText" type="text" :maxlength="TEMPLATE_LIMITS.logoText" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block sm:col-span-2">
            <span class="text-[0.62rem] text-ink-faint">一句话主张 *</span>
            <input v-model="form.tagline" type="text" :maxlength="TEMPLATE_LIMITS.tagline" class="field mt-1 w-full text-[0.78rem]" />
          </label>
          <label class="block sm:col-span-2">
            <span class="text-[0.62rem] text-ink-faint">公司简介 *</span>
            <textarea v-model="form.description" rows="3" :maxlength="TEMPLATE_LIMITS.description" class="field mt-1 w-full text-[0.78rem] leading-6" />
          </label>
          <div class="block sm:col-span-2">
            <span class="text-[0.62rem] text-ink-faint">亮点标签（最多 {{ TEMPLATE_LIMITS.highlightItems }} 条）</span>
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
            <span class="text-[0.62rem] text-ink-faint">品牌色（点色块选择）</span>
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
                @click="form.accent = option.value"
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

        <div class="mt-6 flex flex-wrap items-center justify-end gap-2 border-t border-line pt-4">
          <RouterLink :to="{ name: 'companyDetail', params: { id: companyId } }" class="btn-line py-1.5 text-[0.7rem]">
            取消
          </RouterLink>
          <button
            type="button"
            class="btn-solid py-1.5 text-[0.7rem]"
            :disabled="saving"
            @click="save"
          >
            {{ saving ? '保存中…' : '保存修改 →' }}
          </button>
        </div>
      </section>

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
</template>
