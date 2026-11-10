import { ApiError } from '@/api/error'
import type { OffersApi } from '@/api/endpoints/offers'
import type { Offer, OfferStatus } from '@/domain/types'

import { seedOffers } from './data/offers'

// 后端 Offer 接口尚未接入：内存里维护答复状态。
let offers: Offer[] = structuredClone(seedOffers)

export const mockOffersApi: OffersApi = {
  async list(): Promise<Offer[]> {
    return structuredClone(offers)
  },

  async respond(id: string, status: OfferStatus): Promise<Offer> {
    const offer = offers.find((item) => item.id === id)
    if (!offer) {
      throw new ApiError('Offer 不存在', 404, 'OFFER_NOT_FOUND')
    }
    offer.status = status
    return structuredClone(offer)
  },
}
