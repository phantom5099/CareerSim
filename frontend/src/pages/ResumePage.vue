<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import { RESUME_LIMIT } from '@/api/types'
import type {
  ResumeCertificate,
  ResumeDocument,
  ResumeEducation,
  ResumeExperience,
  ResumeHonor,
  ResumePayload,
  ResumePractice,
  ResumeProject,
  ResumeSkill,
} from '@/api/types'
import { formatDate } from '@/domain/types'
import { useResumesStore } from '@/stores/resumes'

const resumes = useResumesStore()

const fileInput = ref<HTMLInputElement | null>(null)
const notice = ref<{ tone: 'ok' | 'warn'; text: string } | null>(null)
const attachNotice = ref<{ tone: 'ok' | 'warn' | 'busy'; text: string } | null>(null)
/** 附件正在上传+解析；期间禁用上传按钮并显示进度提示。 */
const parsing = ref(false)
const pendingDeleteId = ref('')
const aiDemoOpen = ref(false)

const editingId = ref<string | null>(null)
const creating = ref(false)
const editorOpen = computed(() => creating.value || editingId.value !== null)
const editorTitle = computed(() => (creating.value ? '新建在线简历' : '编辑简历'))
const editingResume = computed(() =>
  editingId.value ? (resumes.byId(editingId.value) ?? null) : null,
)
const title = ref('')
const draft = ref<ResumePayload>(blankPayload())
const saving = ref(false)
const editorError = ref('')

const atLimit = computed(() => resumes.resumes.length >= RESUME_LIMIT)

onMounted(() => {
  void resumes.init()
})

function blankEducation(): ResumeEducation {
  return { school: '', degree: '本科', major: '', startDate: '', endDate: '', ranking: '' }
}

function blankExperience(): ResumeExperience {
  return { company: '', role: '', type: '实习', startDate: '', endDate: '', description: '' }
}

function blankProject(): ResumeProject {
  return { name: '', role: '', startDate: '', endDate: '', description: '' }
}

function blankPractice(): ResumePractice {
  return { name: '', role: '', startDate: '', endDate: '', description: '' }
}

function blankSkill(): ResumeSkill {
  return { name: '', level: '掌握' }
}

function blankCertificate(): ResumeCertificate {
  return { name: '', date: '' }
}

function blankHonor(): ResumeHonor {
  return { name: '', level: '校级', date: '' }
}

function blankPayload(): ResumePayload {
  return {
    name: '',
    phone: '',
    email: '',
    city: '',
    targetRole: '',
    targetCity: '',
    jobType: '',
    availableDate: '',
    educations: [],
    experiences: [],
    projects: [],
    practices: [],
    skills: [],
    certificates: [],
    honors: [],
    summary: '',
  }
}

function lines(value: string): string[] {
  return value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
}

function resumeMeta(resume: ResumeDocument): string {
  const education = resume.payload.educations[0]
  const contentMeta = [
    resume.payload.targetRole,
    resume.payload.targetCity,
    education ? `${education.school} · ${education.degree}` : '',
  ]
    .filter(Boolean)
    .join(' · ')
  if (!resume.attachment) {
    return contentMeta
  }
  const status = parseStatusText(resume.parseStatus, resume.parseErrorCode)
  return [
    contentMeta,
    `附件 ${resume.attachment.fileName}`,
    status ? `· ${status}` : '',
  ]
    .filter(Boolean)
    .join(' ')
}

/** 附件解析状态文案；not_applicable 表示无附件，failed 时按错误码细分提示。 */
function parseStatusText(
  status: 'not_applicable' | 'pending' | 'ready' | 'failed',
  errorCode?: string,
): string {
  switch (status) {
    case 'not_applicable':
      return ''
    case 'pending':
      return '解析中'
    case 'ready':
      return '解析完成'
    case 'failed':
      switch (errorCode) {
        case 'empty_text':
          return '解析失败：未找到文本（可能是扫描件），可编辑手填'
        case 'unsupported_format':
          return '解析失败：文件格式不受支持'
        case 'timeout':
          return '解析失败：解析超时，可重试上传'
        default:
          return '解析失败：文件可能已损坏，可编辑手填'
      }
    default:
      return ''
  }
}

function dateRange(start: string, end: string): string {
  const from = start.trim()
  const to = end.trim()
  if (!from && !to) {
    return ''
  }
  return `${from || '—'} – ${to || '至今'}`
}

function togglePresent(item: { endDate: string }, event: Event) {
  item.endDate = (event.target as HTMLInputElement).checked ? '至今' : ''
}

function warn(text: string) {
  notice.value = { tone: 'warn', text }
}

function startOnline() {
  if (atLimit.value) {
    warn(`最多保存 ${RESUME_LIMIT} 份简历，请先删除一份。`)
    return
  }
  creating.value = true
  editingId.value = null
  title.value = ''
  draft.value = blankPayload()
  pendingDeleteId.value = ''
  editorError.value = ''
  attachNotice.value = null
  notice.value = null
}

function editResume(resume: ResumeDocument) {
  creating.value = false
  editingId.value = resume.id
  title.value = resume.title
  // Pinia 的 payload 是响应式 Proxy，structuredClone 会抛 DataCloneError。
  draft.value = JSON.parse(JSON.stringify(resume.payload)) as ResumePayload
  pendingDeleteId.value = ''
  editorError.value = ''
  attachNotice.value = null
  notice.value = null
}

function closeEditor() {
  if (saving.value) {
    return
  }
  creating.value = false
  editingId.value = null
  editorError.value = ''
  aiDemoOpen.value = false
}

function addEducation() {
  draft.value.educations.push(blankEducation())
}

function removeEducation(index: number) {
  draft.value.educations.splice(index, 1)
}

function addExperience() {
  draft.value.experiences.push(blankExperience())
}

function removeExperience(index: number) {
  draft.value.experiences.splice(index, 1)
}

function addProject() {
  draft.value.projects.push(blankProject())
}

function removeProject(index: number) {
  draft.value.projects.splice(index, 1)
}

function addPractice() {
  draft.value.practices.push(blankPractice())
}

function removePractice(index: number) {
  draft.value.practices.splice(index, 1)
}

function addSkill() {
  draft.value.skills.push(blankSkill())
}

function removeSkill(index: number) {
  draft.value.skills.splice(index, 1)
}

function addCertificate() {
  draft.value.certificates.push(blankCertificate())
}

function removeCertificate(index: number) {
  draft.value.certificates.splice(index, 1)
}

function addHonor() {
  draft.value.honors.push(blankHonor())
}

function removeHonor(index: number) {
  draft.value.honors.splice(index, 1)
}

function cleanedPayload(): ResumePayload {
  const text = (value: string) => value.trim()
  return {
    name: text(draft.value.name),
    phone: text(draft.value.phone),
    email: text(draft.value.email),
    city: text(draft.value.city),
    targetRole: text(draft.value.targetRole),
    targetCity: text(draft.value.targetCity),
    jobType: text(draft.value.jobType),
    availableDate: text(draft.value.availableDate),
    educations: draft.value.educations
      .map((item) => ({
        school: text(item.school),
        degree: text(item.degree),
        major: text(item.major),
        startDate: text(item.startDate),
        endDate: text(item.endDate),
        ranking: text(item.ranking),
      }))
      .filter((item) => item.school || item.major || item.degree),
    experiences: draft.value.experiences
      .map((item) => ({
        company: text(item.company),
        role: text(item.role),
        type: text(item.type),
        startDate: text(item.startDate),
        endDate: text(item.endDate),
        description: item.description.trim(),
      }))
      .filter((item) => item.company || item.role || item.description),
    projects: draft.value.projects
      .map((item) => ({
        name: text(item.name),
        role: text(item.role),
        startDate: text(item.startDate),
        endDate: text(item.endDate),
        description: item.description.trim(),
      }))
      .filter((item) => item.name || item.role || item.description),
    practices: draft.value.practices
      .map((item) => ({
        name: text(item.name),
        role: text(item.role),
        startDate: text(item.startDate),
        endDate: text(item.endDate),
        description: item.description.trim(),
      }))
      .filter((item) => item.name || item.role || item.description),
    skills: draft.value.skills
      .map((item) => ({ name: text(item.name), level: text(item.level) }))
      .filter((item) => item.name),
    certificates: draft.value.certificates
      .map((item) => ({ name: text(item.name), date: text(item.date) }))
      .filter((item) => item.name),
    honors: draft.value.honors
      .map((item) => ({ name: text(item.name), level: text(item.level), date: text(item.date) }))
      .filter((item) => item.name),
    summary: draft.value.summary.trim(),
  }
}

async function saveEditor() {
  if (saving.value) {
    return
  }
  const payload = cleanedPayload()
  if (!title.value.trim()) {
    editorError.value = '请先填写简历名称。'
    return
  }
  if (!payload.name) {
    editorError.value = '姓名不能为空。'
    return
  }
  if (!payload.targetRole) {
    editorError.value = '求职方向不能为空。'
    return
  }
  if (!payload.phone && !payload.email) {
    editorError.value = '请至少填写手机号或邮箱，招聘方需要能联系到你。'
    return
  }
  if (!payload.educations.length) {
    editorError.value = '请至少填写一段教育经历。'
    return
  }
  const dateSections = [
    { label: '教育经历', items: payload.educations },
    { label: '实习与工作经历', items: payload.experiences },
    { label: '项目经历', items: payload.projects },
    { label: '在校实践', items: payload.practices },
  ]
  for (const section of dateSections) {
    for (const [index, item] of section.items.entries()) {
      const name = `${section.label}第 ${index + 1} 条`
      if (!item.startDate) {
        editorError.value = `${name}缺少开始时间。`
        return
      }
      if (!item.endDate) {
        editorError.value = `${name}缺少结束时间，进行中的请勾选“至今”。`
        return
      }
      if (item.endDate !== '至今' && item.endDate < item.startDate) {
        editorError.value = `${name}的结束时间早于开始时间。`
        return
      }
    }
  }
  saving.value = true
  editorError.value = ''
  const input = { title: title.value.trim(), payload }
  try {
    if (creating.value) {
      await resumes.createOnline(input)
      notice.value = { tone: 'ok', text: '在线简历已创建，投递时可直接选用。' }
    } else if (editingId.value) {
      await resumes.update(editingId.value, input)
      notice.value = { tone: 'ok', text: '简历修改已保存。' }
    }
    creating.value = false
    editingId.value = null
  } catch (error) {
    editorError.value = error instanceof Error ? error.message : '保存失败，请稍后重试。'
  } finally {
    saving.value = false
  }
}

/** 标量字段中文名，用于回填摘要。 */
const FIELD_NAMES: Array<[keyof ResumePayload, string]> = [
  ['name', '姓名'],
  ['phone', '手机号'],
  ['email', '邮箱'],
  ['city', '现居城市'],
  ['targetRole', '目标岗位'],
  ['targetCity', '目标城市'],
  ['jobType', '求职类型'],
  ['availableDate', '到岗时间'],
  ['summary', '自我介绍'],
]

/** 列表字段中文名，用于回填摘要。 */
const LIST_NAMES: Array<[keyof ResumePayload, string]> = [
  ['educations', '教育'],
  ['experiences', '经历'],
  ['projects', '项目'],
  ['practices', '实践'],
  ['skills', '技能'],
  ['certificates', '证书'],
  ['honors', '荣誉'],
]

/** 对比解析前后的内容，生成"回填了什么/保留了什么"的人类可读摘要。 */
function describeBackfill(before: ResumePayload, after: ResumePayload): string {
  const filled = FIELD_NAMES.filter(
    ([key]) => (before[key] as string).trim() === '' && (after[key] as string).trim() !== '',
  ).map(([, label]) => label)
  const appended = LIST_NAMES.filter(
    ([key]) => (after[key] as unknown[]).length > (before[key] as unknown[]).length,
  ).map(([key, label]) => {
    const added = (after[key] as unknown[]).length - (before[key] as unknown[]).length
    return `${label} ${added} 条`
  })
  const kept = FIELD_NAMES.filter(([key]) => (before[key] as string).trim() !== '').length

  const parts: string[] = []
  if (filled.length > 0) {
    parts.push(`识别出并回填：${filled.join('、')}`)
  }
  if (appended.length > 0) {
    parts.push(`新增条目：${appended.join('、')}`)
  }
  if (parts.length === 0) {
    parts.push('本次未识别出可回填的新字段（标准单栏排版的简历识别效果最好）')
  }
  if (kept > 0) {
    parts.push(`已有内容保持不变`)
  }
  return parts.join('；') + '。'
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) {
    return
  }
  // 上传与解析在后端同步完成，响应返回即携带回填结果；
  // 期间显示解析中提示，避免大文件时界面无反馈。
  parsing.value = true
  attachNotice.value = { tone: 'busy', text: `正在上传并解析「${file.name}」…` }
  // 编辑已有简历：附件挂到该简历并回填；否则上传即创建新简历。
  if (editingId.value) {
    try {
      const before = draft.value
      const updated = await resumes.attachToResume(editingId.value, file)
      title.value = updated.title
      draft.value = JSON.parse(JSON.stringify(updated.payload)) as ResumePayload
      if (updated.parseStatus === 'failed') {
        const text = `附件「${file.name}」已挂到当前简历，但解析未成功：${parseStatusText(updated.parseStatus, updated.parseErrorCode)}`
        warn(text)
        attachNotice.value = { tone: 'warn', text }
      } else {
        const text = `附件「${file.name}」解析完成：${describeBackfill(before, updated.payload)}`
        notice.value = { tone: 'ok', text: '附件解析完成，表单已更新。' }
        attachNotice.value = { tone: 'ok', text }
      }
    } catch (error) {
      const text = error instanceof Error ? error.message : '上传失败，请稍后重试。'
      editorError.value = text
      attachNotice.value = { tone: 'warn', text }
    } finally {
      parsing.value = false
    }
    return
  }

  if (atLimit.value) {
    const text = `最多保存 ${RESUME_LIMIT} 份简历，请先删除一份。`
    warn(text)
    attachNotice.value = { tone: 'warn', text }
    parsing.value = false
    return
  }
  const uploadTitle = file.name.replace(/\.[^.]+$/, '') || file.name
  try {
    const created = await resumes.uploadAttachment(uploadTitle, file)
    // 直接切换为编辑这份新简历，表单立即显示回填结果，
    // 避免“弹窗里上传却建了另一份简历、当前表单纹丝不动”的错觉。
    creating.value = false
    editingId.value = created.id
    title.value = created.title
    draft.value = JSON.parse(JSON.stringify(created.payload)) as ResumePayload
    if (created.parseStatus === 'failed') {
      const text = `附件「${file.name}」已保存为新简历，但解析未成功：${parseStatusText(created.parseStatus, created.parseErrorCode)}`
      warn(text)
      attachNotice.value = { tone: 'warn', text }
      return
    }
    attachNotice.value = {
      tone: 'ok',
      text: `附件「${file.name}」解析完成：${describeBackfill(blankPayload(), created.payload)}`,
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : '添加失败，请稍后重试。'
    warn(text)
    attachNotice.value = { tone: 'warn', text }
  } finally {
    parsing.value = false
  }
}

async function removeAttachment(resumeId: string) {
  try {
    await resumes.removeAttachment(resumeId)
    if (editingId.value === resumeId) {
      draft.value = JSON.parse(
        JSON.stringify(resumes.byId(resumeId)?.payload ?? draft.value),
      ) as ResumePayload
    }
    notice.value = { tone: 'ok', text: '附件已移除，在线内容保持不变。' }
  } catch (error) {
    warn(error instanceof Error ? error.message : '移除失败，请稍后重试。')
  }
}

async function makeDefault(id: string) {
  if (resumes.byId(id)?.isDefault) {
    return
  }
  try {
    await resumes.setDefault(id)
  } catch (error) {
    warn(error instanceof Error ? error.message : '设置失败，请稍后重试。')
  }
}

async function requestDelete(id: string) {
  if (pendingDeleteId.value !== id) {
    pendingDeleteId.value = id
    return
  }
  pendingDeleteId.value = ''
  try {
    await resumes.remove(id)
    if (editingId.value === id) {
      closeEditor()
    }
    notice.value = { tone: 'ok', text: '简历已删除。' }
  } catch (error) {
    warn(error instanceof Error ? error.message : '删除失败，请稍后重试。')
  }
}

const previewEducations = computed(() =>
  draft.value.educations.filter((item) => item.school || item.major || item.degree),
)
const previewExperiences = computed(() =>
  draft.value.experiences.filter((item) => item.company || item.role || item.description),
)
const previewProjects = computed(() =>
  draft.value.projects.filter((item) => item.name || item.role || item.description),
)
const previewPractices = computed(() =>
  draft.value.practices.filter((item) => item.name || item.role || item.description),
)
const previewSkills = computed(() => draft.value.skills.filter((item) => item.name.trim()))
const previewCertificates = computed(() => draft.value.certificates.filter((item) => item.name.trim()))
const previewHonors = computed(() => draft.value.honors.filter((item) => item.name.trim()))

</script>

<template>
  <div>
    <h1 class="sr-only">简历管理</h1>

    <div class="mt-1 flex flex-wrap items-end justify-between gap-4 border-b border-ink pb-3">
      <div>
        <h2 class="font-display text-3xl leading-none">简历管理</h2>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="flex h-9 w-9 items-center justify-center rounded-full border border-ink text-xl leading-none transition hover:border-vermilion hover:text-vermilion disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="atLimit"
          aria-label="新建在线简历"
          title="新建在线简历"
          @click="startOnline"
        >
          +
        </button>
        <input
          ref="fileInput"
          type="file"
          accept=".pdf,.doc,.docx"
          class="hidden"
          @change="onFile"
        />
        <span class="font-mono text-sm text-ink-faint">
          {{ resumes.resumes.length }} / {{ RESUME_LIMIT }}
        </span>
      </div>
    </div>

    <p
      v-if="notice && !editorOpen"
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

    <ul class="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      <li
        v-for="resume in resumes.resumes"
        :key="resume.id"
        class="flex flex-col border border-line bg-paper-light p-4"
      >
        <div class="flex items-start gap-2.5">
          <input
            type="radio"
            name="default-resume"
            class="mt-1.5 h-3.5 w-3.5 shrink-0 accent-vermilion"
            :checked="resume.isDefault"
            :aria-label="`将「${resume.title}」设为默认简历`"
            :title="resume.isDefault ? '当前默认简历' : '设为默认简历'"
            @change="makeDefault(resume.id)"
          />
          <div class="min-w-0 flex-1">
            <h3 class="min-w-0 font-display text-lg leading-tight">{{ resume.title }}</h3>
            <p class="mt-1.5 text-[0.68rem] leading-5 text-ink-soft">{{ resumeMeta(resume) }}</p>
            <p class="mt-1 text-[0.6rem] text-ink-faint">更新于 {{ formatDate(resume.updatedAt) }}</p>
          </div>
        </div>
        <div
          class="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-line/70 pt-3 text-[0.64rem]"
        >
          <button
            type="button"
            class="text-ink-soft transition hover:text-vermilion"
            @click="editResume(resume)"
          >
            编辑
          </button>
          <button
            type="button"
            class="ml-auto text-ink-faint transition hover:text-vermilion"
            @click="requestDelete(resume.id)"
          >
            {{ pendingDeleteId === resume.id ? '确认删除' : '删除' }}
          </button>
          <button
            v-if="pendingDeleteId === resume.id"
            type="button"
            class="text-ink-faint transition hover:text-ink"
            @click="pendingDeleteId = ''"
          >
            取消
          </button>
        </div>
      </li>
    </ul>

    <div
      v-if="editorOpen"
      class="fixed inset-0 z-50 flex items-end justify-center bg-ink/40 sm:items-center sm:p-6"
      role="presentation"
      @click.self="closeEditor"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="resume-editor-title"
        class="flex max-h-[94dvh] w-full max-w-6xl flex-col border border-ink bg-paper-light shadow-xl"
      >
        <header class="flex items-start justify-between gap-4 border-b border-ink px-5 py-3">
          <div class="min-w-0">
            <h2 id="resume-editor-title" class="font-display text-xl leading-tight">
              {{ editorTitle }}
            </h2>
            <p class="mt-1 text-[0.62rem] leading-4 text-ink-faint">
              按招聘官网申请表的顺序填写：基本信息、求职意向、教育、经历、项目与技能。
            </p>
          </div>
          <button
            type="button"
            class="text-lg leading-none text-ink-faint transition hover:text-vermilion"
            aria-label="关闭简历编辑"
            @click="closeEditor"
          >
            ×
          </button>
        </header>

        <form class="flex min-h-0 flex-1 flex-col" @submit.prevent="saveEditor">
          <div class="min-h-0 flex-1 overflow-y-auto px-5 py-5">
            <div class="grid gap-8 lg:grid-cols-[minmax(0,1fr)_19rem] lg:items-start">
              <div class="space-y-8">
                <section>
                  <div class="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-ink pb-2">
                    <h3 class="shrink-0 font-display text-lg">简历名称</h3>
                    <input
                      v-model="title"
                      type="text"
                      class="field min-w-0 flex-1 text-sm"
                      placeholder="如：前端工程师"
                      aria-label="简历名称"
                      autocomplete="off"
                    />
                    <span class="shrink-0 font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      TITLE
                    </span>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">附件解析回填</h3>
                    <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      ATTACHMENT
                    </span>
                  </div>
                  <div class="mt-3 border border-dashed border-line-strong px-4 py-4">
                    <p class="text-[0.68rem] leading-5 text-ink-soft">
                      支持 PDF / Word。上传后自动解析，识别出的字段回填到下方表单；
                      <template v-if="editingId">已有内容不会被覆盖，重复上传会替换附件。</template>
                      <template v-else>上传即创建一份新简历。</template>
                    </p>
                    <div
                      v-if="editingId && editingResume?.attachment"
                      class="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[0.64rem]"
                    >
                      <span class="text-ink-soft">
                        当前附件：{{ editingResume.attachment.fileName }}
                      </span>
                      <span
                        v-if="parseStatusText(editingResume.parseStatus, editingResume.parseErrorCode)"
                        class="text-ink-faint"
                      >
                        {{
                          parseStatusText(editingResume.parseStatus, editingResume.parseErrorCode)
                        }}
                      </span>
                      <button
                        type="button"
                        class="text-ink-soft transition hover:text-vermilion"
                        @click="removeAttachment(editingResume.id)"
                      >
                        移除附件
                      </button>
                    </div>
                    <div class="mt-3 flex flex-wrap items-center gap-3">
                      <button
                        type="button"
                        class="btn-line py-1.5 text-[0.68rem] disabled:cursor-not-allowed disabled:opacity-50"
                        :disabled="parsing"
                        @click="fileInput?.click()"
                      >
                        {{ parsing ? '解析中…' : editingId ? '选择文件替换附件' : '选择文件上传' }}
                      </button>
                      <span
                        v-if="!editingId && atLimit"
                        class="text-[0.62rem] text-ink-faint"
                      >
                        已保存 {{ RESUME_LIMIT }} 份，删除一份后可继续上传。
                      </span>
                      <span class="font-mono text-[0.58rem] text-ink-faint">
                        {{ resumes.resumes.length }} / {{ RESUME_LIMIT }}
                      </span>
                    </div>
                    <p
                      v-if="attachNotice"
                      class="mt-2 text-[0.62rem]"
                      :class="{
                        'text-moss': attachNotice.tone === 'ok',
                        'text-vermilion': attachNotice.tone === 'warn',
                        'text-ink-soft': attachNotice.tone === 'busy',
                      }"
                    >
                      {{ attachNotice.text }}
                    </p>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">基本信息</h3>
                    <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      BASIC
                    </span>
                  </div>
                  <div class="mt-3 grid gap-3 sm:grid-cols-2">
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">姓名 *</span>
                      <input
                        v-model="draft.name"
                        type="text"
                        class="field mt-1.5 text-sm"
                        placeholder="你的姓名"
                        autocomplete="off"
                      />
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">手机号 *</span>
                      <input
                        v-model="draft.phone"
                        type="tel"
                        class="field mt-1.5 text-sm"
                        placeholder="138-0000-0000"
                        autocomplete="off"
                      />
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">邮箱 *</span>
                      <input
                        v-model="draft.email"
                        type="email"
                        class="field mt-1.5 text-sm"
                        placeholder="name@example.com"
                        autocomplete="off"
                      />
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">现居城市</span>
                      <input
                        v-model="draft.city"
                        type="text"
                        class="field mt-1.5 text-sm"
                        placeholder="如：杭州"
                        autocomplete="off"
                      />
                    </label>
                  </div>
                  <p class="mt-1.5 text-[0.58rem] text-ink-faint">手机号与邮箱至少填写一个。</p>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">求职意向</h3>
                    <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      INTENTION
                    </span>
                  </div>
                  <div class="mt-3 grid gap-3 sm:grid-cols-2">
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">期望职位 *</span>
                      <input
                        v-model="draft.targetRole"
                        type="text"
                        class="field mt-1.5 text-sm"
                        placeholder="如：前端工程师"
                        autocomplete="off"
                      />
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">期望城市</span>
                      <input
                        v-model="draft.targetCity"
                        type="text"
                        class="field mt-1.5 text-sm"
                        placeholder="如：杭州"
                        autocomplete="off"
                      />
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">工作类型</span>
                      <select v-model="draft.jobType" class="field mt-1.5 text-sm">
                        <option value="">未选择</option>
                        <option>校招</option>
                        <option>实习</option>
                        <option>社招</option>
                      </select>
                    </label>
                    <label class="block">
                      <span class="text-[0.62rem] text-ink-faint">可到岗时间</span>
                      <input
                        v-model="draft.availableDate"
                        type="month"
                        class="field mt-1.5 text-sm"
                      />
                    </label>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">教育经历</h3>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addEducation"
                    >
                      + 添加教育经历
                    </button>
                  </div>
                  <p
                    v-if="!draft.educations.length"
                    class="mt-3 border border-dashed border-line-strong px-3 py-4 text-[0.68rem] text-ink-faint"
                  >
                    至少填写一段教育经历，校招筛选会重点看学校、专业与毕业时间。
                  </p>
                  <div
                    v-for="(education, index) in draft.educations"
                    :key="`education-${index}`"
                    class="mt-3 border border-line p-3"
                  >
                    <div class="flex items-center justify-between">
                      <p class="text-[0.62rem] text-ink-faint">教育经历 {{ index + 1 }}</p>
                      <button
                        type="button"
                        class="text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                        @click="removeEducation(index)"
                      >
                        移除
                      </button>
                    </div>
                    <div class="mt-2 grid gap-3 sm:grid-cols-2">
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">学校</span>
                        <input
                          v-model="education.school"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="学校名称"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">学历</span>
                        <select v-model="education.degree" class="field mt-1 text-sm">
                          <option>本科</option>
                          <option>硕士</option>
                          <option>博士</option>
                          <option>大专</option>
                          <option>其他</option>
                        </select>
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">专业</span>
                        <input
                          v-model="education.major"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="专业名称"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">成绩排名</span>
                        <select v-model="education.ranking" class="field mt-1 text-sm">
                          <option value="">未填写</option>
                          <option>前 5%</option>
                          <option>前 10%</option>
                          <option>前 20%</option>
                          <option>前 30%</option>
                          <option>前 50%</option>
                        </select>
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">入学时间</span>
                        <input
                          v-model="education.startDate"
                          type="month"
                          class="field mt-1 text-sm"
                        />
                      </label>
                      <div class="block">
                        <span class="text-[0.6rem] text-ink-faint">毕业时间</span>
                        <div class="mt-1 flex items-center gap-2">
                          <input
                            v-if="education.endDate !== '至今'"
                            v-model="education.endDate"
                            type="month"
                            class="field min-w-0 flex-1 text-sm"
                          />
                          <span v-else class="flex-1 text-sm text-ink-soft">至今</span>
                          <label class="flex shrink-0 items-center gap-1 text-[0.6rem] text-ink-faint">
                            <input
                              type="checkbox"
                              class="h-3 w-3 accent-vermilion"
                              :checked="education.endDate === '至今'"
                              @change="togglePresent(education, $event)"
                            />
                            至今
                          </label>
                        </div>
                      </div>
                    </div>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">实习与工作经历</h3>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addExperience"
                    >
                      + 添加经历
                    </button>
                  </div>
                  <div
                    v-for="(experience, index) in draft.experiences"
                    :key="`experience-${index}`"
                    class="mt-3 border border-line p-3"
                  >
                    <div class="flex items-center justify-between">
                      <p class="text-[0.62rem] text-ink-faint">经历 {{ index + 1 }}</p>
                      <button
                        type="button"
                        class="text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                        @click="removeExperience(index)"
                      >
                        移除
                      </button>
                    </div>
                    <div class="mt-2 grid gap-3 sm:grid-cols-2">
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">公司</span>
                        <input
                          v-model="experience.company"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="公司名称"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">职位</span>
                        <input
                          v-model="experience.role"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="岗位名称"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">类型</span>
                        <select v-model="experience.type" class="field mt-1 text-sm">
                          <option>实习</option>
                          <option>全职</option>
                          <option>兼职</option>
                        </select>
                      </label>
                      <div class="block">
                        <span class="text-[0.6rem] text-ink-faint">起止时间</span>
                        <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1.5">
                          <div class="flex min-w-0 flex-1 items-center gap-1.5">
                            <input
                              v-model="experience.startDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span class="shrink-0 text-[0.6rem] text-ink-faint">–</span>
                            <input
                              v-if="experience.endDate !== '至今'"
                              v-model="experience.endDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span v-else class="shrink-0 text-sm text-ink-soft">至今</span>
                          </div>
                          <label class="flex shrink-0 items-center gap-1 text-[0.6rem] text-ink-faint">
                            <input
                              type="checkbox"
                              class="h-3 w-3 accent-vermilion"
                              :checked="experience.endDate === '至今'"
                              @change="togglePresent(experience, $event)"
                            />
                            至今
                          </label>
                        </div>
                      </div>
                    </div>
                    <label class="mt-3 block">
                      <span class="text-[0.6rem] text-ink-faint">职责与业绩</span>
                      <textarea
                        v-model="experience.description"
                        class="field mt-1 text-sm leading-6"
                        rows="3"
                        placeholder="交代背景、动作和结果，尽量带上数字。"
                      />
                    </label>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">项目经历</h3>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addProject"
                    >
                      + 添加项目
                    </button>
                  </div>
                  <div
                    v-for="(project, index) in draft.projects"
                    :key="`project-${index}`"
                    class="mt-3 border border-line p-3"
                  >
                    <div class="flex items-center justify-between">
                      <p class="text-[0.62rem] text-ink-faint">项目 {{ index + 1 }}</p>
                      <button
                        type="button"
                        class="text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                        @click="removeProject(index)"
                      >
                        移除
                      </button>
                    </div>
                    <div class="mt-2 grid gap-3 sm:grid-cols-2">
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">项目名称</span>
                        <input
                          v-model="project.name"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="项目名称"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">担任角色</span>
                        <input
                          v-model="project.role"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="如：前端负责人"
                          autocomplete="off"
                        />
                      </label>
                      <div class="block">
                        <span class="text-[0.6rem] text-ink-faint">起止时间</span>
                        <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1.5">
                          <div class="flex min-w-0 flex-1 items-center gap-1.5">
                            <input
                              v-model="project.startDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span class="shrink-0 text-[0.6rem] text-ink-faint">–</span>
                            <input
                              v-if="project.endDate !== '至今'"
                              v-model="project.endDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span v-else class="shrink-0 text-sm text-ink-soft">至今</span>
                          </div>
                          <label class="flex shrink-0 items-center gap-1 text-[0.6rem] text-ink-faint">
                            <input
                              type="checkbox"
                              class="h-3 w-3 accent-vermilion"
                              :checked="project.endDate === '至今'"
                              @change="togglePresent(project, $event)"
                            />
                            至今
                          </label>
                        </div>
                      </div>
                    </div>
                    <label class="mt-3 block">
                      <span class="text-[0.6rem] text-ink-faint">项目职责与成果</span>
                      <textarea
                        v-model="project.description"
                        class="field mt-1 text-sm leading-6"
                        rows="3"
                        placeholder="写清技术方案、个人贡献和量化结果。"
                      />
                    </label>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">在校实践</h3>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addPractice"
                    >
                      + 添加实践
                    </button>
                  </div>
                  <div
                    v-for="(practice, index) in draft.practices"
                    :key="`practice-${index}`"
                    class="mt-3 border border-line p-3"
                  >
                    <div class="flex items-center justify-between">
                      <p class="text-[0.62rem] text-ink-faint">实践 {{ index + 1 }}</p>
                      <button
                        type="button"
                        class="text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                        @click="removePractice(index)"
                      >
                        移除
                      </button>
                    </div>
                    <div class="mt-2 grid gap-3 sm:grid-cols-2">
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">组织 / 活动</span>
                        <input
                          v-model="practice.name"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="如：校学生会技术部"
                          autocomplete="off"
                        />
                      </label>
                      <label class="block">
                        <span class="text-[0.6rem] text-ink-faint">担任角色</span>
                        <input
                          v-model="practice.role"
                          type="text"
                          class="field mt-1 text-sm"
                          placeholder="如：技术部负责人"
                          autocomplete="off"
                        />
                      </label>
                      <div class="block">
                        <span class="text-[0.6rem] text-ink-faint">起止时间</span>
                        <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1.5">
                          <div class="flex min-w-0 flex-1 items-center gap-1.5">
                            <input
                              v-model="practice.startDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span class="shrink-0 text-[0.6rem] text-ink-faint">–</span>
                            <input
                              v-if="practice.endDate !== '至今'"
                              v-model="practice.endDate"
                              type="month"
                              class="field min-w-0 flex-1 text-sm"
                            />
                            <span v-else class="shrink-0 text-sm text-ink-soft">至今</span>
                          </div>
                          <label class="flex shrink-0 items-center gap-1 text-[0.6rem] text-ink-faint">
                            <input
                              type="checkbox"
                              class="h-3 w-3 accent-vermilion"
                              :checked="practice.endDate === '至今'"
                              @change="togglePresent(practice, $event)"
                            />
                            至今
                          </label>
                        </div>
                      </div>
                    </div>
                    <label class="mt-3 block">
                      <span class="text-[0.6rem] text-ink-faint">实践内容与成果</span>
                      <textarea
                        v-model="practice.description"
                        class="field mt-1 text-sm leading-6"
                        rows="3"
                        placeholder="写清职责、规模和结果。"
                      />
                    </label>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">技能与证书</h3>
                    <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      SKILLS
                    </span>
                  </div>
                  <div class="mt-3 flex items-center justify-between">
                    <p class="text-[0.62rem] text-ink-faint">专业技能</p>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addSkill"
                    >
                      + 添加技能
                    </button>
                  </div>
                  <div
                    v-for="(skill, index) in draft.skills"
                    :key="`skill-${index}`"
                    class="mt-2 flex items-center gap-2"
                  >
                    <input
                      v-model="skill.name"
                      type="text"
                      class="field text-sm"
                      placeholder="如：Vue 3"
                      autocomplete="off"
                    />
                    <select v-model="skill.level" class="field w-24 shrink-0 text-sm">
                      <option>熟练</option>
                      <option>掌握</option>
                      <option>了解</option>
                    </select>
                    <button
                      type="button"
                      class="shrink-0 text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                      @click="removeSkill(index)"
                    >
                      移除
                    </button>
                  </div>

                  <div class="mt-4 flex items-center justify-between">
                    <p class="text-[0.62rem] text-ink-faint">资格证书</p>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addCertificate"
                    >
                      + 添加证书
                    </button>
                  </div>
                  <div
                    v-for="(certificate, index) in draft.certificates"
                    :key="`certificate-${index}`"
                    class="mt-2 flex items-center gap-2"
                  >
                    <input
                      v-model="certificate.name"
                      type="text"
                      class="field text-sm"
                      placeholder="如：CET-6"
                      autocomplete="off"
                    />
                    <input
                      v-model="certificate.date"
                      type="month"
                      class="field w-28 shrink-0 text-sm"
                    />
                    <button
                      type="button"
                      class="shrink-0 text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                      @click="removeCertificate(index)"
                    >
                      移除
                    </button>
                  </div>

                  <div class="mt-4 flex items-center justify-between">
                    <p class="text-[0.62rem] text-ink-faint">校内荣誉</p>
                    <button
                      type="button"
                      class="text-[0.64rem] text-ink-soft transition hover:text-vermilion"
                      @click="addHonor"
                    >
                      + 添加荣誉
                    </button>
                  </div>
                  <div
                    v-for="(honor, index) in draft.honors"
                    :key="`honor-${index}`"
                    class="mt-2 flex items-center gap-2"
                  >
                    <input
                      v-model="honor.name"
                      type="text"
                      class="field text-sm"
                      placeholder="如：校级一等奖学金"
                      autocomplete="off"
                    />
                    <select v-model="honor.level" class="field w-24 shrink-0 text-sm">
                      <option>国家级</option>
                      <option>省级</option>
                      <option>校级</option>
                      <option>其他</option>
                    </select>
                    <input
                      v-model="honor.date"
                      type="month"
                      class="field w-28 shrink-0 text-sm"
                    />
                    <button
                      type="button"
                      class="shrink-0 text-[0.62rem] text-ink-faint transition hover:text-vermilion"
                      @click="removeHonor(index)"
                    >
                      移除
                    </button>
                  </div>
                </section>

                <section>
                  <div class="flex items-baseline justify-between border-b border-ink pb-1.5">
                    <h3 class="font-display text-lg">个人优势</h3>
                    <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">
                      SUMMARY
                    </span>
                  </div>
                  <label class="mt-3 block">
                    <textarea
                      v-model="draft.summary"
                      class="field text-sm leading-6"
                      rows="4"
                      placeholder="如：关注前端工程化与交互细节，有 6 个月实习经验，能独立推进需求落地。"
                    />
                  </label>
                </section>
              </div>

              <aside class="lg:sticky lg:top-0 lg:self-start">
                <div class="flex items-baseline justify-between border-b border-ink pb-2">
                  <h3 class="font-display text-lg">简历预览</h3>
                  <span class="text-[0.6rem] text-ink-faint">A4</span>
                </div>

                <div class="mt-3 border border-line bg-paper p-4">
                  <p class="font-mono text-[0.52rem] tracking-[0.22em] text-ink-faint">
                    RESUME / 2026
                  </p>
                  <p class="mt-2 font-display text-2xl leading-none">
                    {{ draft.name || '姓名' }}
                  </p>
                  <p class="mt-1.5 text-[0.64rem] text-ink-soft">
                    {{
                      [draft.targetRole, draft.targetCity, draft.jobType].filter(Boolean).join(' · ') ||
                      '求职意向'
                    }}
                  </p>
                  <p v-if="draft.availableDate" class="mt-0.5 text-[0.6rem] text-ink-faint">
                    {{ draft.availableDate }} 可到岗
                  </p>
                  <p class="mt-1 text-[0.6rem] text-ink-faint">
                    {{ [draft.phone, draft.email].filter(Boolean).join(' · ') }}
                  </p>
                  <p v-if="draft.city" class="mt-0.5 text-[0.6rem] text-ink-faint">
                    现居 {{ draft.city }}
                  </p>

                  <div v-if="previewEducations.length" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">教育经历</p>
                    <div
                      v-for="(education, index) in previewEducations"
                      :key="`preview-education-${index}`"
                      class="mt-1.5"
                    >
                      <p class="text-[0.68rem] leading-4">
                        {{
                          [education.school, education.degree, education.major]
                            .filter(Boolean)
                            .join(' · ')
                        }}
                      </p>
                      <p class="text-[0.58rem] text-ink-faint">
                        {{ dateRange(education.startDate, education.endDate) }}
                        <span v-if="education.ranking"> · {{ education.ranking }}</span>
                      </p>
                    </div>
                  </div>

                  <div v-if="previewExperiences.length" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">实习与工作经历</p>
                    <div
                      v-for="(experience, index) in previewExperiences"
                      :key="`preview-experience-${index}`"
                      class="mt-1.5"
                    >
                      <p class="text-[0.68rem] leading-4">
                        {{ [experience.company, experience.role].filter(Boolean).join(' · ') }}
                      </p>
                      <p class="text-[0.58rem] text-ink-faint">
                        {{
                          [experience.type, dateRange(experience.startDate, experience.endDate)]
                            .filter(Boolean)
                            .join(' · ')
                        }}
                      </p>
                      <ul v-if="lines(experience.description).length" class="mt-0.5 space-y-0.5">
                        <li
                          v-for="line in lines(experience.description)"
                          :key="line"
                          class="flex gap-1.5 text-[0.64rem] leading-4"
                        >
                          <span class="text-vermilion" aria-hidden="true">—</span>
                          <span>{{ line }}</span>
                        </li>
                      </ul>
                    </div>
                  </div>

                  <div v-if="previewProjects.length" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">项目经历</p>
                    <div
                      v-for="(project, index) in previewProjects"
                      :key="`preview-project-${index}`"
                      class="mt-1.5"
                    >
                      <p class="text-[0.68rem] leading-4">
                        {{ [project.name, project.role].filter(Boolean).join(' · ') }}
                      </p>
                      <p class="text-[0.58rem] text-ink-faint">
                        {{ dateRange(project.startDate, project.endDate) }}
                      </p>
                      <ul v-if="lines(project.description).length" class="mt-0.5 space-y-0.5">
                        <li
                          v-for="line in lines(project.description)"
                          :key="line"
                          class="flex gap-1.5 text-[0.64rem] leading-4"
                        >
                          <span class="text-vermilion" aria-hidden="true">—</span>
                          <span>{{ line }}</span>
                        </li>
                      </ul>
                    </div>
                  </div>

                  <div v-if="previewPractices.length" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">在校实践</p>
                    <div
                      v-for="(practice, index) in previewPractices"
                      :key="`preview-practice-${index}`"
                      class="mt-1.5"
                    >
                      <p class="text-[0.68rem] leading-4">
                        {{ [practice.name, practice.role].filter(Boolean).join(' · ') }}
                      </p>
                      <p class="text-[0.58rem] text-ink-faint">
                        {{ dateRange(practice.startDate, practice.endDate) }}
                      </p>
                      <ul v-if="lines(practice.description).length" class="mt-0.5 space-y-0.5">
                        <li
                          v-for="line in lines(practice.description)"
                          :key="line"
                          class="flex gap-1.5 text-[0.64rem] leading-4"
                        >
                          <span class="text-vermilion" aria-hidden="true">—</span>
                          <span>{{ line }}</span>
                        </li>
                      </ul>
                    </div>
                  </div>

                  <div v-if="previewSkills.length" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">技能</p>
                    <div class="mt-1 flex flex-wrap gap-1">
                      <span
                        v-for="skill in previewSkills"
                        :key="skill.name"
                        class="chip text-[0.58rem]"
                      >
                        {{ skill.name }}<span v-if="skill.level"> · {{ skill.level }}</span>
                      </span>
                    </div>
                  </div>

                  <div
                    v-if="previewCertificates.length || previewHonors.length"
                    class="mt-3 border-t border-line pt-2"
                  >
                    <p v-if="previewCertificates.length" class="text-[0.58rem] text-ink-faint">
                      证书 · {{ previewCertificates.map((item) => item.name).join('、') }}
                    </p>
                    <p v-if="previewHonors.length" class="mt-1 text-[0.58rem] text-ink-faint">
                      荣誉 ·
                      {{
                        previewHonors
                          .map((item) => `${item.name}${item.level ? `（${item.level}）` : ''}`)
                          .join('、')
                      }}
                    </p>
                  </div>

                  <div v-if="draft.summary.trim()" class="mt-3 border-t border-line pt-2">
                    <p class="text-[0.58rem] text-ink-faint">个人优势</p>
                    <p class="mt-1 text-[0.64rem] leading-4 text-ink-soft">{{ draft.summary }}</p>
                  </div>
                </div>

                <section class="mt-4 border border-line bg-paper-light p-3">
                  <div class="flex items-baseline justify-between border-b border-line pb-1.5">
                    <h3 class="font-display text-base">简历优化</h3>
                    <span class="font-mono text-[0.52rem] tracking-[0.14em] text-ink-faint">
                      OPTIMIZE
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn-solid mt-3 w-full py-2 text-[0.7rem]"
                    @click="aiDemoOpen = true"
                  >
                    AI 帮忙优化简历
                  </button>
                </section>
              </aside>
            </div>
          </div>

          <footer class="flex flex-wrap items-center gap-3 border-t border-ink px-5 py-3">
            <p
              v-if="editorError"
              role="alert"
              class="min-w-0 flex-1 border border-vermilion/40 bg-vermilion/5 px-3 py-2 text-[0.68rem] text-vermilion"
            >
              {{ editorError }}
            </p>
            <p v-else class="min-w-0 flex-1 text-[0.6rem] text-ink-faint">
              在线简历会用于投递；附件可选上传，解析出的字段会自动回填到表单。
            </p>
            <div class="ml-auto flex items-center gap-2">
              <button type="button" class="btn-line py-1.5 text-[0.7rem]" @click="closeEditor">
                取消
              </button>
              <button type="submit" class="btn-solid py-1.5 text-[0.7rem]" :disabled="saving">
                {{ saving ? '保存中…' : '保存简历 →' }}
              </button>
            </div>
          </footer>
        </form>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="aiDemoOpen"
        class="fixed inset-0 z-[60] flex items-center justify-center bg-ink/40 p-6"
        role="presentation"
        @click.self="aiDemoOpen = false"
      >
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="ai-optimize-demo-title"
          class="w-full max-w-md border border-ink bg-paper-light p-5 shadow-xl"
        >
          <div class="flex items-baseline justify-between border-b border-line pb-2">
            <h3 id="ai-optimize-demo-title" class="font-display text-xl">AI 简历优化</h3>
            <span class="font-mono text-[0.54rem] tracking-[0.14em] text-ink-faint">DEMO</span>
          </div>
          <p class="mt-3 text-sm leading-6 text-ink-soft">
            演示占位：AI 优化页面尚未实现，具体交互会在后续版本规划。
          </p>
          <div class="mt-5 flex justify-end">
            <button type="button" class="btn-line py-1.5 text-[0.7rem]" @click="aiDemoOpen = false">
              知道了
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>
