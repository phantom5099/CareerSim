import { request } from '@/api/http'
import type { IndustriesResponse } from '@/api/types'

export const companiesApi = {
  industries(): Promise<IndustriesResponse> {
    return request<IndustriesResponse>('/companies/industries')
  },
}
