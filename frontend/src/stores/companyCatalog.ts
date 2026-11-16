import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { companiesApi } from '@/api/endpoints/companies'
import type { Company } from '@/domain/types'

export const useCompanyCatalogStore = defineStore('companyCatalog', () => {
  const companies = ref<Company[]>([])
  // 首次加载失败时置位；页面据此显示错误态而不是"没有匹配"。
  const loadError = ref<string | null>(null)

  const jobCount = computed(() =>
    companies.value.reduce((total, company) => total + company.jobs.length, 0),
  )

  let initPromise: Promise<void> | null = null

  function init(): Promise<void> {
    if (initPromise) {
      return initPromise
    }
    initPromise = companiesApi
      .list()
      .then((value) => {
        companies.value = value
        loadError.value = null
      })
      .catch((error: unknown) => {
        // 失败不缓存：置空后下次进入页面可重试，避免一次性失败被永久记住。
        initPromise = null
        loadError.value =
          error instanceof Error ? error.message : '目录加载失败，请稍后重试'
        throw error
      })
    return initPromise
  }

  /** 写操作（导入/编辑/删除）后重新拉取，保证列表与详情页看到最新数据。 */
  async function refresh(): Promise<void> {
    const value = await companiesApi.list()
    companies.value = value
    loadError.value = null
    initPromise = Promise.resolve()
  }

  function findCompany(id: string): Company | undefined {
    return companies.value.find((company) => company.id === id)
  }

  return {
    companies,
    jobCount,
    loadError,
    init,
    refresh,
    findCompany,
  }
})
