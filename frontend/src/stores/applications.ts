import { ref } from 'vue'
import { defineStore } from 'pinia'

import { applicationsApi } from '@/api/endpoints/applications'
import type { Application, Company, Job } from '@/domain/types'

export const useApplicationsStore = defineStore('applications', () => {
  const applications = ref<Application[]>([])

  let initPromise: Promise<void> | null = null

  function init(): Promise<void> {
    if (!initPromise) {
      initPromise = applicationsApi.list().then((value) => {
        applications.value = value
      })
    }
    return initPromise
  }

  function byId(id: string): Application | undefined {
    return applications.value.find((application) => application.id === id)
  }

  function forCompany(companyId: string): Application[] {
    return applications.value.filter((application) => application.companyId === companyId)
  }

  async function applyToJob(company: Company, job: Job): Promise<Application> {
    const existing = applications.value.find(
      (application) => application.companyId === company.id && application.jobId === job.id,
    )
    if (existing) {
      return existing
    }

    const created = await applicationsApi.apply(company, job)
    applications.value = [created, ...applications.value]
    return created
  }

  async function advanceStage(id: string): Promise<Application | undefined> {
    if (!byId(id)) {
      return undefined
    }
    const updated = await applicationsApi.advance(id)
    applications.value = applications.value.map((application) =>
      application.id === id ? updated : application,
    )
    return updated
  }

  return {
    applications,
    init,
    byId,
    forCompany,
    applyToJob,
    advanceStage,
  }
})
