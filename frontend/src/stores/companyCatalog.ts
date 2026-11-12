import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { companiesApi } from '@/api/endpoints/companies'
import type { Company } from '@/domain/types'

export const useCompanyCatalogStore = defineStore('companyCatalog', () => {
  const companies = ref<Company[]>([])

  const jobCount = computed(() =>
    companies.value.reduce((total, company) => total + company.jobs.length, 0),
  )

  let initPromise: Promise<void> | null = null

  function init(): Promise<void> {
    if (!initPromise) {
      initPromise = companiesApi.list().then((value) => {
        companies.value = value
      })
    }
    return initPromise
  }

  /** 写操作（导入/编辑/删除）后重新拉取，保证列表与详情页看到最新数据。 */
  async function refresh(): Promise<void> {
    const value = await companiesApi.list()
    companies.value = value
    initPromise = Promise.resolve()
  }

  function findCompany(id: string): Company | undefined {
    return companies.value.find((company) => company.id === id)
  }

  return {
    companies,
    jobCount,
    init,
    refresh,
    findCompany,
  }
})
