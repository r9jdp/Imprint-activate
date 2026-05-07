import { useAgentStore } from '../store/agent'
import { ApiKeyGate } from './ApiKeyGate'
import { InputBar } from './InputBar'
import { EventFeed } from './EventFeed'

export function Overlay() {
  const apiKey = useAgentStore((s) => s.apiKey)

  return (
    <div className="w-screen h-screen flex items-center justify-center bg-transparent" data-tauri-drag-region>
      <div className="w-[640px] bg-[#111111] rounded-2xl border border-white/[0.08] shadow-2xl flex flex-col overflow-hidden max-h-[500px]" data-tauri-drag-region>
        {/* Removed thin drag strip since the whole background is now acting as one */}

        {apiKey ? (
          <div className="flex flex-col flex-1 overflow-hidden">
            <InputBar />
            <EventFeed />
          </div>
        ) : (
          <ApiKeyGate />
        )}
      </div>
    </div>
  )
}
