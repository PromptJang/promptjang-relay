<script setup lang="ts">
import McpClientRecipe from '../components/integrations/McpClientRecipe.vue'
import McpConnectionCard from '../components/integrations/McpConnectionCard.vue'
import { mcpClients, useMcpRecipes } from '../composables/useMcpRecipes'
import type { ApiKey, SystemStatus } from '../types'

const props = defineProps<{
  keys: readonly ApiKey[]
  system?: SystemStatus
  revealKey: (id: string) => Promise<string>
}>()
const emit = defineEmits<{ navigate: [view: 'keys'] }>()
const recipes = useMcpRecipes({
  keys: () => props.keys,
  publicUrl: () => props.system?.mcp.public_url ?? '',
  revealKey: props.revealKey,
})
</script>

<template>
  <section class="integrations-grid">
    <McpConnectionCard
      :enabled="system?.mcp.enabled ?? false"
      :public-url="system?.mcp.public_url ?? 'MCP URL unavailable'"
      :transport="system?.mcp.transport ?? 'streamable-http'"
      :session-store="system?.mcp.session_store ?? 'postgresql'"
    />
    <article class="panel key-card">
      <p class="eyebrow">CREDENTIAL</p>
      <h3>Choose an unrestricted API key</h3>
      <label v-if="recipes.eligibleKeys.value.length">API key
        <select v-model="recipes.selectedKeyId.value">
          <option v-for="key in recipes.eligibleKeys.value" :key="key.id" :value="key.id">{{ key.name }} · {{ key.prefix }}…</option>
        </select>
      </label>
      <div v-else class="empty-key">
        <p>No retrievable unrestricted key is available.</p>
        <button class="secondary" @click="emit('navigate', 'keys')">Create API key</button>
      </div>
      <p class="muted">The full key is decrypted only when you copy a configured command. Relay does not display it on this screen.</p>
    </article>
  </section>

  <section class="recipes">
    <div class="section-heading"><div><p class="eyebrow">CLIENT RECIPES</p><h2>Connect a CLI agent</h2></div></div>
    <p v-if="recipes.error.value" class="error banner" role="alert">{{ recipes.error.value }}</p>
    <McpClientRecipe
      v-for="client in mcpClients"
      :key="client.id"
      :client="client.id"
      :name="client.name"
      :secret-storage="client.storesSecret"
      :copying="recipes.copyingClient.value === client.id"
      :copied="recipes.copiedClient.value === client.id"
      :disabled="!system?.mcp.enabled || !recipes.selectedKeyId.value"
      @copy="recipes.copy"
    />
    <p class="copy-status" aria-live="polite">{{ recipes.copiedClient.value ? `${recipes.copiedClient.value} MCP setup copied.` : '' }}</p>
  </section>
</template>

<style scoped>
.integrations-grid{display:grid;grid-template-columns:minmax(0,1.2fr) minmax(280px,.8fr);gap:18px}.key-card{padding:20px}.key-card h3{margin-bottom:18px}.key-card .muted{margin:14px 0 0;font-size:12px}.empty-key{display:flex;align-items:center;justify-content:space-between;gap:12px}.empty-key p{margin:0;color:var(--amber)}.recipes{display:grid;gap:10px;margin-top:24px}.section-heading h2{margin-bottom:0}.copy-status{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}@media(max-width:860px){.integrations-grid{grid-template-columns:1fr}}
</style>
