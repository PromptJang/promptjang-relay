<script setup lang="ts">
import StatusBadge from '../StatusBadge.vue'

defineProps<{
  enabled: boolean
  publicUrl: string
  transport: string
  sessionStore: string
}>()
</script>

<template>
  <article class="panel connection-card">
    <div class="connection-heading">
      <div>
        <p class="eyebrow">REMOTE MCP</p>
        <h3>Relay mailbox connection</h3>
      </div>
      <StatusBadge :status="enabled ? 'enabled' : 'disabled'" />
    </div>
    <dl>
      <div><dt>URL</dt><dd><code>{{ publicUrl }}</code></dd></div>
      <div><dt>Transport</dt><dd>{{ transport }}</dd></div>
      <div><dt>Authentication</dt><dd>Bearer API key</dd></div>
      <div><dt>Session recovery</dt><dd>{{ sessionStore }}</dd></div>
    </dl>
    <p class="muted">Use HTTPS outside loopback. Relay owns PostgreSQL; agents receive only this URL and an API key.</p>
  </article>
</template>

<style scoped>
.connection-card{padding:20px}.connection-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:16px}.connection-heading h3{margin-bottom:0}dl{display:grid;margin:12px 0}dl div{display:flex;justify-content:space-between;gap:18px;padding:10px 0;border-bottom:1px solid var(--border)}dt{color:var(--muted)}dd{margin:0;text-align:right}code{overflow-wrap:anywhere}.muted{margin:16px 0 0;font-size:13px}@media(max-width:560px){dl div{display:grid;gap:5px}dd{text-align:left}}
</style>
