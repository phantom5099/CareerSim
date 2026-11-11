import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { resumeApi } from '@/api/endpoints/resume'
import type { ResumeCreateInput, ResumeDocument, ResumeUpdateInput } from '@/api/types'

export const useResumesStore = defineStore('resumes', () => {
  const resumes = ref<ResumeDocument[]>([])

  let initPromise: Promise<void> | null = null

  function init(): Promise<void> {
    if (!initPromise) {
      initPromise = resumeApi.list().then((value) => {
        resumes.value = value
      })
    }
    return initPromise
  }

  const defaultResume = computed(
    () => resumes.value.find((resume) => resume.isDefault) ?? resumes.value[0],
  )

  function byId(id: string): ResumeDocument | undefined {
    return resumes.value.find((resume) => resume.id === id)
  }

  async function createOnline(input: ResumeCreateInput): Promise<ResumeDocument> {
    const created = await resumeApi.create(input)
    resumes.value = [created, ...resumes.value]
    return created
  }

  async function uploadAttachment(title: string, file: File): Promise<ResumeDocument> {
    const created = await resumeApi.uploadAttachment(title, file)
    resumes.value = [created, ...resumes.value]
    return created
  }

  async function update(id: string, input: ResumeUpdateInput): Promise<ResumeDocument> {
    const updated = await resumeApi.update(id, input)
    resumes.value = resumes.value.map((resume) => (resume.id === id ? updated : resume))
    return updated
  }

  async function remove(id: string): Promise<void> {
    await resumeApi.remove(id)
    // 删除默认简历时后端会提升最新一份，整表重拉保证默认标记正确。
    resumes.value = await resumeApi.list()
  }

  async function setDefault(id: string): Promise<ResumeDocument> {
    const updated = await resumeApi.setDefault(id)
    resumes.value = resumes.value.map((resume) => ({
      ...resume,
      isDefault: resume.id === id,
    }))
    return updated
  }

  return {
    resumes,
    defaultResume,
    init,
    byId,
    createOnline,
    uploadAttachment,
    update,
    remove,
    setDefault,
  }
})
