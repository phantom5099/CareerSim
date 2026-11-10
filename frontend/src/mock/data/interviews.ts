import type { InterviewScript, InterviewTurn, ResumeScreening, StageKey } from '@/domain/types'

export interface StageProfile {
  interviewer: string
  interviewerTitle: string
  focus: string[]
  estimatedMinutes: number
  turns: InterviewTurn[]
}

export const STAGE_PROFILES: Record<StageKey, StageProfile> = {
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

export interface ScreeningRecord extends ResumeScreening {
  applicationId: string
}

export const screeningReports: ScreeningRecord[] = [
  {
    applicationId: 'app-wuyu',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 86,
    verdict: '通过，进入下一环节',
    summary:
      '雾屿科技的「前端工程师」简历初筛已完成。简历与岗位要求的匹配度为 86/100，项目经历、技能覆盖与岗位方向基本吻合，达到进入下一环节的标准。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 86,
        note: '岗位关键词与投递方向一致，目标城市与岗位地点吻合。',
      },
      {
        label: '项目经历',
        score: 82,
        note: '主导项目可追溯，但量化结果只出现在部分经历中。',
      },
      {
        label: '技能覆盖',
        score: 88,
        note: '核心技能齐备，岗位加分项缺少直接证据。',
      },
      {
        label: '稳定性',
        score: 80,
        note: '两段经历衔接清晰，长期投入意愿需要面试确认。',
      },
    ],
    highlights: [
      '有与「前端工程师」直接相关的完整项目经历，能说清个人贡献。',
      '简历结构清晰，技能与项目按岗位要求组织，阅读成本低。',
      '关键结果有数字支撑，能快速判断经历的深度。',
    ],
    concerns: [
      '部分项目的指标口径不明确，需要在后续环节追问。',
      '最近一段经历的时长与岗位投入周期相关，建议确认。',
      '补充材料（作品集、代码仓库）尚未附上。',
    ],
  },
  {
    applicationId: 'app-bailu',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 84,
    verdict: '通过，进入下一环节',
    summary:
      '白露设计的「视觉设计师」简历初筛已完成。作品集与岗位方向的匹配度为 84/100，版式与品牌项目符合要求，可以进入笔试环节。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 84,
        note: '作品集方向与视觉设计师岗位一致，版式与品牌项目占比较高。',
      },
      {
        label: '项目经历',
        score: 81,
        note: '商业项目完整，但个人主导的部分需要进一步说明。',
      },
      {
        label: '技能覆盖',
        score: 86,
        note: '版式、字体与品牌规范熟练，动效经验较少。',
      },
      {
        label: '稳定性',
        score: 77,
        note: '最近一段经历较短，需要确认离职原因。',
      },
    ],
    highlights: [
      '作品集以品牌与活动视觉为主，与岗位方向直接相关。',
      '版式与信息层级处理成熟，能看出长期训练。',
      '有完整的项目过程记录，沟通成本低。',
    ],
    concerns: [
      '部分作品的个人贡献边界不清晰。',
      '动效与三维能力缺少案例。',
      '作品集更新到今年年中，建议补充近期项目。',
    ],
  },
  {
    applicationId: 'app-yuanshan',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 78,
    verdict: '筛选中，等待结果',
    summary:
      '远山出行的「数据分析师」简历初筛正在评估。当前匹配度为 78/100，需要结合城市选择与稳定性进一步确认。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 78,
        note: 'SQL 与指标分析经验符合要求，出行行业经验欠缺。',
      },
      {
        label: '项目经历',
        score: 74,
        note: '分析项目有完整链路，但业务结果描述偏弱。',
      },
      {
        label: '技能覆盖',
        score: 80,
        note: '取数与可视化工具齐备，实验设计经验较少。',
      },
      {
        label: '稳定性',
        score: 72,
        note: '跨城市求职，长期留下的意愿需要沟通确认。',
      },
    ],
    highlights: [
      '有与「数据分析师」直接相关的分析项目。',
      '能独立完成取数、清洗与结论输出。',
      '对业务指标有基本理解，表达清晰。',
    ],
    concerns: [
      '缺少出行或平台型业务的分析经验。',
      '项目中的业务收益缺少量化说明。',
      '异地求职的稳定性需要确认。',
    ],
  },
  {
    applicationId: 'app-beichen',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 74,
    verdict: '通过，进入下一环节',
    summary:
      '北辰云的「后端工程师（Go）」简历初筛已完成。简历与岗位要求的匹配度为 74/100，达到进入下一环节的标准。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 74,
        note: 'Go 与分布式系统经验符合要求，云厂商背景是加分项。',
      },
      {
        label: '项目经历',
        score: 70,
        note: '系统规模描述清楚，但故障与复盘的细节不足。',
      },
      {
        label: '技能覆盖',
        score: 76,
        note: '并发与存储经验齐备，可观测性实践较少。',
      },
      {
        label: '稳定性',
        score: 68,
        note: '近两段经历时长偏短，需要确认变动原因。',
      },
    ],
    highlights: [
      '有与「后端工程师（Go）」直接相关的系统项目。',
      '能说清服务的读写路径与容量边界。',
      '有线上问题排查经验，思路完整。',
    ],
    concerns: [
      '项目中的故障复盘缺少根因层面的描述。',
      '最近两段经历时长偏短，建议确认。',
      '缺少可观测性体系建设的直接经验。',
    ],
  },
  {
    applicationId: 'app-paperplane',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 92,
    verdict: '通过，进入下一环节',
    summary:
      '纸飞机实验室的「全栈工程师」简历初筛已完成。简历与岗位要求的匹配度为 92/100，项目经历与技术覆盖均高于岗位基准。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 92,
        note: '全栈项目与远程协作经验同时符合岗位要求。',
      },
      {
        label: '项目经历',
        score: 90,
        note: '项目从设计到上线完整，个人贡献边界清晰。',
      },
      {
        label: '技能覆盖',
        score: 94,
        note: '前后端与基础设施均有实践，技术栈匹配度高。',
      },
      {
        label: '稳定性',
        score: 86,
        note: '远程工作经历完整，投入周期与岗位一致。',
      },
    ],
    highlights: [
      '有与「全栈工程师」直接相关的完整产品经历。',
      '技术栈覆盖前后端与部署，能独立推进项目。',
      '项目结果有明确指标，能快速判断深度。',
    ],
    concerns: [
      '团队规模较小的经历，需要确认协作方式。',
      '部分技术选型的取舍原因需要追问。',
      '远程协作的沟通习惯需要在面试中确认。',
    ],
  },
  {
    applicationId: 'app-shiguang',
    reviewer: '陈可',
    reviewerTitle: '招聘 HR',
    score: 88,
    verdict: '通过，进入下一环节',
    summary:
      '拾光互动的「内容运营」简历初筛已完成。简历与岗位要求的匹配度为 88/100，内容项目与品牌经验符合岗位方向。',
    dimensions: [
      {
        label: '岗位匹配',
        score: 88,
        note: '内容策划与品牌项目经验符合岗位方向。',
      },
      {
        label: '项目经历',
        score: 85,
        note: '内容项目完整，数据结果有记录但口径不统一。',
      },
      {
        label: '技能覆盖',
        score: 90,
        note: '选题、撰写与活动执行均有实践，视频经验较少。',
      },
      {
        label: '稳定性',
        score: 84,
        note: '经历衔接清晰，长期投入意愿明确。',
      },
    ],
    highlights: [
      '有与「内容运营」直接相关的完整项目经历。',
      '能说清选题判断与内容节奏的设计依据。',
      '关键内容有数据反馈，能快速判断效果。',
    ],
    concerns: [
      '部分项目的数据口径不统一，需要追问。',
      '视频与直播内容经验较少。',
      '跨团队协作的案例需要补充。',
    ],
  },
]

export const fallbackScreening: ResumeScreening = {
  reviewer: '陈可',
  reviewerTitle: '招聘 HR',
  score: 80,
  verdict: '通过，进入下一环节',
  summary: '简历初筛已完成。简历与岗位要求的匹配度达到进入下一环节的标准。',
  dimensions: [
    { label: '岗位匹配', score: 80, note: '岗位关键词与投递方向基本一致。' },
    { label: '项目经历', score: 76, note: '项目经历完整，量化结果需要进一步确认。' },
    { label: '技能覆盖', score: 82, note: '核心技能齐备，加分项需要面试确认。' },
    { label: '稳定性', score: 74, note: '长期投入意愿需要面试确认。' },
  ],
  highlights: [
    '有与目标岗位相关的项目经历。',
    '简历结构清晰，阅读成本低。',
    '关键结果有基本的数据支撑。',
  ],
  concerns: [
    '部分项目的指标口径不明确，需要在后续环节追问。',
    '补充材料尚未附上。',
    '最近一段经历的投入周期需要确认。',
  ],
}

export const curatedScripts: InterviewScript[] = [
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
