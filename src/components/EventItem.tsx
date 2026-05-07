import { Check } from 'lucide-react'
import type { AgentEvent } from '../store/agent'
import { useThemeStore } from '../theme'

interface EventItemProps {
  event: AgentEvent
}

export function EventItem({ event }: EventItemProps) {
  const expandedUI = useThemeStore((s) => s.expandedUI)

  if (event.kind === 'done') return null

  if (event.kind === 'tool_call') {
    // Parse "tool_name: {args}" format
    const colonIdx = event.content.indexOf(':')
    const toolName = colonIdx > -1 ? event.content.substring(0, colonIdx) : event.content
    const args = colonIdx > -1 ? event.content.substring(colonIdx + 1).trim() : ''

    return (
      <div className="flex items-start gap-2 py-1">
        <span className="shrink-0 px-1.5 py-0.5 bg-[#f59e0b]/15 text-[#f59e0b] text-[10px] font-semibold rounded uppercase tracking-wide mt-0.5">
          TOOL
        </span>
        <div className="flex flex-col gap-0.5 min-w-0">
          <span className="text-[#f5f5f5] text-sm font-medium">{toolName}</span>
          {args && (
            <span className={`text-[#555555] text-xs ${expandedUI ? 'break-all whitespace-pre-wrap' : 'truncate max-w-[480px]'}`}>{args}</span>
          )}
        </div>
      </div>
    )
  }

  if (event.kind === 'tool_result') {
    const truncated =
      !expandedUI && event.content.length > 120
        ? event.content.substring(0, 120) + '…'
        : event.content

    return (
      <div className="flex items-start gap-2 py-1">
        <Check className="shrink-0 w-3.5 h-3.5 text-[#22c55e] mt-0.5" />
        <span className={`text-[#555555] text-xs leading-relaxed ${expandedUI ? 'break-all whitespace-pre-wrap' : ''}`}>{truncated}</span>
      </div>
    )
  }

  if (event.kind === 'message') {
    return (
      <div className="py-1.5">
        <p className="text-[#f5f5f5] text-sm leading-relaxed">{event.content}</p>
      </div>
    )
  }

  if (event.kind === 'error') {
    return (
      <div className="py-1.5 px-3 bg-[#ef4444]/10 rounded-lg">
        <p className="text-[#ef4444] text-sm">{event.content}</p>
      </div>
    )
  }

  return null
}
