import React, { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { AnimatePresence, motion } from 'framer-motion'
import Markdown from 'react-markdown'
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
import remarkGfm from 'remark-gfm'
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
  web_link?: string | null
}

type ClassroomAssignment = {
  id: string
  course_id: string
  course_name: string
  course_link?: string | null
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

type AgentEvent = {
  kind: string
  content: string
}

type WorkspaceTab = 'gmail' | 'classroom' | 'calendar'
type MainView = 'workspace' | 'assistant'

type ProfileDocument = {
  name: string
  path: string
  mime_type: string
  extracted_summary: string
  extracted_text: string
}

type StudentProfile = {
  full_name: string
  degree_program: string
  semester: string
  about_me: string
  additional_context: string
  top_priorities: string[]
  must_not_ignore: string[]
  important_courses: string[]
  default_help: string
  reminder_style: string
  output_style: string
  documents: ProfileDocument[]
}

type StudentProfileStatus = {
  profile?: StudentProfile | null
  has_profile: boolean
}

type StudentProfileDraft = {
  full_name: string
  degree_program: string
  semester: string
  about_me: string
  additional_context: string
  top_priorities: string
  must_not_ignore: string
  important_courses: string
  default_help: string
  reminder_style: string
  output_style: string
  documents: ProfileDocument[]
}

const EMPTY_PROFILE_DRAFT: StudentProfileDraft = {
  full_name: '',
  degree_program: '',
  semester: '',
  about_me: '',
  additional_context: '',
  top_priorities: '',
  must_not_ignore: '',
  important_courses: '',
  default_help: '',
  reminder_style: '',
  output_style: '',
  documents: [],
}

export default function App() {
  const colors = useColors()
  const setThemeMode = useThemeStore((s) => s.setThemeMode)
  const shortcutLabel = /Mac|iPhone|iPad|iPod/i.test(window.navigator.platform)
    ? 'Command + Shift + Space'
    : /Win/i.test(window.navigator.platform)
      ? 'Alt + Shift + Space'
      : 'Alt + Space'
  const [status, setStatus] = useState<GoogleConnectionStatus>({ configured: false, connected: false })
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null)
  const [busy, setBusy] = useState<'idle' | 'signing-in' | 'refreshing' | 'disconnecting' | 'asking' | 'saving-profile' | 'uploading-documents'>('idle')
  const [feedback, setFeedback] = useState('')
  const [error, setError] = useState('')
  const [query, setQuery] = useState('')
  const [assistantAnswer, setAssistantAnswer] = useState('')
  const [studentProfile, setStudentProfile] = useState<StudentProfile | null>(null)
  const [profileDraft, setProfileDraft] = useState<StudentProfileDraft>(EMPTY_PROFILE_DRAFT)
  const [showOnboarding, setShowOnboarding] = useState(false)
  const [browserConnected, setBrowserConnected] = useState(false)
  const [agentActivity, setAgentActivity] = useState<string[]>([])
  const [activeWorkspaceTab, setActiveWorkspaceTab] = useState<WorkspaceTab>('gmail')
  const [activeMainView, setActiveMainView] = useState<MainView>('workspace')

  useEffect(() => {
    setThemeMode('dark')
    void bootstrap()
  }, [setThemeMode])

  useEffect(() => {
    let unsubscribe: (() => void) | undefined

    void (async () => {
      unsubscribe = await listen<AgentEvent>('agent_event', (event) => {
        const payload = event.payload
        if (payload.kind === 'tool_call') {
          const toolName = parseToolName(payload.content)
          if (toolName.startsWith('browser_')) {
            setBrowserConnected(true)
          }
          setAgentActivity((current) => [...current, `Running ${toolName}...`].slice(-8))
          return
        }

        if (payload.kind === 'tool_result') {
          const summary = summarizeToolResult(payload.content)
          if (summary) {
            setAgentActivity((current) => [...current, summary].slice(-8))
          }
          return
        }

        if (payload.kind === 'message') {
          setAssistantAnswer(payload.content.trim())
          return
        }

        if (payload.kind === 'error') {
          setError(payload.content)
          return
        }
      })
    })()

    return () => {
      unsubscribe?.()
    }
  }, [])

  async function bootstrap() {
    try {
      const connectionStatus = await invoke<GoogleConnectionStatus>('google_connection_status')
      const profileStatus = await invoke<StudentProfileStatus>('get_student_profile')
      setStatus(connectionStatus)
      hydrateProfile(profileStatus.profile ?? null)
      setShowOnboarding(!profileStatus.has_profile)
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
    setFeedback('')
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

  async function saveProfile() {
    setBusy('saving-profile')
    setFeedback('')
    setError('')
    try {
      const nextProfile = draftToProfile(profileDraft)
      await invoke('save_student_profile', { studentProfile: nextProfile })
      setStudentProfile(nextProfile)
      setShowOnboarding(false)
      setFeedback('Student profile saved locally. Imprint will now use it for prioritization and reminders.')
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function uploadProfileDocuments() {
    setBusy('uploading-documents')
    setFeedback('')
    setError('')
    try {
      const selectedFiles = await invoke<Array<{ path: string; name: string }>>('attach_files_command')
      const paths = selectedFiles.map((file) => file.path).filter(Boolean)
      if (paths.length === 0) {
        return
      }

      const extracted = await invoke<ProfileDocument[]>('ingest_profile_documents', {
        request: { paths },
      })

      const nextDraft = {
        ...profileDraft,
        documents: mergeDocuments(profileDraft.documents, extracted),
      }
      setProfileDraft(nextDraft)

      const nextProfile = draftToProfile(nextDraft)
      await invoke('save_student_profile', { studentProfile: nextProfile })
      setStudentProfile(nextProfile)
      setFeedback('Documents processed, extracted with Gemini, and saved into your local profile.')
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function connectBrowser() {
    setBusy('refreshing')
    setFeedback('')
    setError('')
    try {
      const message = await invoke<string>('browser_connect')
      setBrowserConnected(true)
      setFeedback(message)
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function setupBrowser() {
    setBusy('refreshing')
    setFeedback('')
    setError('')
    try {
      const message = await invoke<string>('browser_setup_chrome')
      setFeedback(message)
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function disconnectBrowser() {
    setBusy('disconnecting')
    setFeedback('')
    setError('')
    try {
      await invoke('browser_disconnect')
      setBrowserConnected(false)
      setFeedback('Disconnected Imprint browser session.')
    } catch (e) {
      setError(normalizeError(e))
    } finally {
      setBusy('idle')
    }
  }

  async function askAssistant() {
    if (!snapshot || !query.trim()) return
    setBusy('asking')
    setFeedback('')
    setError('')
    setAssistantAnswer('')
    setAgentActivity([])
    try {
      const answer = await invoke<string>('ask_workspace_assistant', {
        request: {
          query: query.trim(),
          snapshot,
        },
      })
      setAssistantAnswer(answer)
      setFeedback('Run completed.')
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
  const profileStatusLabel = studentProfile ? 'Configured' : 'Missing'

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
        <div className="h-full grid grid-cols-[248px_1fr]">
          <aside
            className="p-4 border-r flex flex-col overflow-y-auto"
            style={{ borderColor: colors.containerBorder, background: colors.containerBgCollapsed }}
          >
            <div className="flex items-center gap-3 mb-6">
              <div
                className="w-10 h-10 rounded-xl flex items-center justify-center"
                style={{ background: colors.accentLight, color: colors.accent }}
              >
                <GraduationCap size={22} weight="duotone" />
              </div>
              <div>
                <div className="text-[18px] font-semibold" style={{ color: colors.textPrimary }}>
                  Imprint
                </div>
                <div className="text-[11px]" style={{ color: colors.textTertiary }}>
                  Student workspace
                </div>
              </div>
            </div>

            <div className="rounded-xl p-4 mb-3" style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}>
              <div className="flex items-center justify-between gap-2 mb-3">
                <div className="text-[11px] uppercase tracking-[0.16em]" style={{ color: colors.textTertiary }}>Google workspace</div>
                <StatusPill configured={status.configured} connected={status.connected} />
              </div>
              <div className="text-[12px] leading-5 mb-3" style={{ color: colors.textSecondary }}>
                {status.connected ? `Connected as ${profileName}.` : 'Connect to load Gmail, Classroom, and Calendar.'}
              </div>
              <button
                onClick={signIn}
                disabled={!status.configured || busy !== 'idle' || status.connected}
                className="w-full h-10 rounded-lg text-[12px] font-medium flex items-center justify-center gap-2"
                style={{ background: colors.accent, color: colors.textOnAccent, opacity: !status.configured || busy !== 'idle' || status.connected ? 0.6 : 1 }}
              >
                <GoogleLogo size={15} weight="fill" />
                {busy === 'signing-in' ? 'Waiting for approval...' : status.connected ? 'Google connected' : 'Connect Google'}
              </button>
            </div>

            <div className="rounded-xl p-4 mb-3" style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}>
              <div className="flex items-center justify-between gap-2 mb-2">
                <div className="text-[11px] uppercase tracking-[0.16em]" style={{ color: colors.textTertiary }}>Local profile</div>
                <StatusPill configured={Boolean(studentProfile)} connected={false} configuredLabel={profileStatusLabel} missingLabel="Not set" />
              </div>
              <div className="text-[12px] leading-5 mb-3" style={{ color: colors.textSecondary }}>
                {studentProfile ? `${studentProfile.full_name || 'Student'} · ${studentProfile.documents.length} saved document${studentProfile.documents.length === 1 ? '' : 's'}` : 'Add context to personalize summaries and priorities.'}
              </div>
              <button
                onClick={() => {
                  setShowOnboarding((current) => !current)
                  setActiveMainView('assistant')
                }}
                className="w-full h-9 rounded-lg text-[12px] font-medium"
                style={{ background: colors.surfaceSecondary, color: colors.textPrimary, border: `1px solid ${colors.containerBorder}` }}
              >
                {studentProfile ? 'Edit profile' : 'Set up profile'}
              </button>
            </div>

            <details className="rounded-xl px-4 py-3" style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}>
              <summary className="cursor-pointer text-[12px] font-medium" style={{ color: colors.textSecondary }}>
                Browser tools {browserConnected ? '· ready' : ''}
              </summary>
              <div className="grid grid-cols-2 gap-2 mt-3">
                <button onClick={() => void connectBrowser()} disabled={busy !== 'idle'} className="h-9 rounded-lg text-[11px] font-medium" style={{ background: colors.accent, color: colors.textOnAccent, opacity: busy !== 'idle' ? 0.6 : 1 }}>Connect</button>
                <button onClick={() => void setupBrowser()} disabled={busy !== 'idle'} className="h-9 rounded-lg text-[11px] font-medium" style={{ background: colors.surfaceSecondary, color: colors.textPrimary, border: `1px solid ${colors.containerBorder}`, opacity: busy !== 'idle' ? 0.6 : 1 }}>Setup</button>
              </div>
              <button onClick={() => void disconnectBrowser()} disabled={busy !== 'idle' || !browserConnected} className="w-full h-9 rounded-lg mt-2 text-[11px] font-medium" style={{ background: colors.statusErrorBg, color: colors.statusError, opacity: busy !== 'idle' || !browserConnected ? 0.6 : 1 }}>Disconnect browser</button>
            </details>

            <div className="mt-auto pt-4">
              <div className="text-[10px] mb-2" style={{ color: colors.textTertiary }}>Open anytime · {shortcutLabel}</div>
              <div className="flex items-center gap-2">
              <button
                onClick={() => void refreshSnapshot()}
                disabled={!status.connected || busy !== 'idle'}
                className="flex-1 h-10 rounded-lg text-[12px] font-medium flex items-center justify-center gap-2"
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
                className="w-10 h-10 rounded-lg flex items-center justify-center"
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
            </div>
          </aside>

          <main className="p-6 overflow-y-auto overflow-x-hidden min-h-0">
            <div className="flex items-start justify-between gap-6 mb-5">
              <div>
                <div className="text-[11px] uppercase tracking-[0.18em] mb-2" style={{ color: colors.textTertiary }}>
                  Workspace Status
                </div>
                <h1 className="text-[28px] leading-[1.1] font-semibold" style={{ color: colors.textPrimary }}>
                  {status.connected ? `Ready, ${profileName}` : 'Connect your Google workspace'}
                </h1>
                <p className="text-[13px] mt-2 max-w-[560px] leading-6" style={{ color: colors.textSecondary }}>
                  Your live mail, coursework, and calendar are organized here. Ask for a summary whenever you need the bigger picture.
                </p>
              </div>
              <div
                className="rounded-xl px-4 py-3 w-[250px] shrink-0"
                style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
              >
                <div className="text-[11px] uppercase tracking-[0.18em] mb-2" style={{ color: colors.textTertiary }}>
                  Current Priority
                </div>
                <div className="text-[14px] font-medium" style={{ color: colors.textPrimary }}>
                  {topPriority || 'No live ranking yet'}
                </div>
                <div className="text-[12px] mt-2 leading-5" style={{ color: colors.textSecondary }}>
                  Based on your latest workspace signal.
                </div>
              </div>
            </div>

            <AnimatePresence mode="wait">
              {(feedback || error) && (
                <motion.div
                  key={error ? 'error' : 'feedback'}
                  initial={{ opacity: 0, y: -8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  className="rounded-xl px-4 py-3 mb-5 text-[13px]"
                  style={{
                    background: error ? colors.statusErrorBg : colors.statusCompleteBg,
                    color: error ? colors.statusError : colors.statusComplete,
                    border: `1px solid ${error ? colors.statusError : colors.statusComplete}22`,
                  }}
                >
                  {error || feedback}
                </motion.div>
              )}
            </AnimatePresence>

            <div className="inline-flex items-center gap-1 p-1 rounded-lg mb-4" style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}>
              <button
                onClick={() => setActiveMainView('workspace')}
                className="h-9 px-3 rounded-md text-[12px] font-medium"
                style={{ background: activeMainView === 'workspace' ? colors.surfaceSecondary : 'transparent', color: activeMainView === 'workspace' ? colors.textPrimary : colors.textSecondary }}
              >
                Workspace
              </button>
              <button
                onClick={() => {
                  setShowOnboarding(false)
                  setActiveMainView('assistant')
                }}
                className="h-9 px-3 rounded-md text-[12px] font-medium inline-flex items-center gap-2"
                style={{ background: activeMainView === 'assistant' ? colors.accent : 'transparent', color: activeMainView === 'assistant' ? colors.textOnAccent : colors.textSecondary }}
              >
                <Sparkle size={14} weight="fill" />
                Ask Imprint
              </button>
            </div>

            <AnimatePresence mode="wait">
              <motion.div
                key={activeMainView === 'workspace' ? 'workspace' : showOnboarding ? 'profile' : 'assistant'}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.18 }}
              >
            {activeMainView === 'workspace' ? (
            <section>
              <div className="flex items-center gap-2 mb-3" role="tablist" aria-label="Workspace sources">
                {([
                  ['gmail', 'Gmail', EnvelopeSimple, snapshot?.gmail.length || 0],
                  ['classroom', 'Classroom', ChalkboardTeacher, snapshot?.classroom.length || 0],
                  ['calendar', 'Calendar', CalendarDots, snapshot?.calendar.length || 0],
                ] as const).map(([id, label, Icon, count]) => {
                  const active = activeWorkspaceTab === id
                  return (
                    <button
                      key={id}
                      role="tab"
                      aria-selected={active}
                      onClick={() => setActiveWorkspaceTab(id)}
                      className="h-10 px-3 rounded-lg text-[12px] font-medium inline-flex items-center gap-2"
                      style={{
                        background: active ? colors.surfacePrimary : 'transparent',
                        color: active ? colors.textPrimary : colors.textSecondary,
                        border: `1px solid ${active ? colors.containerBorder : 'transparent'}`,
                      }}
                    >
                      <Icon size={16} weight="duotone" style={{ color: active ? colors.accent : undefined }} />
                      {label}
                      <span className="text-[10px]" style={{ color: colors.textTertiary }}>{count}</span>
                    </button>
                  )
                })}
              </div>

              <div className="h-[430px] min-h-0">
                {activeWorkspaceTab === 'gmail' && (
                  <DataColumn
                    title="Gmail"
                    icon={<EnvelopeSimple size={17} weight="duotone" />}
                    subtitle="Recent inbox signals"
                    items={snapshot?.gmail || []}
                    empty="Connect Google to read recent inbox messages."
                    renderItem={(item) => (
                      <div>
                        <div className="text-[14px] font-medium" style={{ color: colors.textPrimary }}>{(item as GmailMessage).subject || '(no subject)'}</div>
                        <div className="text-[12px] mt-1" style={{ color: colors.textTertiary }}>{(item as GmailMessage).from}</div>
                        <div className="text-[13px] mt-3 leading-6" style={{ color: colors.textSecondary }}>{(item as GmailMessage).snippet}</div>
                        {(item as GmailMessage).web_link && <ExternalLinkButton href={(item as GmailMessage).web_link!} label="Open in Imprint browser" mode="browser" />}
                      </div>
                    )}
                  />
                )}
                {activeWorkspaceTab === 'classroom' && (
                  <DataColumn
                    title="Classroom"
                    icon={<ChalkboardTeacher size={17} weight="duotone" />}
                    subtitle="Pending coursework"
                    items={snapshot?.classroom || []}
                    empty="Connect Google to read course work."
                    renderItem={(item) => {
                      const assignment = item as ClassroomAssignment
                      return <div><div className="text-[11px] uppercase tracking-[0.14em]" style={{ color: colors.textTertiary }}>{assignment.course_name}</div><div className="text-[14px] font-medium mt-2" style={{ color: colors.textPrimary }}>{assignment.title}</div><div className="text-[13px] mt-3" style={{ color: colors.textSecondary }}>Due {formatDue(assignment.due_date, assignment.due_time)}</div>{assignment.alternate_link && <ExternalLinkButton href={assignment.alternate_link} label="Open in Imprint browser" mode="browser" />}</div>
                    }}
                  />
                )}
                {activeWorkspaceTab === 'calendar' && (
                  <DataColumn
                    title="Calendar"
                    icon={<CalendarDots size={17} weight="duotone" />}
                    subtitle="Upcoming deadlines"
                    items={snapshot?.calendar || []}
                    empty="Connect Google to read upcoming events."
                    renderItem={(item) => {
                      const event = item as CalendarEvent
                      return <div><div className="text-[14px] font-medium" style={{ color: colors.textPrimary }}>{event.summary}</div><div className="text-[13px] mt-3" style={{ color: colors.textSecondary }}>{event.start ? formatDate(event.start) : 'No start time'}</div>{event.html_link && <ExternalLinkButton href={event.html_link} label="Open in Imprint browser" mode="browser" />}</div>
                    }}
                  />
                )}
              </div>
            </section>

            ) : showOnboarding ? (
              <div
                className="mt-5 rounded-2xl p-5 shrink-0 max-h-[70vh] overflow-y-auto"
                style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
              >
                <div className="flex items-start gap-3">
                  <div
                    className="w-10 h-10 rounded-2xl flex items-center justify-center shrink-0"
                    style={{ background: colors.accentLight, color: colors.accent }}
                  >
                    <Sparkle size={18} weight="fill" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-[15px] font-medium" style={{ color: colors.textPrimary }}>
                      Set up your local student profile
                    </div>
                    <div className="text-[12px] mt-1 mb-4 leading-6" style={{ color: colors.textSecondary }}>
                      This stays on your device and becomes the first layer of state for prioritization, reminders, and
                      future browser actions.
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <ProfileField
                        label="Full name"
                        value={profileDraft.full_name}
                        onChange={(value) => updateDraft('full_name', value)}
                        placeholder="Rajdeep Pandey"
                      />
                      <ProfileField
                        label="Degree / program"
                        value={profileDraft.degree_program}
                        onChange={(value) => updateDraft('degree_program', value)}
                        placeholder="B.Tech Computer Science"
                      />
                      <ProfileField
                        label="Semester / year"
                        value={profileDraft.semester}
                        onChange={(value) => updateDraft('semester', value)}
                        placeholder="6th semester"
                      />
                      <ProfileArea
                        label="About you"
                        value={profileDraft.about_me}
                        onChange={(value) => updateDraft('about_me', value)}
                        placeholder="Add anything about yourself that the agent should know directly."
                      />
                      <ProfileArea
                        label="Extra context"
                        value={profileDraft.additional_context}
                        onChange={(value) => updateDraft('additional_context', value)}
                        placeholder="Personal goals, constraints, writing preferences, internships, placements, anything useful."
                      />
                      <ProfileField
                        label="Default help mode"
                        value={profileDraft.default_help}
                        onChange={(value) => updateDraft('default_help', value)}
                        placeholder="Prioritize, summarize, draft first versions"
                      />
                      <ProfileArea
                        label="Top priorities"
                        value={profileDraft.top_priorities}
                        onChange={(value) => updateDraft('top_priorities', value)}
                        placeholder="Placements, pending assignments, internship prep"
                      />
                      <ProfileArea
                        label="Must-not-ignore signals"
                        value={profileDraft.must_not_ignore}
                        onChange={(value) => updateDraft('must_not_ignore', value)}
                        placeholder="Placement cell emails, same-day deadlines, professor updates"
                      />
                      <ProfileArea
                        label="Important courses"
                        value={profileDraft.important_courses}
                        onChange={(value) => updateDraft('important_courses', value)}
                        placeholder="DBMS, OS, DWM"
                      />
                      <ProfileField
                        label="Reminder style"
                        value={profileDraft.reminder_style}
                        onChange={(value) => updateDraft('reminder_style', value)}
                        placeholder="Urgent-first with short follow-ups"
                      />
                    </div>
                    <div className="mt-3">
                      <ProfileField
                        label="Output style"
                        value={profileDraft.output_style}
                        onChange={(value) => updateDraft('output_style', value)}
                        placeholder="Concise bullets with clear next actions"
                      />
                    </div>
                    <div className="mt-4 rounded-2xl p-4" style={{ background: colors.containerBg, border: `1px solid ${colors.containerBorder}` }}>
                      <div className="flex items-start justify-between gap-3">
                        <div>
                          <div className="text-[13px] font-medium" style={{ color: colors.textPrimary }}>
                            Profile documents
                          </div>
                          <div className="text-[12px] mt-1 leading-6" style={{ color: colors.textSecondary }}>
                            Upload documents once. Imprint extracts the useful text with Gemini and stores it locally in your profile. The original documents are not resent on every query.
                          </div>
                        </div>
                        <button
                          onClick={() => void uploadProfileDocuments()}
                          disabled={busy !== 'idle'}
                          className="h-10 px-4 rounded-xl text-[12px] font-medium"
                          style={{
                            background: colors.surfaceSecondary,
                            color: colors.textPrimary,
                            border: `1px solid ${colors.containerBorder}`,
                            opacity: busy !== 'idle' ? 0.6 : 1,
                          }}
                        >
                          {busy === 'uploading-documents' ? 'Extracting...' : 'Upload documents'}
                        </button>
                      </div>
                      <div className="mt-4 space-y-3 max-h-[260px] overflow-y-auto">
                        {profileDraft.documents.length === 0 ? (
                          <div className="text-[12px] leading-6" style={{ color: colors.textTertiary }}>
                            No documents added yet.
                          </div>
                        ) : (
                          profileDraft.documents.map((document) => (
                            <div
                              key={`${document.path}-${document.name}`}
                              className="rounded-2xl p-3"
                              style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
                            >
                              <div className="text-[13px] font-medium" style={{ color: colors.textPrimary }}>
                                {document.name}
                              </div>
                              <div className="text-[11px] mt-1" style={{ color: colors.textTertiary }}>
                                {document.mime_type || 'document'}
                              </div>
                              <div className="text-[12px] mt-2 leading-6" style={{ color: colors.textSecondary }}>
                                {document.extracted_summary || 'No extracted summary available.'}
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                    <div className="mt-4 flex items-center gap-3">
                      <button
                        onClick={() => void saveProfile()}
                        disabled={busy !== 'idle'}
                        className="h-11 px-4 rounded-xl text-[13px] font-medium"
                        style={{
                          background: colors.accent,
                          color: colors.textOnAccent,
                          opacity: busy !== 'idle' ? 0.6 : 1,
                        }}
                      >
                        {busy === 'saving-profile' ? 'Saving profile...' : 'Save local profile'}
                      </button>
                      {studentProfile && (
                        <button
                          onClick={() => setShowOnboarding(false)}
                          className="h-11 px-4 rounded-xl text-[13px] font-medium"
                          style={{
                            background: colors.surfaceSecondary,
                            color: colors.textPrimary,
                            border: `1px solid ${colors.containerBorder}`,
                          }}
                        >
                          Close
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              <div
                className="rounded-xl p-5 flex items-start gap-4 min-h-[500px]"
                style={{ background: colors.surfacePrimary, border: `1px solid ${colors.containerBorder}` }}
              >
                <div
                  className="w-10 h-10 rounded-xl flex items-center justify-center shrink-0"
                  style={{ background: colors.accentLight, color: colors.accent }}
                >
                  <Sparkle size={18} weight="fill" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[15px] font-medium" style={{ color: colors.textPrimary }}>
                    Ask Imprint
                  </div>
                  <div className="text-[13px] mt-1 mb-4 leading-6" style={{ color: colors.textSecondary }}>
                    Ask a question about the workspace. Imprint uses your profile, Gmail, Classroom, and Calendar context.
                  </div>
                  <div className="flex items-center gap-3">
                    <input
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && !e.shiftKey) {
                          e.preventDefault()
                          void askAssistant()
                        }
                      }}
                      placeholder="What should I focus on right now?"
                      className="flex-1 h-12 px-4 rounded-lg text-[13px]"
                      style={{
                        background: colors.containerBg,
                        color: colors.textPrimary,
                        border: `1px solid ${colors.containerBorder}`,
                      }}
                    />
                    <button
                      onClick={() => void askAssistant()}
                      disabled={!snapshot || !query.trim() || busy !== 'idle'}
                      className="h-12 px-5 rounded-lg text-[13px] font-medium"
                      style={{
                        background: colors.accent,
                        color: colors.textOnAccent,
                        opacity: !snapshot || !query.trim() || busy !== 'idle' ? 0.6 : 1,
                      }}
                    >
                      {busy === 'asking' ? 'Asking...' : 'Ask'}
                    </button>
                  </div>
                  {agentActivity.length > 0 && (
                    <div
                    className="mt-4 rounded-lg px-4 py-3 max-h-[120px] overflow-y-auto"
                      style={{ background: colors.surfaceSecondary, border: `1px solid ${colors.containerBorder}` }}
                    >
                      <div className="text-[11px] uppercase tracking-[0.14em] mb-2" style={{ color: colors.textTertiary }}>
                        Agent Activity
                      </div>
                      <div className="space-y-1">
                        {agentActivity.map((item, index) => (
                          <div key={`${item}-${index}`} className="text-[12px]" style={{ color: colors.textSecondary }}>
                            {item}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                  <div
                    className="mt-4 rounded-lg px-5 py-4 min-h-[360px] max-h-[430px] overflow-y-auto text-[13px] leading-7"
                    style={{
                      background: colors.containerBg,
                      color: assistantAnswer || busy === 'asking' ? colors.textPrimary : colors.textTertiary,
                      border: `1px solid ${colors.containerBorder}`,
                    }}
                  >
                    {assistantAnswer ? (
                        <div className="prose-cloud conversation-selectable text-[13px] leading-7 [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
                          <Markdown remarkPlugins={[remarkGfm]}>
                            {assistantAnswer}
                          </Markdown>
                        </div>
                      ) : <div className="flex h-[320px] items-center justify-center">Your summary will appear here.</div>}
                  </div>
                </div>
              </div>
            )}
              </motion.div>
            </AnimatePresence>
          </main>
        </div>
      </div>
    </div>
  )

  function updateDraft<K extends keyof StudentProfileDraft>(key: K, value: StudentProfileDraft[K]) {
    setProfileDraft((current) => ({ ...current, [key]: value }))
  }

  function hydrateProfile(profile: StudentProfile | null) {
    setStudentProfile(profile)
    if (profile) {
      setProfileDraft(profileToDraft(profile))
    } else {
      setProfileDraft(EMPTY_PROFILE_DRAFT)
    }
  }
}

function StatusPill({
  configured,
  connected,
  configuredLabel,
  connectedLabel,
  missingLabel,
}: {
  configured: boolean
  connected: boolean
  configuredLabel?: string
  connectedLabel?: string
  missingLabel?: string
}) {
  const colors = useColors()
  const label = connected
    ? connectedLabel || 'Connected'
    : configured
      ? configuredLabel || 'Configured'
      : missingLabel || 'Missing env'
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

function ProfileField({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  const colors = useColors()
  return (
    <label className="block">
      <div className="text-[11px] uppercase tracking-[0.14em] mb-2" style={{ color: colors.textTertiary }}>
        {label}
      </div>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full h-11 px-3 rounded-xl text-[13px]"
        style={{
          background: colors.containerBg,
          color: colors.textPrimary,
          border: `1px solid ${colors.containerBorder}`,
        }}
      />
    </label>
  )
}

function ProfileArea({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  const colors = useColors()
  return (
    <label className="block">
      <div className="text-[11px] uppercase tracking-[0.14em] mb-2" style={{ color: colors.textTertiary }}>
        {label}
      </div>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        rows={4}
        className="w-full px-3 py-3 rounded-xl text-[13px] resize-none"
        style={{
          background: colors.containerBg,
          color: colors.textPrimary,
          border: `1px solid ${colors.containerBorder}`,
        }}
      />
    </label>
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
      className="rounded-xl h-full min-h-0 flex flex-col overflow-hidden"
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

function ExternalLinkButton({ href, label, mode = 'external' }: { href: string; label: string; mode?: 'external' | 'browser' }) {
  const colors = useColors()
  return (
    <button
      onClick={() => void invoke(mode === 'browser' ? 'browser_navigate_command' : 'open_external_command', { url: href })}
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

function draftToProfile(draft: StudentProfileDraft): StudentProfile {
  return {
    full_name: draft.full_name.trim(),
    degree_program: draft.degree_program.trim(),
    semester: draft.semester.trim(),
    about_me: draft.about_me.trim(),
    additional_context: draft.additional_context.trim(),
    top_priorities: splitList(draft.top_priorities),
    must_not_ignore: splitList(draft.must_not_ignore),
    important_courses: splitList(draft.important_courses),
    default_help: draft.default_help.trim(),
    reminder_style: draft.reminder_style.trim(),
    output_style: draft.output_style.trim(),
    documents: draft.documents,
  }
}

function profileToDraft(profile: StudentProfile): StudentProfileDraft {
  return {
    full_name: profile.full_name,
    degree_program: profile.degree_program,
    semester: profile.semester,
    about_me: profile.about_me,
    additional_context: profile.additional_context,
    top_priorities: profile.top_priorities.join(', '),
    must_not_ignore: profile.must_not_ignore.join(', '),
    important_courses: profile.important_courses.join(', '),
    default_help: profile.default_help,
    reminder_style: profile.reminder_style,
    output_style: profile.output_style,
    documents: profile.documents,
  }
}

function splitList(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
}

function parseToolName(raw: string): string {
  try {
    const parsed = JSON.parse(raw)
    if (parsed && typeof parsed.name === 'string') {
      return parsed.name
    }
  } catch {}

  const colonIndex = raw.indexOf(':')
  return colonIndex >= 0 ? raw.slice(0, colonIndex).trim() : raw.trim()
}

function summarizeToolResult(raw: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return ''
  return trimmed.length > 140 ? `${trimmed.slice(0, 137)}...` : trimmed
}

function mergeDocuments(current: ProfileDocument[], incoming: ProfileDocument[]): ProfileDocument[] {
  const byPath = new Map<string, ProfileDocument>()
  for (const document of current) {
    byPath.set(document.path, document)
  }
  for (const document of incoming) {
    byPath.set(document.path, document)
  }
  return Array.from(byPath.values())
}
