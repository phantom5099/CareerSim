import type { Offer } from '@/domain/types'

export const seedOffers: Offer[] = [
  {
    id: 'offer-paperplane',
    companyId: 'paperplane',
    jobTitle: '全栈工程师',
    baseSalary: 25000,
    months: 14,
    bonus: '签字费 20,000',
    city: '远程',
    issuedAt: '2026-11-03',
    deadline: '2026-11-20',
    status: 'pending',
  },
  {
    id: 'offer-shiguang',
    companyId: 'shiguang',
    jobTitle: '内容运营',
    baseSalary: 18000,
    months: 15,
    bonus: '季度内容奖金',
    city: '广州',
    issuedAt: '2026-11-01',
    deadline: '2026-11-18',
    status: 'pending',
  },
]
