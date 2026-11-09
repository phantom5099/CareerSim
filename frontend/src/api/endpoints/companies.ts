import { mockCompaniesApi } from '@/mock/companies'

import type { Company } from '@/domain/types'

import type { IndustriesResponse } from '../types'

export interface CompaniesApi {
  list(): Promise<Company[]>
  industries(): Promise<IndustriesResponse>
  create(company: Company): Promise<Company>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const companiesApi: CompaniesApi = mockCompaniesApi
