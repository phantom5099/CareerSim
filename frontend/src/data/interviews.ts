import type {
  Application,
  Company,
  InterviewScript,
  InterviewTurn,
  StageKey,
} from '@/domain/types'

interface StageProfile {
  interviewer: string
  interviewerTitle: string
  focus: string[]
  estimatedMinutes: number
  turns: InterviewTurn[]
}

const STAGE_PROFILES: Record<StageKey, StageProfile> = {
  hr_screen: {
    interviewer: '陈可',
    interviewerTitle: '招聘 HR',
    focus: ['动机', '匹配度', '到岗时间'],
    estimatedMinutes: 15,
    turns: [
      {
        question: '先用一分钟介绍你自己，重点讲和「{{jobTitle}}」相关的经历。',
        followUp: '为什么是现在，为什么是这份工作？',
      },
      {
        question: '你为什么选择{{company}}？除了我们，你还在看哪些方向？',
      },
      {
        question: '你希望第一份工作里获得什么？哪些条件是你不能接受的？',
      },
      {
        question: '最快什么时候可以到岗？能稳定投入多长时间？',
      },
    ],
  },
  exam: {
    interviewer: '在线笔试',
    interviewerTitle: '限时作答',
    focus: ['基础', '逻辑', '表达'],
    estimatedMinutes: 30,
    turns: [
      {
        question: '用 200 字说明你对「{{jobTitle}}」日常工作的理解。',
      },
      {
        question: '面对一个陌生的业务问题，写下你会先确认的三个信息。',
        followUp: '如果只能拿到其中一个，你选哪个，为什么？',
      },
      {
        question: '选一个你熟悉的工具或方法，说明它解决什么问题、代价是什么。',
      },
    ],
  },
  tech_interview: {
    interviewer: '技术面试官',
    interviewerTitle: '团队一线',
    focus: ['项目深挖', '技术判断', '协作'],
    estimatedMinutes: 35,
    turns: [
      {
        question: '挑一个你主导的项目，讲清楚目标、关键决策和最终结果。',
        followUp: '如果重来一次，你会改哪一步？',
      },
      {
        question: '讲一个你排查过的最难的问题，你是如何定位根因的？',
      },
      {
        question: '你如何判断一个方案该现在做，还是以后再做？',
        followUp: '举一个你主动推迟需求并说服别人的例子。',
      },
      {
        question: '和同事在方案上产生分歧时，你通常怎么推进决策？',
      },
      {
        question: '入职第一个月，你希望先补齐哪块能力？',
      },
    ],
  },
  manager_interview: {
    interviewer: '业务负责人',
    interviewerTitle: '团队负责人',
    focus: ['业务理解', '判断力', '复盘'],
    estimatedMinutes: 30,
    turns: [
      {
        question: '你如何看待{{company}}所在的行业，未来一年最大的变化是什么？',
      },
      {
        question: '如果让你负责一个目标模糊的项目，你会怎么拆解第一步？',
        followUp: '你用什么标准判断它值得继续投入？',
      },
      {
        question: '讲一次你做出错误判断的经历，后来怎么修正？',
      },
      {
        question: '你希望三年后自己在这个岗位上是什么状态？',
      },
    ],
  },
  hrbp_interview: {
    interviewer: 'HRBP',
    interviewerTitle: '组织与人才',
    focus: ['稳定性', '价值观', '薪资预期'],
    estimatedMinutes: 20,
    turns: [
      {
        question: '你怎么看阶段性的加班和高压？说说你真实经历过的强度。',
      },
      {
        question: '你对自己未来两年的成长路径有什么设想？',
        followUp: '如果公司给不了这个路径，你会怎么调整？',
      },
      {
        question: '如果同时拿到两个 Offer，你会依据什么做选择？',
      },
      {
        question: '你对薪资和福利的预期是什么？',
      },
    ],
  },
  offer: {
    interviewer: '招聘组',
    interviewerTitle: 'Offer 沟通',
    focus: ['薪资结构', '入职安排', '选择'],
    estimatedMinutes: 15,
    turns: [
      {
        question: '这份 Offer 里，你最关心的是哪一部分？',
      },
      {
        question: '你对入职时间和办公地点有什么顾虑？',
      },
      {
        question: '做出最终决定前，你还需要我们补充什么信息？',
      },
    ],
  },
}

const curatedScripts: InterviewScript[] = [
  {
    applicationId: 'app-wuyu',
    stage: 'tech_interview',
    interviewer: '林泊',
    interviewerTitle: '前端负责人',
    focus: ['工程化', '状态管理', '性能'],
    estimatedMinutes: 35,
    turns: [
      {
        question: '先用两分钟介绍一下最近一个你主导的前端项目。',
        followUp: '这个项目里，哪一部分只有你能讲清楚？',
      },
      {
        question: '如果首屏加载从 1.2 秒优化到 0.8 秒，你会先查什么？',
        followUp: '你会用什么指标证明优化真的生效了？',
      },
      {
        question: 'Vue 3 的响应式系统在什么场景下会让你觉得“不该用它”？',
        followUp: '如果团队里有人坚持另一套方案，你会怎么推进决策？',
      },
      {
        question: '描述一次你和后端在接口设计上产生分歧的经历。',
        followUp: '最后是谁让步，为什么？',
      },
      {
        question: '如果入职后第一个月只能完成一件事，你希望是什么？',
      },
    ],
  },
  {
    applicationId: 'app-bailu',
    stage: 'exam',
    interviewer: '白露招聘组',
    interviewerTitle: '笔试监考 · 设计评审',
    focus: ['版式', '信息层级', '品牌一致性'],
    estimatedMinutes: 30,
    turns: [
      {
        question: '给你一段品牌简介和一张主视觉，请描述你会如何搭建一张活动海报的版式。',
        followUp: '你的第一视觉落点放在哪里？',
      },
      {
        question: '客户要求“高级一点”，但预算只够一套字体，你会怎么处理？',
        followUp: '你会如何向客户解释这个取舍？',
      },
      {
        question: '选一个你最近看到的、觉得版式很差的设计，说明问题出在哪。',
      },
      {
        question: '如果作品集只能保留三件作品，你会删掉哪一件，为什么？',
      },
    ],
  },
  {
    applicationId: 'app-yuanshan',
    stage: 'hr_screen',
    interviewer: '周岚',
    interviewerTitle: '招聘经理',
    focus: ['动机', '城市选择', '稳定性'],
    estimatedMinutes: 15,
    turns: [
      {
        question: '为什么选择成都，而不是你读书的城市？',
        followUp: '家人对你长期在成都发展的看法是什么？',
      },
      {
        question: '你理解的“数据分析师”在出行公司里每天做什么？',
        followUp: '如果业务方只给你一句模糊的需求，你会怎么拆？',
      },
      {
        question: '手上还有其他流程吗？如果同时拿到 Offer，你会怎么选？',
      },
      {
        question: '最快什么时候可以到岗实习？',
      },
    ],
  },
  {
    applicationId: 'app-beichen',
    stage: 'manager_interview',
    interviewer: '陈默',
    interviewerTitle: '后端架构师',
    focus: ['系统设计', '故障处理', '复盘'],
    estimatedMinutes: 40,
    turns: [
      {
        question: '设计一个支持多租户的配置中心，你会如何划分读写路径？',
        followUp: '配置变更如何做到可回滚？',
      },
      {
        question: '线上出现 5% 的请求超时，但监控没有明显错误，你怎么排查？',
      },
      {
        question: '讲一次你参与的故障复盘，真正的根因是什么？',
        followUp: '复盘之后你们改了什么流程？',
      },
      {
        question: '如果让你给一个刚入职的应届生讲 Go 的并发模型，你会怎么讲？',
      },
    ],
  },
]

function fill(text: string, companyName: string, jobTitle: string): string {
  return text.replace(/{{company}}/g, companyName).replace(/{{jobTitle}}/g, jobTitle)
}

export interface ResumeDimension {
  label: string
  score: number
  note: string
}

export interface ResumeScreening {
  reviewer: string
  reviewerTitle: string
  score: number
  verdict: string
  summary: string
  dimensions: ResumeDimension[]
  highlights: string[]
  concerns: string[]
}

function clamp(value: number, min = 55, max = 96): number {
  return Math.min(max, Math.max(min, Math.round(value)))
}

/** 简历初筛是对简历的解析与适配度评估，不进入问答。 */
export function screeningFor(
  application: Application,
  company?: Company,
): ResumeScreening {
  const base = clamp(application.score ?? 80)
  const companyName = company?.name ?? '该公司'
  const hrStage = application.stages.find((stage) => stage.key === 'hr_screen')
  const passed = hrStage?.state === 'done'

  return {
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: base,
    verdict: passed ? '通过，进入下一环节' : '未通过，流程结束',
    summary: `${companyName} 的「${application.jobTitle}」简历初筛已完成。简历与岗位要求的匹配度为 ${base}/100，${
      passed
        ? '项目经历、技能覆盖与岗位方向基本吻合，达到进入下一环节的标准。'
        : '部分关键要求缺少直接证据，未达到进入下一环节的标准。'
    }`,
    dimensions: [
      {
        label: '岗位匹配',
        score: base,
        note: '岗位关键词与投递方向一致，目标城市与岗位地点吻合。',
      },
      {
        label: '项目经历',
        score: clamp(base - 4),
        note: '主导项目可追溯，但量化结果只出现在部分经历中。',
      },
      {
        label: '技能覆盖',
        score: clamp(base + 2),
        note: '核心技能齐备，岗位加分项缺少直接证据。',
      },
      {
        label: '稳定性',
        score: clamp(base - 6),
        note: '两段经历衔接清晰，长期投入意愿需要面试确认。',
      },
    ],
    highlights: [
      `有与「${application.jobTitle}」直接相关的完整项目经历，能说清个人贡献。`,
      '简历结构清晰，技能与项目按岗位要求组织，阅读成本低。',
      '关键结果有数字支撑，能快速判断经历的深度。',
    ],
    concerns: [
      '部分项目的指标口径不明确，需要在后续环节追问。',
      '最近一段经历的时长与岗位投入周期相关，建议确认。',
      '补充材料（作品集、代码仓库）尚未附上。',
    ],
  }
}

export function scriptFor(
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

/** 当前阶段是否由候选人操作（进行中），否则视为等待对方结果。 */
export function actionableStage(application: Application): StageKey | undefined {
  if (application.status !== 'active') {
    return undefined
  }
  if (application.currentStage === 'hr_screen' || application.currentStage === 'offer') {
    return undefined
  }
  return application.currentStage
}
