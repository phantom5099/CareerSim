import type { ResumeDocument } from '@/api/types'

export const seedResumes: ResumeDocument[] = [
  {
    id: 'resume-main',
    title: '林同学',
    kind: 'online',
    updatedAt: '2026-11-06',
    isDefault: true,
    payload: {
      name: '林同学',
      phone: '138-0000-1234',
      email: 'lin@example.com',
      city: '杭州',
      targetRole: '前端工程师',
      targetCity: '杭州',
      jobType: '校招',
      availableDate: '2027-07',
      educations: [
        {
          school: '南山大学',
          degree: '本科',
          major: '计算机科学',
          startDate: '2023-09',
          endDate: '2027-06',
          ranking: '前 10%',
        },
      ],
      experiences: [
        {
          company: '云栖科技',
          role: '前端开发实习生',
          type: '实习',
          startDate: '2026-03',
          endDate: '2026-08',
          description:
            '负责公司官网改版，首屏加载从 1.2s 降到 0.8s，转化率提升 12%\n搭建内部组件库，覆盖 30+ 业务页面，减少重复开发约 40%',
        },
      ],
      projects: [
        {
          name: '校招信息聚合站',
          role: '前端负责人',
          startDate: '2025-10',
          endDate: '2026-01',
          description: '使用 Vue 3 + TypeScript 实现岗位筛选与收藏，累计 2,000+ 校内用户。',
        },
      ],
      practices: [
        {
          name: '校学生会技术部',
          role: '技术部负责人',
          startDate: '2024-09',
          endDate: '2025-06',
          description: '组织 4 场前端入门工作坊，带 12 名同学完成校园活动报名系统。',
        },
      ],
      skills: [
        { name: 'Vue 3', level: '熟练' },
        { name: 'TypeScript', level: '熟练' },
        { name: 'Vite', level: '掌握' },
        { name: 'Tailwind CSS', level: '掌握' },
      ],
      certificates: [
        { name: 'CET-6', date: '2024-12' },
        { name: '软件设计师', date: '2026-05' },
      ],
      honors: [{ name: '校级一等奖学金', level: '校级', date: '2025-10' }],
      summary: '关注前端工程化与交互细节，习惯用数据验证方案，能在跨团队协作中推进落地。',
    },
  },
  {
    id: 'resume-design',
    title: '视觉设计作品集',
    kind: 'attachment',
    updatedAt: '2026-10-28',
    isDefault: false,
    attachment: {
      fileName: '林同学-视觉设计作品集.pdf',
      fileSize: 5_452_595,
    },
  },
]
