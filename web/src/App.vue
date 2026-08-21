<script setup lang="ts">
import { onMounted, shallowRef } from 'vue'
import AppShell from './components/AppShell.vue'
import LoginView from './components/LoginView.vue'
import SecretNotice from './components/SecretNotice.vue'
import { useRelayApi } from './composables/useRelayApi'
import { useRelayData } from './composables/useRelayData'
import DestinationsView from './views/DestinationsView.vue'
import EventsView from './views/EventsView.vue'
import KeysView from './views/KeysView.vue'
import OverviewView from './views/OverviewView.vue'
import SystemView from './views/SystemView.vue'
import type { ViewName } from './types'

const view = shallowRef<ViewName>('overview')
const sessionError = shallowRef('')
const sessionLoading = shallowRef(false)
const { token, request, login, logout } = useRelayApi()
const relay = useRelayData(request)

async function signIn(email: string, password: string) {
  sessionLoading.value = true
  sessionError.value = ''
  try { await login(email, password); await relay.refresh() }
  catch (cause) { sessionError.value = cause instanceof Error ? cause.message : 'Login failed' }
  finally { sessionLoading.value = false }
}
async function signOut() { await logout(); view.value = 'overview' }
onMounted(() => { if (token.value) void relay.refresh() })
</script>

<template>
  <LoginView v-if="!token" :error="sessionError" :loading="sessionLoading" @login="signIn" />
  <AppShell v-else :view="view" :version="relay.system.value?.version" :telemetry="relay.system.value?.telemetry.enabled ?? false" @navigate="view=$event" @refresh="relay.refresh" @logout="signOut">
    <p v-if="relay.error.value" class="error banner" role="alert">{{ relay.error.value }}</p>
    <SecretNotice v-if="relay.secret.value" :secret="relay.secret.value" @dismiss="relay.clearSecret" />
    <OverviewView v-if="view==='overview'" :system="relay.system.value" :destinations="relay.destinations.value" :events="relay.events.value" @navigate="view=$event" />
    <DestinationsView v-else-if="view==='destinations'" :destinations="relay.destinations.value" @create="relay.createDestination" @toggle="relay.updateDestination" @test="relay.testDestination" @rotate="relay.rotateSecret" @finish-rotation="relay.finishRotation" @remove="relay.deleteDestination" />
    <EventsView v-else-if="view==='events'" :events="relay.events.value" :selected="relay.selectedEvent.value" @inspect="relay.inspectEvent" @replay="relay.replayEvent" @close="relay.clearSelectedEvent" />
    <KeysView v-else-if="view==='keys'" :keys="relay.keys.value" :destinations="relay.destinations.value" @create="relay.createKey" @revoke="relay.revokeKey" />
    <SystemView v-else :system="relay.system.value" />
  </AppShell>
</template>
