<script setup lang="ts">
withDefaults(
  defineProps<{
    text: string
    accent: string
    size?: 'sm' | 'md' | 'lg'
  }>(),
  { size: 'md' },
)

const sizeClass = {
  sm: 'h-8 w-8 text-[0.68rem]',
  md: 'h-12 w-12 text-base',
  lg: 'h-16 w-16 text-2xl',
} as const
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center justify-center border border-ink/10 font-display leading-none tracking-tight text-paper-light"
    :class="sizeClass[size]"
    :style="{ backgroundColor: accent }"
    aria-hidden="true"
  >
    {{ text }}
  </span>
</template>
