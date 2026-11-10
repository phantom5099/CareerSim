import { mockCompaniesApi } from '@/mock/companies'

import type { Company } from '@/domain/types'

import type { IndustriesResponse } from '../types'

export type CompanyTemplateResult =
  | { ok: true; company: Company }
  | { ok: false; errors: string[] }

export interface CompaniesApi {
  list(): Promise<Company[]>
  industries(): Promise<IndustriesResponse>
  create(company: Company): Promise<Company>
  /** 示例模板 JSON 文本，由服务端提供，保证字段说明与解析规则一致。 */
  templateSample(): Promise<string>
  /** 解析并校验模板（dry-run），不落库。 */
  validateImport(raw: unknown): Promise<CompanyTemplateResult>
  /** 由一句话描述生成模板草稿；生成结果仍需 `validateImport` 校验。 */
  generateTemplate(prompt: string): Promise<Record<string, unknown>>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const companiesApi: CompaniesApi = mockCompaniesApi
