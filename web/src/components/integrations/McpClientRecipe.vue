<script setup lang="ts">
import type { McpClientId } from '../../composables/useMcpRecipes'

defineProps<{
  client: McpClientId
  name: string
  secretStorage: string
  copying: boolean
  copied: boolean
  disabled: boolean
}>()
const emit = defineEmits<{ copy: [client: McpClientId] }>()
</script>

<template>
  <article class="panel recipe-card">
    <div>
      <h3>{{ name }}</h3>
      <p>{{ secretStorage }}</p>
    </div>
    <button :disabled="disabled || copying" @click="emit('copy', client)">
      {{ copying ? 'Preparing…' : copied ? 'Copied' : 'Copy configured command' }}
    </button>
  </article>
</template>

<style scoped>
.recipe-card{display:flex;align-items:center;justify-content:space-between;gap:18px;padding:18px}.recipe-card h3{margin-bottom:5px}.recipe-card p{margin:0;color:var(--muted);font-size:12px;max-width:620px}.recipe-card button{flex-shrink:0}@media(max-width:620px){.recipe-card{align-items:stretch;flex-direction:column}}
</style>
