export interface ContentCard {
  readonly title: string
  readonly text: string
}

export interface WorkflowItem {
  readonly step: string
  readonly title: string
  readonly text: string
}

export interface ComparisonCard {
  readonly label: string
  readonly text: string
  readonly emphasis?: boolean
}

export const heroTags = ['Local-first', 'Student-specific', 'Stateful', 'Executional'] as const

export const capabilities: readonly ContentCard[] = [
  {
    title: 'Acts on the real workspace',
    text: 'Imprint works inside actual files, browser tabs, and machine context instead of behaving like a detached assistant.',
  },
  {
    title: 'Built for student workflows',
    text: 'It is tuned for coursework, applications, research, writing, and the repetitive admin that usually fragments a student’s time.',
  },
  {
    title: 'Execution over suggestion',
    text: 'The agent can inspect state, decide on the next step, and carry the task forward instead of stopping at advice.',
  },
] as const

export const pillars: readonly string[] = [
  'Local-first by default, so files and persistent state stay on the student machine.',
  'Stateful across sessions, which means progress and prior decisions remain available later.',
  'Transparent while running, with visible actions and understandable execution steps.',
  'Designed for long, messy real work rather than one-shot prompt exchanges.',
] as const

export const comparisonCards: readonly ComparisonCard[] = [
  {
    label: 'Without state',
    text: 'Students repeat context, manually reconstruct progress, and lose momentum between sessions.',
  },
  {
    label: 'With Imprint',
    text: 'The system resumes from prior work and helps complete the next meaningful action with less re-briefing.',
    emphasis: true,
  },
] as const

export const workflow: readonly WorkflowItem[] = [
  {
    step: '01',
    title: 'Observe',
    text: 'Imprint reads the current workspace, active task, and relevant materials before doing anything else.',
  },
  {
    step: '02',
    title: 'Reason',
    text: 'It forms a plan around what the student is actually trying to finish, not only the latest message.',
  },
  {
    step: '03',
    title: 'Execute',
    text: 'It uses the browser, files, and local machine context to move the task forward step by step.',
  },
  {
    step: '04',
    title: 'Carry forward',
    text: 'The next session starts with continuity, which is why the product stays useful over time.',
  },
] as const
