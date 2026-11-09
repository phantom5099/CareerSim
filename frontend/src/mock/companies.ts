import { ApiError } from '@/api/error'
import type { CompaniesApi } from '@/api/endpoints/companies'
import type { IndustriesResponse } from '@/api/types'
import type { Company } from '@/domain/types'

import { seedCompanies } from './data/companies'

// 后端公司与行业接口尚未接入：内存里保存导入的公司。
const INDUSTRIES = [
  '人工智能',
  '设计服务',
  '云计算',
  '文化传媒',
  '出行服务',
  '计算机软件',
  '智能硬件',
  '数据服务',
  '游戏',
  '电子商务',
  '金融科技',
  '教育培训',
  '医疗健康',
]

let imported: Company[] = []

export const mockCompaniesApi: CompaniesApi = {
  async list(): Promise<Company[]> {
    return [...imported, ...seedCompanies]
  },

  async industries(): Promise<IndustriesResponse> {
    return { industries: [...INDUSTRIES] }
  },

  async create(company: Company): Promise<Company> {
    if (
      seedCompanies.some((item) => item.id === company.id) ||
      imported.some((item) => item.id === company.id)
    ) {
      throw new ApiError('这家公司已经在列表里了', 409, 'DUPLICATE_COMPANY')
    }
    imported = [company, ...imported]
    return company
  },
}
