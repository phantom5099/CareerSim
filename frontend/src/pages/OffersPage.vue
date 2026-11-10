<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import CompanyMark from '@/components/CompanyMark.vue'
import { formatDate, formatDateLong } from '@/domain/types'
import type { Offer, OfferStatus } from '@/domain/types'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'
import { useOffersStore } from '@/stores/offers'

const offersStore = useOffersStore()
const catalog = useCompanyCatalogStore()

const statusLabel: Record<OfferStatus, string> = {
  pending: '待答复',
  accepted: '已接受',
  declined: '已回绝',
}

const statusTone: Record<OfferStatus, string> = {
  pending: 'border-vermilion/50 text-vermilion',
  accepted: 'border-moss/60 text-moss',
  declined: 'border-line text-ink-faint',
}

const rank: Record<OfferStatus, number> = { pending: 0, accepted: 1, declined: 2 }

const cards = computed(() =>
  [...offersStore.offers]
    .sort((a, b) => rank[a.status] - rank[b.status] || a.deadline.localeCompare(b.deadline))
    .map((offer) => ({ offer, company: catalog.findCompany(offer.companyId) })),
)

const maxSalary = computed(() =>
  offersStore.offers.length
    ? Math.max(...offersStore.offers.map((offer) => offer.baseSalary))
    : 0,
)

function annual(offer: Offer): string {
  const total = (offer.baseSalary * offer.months) / 10000
  return `${Number.isInteger(total) ? total : total.toFixed(1)} 万`
}

async function respond(offer: Offer, status: OfferStatus) {
  await offersStore.setStatus(offer.id, status)
}

function deadlinePassed(offer: Offer): boolean {
  return offer.status === 'pending' && new Date(offer.deadline).getTime() < Date.now()
}
</script>

<template>
  <div>
    <h1 class="sr-only">Offer 清单</h1>
    <section class="mt-1 grid grid-cols-3 border-y border-line">
      <div class="px-4 py-5">
        <p class="text-[0.66rem] text-ink-faint">待答复</p>
        <p class="mt-2 font-display text-4xl leading-none text-vermilion">
          {{ offersStore.pendingCount }}
        </p>
      </div>
      <div class="border-l border-line px-4 py-5">
        <p class="text-[0.66rem] text-ink-faint">最高月薪</p>
        <p class="mt-2 font-display text-4xl leading-none">
          {{ maxSalary ? `${maxSalary / 1000}K` : '—' }}
        </p>
      </div>
      <div class="border-l border-line px-4 py-5">
        <p class="text-[0.66rem] text-ink-faint">已接受</p>
        <p class="mt-2 font-display text-4xl leading-none text-moss">
          {{ offersStore.acceptedCount }}
        </p>
      </div>
    </section>

    <div
      v-if="!cards.length"
      class="mt-8 border border-dashed border-line-strong px-6 py-12 text-center"
    >
      <p class="font-display text-2xl">还没有 Offer</p>
      <p class="mt-2 text-sm text-ink-soft">继续推进面试进度，Offer 会在这里出现。</p>
      <RouterLink :to="{ name: 'applications' }" class="btn-line mt-6">查看面试进度</RouterLink>
    </div>

    <template v-else>
      <article
        v-for="card in cards"
        :key="card.offer.id"
        class="grid gap-5 border-b border-line pt-7 pb-5 lg:grid-cols-[minmax(0,1fr)_17rem] lg:items-start"
      >
        <div class="min-w-0">
          <div class="flex items-start gap-3">
            <CompanyMark
              v-if="card.company"
              :text="card.company.logoText"
              :accent="card.company.accent"
              size="md"
            />
            <div class="min-w-0">
              <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <RouterLink
                  v-if="card.company"
                  :to="{ name: 'companyDetail', params: { id: card.company.id } }"
                  class="font-display text-2xl leading-tight transition hover:text-vermilion"
                >
                  {{ card.company.name }}
                </RouterLink>
                <span v-else class="font-display text-2xl leading-tight">已移除的公司</span>
                <span class="text-[0.7rem] text-ink-soft">
                  {{ card.offer.jobTitle }}
                </span>
              </div>
              <p class="mt-1 text-[0.66rem] text-ink-faint">
                {{ card.offer.city }} · 发出于 {{ formatDate(card.offer.issuedAt) }} · 答复截止
                {{ formatDateLong(card.offer.deadline) }}
                <span v-if="deadlinePassed(card.offer)" class="text-vermilion">· 已过答复期</span>
              </p>
            </div>
          </div>

          <dl class="mt-5 grid grid-cols-2 gap-x-6 gap-y-4 border-t border-line pt-4 sm:grid-cols-4">
            <div>
              <dt class="text-[0.62rem] text-ink-faint">月薪</dt>
              <dd class="mt-1 font-display text-2xl leading-none">
                {{ card.offer.baseSalary / 1000 }}K
              </dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">薪数</dt>
              <dd class="mt-1 font-display text-2xl leading-none">{{ card.offer.months }} 薪</dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">年薪约</dt>
              <dd class="mt-1 font-display text-2xl leading-none text-vermilion">
                {{ annual(card.offer) }}
              </dd>
            </div>
            <div>
              <dt class="text-[0.62rem] text-ink-faint">其他</dt>
              <dd class="mt-1 text-[0.74rem] leading-5">{{ card.offer.bonus }}</dd>
            </div>
          </dl>
        </div>

        <aside class="border border-line p-3.5">
          <div class="flex items-center justify-between gap-2">
            <p class="text-[0.64rem] text-ink-faint">答复状态</p>
            <span
              class="border px-1.5 py-0.5 text-[0.6rem]"
              :class="statusTone[card.offer.status]"
            >
              {{ statusLabel[card.offer.status] }}
            </span>
          </div>

          <div v-if="card.offer.status === 'pending'" class="mt-4 flex flex-col gap-2">
            <button type="button" class="btn-solid w-full py-1.5 text-[0.7rem]" @click="respond(card.offer, 'accepted')">
              接受这份 Offer
            </button>
            <button type="button" class="btn-line w-full py-1.5 text-[0.7rem]" @click="respond(card.offer, 'declined')">
              回绝
            </button>
          </div>

          <div v-else class="mt-4 flex flex-col items-center gap-3 py-2">
            <span
              class="stamp"
              :class="card.offer.status === 'accepted' ? 'text-moss' : 'text-ink-faint'"
            >
              {{ statusLabel[card.offer.status] }}
            </span>
            <button
              type="button"
              class="border-b border-line-strong pb-0.5 text-[0.68rem] text-ink-soft transition hover:border-vermilion hover:text-vermilion"
              @click="respond(card.offer, 'pending')"
            >
              撤销答复
            </button>
          </div>
        </aside>
      </article>
    </template>
  </div>
</template>
