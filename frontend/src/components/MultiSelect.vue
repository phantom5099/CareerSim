<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    options: string[]
    label: string
    placeholder?: string
    buttonId?: string
  }>(),
  { placeholder: '全部', buttonId: undefined },
)

const selected = defineModel<string[]>({ required: true })

const open = ref(false)
const root = ref<HTMLElement | null>(null)

const summary = computed(() => {
  if (!selected.value.length) {
    return props.placeholder
  }
  if (selected.value.length === 1) {
    return selected.value[0]
  }
  return `已选 ${selected.value.length} 项`
})

function toggle(option: string) {
  selected.value = selected.value.includes(option)
    ? selected.value.filter((item) => item !== option)
    : [...selected.value, option]
}

function clear() {
  selected.value = []
}

function onPointerDown(event: PointerEvent) {
  if (open.value && root.value && !root.value.contains(event.target as Node)) {
    open.value = false
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    open.value = false
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onPointerDown)
  document.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onPointerDown)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div ref="root" class="relative">
    <button
      :id="buttonId"
      type="button"
      class="flex w-full items-center justify-between gap-2 border border-line bg-paper-light px-2.5 py-[0.45rem] text-left text-[0.72rem] leading-[1.45] text-ink transition hover:border-line-strong focus:border-ink focus:outline-none"
      :aria-label="label"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="open = !open"
    >
      <span class="truncate" :class="selected.length ? 'text-ink' : 'text-ink-faint'">
        {{ summary }}
      </span>
      <span class="flex shrink-0 items-center gap-1.5">
        <span v-if="selected.length" class="font-mono text-[0.58rem] text-vermilion">
          {{ selected.length }}
        </span>
        <svg
          viewBox="0 0 10 6"
          class="h-1.5 w-2.5 text-ink-faint transition-transform"
          :class="open ? 'rotate-180' : ''"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          aria-hidden="true"
        >
          <path d="M1 1l4 4 4-4" />
        </svg>
      </span>
    </button>

    <div
      v-if="open"
      class="absolute left-0 right-0 z-30 mt-1 min-w-full border border-line bg-paper-light shadow-[0_10px_24px_rgba(23,21,15,0.12)]"
    >
      <ul
        class="max-h-56 overflow-y-auto py-1"
        role="listbox"
        :aria-label="label"
        aria-multiselectable="true"
      >
        <li>
          <button
            type="button"
            role="option"
            :aria-selected="selected.length === 0"
            class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[0.7rem] text-ink-soft transition hover:bg-paper-deep/60"
            @click="clear"
          >
            <span class="flex h-3 w-3 items-center justify-center border border-line-strong">
              <svg
                v-if="selected.length === 0"
                viewBox="0 0 10 8"
                class="h-2 w-2"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                aria-hidden="true"
              >
                <path d="M1 4l2.5 2.5L9 1" />
              </svg>
            </span>
            {{ placeholder }}
          </button>
        </li>
        <li v-for="option in options" :key="option">
          <button
            type="button"
            role="option"
            :aria-selected="selected.includes(option)"
            class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[0.7rem] text-ink-soft transition hover:bg-paper-deep/60"
            @click="toggle(option)"
          >
            <span
              class="flex h-3 w-3 shrink-0 items-center justify-center border"
              :class="
                selected.includes(option)
                  ? 'border-ink bg-ink text-paper-light'
                  : 'border-line-strong'
              "
            >
              <svg
                v-if="selected.includes(option)"
                viewBox="0 0 10 8"
                class="h-2 w-2"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                aria-hidden="true"
              >
                <path d="M1 4l2.5 2.5L9 1" />
              </svg>
            </span>
            <span class="truncate">{{ option }}</span>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>
