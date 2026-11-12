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

  function findCompany(id: string): Company | undefined {
    return companies.value.find((company) => company.id === id)
  }

  return {
    companies,
    jobCount,
    init,
    findCompany,
  }
})
