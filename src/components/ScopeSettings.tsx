import { useState, useEffect } from 'react'
import { ShieldCheck, FolderOpen, Plus, X } from '@phosphor-icons/react'
import { invoke } from '@tauri-apps/api/core'
import { useColors } from '../theme'
import type { ScopeConfig } from '../types'

function RowToggle({
  checked,
  onChange,
  colors,
  label,
}: {
  checked: boolean
  onChange: (next: boolean) => void
  colors: ReturnType<typeof useColors>
  label: string
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={checked}
      onClick={() => onChange(!checked)}
      className="relative w-9 h-5 rounded-full transition-colors"
      style={{
        background: checked ? colors.accent : colors.surfaceSecondary,
        border: `1px solid ${checked ? colors.accent : colors.containerBorder}`,
      }}
    >
      <span
        className="absolute top-1/2 -translate-y-1/2 w-4 h-4 rounded-full transition-all"
        style={{
          left: checked ? 18 : 2,
          background: '#fff',
        }}
      />
    </button>
  )
}

export function ScopeSettings() {
  const colors = useColors()
  const [config, setConfig] = useState<ScopeConfig | null>(null)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    invoke<ScopeConfig>('get_scope_config')
      .then(setConfig)
      .catch(() => {})
  }, [])

  const saveConfig = async (newConfig: ScopeConfig) => {
    setConfig(newConfig)
    setSaving(true)
    try {
      await invoke('set_scope_config', { config: newConfig })
    } catch (e) {
      console.error('Failed to save scope config:', e)
    } finally {
      setSaving(false)
    }
  }

  const addPath = async () => {
    if (!config) return
    try {
      const dir = await invoke<string | null>('select_directory_command')
      if (dir && !config.allowed_paths.includes(dir)) {
        saveConfig({ ...config, allowed_paths: [...config.allowed_paths, dir] })
      }
    } catch {}
  }

  const removePath = (path: string) => {
    if (!config) return
    // Don't allow removing all paths
    if (config.allowed_paths.length <= 1) return
    saveConfig({ ...config, allowed_paths: config.allowed_paths.filter((p) => p !== path) })
  }

  if (!config) return null

  return (
    <div className="flex flex-col gap-2.5">
      {/* Section header */}
      <div className="flex items-center gap-2">
        <ShieldCheck size={14} style={{ color: colors.accent }} />
        <div className="text-[12px] font-semibold" style={{ color: colors.textPrimary }}>
          Safety scope
        </div>
        {saving && (
          <div className="text-[10px]" style={{ color: colors.textTertiary }}>saving...</div>
        )}
      </div>

      {/* Allowed paths */}
      <div>
        <div className="text-[11px] mb-1.5" style={{ color: colors.textTertiary }}>
          Allowed paths
        </div>
        <div className="flex flex-col gap-1">
          {config.allowed_paths.map((p) => (
            <div
              key={p}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px]"
              style={{ background: colors.surfaceSecondary, color: colors.textSecondary }}
            >
              <FolderOpen size={12} style={{ flexShrink: 0 }} />
              <span className="truncate flex-1 font-mono">{p}</span>
              {config.allowed_paths.length > 1 && (
                <button
                  onClick={() => removePath(p)}
                  className="flex-shrink-0 rounded-full p-0.5 hover:bg-white/10 transition-colors"
                  title="Remove path"
                >
                  <X size={10} />
                </button>
              )}
            </div>
          ))}
          <button
            onClick={addPath}
            className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] transition-colors hover:bg-white/5"
            style={{ color: colors.accent, border: `1px dashed ${colors.containerBorder}` }}
          >
            <Plus size={12} />
            Add folder
          </button>
        </div>
      </div>

      {/* Toggle rows */}
      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px]" style={{ color: colors.textSecondary }}>
          Block sudo / privilege escalation
        </div>
        <RowToggle
          checked={config.deny_sudo}
          onChange={(next) => saveConfig({ ...config, deny_sudo: next })}
          colors={colors}
          label="Toggle sudo blocking"
        />
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px]" style={{ color: colors.textSecondary }}>
          Block network commands
        </div>
        <RowToggle
          checked={config.deny_network_commands}
          onChange={(next) => saveConfig({ ...config, deny_network_commands: next })}
          colors={colors}
          label="Toggle network blocking"
        />
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px]" style={{ color: colors.textSecondary }}>
          Protect system paths
        </div>
        <RowToggle
          checked={config.deny_system_paths}
          onChange={(next) => saveConfig({ ...config, deny_system_paths: next })}
          colors={colors}
          label="Toggle system path protection"
        />
      </div>
    </div>
  )
}
