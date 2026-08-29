<script setup lang="ts">
import { reactive, ref } from 'vue'
import type { ApiKey, Destination } from '../types'
import ConfirmModal from '../components/ConfirmModal.vue'
defineProps<{ keys: readonly ApiKey[]; destinations: readonly Destination[] }>()
const emit = defineEmits<{ create: [input: { name: string; destination_ids: string[] }]; revoke: [id: string] }>()
const form = reactive({ name: '', destination_ids: [] as string[] })
const pendingRevoke = ref<string | null>(null)
const copiedPrefix = ref<string | null>(null)
const copyError = ref('')
function submit() { emit('create', { name: form.name, destination_ids: [...form.destination_ids] }); form.name = ''; form.destination_ids = [] }
function revoke(id: string) { pendingRevoke.value = id }
function confirmRevoke() { if (pendingRevoke.value) emit('revoke', pendingRevoke.value); pendingRevoke.value = null }
async function copyPrefix(key: ApiKey) {
  copyError.value = ''
  try {
    await navigator.clipboard.writeText(key.prefix)
    copiedPrefix.value = key.id
  } catch {
    copyError.value = 'Clipboard access failed. Select the prefix and copy it manually.'
  }
}
</script>

<template>
  <section class="split">
    <form class="panel key-form" @submit.prevent="submit">
      <h3>Create API key</h3>
      <label>Name<input v-model="form.name" required maxlength="100" placeholder="Order producer"></label>
      <fieldset>
        <legend>Destination scope</legend>
        <label v-for="destination in destinations" :key="destination.id" class="check"><input v-model="form.destination_ids" type="checkbox" :value="destination.id">{{ destination.name }}</label>
        <p v-if="!destinations.length" class="muted">No destinations exist. An empty scope creates an unrestricted key.</p>
      </fieldset>
      <button>Create API key</button>
      <p class="muted">Keys use the <code>pj_relay_</code> prefix. The full key is shown once after creation. Leaving every destination unchecked grants access to all destinations.</p>
    </form>
    <div class="cards">
      <p v-if="copyError" class="error" role="alert">{{ copyError }}</p>
      <article v-for="key in keys" :key="key.id" class="key-card">
        <div>
          <h3>{{ key.name }}</h3>
          <code>{{ key.prefix }}…</code>
          <p>{{ key.unrestricted ? 'All destinations' : `${key.destination_ids.length} destinations` }} · Last used {{ key.last_used_at ? new Date(key.last_used_at).toLocaleString() : 'never' }}</p>
        </div>
        <div class="key-actions">
          <button class="secondary" :aria-label="`Copy visible prefix for ${key.name}`" @click="copyPrefix(key)">{{ copiedPrefix === key.id ? 'Copied' : 'Copy prefix' }}</button>
          <button class="danger" @click="revoke(key.id)">Revoke</button>
        </div>
      </article>
      <p v-if="!keys.length" class="empty panel">No producer API keys.</p>
      <p class="copy-status" aria-live="polite">{{ copiedPrefix ? 'API key prefix copied to clipboard.' : '' }}</p>
    </div>
  </section>
  <ConfirmModal :open="pendingRevoke !== null" title="Revoke API key" message="Producers using this key will stop immediately. This cannot be undone." confirm-label="Revoke key" danger @confirm="confirmRevoke" @cancel="pendingRevoke = null" />
</template>
<style scoped>.split{display:grid;grid-template-columns:minmax(290px,.7fr) minmax(0,1.3fr);gap:18px}.key-form{display:grid;gap:16px;align-self:start;padding:20px}.key-form fieldset{display:grid;gap:8px;padding:12px;border:1px solid var(--border);border-radius:9px}.key-form legend{padding:0 6px;color:var(--muted);font-size:12px}.check{display:flex;grid-template-columns:none;align-items:center;gap:8px}.check input{width:auto}.cards{display:grid;gap:10px}.key-card{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:18px;border:1px solid var(--border);border-radius:12px;background:var(--surface)}.key-card p{margin:7px 0 0;color:var(--muted);font-size:12px}.key-actions{display:flex;gap:8px;align-items:center;flex-shrink:0}.copy-status{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}@media(max-width:860px){.split{grid-template-columns:1fr}}@media(max-width:560px){.key-card{align-items:stretch;flex-direction:column}.key-actions{justify-content:flex-end}}</style>
