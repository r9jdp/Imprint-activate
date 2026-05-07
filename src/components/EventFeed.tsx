import { useEffect, useRef } from 'react'
import { useAgentStore } from '../store/agent'
import { EventItem } from './EventItem'

export function EventFeed() {
  const events = useAgentStore((s) => s.events)
  const status = useAgentStore((s) => s.status)
  const reset = useAgentStore((s) => s.reset)
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [events])

  if (status === 'idle') return null

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2 max-h-72">
        {events.map((event, i) => (
          <EventItem key={i} event={event} />
        ))}

        {status === 'running' && (
          <div className="flex items-center gap-2 py-1">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#6366f1] opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-[#6366f1]" />
            </span>
            <span className="text-[#555555] text-xs">Processing…</span>
          </div>
        )}

        <div ref={bottomRef} />
      </div>

      {(status === 'done' || status === 'error') && (
        <div className="px-4 py-3 border-t border-white/[0.08] shrink-0">
          <button
            onClick={reset}
            className="w-full py-2 bg-[#1a1a1a] hover:bg-[#222222] border border-white/[0.08] text-[#f5f5f5] text-sm rounded-lg transition-colors cursor-pointer"
          >
            Run Another
          </button>
        </div>
      )}
    </div>
  )
}
