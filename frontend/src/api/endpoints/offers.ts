import { apiFetch } from '@/api/http'

import type { Offer, OfferStatus } from '@/domain/types'

export interface OffersApi {
  list(): Promise<Offer[]>
  respond(id: string, status: OfferStatus): Promise<Offer>
}

interface OfferDto {
  id: string
  companyId: string
  jobId: string
  jobTitle: string
  resumeId: string
  resumeTitle: string
  baseSalary: number
  months: number
  bonus: string
  city: string
  issuedAt: string
  deadline: string
  status: OfferStatus
}

function toOffer(dto: OfferDto): Offer {
  return {
    id: dto.id,
    companyId: dto.companyId,
    jobTitle: dto.jobTitle,
    baseSalary: dto.baseSalary,
    months: dto.months,
    bonus: dto.bonus,
    city: dto.city,
    issuedAt: dto.issuedAt,
    deadline: dto.deadline,
    status: dto.status,
  }
}

export const offersApi: OffersApi = {
  async list(): Promise<Offer[]> {
    const payload = await apiFetch<OfferDto[]>('/api/v1/offers')
    return payload.map(toOffer)
  },

  async respond(id: string, status: OfferStatus): Promise<Offer> {
    const payload = await apiFetch<OfferDto>(`/api/v1/offers/${id}/respond`, {
      method: 'POST',
      body: JSON.stringify({ status }),
    })
    return toOffer(payload)
  },
}
