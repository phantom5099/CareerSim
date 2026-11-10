export interface User {
  id: string
  username: string
  displayName: string
  school?: string
  major?: string
  graduationYear?: number
  targetRoles: string[]
  targetCities: string[]
  avatarText: string
  avatarColor: string
}

export interface AuthSession {
  token: string
  expiresAt: string
  user: User
}

export interface LoginPayload {
  username: string
  password: string
}

export interface IndustriesResponse {
  industries: string[]
}

export interface ResumeEducation {
  school: string
  degree: string
  major: string
  startDate: string
  endDate: string
  ranking: string
}

export interface ResumeExperience {
  company: string
  role: string
  type: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumeProject {
  name: string
  role: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumePractice {
  name: string
  role: string
  startDate: string
  endDate: string
  description: string
}

export interface ResumeSkill {
  name: string
  level: string
}

export interface ResumeCertificate {
  name: string
  date: string
}

export interface ResumeHonor {
  name: string
  level: string
  date: string
}

export interface ResumePayload {
  name: string
  phone: string
  email: string
  city: string
  targetRole: string
  targetCity: string
  jobType: string
  availableDate: string
  educations: ResumeEducation[]
  experiences: ResumeExperience[]
  projects: ResumeProject[]
  practices: ResumePractice[]
  skills: ResumeSkill[]
  certificates: ResumeCertificate[]
  honors: ResumeHonor[]
  summary: string
}

export interface ResumeAttachment {
  fileName: string
  fileSize: number
}

interface ResumeMeta {
  id: string
  title: string
  updatedAt: string
  isDefault: boolean
}

export interface OnlineResume extends ResumeMeta {
  kind: 'online'
  payload: ResumePayload
}

export interface AttachmentResume extends ResumeMeta {
  kind: 'attachment'
  attachment: ResumeAttachment
}

export type ResumeDocument = OnlineResume | AttachmentResume

export type ResumeInput =
  | { title: string; kind: 'online'; payload: ResumePayload }
  | { title: string; kind: 'attachment'; attachment: ResumeAttachment }

export const RESUME_LIMIT = 3

