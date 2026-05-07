import { useState, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { ShieldWarning, Check, X, Warning, Skull } from '@phosphor-icons/react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useColors } from '../theme'
import { useSessionStore } from '../stores/sessionStore'
import type { ExecutionPlan, RiskLevel } from '../types'

const RISK_COLORS: Record<RiskLevel, string> = {
  safe: '#22c55e',
  caution: '#f59e0b',
  dangerous: '#ef4444',
  nuclear: '#dc2626',
}

const RISK_LABELS: Record<RiskLevel, string> = {
  safe: 'Safe',
  caution: 'Caution',
  dangerous: 'Dangerous',
  nuclear: 'Nuclear',
}

function RiskBadge({ risk }: { risk: RiskLevel }) {
  return (
    <span
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold"
      style={{ background: `${RISK_COLORS[risk]}20`, color: RISK_COLORS[risk] }}
    >
      {risk === 'nuclear' && <Skull size={10} />}
      {risk === 'dangerous' && <Warning size={10} />}
      {RISK_LABELS[risk]}
    </span>
  )
}

export function PlanApprovalPanel() {
  const colors = useColors()
  const pendingPlan = useSessionStore((s) => s.pendingPlan)
  const setPendingPlan = useSessionStore((s) => s.setPendingPlan)
  const [confirmText, setConfirmText] = useState('')

  // Listen for plan-ready events from the backend
  useEffect(() => {
    const unlisten = listen<ExecutionPlan>('plan_ready', (event) => {
      setPendingPlan(event.payload)
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [setPendingPlan])

  const handleApprove = async () => {
    if (!pendingPlan) return
    try {
      await invoke('approve_plan', { planId: pendingPlan.id })
    } catch (e) {
      console.error('Failed to approve plan:', e)
    }
    setPendingPlan(null)
    setConfirmText('')
  }

  const handleReject = async () => {
    if (!pendingPlan) return
    try {
      await invoke('reject_plan', { planId: pendingPlan.id, reason: null })
    } catch (e) {
      console.error('Failed to reject plan:', e)
    }
    setPendingPlan(null)
    setConfirmText('')
  }

  const isNuclear = pendingPlan?.overall_risk === 'nuclear'
  const canApprove = !isNuclear || confirmText.toLowerCase() === 'i understand'

  return (
    <AnimatePresence>
      {pendingPlan && (
        <motion.div
          data-clui-ui
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 20 }}
          transition={{ duration: 0.22, ease: [0.4, 0, 0.1, 1] }}
          className="rounded-2xl overflow-hidden"
          style={{
            background: colors.popoverBg,
            backdropFilter: 'blur(20px)',
            WebkitBackdropFilter: 'blur(20px)',
            border: `1px solid ${RISK_COLORS[pendingPlan.overall_risk]}40`,
            boxShadow: `0 8px 32px ${RISK_COLORS[pendingPlan.overall_risk]}15, ${colors.popoverShadow}`,
            maxHeight: 440,
            width: '100%',
          }}
        >
          {/* Header */}
          <div
            className="flex items-center justify-between px-4 py-3"
            style={{ borderBottom: `1px solid ${colors.popoverBorder}` }}
          >
            <div className="flex items-center gap-2">
              <ShieldWarning size={18} style={{ color: RISK_COLORS[pendingPlan.overall_risk] }} />
              <span className="text-[13px] font-semibold" style={{ color: colors.textPrimary }}>
                Execution Plan
              </span>
              <RiskBadge risk={pendingPlan.overall_risk} />
            </div>
            <span className="text-[11px]" style={{ color: colors.textTertiary }}>
              {pendingPlan.steps.length} step{pendingPlan.steps.length !== 1 ? 's' : ''}
            </span>
          </div>

          {pendingPlan.preview_image && (
            <div className="px-4 py-2 flex justify-center" style={{ background: 'rgba(0,0,0,0.2)', borderBottom: `1px solid ${colors.popoverBorder}` }}>
              <img 
                src={`data:image/png;base64,${pendingPlan.preview_image}`} 
                alt="Screenshot preview" 
                className="max-h-[160px] object-contain rounded border border-white/5"
              />
            </div>
          )}

          {/* Summary */}
          <div className="px-4 py-2" style={{ borderBottom: `1px solid ${colors.popoverBorder}` }}>
            <div className="text-[12px]" style={{ color: colors.textSecondary }}>
              {pendingPlan.summary}
            </div>
          </div>

          {/* Steps */}
          <div className="overflow-y-auto px-4 py-2" style={{ maxHeight: 260 }}>
            <div className="flex flex-col gap-1.5">
              {pendingPlan.steps.map((step) => (
                <div
                  key={step.index}
                  className="flex items-start gap-2.5 px-3 py-2 rounded-lg"
                  style={{ background: colors.surfaceSecondary }}
                >
                  <span
                    className="flex-shrink-0 w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-bold mt-0.5"
                    style={{
                      background: `${RISK_COLORS[step.risk]}20`,
                      color: RISK_COLORS[step.risk],
                    }}
                  >
                    {step.index + 1}
                  </span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-start gap-1.5 flex-wrap">
                      <span className="text-[11px] font-medium break-all" style={{ color: colors.textPrimary }}>
                        {step.description}
                      </span>
                      <RiskBadge risk={step.risk} />
                    </div>
                    {step.can_undo && step.undo_description && (
                      <div className="text-[10px] mt-0.5" style={{ color: colors.textTertiary }}>
                        Undo: {step.undo_description}
                      </div>
                    )}
                    {!step.can_undo && (
                      <div className="text-[10px] mt-0.5" style={{ color: '#ef4444' }}>
                        Cannot be undone
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Nuclear confirmation */}
          {isNuclear && (
            <div className="px-4 py-2" style={{ borderTop: `1px solid ${colors.popoverBorder}` }}>
              <div className="text-[11px] mb-1.5" style={{ color: '#ef4444' }}>
                This plan contains nuclear-risk operations. Type "I understand" to enable approval.
              </div>
              <input
                type="text"
                value={confirmText}
                onChange={(e) => setConfirmText(e.target.value)}
                placeholder='Type "I understand"'
                className="w-full px-3 py-1.5 rounded-md text-[12px] outline-none"
                style={{
                  background: colors.surfaceSecondary,
                  color: colors.textPrimary,
                  border: `1px solid ${colors.containerBorder}`,
                }}
              />
            </div>
          )}

          {/* Actions */}
          <div
            className="flex items-center justify-end gap-2 px-4 py-3"
            style={{ borderTop: `1px solid ${colors.popoverBorder}` }}
          >
            <button
              onClick={handleReject}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] font-medium transition-colors hover:bg-white/10"
              style={{ color: colors.textSecondary }}
            >
              <X size={14} />
              Cancel
            </button>
            <button
              onClick={handleApprove}
              disabled={!canApprove}
              className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-[12px] font-semibold transition-all"
              style={{
                background: canApprove ? colors.accent : colors.surfaceSecondary,
                color: canApprove ? '#fff' : colors.textTertiary,
                opacity: canApprove ? 1 : 0.5,
              }}
            >
              <Check size={14} />
              Approve &amp; Execute
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
