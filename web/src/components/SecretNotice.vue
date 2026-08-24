<script setup lang="ts">
const props = defineProps<{ secret: string }>()
const emit = defineEmits<{ dismiss: [] }>()
async function copy() { await navigator.clipboard.writeText(props.secret) }
</script>

<template>
  <section class="secret-notice" role="status">
    <div><strong>Copy this secret now</strong><p>It will not be shown again. Use a Standard Webhooks v1 verifier at the receiver.</p></div>
    <code>{{ secret }}</code><button @click="copy">Copy</button><button class="secondary" @click="emit('dismiss')">Dismiss</button>
    <a href="https://github.com/standard-webhooks/standard-webhooks" target="_blank" rel="noreferrer">Verification guide</a>
  </section>
</template>

<style scoped>
.secret-notice{display:flex;align-items:center;gap:12px;padding:14px;margin-bottom:20px;border:1px solid color-mix(in srgb,var(--green) 42%,transparent);border-radius:12px;background:color-mix(in srgb,var(--green) 8%,var(--surface))}.secret-notice p{margin:3px 0 0;color:var(--muted);font-size:12px}.secret-notice code{min-width:0;overflow:hidden;text-overflow:ellipsis}@media(max-width:760px){.secret-notice{align-items:stretch;flex-direction:column}}
</style>
