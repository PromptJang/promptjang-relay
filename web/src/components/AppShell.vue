<script setup lang="ts">
import type { ViewName } from '../types'
defineProps<{ view: ViewName; version?: string; telemetry: boolean }>()
const emit = defineEmits<{ navigate: [view: ViewName]; refresh: []; logout: [] }>()
const items: { id: ViewName; label: string }[] = [
  { id: 'overview', label: 'Overview' }, { id: 'destinations', label: 'Destinations' },
  { id: 'events', label: 'Events' }, { id: 'keys', label: 'API keys' }, { id: 'system', label: 'System' },
]
</script>

<template>
  <div class="app-shell"><aside class="sidebar"><div><p class="eyebrow">RELAY · SELF-HOSTED</p><h2>PromptJang <span>Relay</span></h2></div><nav aria-label="Primary"><button v-for="item in items" :key="item.id" :class="{ active: view === item.id }" @click="emit('navigate', item.id)">{{ item.label }}</button></nav><div class="sidebar-status"><span :class="{ connected: telemetry }"></span>OTel {{ telemetry ? 'connected' : 'off' }} · v{{ version ?? '0.2.0' }}</div><button class="secondary sign-out" @click="emit('logout')">Sign out</button></aside><main class="content"><header class="page-header"><div><p class="eyebrow">LOCAL WORKSPACE</p><h1>{{ items.find(item => item.id === view)?.label }}</h1></div><button class="secondary" @click="emit('refresh')">Refresh</button></header><slot /></main></div>
</template>

<style scoped>
.app-shell{min-height:100vh;display:grid;grid-template-columns:240px minmax(0,1fr)}.sidebar{position:sticky;top:0;height:100vh;display:flex;flex-direction:column;padding:24px 16px;border-right:1px solid var(--border);background:var(--surface)}.sidebar h2 span{color:var(--green)}nav{display:grid;gap:5px;margin-top:30px}nav button{border-color:transparent;color:var(--muted);background:transparent;text-align:left}nav button.active{color:var(--green);background:color-mix(in srgb,var(--green) 9%,transparent)}.sidebar-status{margin-top:auto;padding:10px;color:var(--muted);font-size:11px}.sidebar-status span{display:inline-block;width:7px;height:7px;margin-right:6px;border-radius:50%;background:var(--neutral)}.sidebar-status span.connected{background:var(--green)}.sign-out{margin-top:8px}.content{width:100%;max-width:1500px;padding:32px;margin:auto}.page-header{display:flex;justify-content:space-between;align-items:center;margin-bottom:24px}@media(max-width:760px){.app-shell{grid-template-columns:1fr}.sidebar{position:static;height:auto;padding:12px}.sidebar>div,.sidebar-status,.sign-out{display:none}nav{grid-template-columns:repeat(5,1fr);margin:0;overflow-x:auto}nav button{padding:9px 8px;text-align:center;font-size:12px}.content{padding:20px 14px}}
</style>

