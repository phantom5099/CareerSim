import type { Job } from '@/domain/types'

interface JobDraft {
  title: string
  type: Job['type']
  salary: string
  tags: string[]
  description: string
}

interface IndustryProfile {
  keywords: string[]
  type: string
  industry: string
  field: string
  tagline: string
  blurb: string
  accent: string
  jobs: JobDraft[]
}

const profiles: IndustryProfile[] = [
  {
    keywords: ['ai', '人工智能', '大模型', '算法', '机器学习', '推理', '智能体', 'agent'],
    type: '人工智能 · 基础设施',
    industry: '人工智能',
    field: '模型推理与智能应用',
    tagline: '让智能落在每一个具体场景里',
    blurb: '团队把模型能力做成稳定、可解释的产品，习惯小步验证。',
    accent: '#2F5D50',
    jobs: [
      {
        title: '算法工程师',
        type: '校招',
        salary: '25–40K · 16 薪',
        tags: ['PyTorch', 'LLM', '推理'],
        description: '负责模型训练与推理优化，把实验指标落到线上效果。',
      },
      {
        title: '机器学习平台工程师',
        type: '社招',
        salary: '30–45K · 16 薪',
        tags: ['Kubernetes', 'GPU', '训练平台'],
        description: '建设训练与评测流水线，支撑多团队共享算力。',
      },
      {
        title: 'AI 产品实习生',
        type: '实习',
        salary: '220–300 / 天',
        tags: ['产品', 'Prompt', '评测'],
        description: '参与智能功能的需求梳理与效果评测，每周至少到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['机器人', '硬件', '芯片', '半导体', '嵌入式', '物联网', '制造'],
    type: '智能硬件 · 机器人',
    industry: '智能硬件',
    field: '机器人整机与嵌入式系统',
    tagline: '把机器做得更像一件耐用的工具',
    blurb: '团队同时懂结构、电子与软件，产品在真实场景里迭代。',
    accent: '#4F6042',
    jobs: [
      {
        title: '嵌入式软件工程师',
        type: '校招',
        salary: '18–28K · 14 薪',
        tags: ['C++', 'RTOS', '驱动'],
        description: '负责控制板软件与传感器驱动，参与整机联调。',
      },
      {
        title: '机器人算法工程师',
        type: '社招',
        salary: '28–42K · 15 薪',
        tags: ['SLAM', '运动规划', 'ROS'],
        description: '研发定位与运动规划模块，推动算法在量产设备上落地。',
      },
      {
        title: '硬件测试实习生',
        type: '实习',
        salary: '180–240 / 天',
        tags: ['测试', '电路', '记录'],
        description: '协助完成硬件功能与可靠性测试，整理问题记录。',
      },
    ],
  },
  {
    keywords: ['数据', 'bi', '分析', '指标', '数仓', '商业分析'],
    type: '数据服务 · 商业分析',
    industry: '数据服务',
    field: '经营分析与数据平台',
    tagline: '把报表做成一次真正的决策',
    blurb: '团队由分析师、工程师与行业顾问组成，习惯用数据回答问题。',
    accent: '#3D5A45',
    jobs: [
      {
        title: '数据分析师',
        type: '校招',
        salary: '16–24K · 14 薪',
        tags: ['SQL', 'BI', '指标体系'],
        description: '负责经营指标体系搭建与专题分析，和业务团队一起做决策。',
      },
      {
        title: '数据平台工程师',
        type: '社招',
        salary: '26–38K · 15 薪',
        tags: ['数仓', 'Flink', '治理'],
        description: '建设实时与离线数据链路，保证数据质量与时效。',
      },
      {
        title: '商业分析实习生',
        type: '实习',
        salary: '200–260 / 天',
        tags: ['调研', 'Excel', '写作'],
        description: '协助行业研究与专题报告撰写，每周到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['设计', '品牌', '创意', '视觉', '交互', '工作室'],
    type: '品牌与体验设计',
    industry: '设计服务',
    field: '品牌识别与数字体验',
    tagline: '把品牌做成一门可以翻阅的刊物',
    blurb: '工作室没有冗长的汇报链，设计师直接面对客户与作品。',
    accent: '#A4482F',
    jobs: [
      {
        title: '视觉设计师',
        type: '校招',
        salary: '14–20K · 13 薪',
        tags: ['品牌视觉', '版式', '动效'],
        description: '参与品牌识别、出版物与数字界面设计，需要扎实的版式功底。',
      },
      {
        title: '交互设计师',
        type: '社招',
        salary: '20–30K · 13 薪',
        tags: ['体验策略', '原型', '用户研究'],
        description: '负责从研究到原型的完整体验设计，与品牌团队共同交付。',
      },
      {
        title: '品牌策略实习生',
        type: '实习',
        salary: '180 / 天',
        tags: ['品牌策略', '调研', '写作'],
        description: '协助行业研究、品牌叙事与提案撰写，每周至少到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['游戏', '动漫', '二次元', '互动娱乐', '电竞'],
    type: '游戏 · 互动娱乐',
    industry: '游戏',
    field: '游戏内容与互动体验',
    tagline: '先做让自己想玩的游戏',
    blurb: '团队规模不大，策划、美术与程序坐在同一张桌子上。',
    accent: '#5B4A7A',
    jobs: [
      {
        title: '游戏客户端工程师',
        type: '校招',
        salary: '20–30K · 15 薪',
        tags: ['Unity', 'C#', '性能'],
        description: '负责战斗与界面模块开发，持续优化客户端性能。',
      },
      {
        title: '游戏策划',
        type: '社招',
        salary: '18–30K · 14 薪',
        tags: ['关卡', '数值', '叙事'],
        description: '设计关卡与成长体系，用数据和玩家反馈验证设计。',
      },
      {
        title: '美术实习生',
        type: '实习',
        salary: '200–260 / 天',
        tags: ['原画', 'UI', '沟通'],
        description: '参与角色与界面美术制作，需提供作品集。',
      },
    ],
  },
  {
    keywords: ['电商', '零售', '消费', '直播', '供应链'],
    type: '电商 · 消费品牌',
    industry: '电子商务',
    field: '消费品牌与零售增长',
    tagline: '把好用和好卖放在同一张桌上',
    blurb: '团队同时管货、管内容、管渠道，节奏快但复盘认真。',
    accent: '#8A5A2B',
    jobs: [
      {
        title: '电商运营',
        type: '校招',
        salary: '12–18K · 14 薪',
        tags: ['店铺运营', '活动', '数据'],
        description: '负责店铺日常运营与活动策划，跟踪转化与复购。',
      },
      {
        title: '供应链计划专员',
        type: '社招',
        salary: '15–24K · 13 薪',
        tags: ['预测', '库存', '协同'],
        description: '制定补货与库存计划，协调仓配与销售节奏。',
      },
      {
        title: '内容营销实习生',
        type: '实习',
        salary: '180–220 / 天',
        tags: ['内容', '短视频', '投放'],
        description: '参与内容脚本与达人对接，每周至少到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['金融', '银行', '支付', '保险', '风控', '量化'],
    type: '金融科技',
    industry: '金融科技',
    field: '支付与风控系统',
    tagline: '把风险算清楚，再把体验做简单',
    blurb: '团队在合规与体验之间找平衡，工程与业务紧密协作。',
    accent: '#29405E',
    jobs: [
      {
        title: '后端工程师',
        type: '校招',
        salary: '22–32K · 16 薪',
        tags: ['Java', '交易', '一致性'],
        description: '参与交易与账务系统研发，关注一致性与可观测性。',
      },
      {
        title: '风控策略分析师',
        type: '社招',
        salary: '25–38K · 15 薪',
        tags: ['风控', '模型', '策略'],
        description: '设计风控策略与监控指标，平衡风险与通过率。',
      },
      {
        title: '合规实习生',
        type: '实习',
        salary: '200–260 / 天',
        tags: ['合规', '文档', '流程'],
        description: '协助整理合规材料与流程记录，每周到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['教育', '学习', '课程', '知识', '培训'],
    type: '教育科技',
    industry: '教育培训',
    field: '学习产品与内容',
    tagline: '让学习这件事少一点阻力',
    blurb: '团队由教师、产品与工程师组成，内容与技术一起打磨。',
    accent: '#2F5D50',
    jobs: [
      {
        title: '前端工程师',
        type: '校招',
        salary: '16–26K · 14 薪',
        tags: ['Vue', 'TypeScript', '体验'],
        description: '负责学习端与教师端功能开发，打磨交互细节。',
      },
      {
        title: '课程产品经理',
        type: '社招',
        salary: '18–28K · 14 薪',
        tags: ['课程', '需求', '数据'],
        description: '设计课程结构与学习路径，用数据验证效果。',
      },
      {
        title: '教研实习生',
        type: '实习',
        salary: '160–200 / 天',
        tags: ['教研', '写作', '排版'],
        description: '协助课程内容整理与校对，每周至少到岗 3 天。',
      },
    ],
  },
  {
    keywords: ['医疗', '健康', '生物', '医药', '器械'],
    type: '医疗健康',
    industry: '医疗健康',
    field: '数字医疗与健康服务',
    tagline: '把专业判断变成可用的产品',
    blurb: '团队与临床和科研伙伴长期合作，产品节奏稳、验证扎实。',
    accent: '#2C4A7C',
    jobs: [
      {
        title: '软件工程师',
        type: '校招',
        salary: '18–28K · 14 薪',
        tags: ['后端', '数据', '安全'],
        description: '负责健康数据服务开发，关注隐私与稳定性。',
      },
      {
        title: '临床产品经理',
        type: '社招',
        salary: '20–32K · 14 薪',
        tags: ['医疗', '需求', '合规'],
        description: '对接临床场景，梳理需求并推动产品落地。',
      },
      {
        title: '医学内容实习生',
        type: '实习',
        salary: '180–220 / 天',
        tags: ['医学', '内容', '校对'],
        description: '协助医学内容整理与校对，需相关专业背景。',
      },
    ],
  },
  {
    keywords: ['出行', '汽车', '座舱', '驾驶', '物流', '交通'],
    type: '智能出行',
    industry: '出行服务',
    field: '智能座舱与出行调度',
    tagline: '让城市里的每一段路更从容',
    blurb: '团队由整车、互联网与地图背景的工程师组成。',
    accent: '#4F6042',
    jobs: [
      {
        title: '嵌入式软件工程师',
        type: '校招',
        salary: '18–26K · 14 薪',
        tags: ['C++', '车载', 'RTOS'],
        description: '负责座舱域控制器软件模块的设计与调试。',
      },
      {
        title: '数据分析师',
        type: '校招',
        salary: '15–22K · 14 薪',
        tags: ['SQL', 'Python', '指标体系'],
        description: '支持出行调度与用户体验分析，建设核心指标体系。',
      },
      {
        title: '车辆测试实习生',
        type: '实习',
        salary: '160–200 / 天',
        tags: ['路测', '自动化', '记录'],
        description: '参与整车路测与问题记录，需要接受短期出差。',
      },
    ],
  },
  {
    keywords: ['内容', '社区', '媒体', '视频', '社交', '运营'],
    type: '内容与社区',
    industry: '文化传媒',
    field: '社区内容与创作者生态',
    tagline: '认真对待每一次表达',
    blurb: '团队相信内容的手感来自长期观察，而不是追热点。',
    accent: '#8A5A2B',
    jobs: [
      {
        title: '内容运营',
        type: '校招',
        salary: '12–18K · 15 薪',
        tags: ['内容策划', '社区', '数据复盘'],
        description: '负责社区垂类内容运营，从选题到复盘完整跟到底。',
      },
      {
        title: '社区产品经理',
        type: '社招',
        salary: '20–32K · 15 薪',
        tags: ['社区', '增长', '需求分析'],
        description: '围绕创作者与用户的关系设计产品机制。',
      },
      {
        title: '视频编导实习生',
        type: '实习',
        salary: '200 / 天',
        tags: ['短片', '脚本', '剪辑'],
        description: '参与人物短片与品牌视频的策划和拍摄。',
      },
    ],
  },
  {
    keywords: ['云', '基础设施', 'saas', '企业服务', '数据库', '中间件'],
    type: '云计算 · 企业服务',
    industry: '云计算',
    field: '云平台与企业软件',
    tagline: '把复杂留给平台，把简单交给客户',
    blurb: '工程文化强调可观测性、故障演练与写清楚的文档。',
    accent: '#2C4A7C',
    jobs: [
      {
        title: '后端工程师（Go）',
        type: '校招',
        salary: '22–30K · 15 薪',
        tags: ['Go', '分布式', '云原生'],
        description: '参与云控制面服务研发，关注一致性与可观测性。',
      },
      {
        title: 'SRE 工程师',
        type: '社招',
        salary: '25–40K · 15 薪',
        tags: ['稳定性', 'Linux', 'Prometheus'],
        description: '建设容量模型与故障演练体系，推动稳定性工程落地。',
      },
      {
        title: '解决方案实习生',
        type: '实习',
        salary: '200–240 / 天',
        tags: ['售前', '架构', '文档'],
        description: '协助整理客户方案与技术文档，每周到岗 3 天。',
      },
    ],
  },
]

const fallbackProfile: IndustryProfile = {
  keywords: [],
  type: '软件工具 · 产品研发',
  industry: '计算机软件',
  field: '效率工具与产品研发',
  tagline: '把一个小问题解决得足够彻底',
  blurb: '团队规模克制，产品、设计与工程一起对结果负责。',
  accent: '#3D5A45',
  jobs: [
    {
      title: '前端工程师',
      type: '校招',
      salary: '18–28K · 15 薪',
      tags: ['Vue', 'TypeScript', '体验'],
      description: '负责产品界面与交互实现，持续打磨使用体验。',
    },
    {
      title: '后端工程师',
      type: '社招',
      salary: '22–34K · 15 薪',
      tags: ['Rust', 'API', '数据'],
      description: '负责核心服务与数据链路研发，关注稳定性与可维护性。',
    },
    {
      title: '产品实习生',
      type: '实习',
      salary: '200–260 / 天',
      tags: ['产品', '调研', '写作'],
      description: '协助需求调研与文档整理，每周至少到岗 3 天。',
    },
  ],
}

const cities = [
  '北京',
  '上海',
  '广州',
  '深圳',
  '杭州',
  '成都',
  '南京',
  '武汉',
  '西安',
  '重庆',
  '苏州',
  '长沙',
  '厦门',
  '合肥',
  '天津',
  '青岛',
  '远程',
]

const namePrefixes = ['云岫', '知微', '栖野', '格物', '星澜', '澄明', '岱川', '自远', '未迟', '南枝']
const nameSuffixes = ['科技', '智能', '实验室', '数据', '互动', '软件', '数字']
const englishPrefixes = ['NOVA', 'ORBIT', 'LUMEN', 'ECHO', 'NORTH', 'VELLUM', 'ATLAS', 'KITE']
const englishSuffixes = ['LABS', 'STUDIO', 'WORKS', 'DATA', 'SYSTEMS', 'TECH']
const sizes = ['20–99 人', '100–499 人', '500–999 人', '1000–4999 人']
const workStyles = ['双休 · 弹性工作', '导师制 · 每周复盘', '小团队闭环 · 决策快', '远程友好 · 异步协作']

function hash(text: string): number {
  let value = 0
  for (const char of text) {
    value = (value * 31 + char.codePointAt(0)!) >>> 0
  }
  return value
}

function pick<T>(items: readonly T[], seed: number): T {
  return items[seed % items.length]!
}

function detectProfile(prompt: string): IndustryProfile {
  const lowered = prompt.toLowerCase()
  let best: IndustryProfile | undefined
  let bestHits = 0
  for (const profile of profiles) {
    const hits = profile.keywords.filter((keyword) => lowered.includes(keyword)).length
    if (hits > bestHits) {
      best = profile
      bestHits = hits
    }
  }
  return best ?? fallbackProfile
}

function detectCity(prompt: string): string {
  return cities.find((city) => prompt.includes(city)) ?? '杭州'
}

function detectName(prompt: string, seed: number, profile: IndustryProfile): string {
  const quoted = prompt.match(/[「“"]([^」”"]{2,12})[」”"]/)
  if (quoted?.[1]) {
    return quoted[1].trim()
  }
  const suffix =
    profile === fallbackProfile
      ? pick(nameSuffixes, seed)
      : profile.type.split(' · ')[0]!.replace(/(与|和).*$/, '')
  return `${pick(namePrefixes, seed)}${suffix}`
}

function futureDate(days: number): string {
  const date = new Date()
  date.setDate(date.getDate() + days)
  return date.toISOString().slice(0, 10)
}

export function generateCompanyTemplate(prompt: string): Record<string, unknown> {
  const trimmed = prompt.trim()
  const seed = hash(trimmed)
  const profile = detectProfile(trimmed)
  const city = detectCity(trimmed)
  const name = detectName(trimmed, seed, profile)
  const size = pick(sizes, seed >>> 3)
  const track = profile.type.split(' · ')[0]!
  const campusProject = `${new Date().getFullYear()} 秋季校园招聘`
  const internProject = '日常实习计划'

  const campusJobs = profile.jobs.filter((job) => job.type !== '实习')
  const internJobs = profile.jobs.filter((job) => job.type === '实习')

  return {
    name,
    englishName: `${pick(englishPrefixes, seed >>> 5)} ${pick(englishSuffixes, seed >>> 7)}`,
    industry: profile.industry,
    city,
    size,
    tagline: profile.tagline,
    description: `${name}是一家位于${city}的${profile.type}团队，专注${profile.field}。${profile.blurb}`,
    highlights: [
      `${city} · ${pick(workStyles, seed >>> 9)}`,
      `专注${profile.field}`,
      `${pick(['服务 200+ 客户', '产品用户 8 万+', '连续三年稳定增长', '核心团队共事 5 年'], seed >>> 11)}`,
    ],
    logoText: [...name].slice(0, 2).join(''),
    accent: profile.accent,
    projects: [
      {
        name: campusProject,
        season: `${new Date().getFullYear()} 秋招`,
        status: 'open',
        deadline: futureDate(45),
        description: `面向应届生，开放${campusJobs.map((job) => job.title).join('、')}方向。`,
      },
      {
        name: internProject,
        season: '日常实习',
        status: 'closing',
        deadline: futureDate(20),
        description: `每周至少到岗 3 天，${internJobs.map((job) => job.title).join('、')}方向可转正。`,
      },
    ],
    jobs: profile.jobs.map((job) => ({
      title: job.title,
      project: job.type === '实习' ? internProject : campusProject,
      industry: track,
      city,
      type: job.type,
      salary: job.salary,
      tags: job.tags,
      description: job.description,
    })),
  }
}
