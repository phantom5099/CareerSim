import type { InterviewsApi } from '@/api/endpoints/interviews'
import type {
  Application,
  Company,
  InterviewScript,
  ResumeScreening,
  StageKey,
} from '@/domain/types'

import {
  STAGE_PROFILES,
  curatedScripts,
  fallbackScreening,
  screeningReports,
} from './data/interviews'

function fill(text: string, companyName: string, jobTitle: string): string {
  return text.replace(/{{company}}/g, companyName).replace(/{{jobTitle}}/g, jobTitle)
}

function buildScript(
  application: Application,
  stage: StageKey,
  company?: Company,
): InterviewScript {
  const curated = curatedScripts.find(
    (script) => script.applicationId === application.id && script.stage === stage,
  )
  if (curated) {
    return curated
  }

  const profile = STAGE_PROFILES[stage]
  const companyName = company?.name ?? '这家公司'
  return {
    applicationId: application.id,
    stage,
    interviewer: profile.interviewer,
    interviewerTitle: profile.interviewerTitle,
    focus: profile.focus,
    estimatedMinutes: profile.estimatedMinutes,
    turns: profile.turns.map((turn) => ({
      question: fill(turn.question, companyName, application.jobTitle),
      ...(turn.followUp
        ? { followUp: fill(turn.followUp, companyName, application.jobTitle) }
        : {}),
    })),
  }
}

function findScreening(application: Application): ResumeScreening {
  return (
    screeningReports.find((report) => report.applicationId === application.id) ?? fallbackScreening
  )
}

export const mockInterviewsApi: InterviewsApi = {
  async load(application, stage, company) {
    return {
      script: buildScript(application, stage, company),
      screening: stage === 'hr_screen' ? findScreening(application) : undefined,
    }
  },
}
