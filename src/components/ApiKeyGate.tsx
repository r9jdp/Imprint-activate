import { useState } from 'react'
import { useAgentStore } from '../store/agent'

export function ApiKeyGate() {
  const setApiKey = useAgentStore((s) => s.setApiKey)
  const [key, setKey] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (key.trim()) {
      setApiKey(key.trim())
    }
  }

  return (
    <div className="flex flex-col items-center justify-center px-10 py-12 gap-4">
      <h1 className="text-[#f5f5f5] text-2xl font-semibold tracking-tight">
        Welcome to Imprint
      </h1>
      <p className="text-[#555555] text-sm text-center max-w-xs">
        Enter your Gemini API key to get started. It's stored locally only.
      </p>
      <form onSubmit={handleSubmit} className="w-full max-w-xs flex flex-col gap-3 mt-2">
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="AIza..."
          className="w-full px-4 py-2.5 bg-[#1a1a1a] border border-white/[0.08] rounded-lg text-[#f5f5f5] text-sm placeholder:text-[#555555] outline-none focus:border-[#6366f1]/50 transition-colors"
        />
        <button
          type="submit"
          className="w-full py-2.5 bg-[#6366f1] hover:bg-[#5558e6] text-white text-sm font-medium rounded-lg transition-colors cursor-pointer"
        >
          Save & Continue
        </button>
      </form>
    </div>
  )
}
