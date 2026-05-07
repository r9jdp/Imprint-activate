import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

export interface AgentEvent {
  kind: 'tool_call' | 'tool_result' | 'message' | 'done' | 'error'
  content: string
  timestamp: number
}

interface AgentStore {
  apiKey: string | null
  setApiKey: (key: string) => void

  status: 'idle' | 'running' | 'done' | 'error'
  events: AgentEvent[]
  currentPrompt: string

  runAgent: (prompt: string) => Promise<void>
  reset: () => void
}

export const useAgentStore = create<AgentStore>((set, get) => ({
  apiKey: localStorage.getItem('imprint_api_key'),
  setApiKey: (key) => {
    localStorage.setItem('imprint_api_key', key)
    set({ apiKey: key })
  },

  status: 'idle',
  events: [],
  currentPrompt: '',

  runAgent: async (prompt) => {
    const { apiKey } = get()
    if (!apiKey) return

    set({ status: 'running', events: [], currentPrompt: prompt })

    let unlisten: UnlistenFn | null = null

    unlisten = await listen<AgentEvent>('agent_event', (e) => {
      const event = { ...e.payload, timestamp: Date.now() }
      set((s) => ({ events: [...s.events, event] }))
      if (event.kind === 'done') {
        set({ status: 'done' })
        unlisten?.()
      }
      if (event.kind === 'error') {
        set({ status: 'error' })
        unlisten?.()
      }
    })

    try {
      await invoke('run_agent_command', { prompt, apiKey })
    } catch (err: any) {
      set((s) => ({
        status: 'error',
        events: [...s.events, { kind: 'error' as const, content: String(err), timestamp: Date.now() }]
      }))
      unlisten?.()
    }
  },

  reset: () => set({ status: 'idle', events: [], currentPrompt: '' }),
}))
