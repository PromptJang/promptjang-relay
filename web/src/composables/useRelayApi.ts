import { readonly, shallowRef } from 'vue'

export function useRelayApi() {
  const token = shallowRef(sessionStorage.getItem('pj_session') ?? '')

  async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(path, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        ...(token.value ? { Authorization: `Bearer ${token.value}` } : {}),
        ...init.headers,
      },
    })
    const body = await response.json().catch(() => ({}))
    if (!response.ok) throw new Error(body.error ?? `HTTP ${response.status}`)
    return body as T
  }

  async function login(email: string, password: string) {
    const data = await request<{ token: string }>('/api/v1/session', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    })
    token.value = data.token
    sessionStorage.setItem('pj_session', data.token)
  }

  async function logout() {
    try {
      await request('/api/v1/session', { method: 'DELETE' })
    } finally {
      token.value = ''
      sessionStorage.removeItem('pj_session')
    }
  }

  return { token: readonly(token), request, login, logout }
}

