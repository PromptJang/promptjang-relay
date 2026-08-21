import { readonly, ref, shallowRef } from 'vue'
import type { ApiKey, DeliveryAttempt, Destination, RelayEvent, SystemStatus } from '../types'

type Request = <T>(path: string, init?: RequestInit) => Promise<T>

export function useRelayData(request: Request) {
  const destinations = ref<Destination[]>([])
  const events = ref<RelayEvent[]>([])
  const keys = ref<ApiKey[]>([])
  const system = shallowRef<SystemStatus>()
  const selectedEvent = shallowRef<{ event: RelayEvent; attempts: DeliveryAttempt[] }>()
  const loading = shallowRef(false)
  const error = shallowRef('')
  const secret = shallowRef('')

  async function run<T>(operation: () => Promise<T>): Promise<T | undefined> {
    error.value = ''
    try { return await operation() }
    catch (cause) { error.value = cause instanceof Error ? cause.message : 'Request failed' }
  }

  async function refresh() {
    loading.value = true
    await run(async () => {
      const [destinationData, eventData, keyData, systemData] = await Promise.all([
        request<{ destinations: Destination[] }>('/api/v1/destinations'),
        request<{ events: RelayEvent[] }>('/api/v1/events'),
        request<{ keys: ApiKey[] }>('/api/v1/keys'),
        request<SystemStatus>('/api/v1/system'),
      ])
      destinations.value = destinationData.destinations
      events.value = eventData.events
      keys.value = keyData.keys
      system.value = systemData
    })
    loading.value = false
  }

  async function createDestination(input: { name: string; url: string }) {
    const data = await run(() => request<{ secret: string }>('/api/v1/destinations', { method: 'POST', body: JSON.stringify(input) }))
    if (data) { secret.value = data.secret; await refresh() }
  }
  async function updateDestination(destination: Destination, enabled: boolean) {
    await run(() => request(`/api/v1/destinations/${destination.id}`, { method: 'PATCH', body: JSON.stringify({ name: destination.name, url: destination.url, enabled }) }))
    await refresh()
  }
  async function deleteDestination(id: string) {
    await run(() => request(`/api/v1/destinations/${id}`, { method: 'DELETE' }))
    await refresh()
  }
  async function rotateSecret(id: string) {
    const data = await run(() => request<{ secret: string }>(`/api/v1/destinations/${id}/signing-secret/rotate`, { method: 'POST' }))
    if (data) secret.value = data.secret
  }
  async function testDestination(id: string) {
    await run(() => request(`/api/v1/destinations/${id}/test`, { method: 'POST' }))
    await refresh()
  }
  async function finishRotation(id: string) {
    await run(() => request(`/api/v1/destinations/${id}/signing-secret/previous`, { method: 'DELETE' }))
    await refresh()
  }
  async function createKey(input: { name: string; destination_ids: string[] }) {
    const data = await run(() => request<{ key: string }>('/api/v1/keys', { method: 'POST', body: JSON.stringify(input) }))
    if (data) { secret.value = data.key; await refresh() }
  }
  async function revokeKey(id: string) {
    await run(() => request(`/api/v1/keys/${id}`, { method: 'DELETE' }))
    await refresh()
  }
  async function inspectEvent(id: string) {
    const data = await run(() => request<{ event: RelayEvent; attempts: DeliveryAttempt[] }>(`/api/v1/events/${id}`))
    if (data) selectedEvent.value = data
  }
  async function replayEvent(id: string) {
    await run(() => request(`/api/v1/events/${id}/replay`, { method: 'POST' }))
    await refresh()
  }

  return {
    destinations: readonly(destinations), events: readonly(events), keys: readonly(keys), system: readonly(system),
    selectedEvent: readonly(selectedEvent), loading: readonly(loading), error: readonly(error), secret: readonly(secret),
    refresh, createDestination, updateDestination, deleteDestination, rotateSecret, testDestination, finishRotation, createKey, revokeKey,
    inspectEvent, replayEvent, clearSelectedEvent: () => { selectedEvent.value = undefined }, clearSecret: () => { secret.value = '' },
  }
}
