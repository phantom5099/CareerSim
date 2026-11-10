import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { offersApi } from '@/api/endpoints/offers'
import type { Offer, OfferStatus } from '@/domain/types'

export const useOffersStore = defineStore('offers', () => {
  const offers = ref<Offer[]>([])

  const pendingCount = computed(
    () => offers.value.filter((offer) => offer.status === 'pending').length,
  )
  const acceptedCount = computed(
    () => offers.value.filter((offer) => offer.status === 'accepted').length,
  )

  let initPromise: Promise<void> | null = null

  function init(): Promise<void> {
    if (!initPromise) {
      initPromise = offersApi.list().then((value) => {
        offers.value = value
      })
    }
    return initPromise
  }

  async function setStatus(id: string, status: OfferStatus): Promise<void> {
    const updated = await offersApi.respond(id, status)
    offers.value = offers.value.map((offer) => (offer.id === id ? updated : offer))
  }

  return { offers, pendingCount, acceptedCount, init, setStatus }
})
