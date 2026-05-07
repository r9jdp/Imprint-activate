import React, { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { AnimatePresence, motion } from 'framer-motion'
import {
  ArrowClockwise,
  CalendarDots,
  ChalkboardTeacher,
  EnvelopeSimple,
  GoogleLogo,
  GraduationCap,
  Link,
  SignOut,
  Sparkle,
} from '@phosphor-icons/react'
import { useColors, useThemeStore } from './theme'

type GoogleConnectionStatus = {
  configured: boolean
  connected: boolean
}

type GoogleProfile = {
  email: string
  name: string
  picture?: string | null
}

type GmailMessage = {
  id: string
  from: string
  subject: string
  snippet: string
  internal_date?: string | null
}

type ClassroomAssignment = {
  id: string
  course_id: string
  course_name: string
  title: string
  state: string
  due_date?: string | null
  due_time?: string | null
  alternate_link?: string | null
}

type CalendarEvent = {
  id: string
  summary: string
  start?: string | null
  end?: string | null
  html_link?: string | null
}

type WorkspaceSnapshot = {
  profile?: GoogleProfile | null
  gmail: GmailMessage[]
  classroom: ClassroomAssignment[]
  calendar: CalendarEvent[]
}

export default function App() {
  const colors = useColors()
  const setThemeMode = useThemeStore((s) => s.setThemeMode)
  const [status, setStatus] = useState<GoogleConnectionStatus>({ configured: false, connected: false })
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null)
  const [busy, setBusy] = useState<'idle' | 'signing-in' | 'refreshing' | 'disconnecting'>('idle')
  const [feedback, setFeedback] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    setThemeMode('dark')
    void bootstrap()
  }, [setThemeMode])

  async function bootstrap() {
    try {
      const connectionStatus = await invoke<GoogleConnectionStatus>('google_connection_status')
      setStatus(connectionStatus)
      if (connectionStatus.connected) {
        await refreshSnapshot()
      }
    } catch (e) {
      setError(normalizeError(e))
    }
  }

  async function signIn() {
    setBusy('signing-in')
    setFeedback('')
    setError('')
    try {
      await invoke<GoogleProfile>('sign_in_google')
      const connectionStatus = await invoke<GoogleConnectionStatus>('google_connection_status')
      setStatus(connectionStatus)
      setFeedback('Google account connected. Reading Gmail, Classroom, and Calendar now.')
      await refreshSnapshot()
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function refreshSnapshot() {
    setBusy('refreshing')
    setError('')
    try {
      const data = await invoke<WorkspaceSnapshot>('get_workspace_snapshot')
      setSnapshot(data)
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function disconnect() {
    setBusy('disconnecting')
    setFeedback('')
    setError('')
    try {
      await invoke('disconnect_google')
      setStatus((prev) => ({ ...prev, connected: false }))
      setSnapshot(null)
      setFeedback('Disconnected Google account and cleared local tokens.')
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  const profileName = snapshot?.profile?.name || 'Student'
  const topPriority = useMemo(() => {
    if (!snapshot) return null
    const firstAssignment = snapshot.classroom.find((item) => item.due_date)
    return firstAssignment?.title || snapshot.gmail[0]?.subject || snapshot.calendar[0]?.summary || null
  }, [snapshot])

  return (
    <div
      className="w-screen h-screen flex items-center justify-center"
      style={{
        background:
          'radial-gradient(circle at top, rgba(217,119,87,0.18), transparent 40%), linear-gradient(180deg, rgba(10,10,10,0.1), rgba(10,10,10,0.25))',
      }}
    >
      <div
        className="w-[1080px] max-w-[calc(100vw-28px)] h-[760px] rounded-[28px] overflow-hidden"
        style={{
          background: colors.containerBg,
          border: `1px solid ${colors.containerBorder}`,
          boxShadow: colors.containerShadow,
        }}
      >
        <div className="h-full grid grid-cols-[320px_1fr]">
          <aside
            className="p-5 border-r flex flex-col overflow-y-auto"
            style={{ borderColor: colors.containerBorder, background: colors.containerBgCollapsed }}
          >
            <div className="flex items-start gap-3 mb-5">
              <div
                className="w-11 h-11 rounded-2xl flex items-center justify-center"
                style={{ background: colors.accentLight, color: colors.accent }}
              >
                <GraduationCap size={24} weight="duotone" />
              </div>
              <div>
                <div className="text-[20px] font-semibold" style={{ color: colors.textPrimary }}>
                  Imprint
                </div>
                <div className="text-[12px] leading-5" style={{ color: colors.textSecondary }}>
                  Stateful student desktop agent
                </div>
              </div>
            </div>

            <InfoCard title="Overlay">
              <div className="text-[13px] leading-6" style={{ color: colors.textSecondary }}>
                Call the app anytime with{' '}
                <span
                  className="px-2 py-1 rounded-md"
                  style={{ background: colors.surfaceSecondary, color: colors.textPrimary }}
                >
                  Alt + Shift + Space
                </span>
              </div>
            </InfoCard>

            <InfoCard
              title="Google Sign-In"
              action={<StatusPill configured={status.configured} connected={status.connected} />}
            >
              <div className="space-y-3">
                <div className="rounded-xl px-3 py-3 text-[12px] leading-6" style={{ background: colors.containerBg }}>
                  {status.configured
                    ? 'Google OAuth is configured from your local env file. Signing in will open Google consent for Gmail, Classroom, and Calendar.'
                    : 'Add GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET to src-tauri/.env, restart the app, and the sign-in button will become active.'}
                </div>
                <button
                  onClick={signIn}
                  disabled={!status.configured || busy !== 'idle'}
                  className="w-full h-11 rounded-xl text-[13px] font-medium flex items-center justify-center gap-2"
                  style={{
                    background: colors.accent,
                    color: colors.textOnAccent,
                    opacity: !status.configured || busy !== 'idle' ? 0.6 : 1,
                  }}
                >
                  <GoogleLogo size={16} weight="fill" />
                  {busy === 'signing-in' ? 'Waiting for Google approval...' : 'Sign in with Google'}
                </button>
              </div>
            </InfoCard>

            <InfoCard title="Next">
              <ul className="space-y-2 text-[12px] leading-5" style={{ color: colors.textSecondary }}>
                <li>1. Load Gmail, Classroom, and Calendar into the desktop shell.</li>
                <li>2. Add onboarding and local `user.md` profile generation.</li>
                <li>3. Add browser control on top of this data layer.</li>
                <li>4. Add the context graph and student-specific ranking logic.</li>
              </ul>
            </InfoCard>

            <div className="mt-auto flex items-center gap-2 pt-1">
              <button
                onClick={() => void refreshSnapshot()}
                disabled={!status.connected || busy !== 'idle'}
                className="flex-1 h-11 rounded-xl text-[13px] font-medium flex items-center justify-center gap-2"
                style={{
                  background: colors.surfaceSecondary,
                  color: colors.textPrimary,
                  border: `1px solid ${colors.containerBorder}`,
                  opacity: !status.connected || busy !== 'idle' ? 0.6 : 1,
                }}
              >
                <ArrowClockwise size={15} />
                Refresh data
              </button>
              <button
                onClick={disconnect}
                disabled={!status.connected || busy !== 'idle'}
                className="w-11 h-11 rounded-xl flex items-center justify-center"
                style={{
                  background: colors.statusErrorBg,
                  color: colors.statusError,
                  opacity: !status.connected || busy !== 'idle' ? 0.6 : 1,
                }}
                title="Disconnect Google"
              >
                <SignOut size={16} />
              </button>
            </div>
          </aside>

          <main className="p-6 overflow-hidden flex flex-col">
            <div className="flex items-start justify-between gap-4 mb-5 shrink-0">
              <div>
                <div className="text-[13px] uppercase tracking-[0.18em] mb-2" style={{ color: colors.textTertiary }}>
                  Workspace Status
                </div>
                <h1 className="text-[30px] leading-[1.1] font-semibold" style={{ color: colors.textPrimary }}>
                  {status.connected ? `Ready, ${profileName}` : 'Connect your Google workspace'}
                </h1>
                <p className="text-[14px] mt-2 max-w-[680px]" style={{ color: colors.textSecondary }}>
                  Start with live Gmail, Classroom, and Calendar reads. Once the data layer is stable, we'll add
                  browser actions and the student context graph on top.
                </p>
              </div>
              <div
                className="rounded-2xl px-4 py-3 min-w-[260px]"
                style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
              >
                <div className="text-[11px] uppercase tracking-[0.18em] mb-2" style={{ color: colors.textTertiary }}>
                  Current Priority
                </div>
                <div className="text-[14px] font-medium" style={{ color: colors.textPrimary }}>
                  {topPriority || 'No live ranking yet'}
                </div>
                <div className="text-[12px] mt-2" style={{ color: colors.textSecondary }}>
                  This will become the first stateful recommendation once profile onboarding is added.
                </div>
              </div>
            </div>

            <AnimatePresence mode="wait">
              {(feedback || error) && (
                <motion.div
                  key={feedback ? 'feedback' : 'error'}
                  initial={{ opacity: 0, y: -8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  className="rounded-2xl px-4 py-3 mb-5 text-[13px] shrink-0"
                  style={{
                    background: feedback ? colors.statusCompleteBg : colors.statusErrorBg,
                    color: feedback ? colors.statusComplete : colors.statusError,
                    border: `1px solid ${feedback ? colors.statusComplete : colors.statusError}22`,
                  }}
                >
                  {feedback || error}
                </motion.div>
              )}
            </AnimatePresence>

            <div className="grid grid-cols-3 gap-4 min-h-0 flex-1 overflow-hidden">
              <DataColumn
                title="Gmail"
                icon={<EnvelopeSimple size={16} weight="duotone" />}
                subtitle="Recent inbox signals"
                items={snapshot?.gmail || []}
                empty="Connect Google to read recent inbox messages."
                renderItem={(item) => (
                  <div>
                    <div className="text-[12px] font-medium truncate" style={{ color: colors.textPrimary }}>
                      {(item as GmailMessage).subject || '(no subject)'}
                    </div>
                    <div className="text-[11px] mt-1 truncate" style={{ color: colors.textTertiary }}>
                      {(item as GmailMessage).from}
                    </div>
                    <div className="text-[12px] mt-2 leading-5" style={{ color: colors.textSecondary }}>
                      {(item as GmailMessage).snippet}
                    </div>
                  </div>
                )}
              />
              <DataColumn
                title="Classroom"
                icon={<ChalkboardTeacher size={16} weight="duotone" />}
                subtitle="Pending coursework"
                items={snapshot?.classroom || []}
                empty="Connect Google to read course work."
                renderItem={(item) => {
                  const assignment = item as ClassroomAssignment
                  return (
                    <div>
                      <div className="text-[11px] uppercase tracking-[0.14em]" style={{ color: colors.textTertiary }}>
                        {assignment.course_name}
                      </div>
                      <div className="text-[13px] font-medium mt-1" style={{ color: colors.textPrimary }}>
                        {assignment.title}
                      </div>
                      <div className="text-[12px] mt-2" style={{ color: colors.textSecondary }}>
                        Due {formatDue(assignment.due_date, assignment.due_time)}
                      </div>
                      {assignment.alternate_link && (
                        <ExternalLinkButton href={assignment.alternate_link} label="Open assignment" />
                      )}
                    </div>
                  )
                }}
              />
              <DataColumn
                title="Calendar"
                icon={<CalendarDots size={16} weight="duotone" />}
                subtitle="Upcoming deadlines"
                items={snapshot?.calendar || []}
                empty="Connect Google to read upcoming events."
                renderItem={(item) => {
                  const event = item as CalendarEvent
                  return (
                    <div>
                      <div className="text-[13px] font-medium" style={{ color: colors.textPrimary }}>
                        {event.summary}
                      </div>
                      <div className="text-[12px] mt-2" style={{ color: colors.textSecondary }}>
                        {event.start ? formatDate(event.start) : 'No start time'}
                      </div>
                      {event.html_link && <ExternalLinkButton href={event.html_link} label="Open event" />}
                    </div>
                  )
                }}
              />
            </div>

            <div
              className="mt-5 rounded-2xl p-4 flex items-start gap-3 shrink-0"
              style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
            >
              <div
                className="w-10 h-10 rounded-2xl flex items-center justify-center shrink-0"
                style={{ background: colors.accentLight, color: colors.accent }}
              >
                <Sparkle size={18} weight="fill" />
              </div>
              <div>
                <div className="text-[13px] font-medium" style={{ color: colors.textPrimary }}>
                  This pass is the data foundation
                </div>
                <div className="text-[12px] mt-1 leading-6" style={{ color: colors.textSecondary }}>
                  You now have the summonable desktop UI and the Google data layer in one place. The next layers are
                  student onboarding (`user.md` / profile graph), browser control, and eventually stateful ranking and
                  reminders driven by recent email and pending coursework.
                </div>
              </div>
            </div>
          </main>
        </div>
      </div>
    </div>
  )
}

function InfoCard({
  title,
  children,
  action,
}: {
  title: string
  children: React.ReactNode
  action?: React.ReactNode
}) {
  const colors = useColors()
  return (
    <div
      className="rounded-2xl p-4 mb-4"
      style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="text-[11px] uppercase tracking-[0.18em]" style={{ color: colors.textTertiary }}>
          {title}
        </div>
        {action}
      </div>
      {children}
    </div>
  )
}

function StatusPill({ configured, connected }: { configured: boolean; connected: boolean }) {
  const colors = useColors()
  const label = connected ? 'Connected' : configured ? 'Configured' : 'Missing env'
  const tone = connected
    ? { bg: colors.statusCompleteBg, fg: colors.statusComplete }
    : configured
      ? { bg: colors.accentLight, fg: colors.accent }
      : { bg: colors.surfaceSecondary, fg: colors.textTertiary }
  return (
    <div className="px-2.5 py-1 rounded-full text-[10px] font-medium" style={{ background: tone.bg, color: tone.fg }}>
      {label}
    </div>
  )
}

function DataColumn<T>({
  title,
  icon,
  subtitle,
  items,
  empty,
  renderItem,
}: {
  title: string
  icon: React.ReactNode
  subtitle: string
  items: T[]
  empty: string
  renderItem: (item: T) => React.ReactNode
}) {
  const colors = useColors()
  return (
    <section
      className="rounded-[24px] min-h-0 flex flex-col overflow-hidden"
      style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
    >
      <div className="px-4 py-4 border-b shrink-0" style={{ borderColor: colors.containerBorder }}>
        <div className="flex items-center gap-2">
          <div style={{ color: colors.accent }}>{icon}</div>
          <div className="text-[15px] font-medium" style={{ color: colors.textPrimary }}>
            {title}
          </div>
        </div>
        <div className="text-[12px] mt-2" style={{ color: colors.textSecondary }}>
          {subtitle}
        </div>
      </div>
      <div className="p-4 overflow-y-auto min-h-0 space-y-3">
        {items.length === 0 ? (
          <div className="text-[12px] leading-6" style={{ color: colors.textTertiary }}>
            {empty}
          </div>
        ) : (
          items.map((item, index) => (
            <div
              key={index}
              className="rounded-2xl p-3"
              style={{ background: colors.containerBg, border: `1px solid ${colors.containerBorder}` }}
            >
              {renderItem(item)}
            </div>
          ))
        )}
      </div>
    </section>
  )
}

function ExternalLinkButton({ href, label }: { href: string; label: string }) {
  const colors = useColors()
  return (
    <button
      onClick={() => void invoke('open_external_command', { url: href })}
      className="mt-3 inline-flex items-center gap-1 text-[11px]"
      style={{ color: colors.accent }}
    >
      <Link size={12} />
      {label}
    </button>
  )
}

function normalizeError(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString()
}

function formatDue(date?: string | null, time?: string | null): string {
  if (!date && !time) return 'No due date'
  return [date, time].filter(Boolean).join(' ')
}
