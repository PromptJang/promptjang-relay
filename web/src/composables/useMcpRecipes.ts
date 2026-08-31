import { computed, readonly, shallowRef, watch } from 'vue'
import type { ApiKey } from '../types'

export type McpClientId = 'codex' | 'claude-code' | 'opencode' | 'qwen'

export interface McpClientRecipe {
  id: McpClientId
  name: string
  storesSecret: string
}

export const mcpClients: readonly McpClientRecipe[] = [
  { id: 'codex', name: 'Codex', storesSecret: 'Reads the key from PJ_RELAY_API_KEY in the Codex process environment.' },
  { id: 'claude-code', name: 'Claude Code', storesSecret: 'Writes the bearer header to the Claude Code MCP configuration.' },
  { id: 'opencode', name: 'OpenCode', storesSecret: 'Writes the bearer header to the OpenCode MCP configuration.' },
  { id: 'qwen', name: 'Qwen Code', storesSecret: 'Writes the bearer header to the Qwen MCP configuration.' },
]

function commandFor(client: McpClientId, url: string, key: string): string {
  switch (client) {
    case 'codex':
      return `export PJ_RELAY_API_KEY='${key}'\ncodex mcp add promptjang-relay --url '${url}' --bearer-token-env-var PJ_RELAY_API_KEY`
    case 'claude-code':
      return `claude mcp add --transport http promptjang-relay '${url}' --header 'Authorization: Bearer ${key}'`
    case 'opencode':
      return `opencode mcp add promptjang-relay --url '${url}' --header 'Authorization=Bearer ${key}'`
    case 'qwen':
      return `qwen mcp add promptjang-relay '${url}' -t http -H 'Authorization: Bearer ${key}'`
  }
}

export function useMcpRecipes(options: {
  keys: () => readonly ApiKey[]
  publicUrl: () => string
  revealKey: (id: string) => Promise<string>
}) {
  const selectedKeyId = shallowRef('')
  const copyingClient = shallowRef<McpClientId | null>(null)
  const copiedClient = shallowRef<McpClientId | null>(null)
  const error = shallowRef('')
  const eligibleKeys = computed(() => options.keys().filter(key => key.unrestricted && key.retrievable))

  watch(eligibleKeys, keys => {
    if (!keys.some(key => key.id === selectedKeyId.value)) selectedKeyId.value = keys[0]?.id ?? ''
  }, { immediate: true })

  async function copy(client: McpClientId) {
    const key = eligibleKeys.value.find(candidate => candidate.id === selectedKeyId.value)
    if (!key) {
      error.value = 'Create a retrievable unrestricted API key before installing MCP.'
      return
    }
    copyingClient.value = client
    copiedClient.value = null
    error.value = ''
    try {
      const secret = await options.revealKey(key.id)
      await navigator.clipboard.writeText(commandFor(client, options.publicUrl(), secret))
      copiedClient.value = client
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : 'The configured MCP command could not be copied.'
    } finally {
      copyingClient.value = null
    }
  }

  return {
    eligibleKeys,
    selectedKeyId,
    copyingClient: readonly(copyingClient),
    copiedClient: readonly(copiedClient),
    error: readonly(error),
    copy,
  }
}
