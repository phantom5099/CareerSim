import { mockOffersApi } from '@/mock/offers'

import type { Offer, OfferStatus } from '@/domain/types'

export interface OffersApi {
  list(): Promise<Offer[]>
  respond(id: string, status: OfferStatus): Promise<Offer>
}

// 后端就绪后把绑定换成 HTTP 实现即可，调用方不变。
export const offersApi: OffersApi = mockOffersApi
