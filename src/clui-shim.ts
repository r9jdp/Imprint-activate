/**
 * Compatibility shim: stubs out every `window.clui.*` method that the
 * Electron-based UI calls. In Tauri, we bridge these calls to our Rust 
 * backend using events and commands.
 */

import { getCurrentWindow, LogicalPosition, LogicalSize, currentMonitor, cursorPosition } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const noop = () => {}
const asyncNoop = async () => {}
const noopUnsub = () => noop  // returns an unsubscribe function

let eventListeners: Array<(tabId: string, event: any) => void> = [];
let statusListeners: Array<(tabId: string, newStatus: string, oldStatus: string) => void> = [];
let errorListeners: Array<(tabId: string, error: any) => void> = [];
let isListening = false;

// ─── Click-through state ───
let _ignoring = false;      // current setIgnoreCursorEvents state
let _dragging = false;      // native drag in progress
let _cachedWinX = 0;        // window outer position, physical px
let _cachedWinY = 0;
let _cachedSf = 1;          // monitor scale factor
let _autoHideRunDepth = 0;  // track overlapping runs so we only re-show after the last one

const SESSION_STORE_KEY = 'imprint_sessions_v1';
const INSTALLED_PLUGINS_KEY = 'imprint_installed_plugins_v1';

type SessionMessage = { role: string; content: string; toolName?: string; timestamp: number };
type SessionRecord = { sessionId: string; firstMessage: string | null; lastTimestamp: string; size: number; messages: SessionMessage[]; projectPath?: string };

const tabToSession = new Map<string, string>();
const tabPendingPrompt = new Map<string, string>();
const tabPendingReasoning = new Map<string, string>();
const tabPendingTaskSwitch = new Map<string, {
  previousTask: string;
  newTask: string;
  reason: string;
  necessary: boolean;
}>();

function normalizeAttachment(raw: any) {
  if (!raw) return null;
  const mimeType = raw.mimeType || raw.mime_type || undefined;
  const type = raw.type || raw.kind || (mimeType?.startsWith('image/') ? 'image' : 'file');
  return {
    id: String(raw.id || crypto.randomUUID()),
    type,
    name: String(raw.name || 'attachment'),
    path: String(raw.path || ''),
    mimeType,
    dataUrl: raw.dataUrl || raw.data_url || undefined,
    size: typeof raw.size === 'number' ? raw.size : undefined,
  };
}

async function pickFilesFallback(): Promise<any[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.style.display = 'none';
    document.body.appendChild(input);

    input.onchange = () => {
      const files = Array.from(input.files || []);
      const mapped = files.map((f) => ({
        id: crypto.randomUUID(),
        type: (f.type || '').startsWith('image/') ? 'image' : 'file',
        name: f.name,
        path: f.name,
        mimeType: f.type || undefined,
        size: typeof f.size === 'number' ? f.size : undefined,
      }));
      document.body.removeChild(input);
      resolve(mapped);
    };

    input.oncancel = () => {
      document.body.removeChild(input);
      resolve([]);
    };

    input.click();
  });
}

function loadSessions(): Record<string, SessionRecord> {
  try {
    const raw = localStorage.getItem(SESSION_STORE_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as Record<string, SessionRecord>;
  } catch {
    return {};
  }
}

function saveSessions(data: Record<string, SessionRecord>) {
  localStorage.setItem(SESSION_STORE_KEY, JSON.stringify(data));
}

function upsertSession(sessionId: string, updater: (prev: SessionRecord | undefined) => SessionRecord) {
  const all = loadSessions();
  all[sessionId] = updater(all[sessionId]);
  saveSessions(all);
}

function getInstalledPlugins(): string[] {
  try {
    const raw = localStorage.getItem(INSTALLED_PLUGINS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function setInstalledPlugins(list: string[]) {
  localStorage.setItem(INSTALLED_PLUGINS_KEY, JSON.stringify(Array.from(new Set(list))));
}

function formatHistoryForGemini(messages: SessionMessage[]): any[] {
  return messages.map((m) => {
    if (m.role === 'user') {
      return { role: 'user', parts: [{ text: m.content }] };
    } else if (m.role === 'assistant') {
      return { role: 'model', parts: [{ text: m.content }] };
    }
    return null;
  }).filter(Boolean);
}

async function ensureListener() {
  if (isListening) return;
  isListening = true;
  await listen<{ kind: string, content: string }>('agent_event', (e) => {
    const activeTabId = (window as any)._clui_last_tab_id || 'default-tab';
    const { kind, content } = e.payload;
    const sessionId = tabToSession.get(activeTabId) || 'local';

    if (kind === 'tool_call') {
      // New structured format from Rust: JSON { name, args, human_desc, risk_tag }
      // Legacy fallback remains: "tool_name: {json args}"
      let toolName = content.trim();
      let argsRaw = '{}';
      let humanDesc: string | undefined;
      try {
        const parsed = JSON.parse(content);
        if (parsed && typeof parsed === 'object') {
          if (typeof parsed.name === 'string') toolName = parsed.name;
          if (parsed.args && typeof parsed.args === 'object') {
            argsRaw = JSON.stringify(parsed.args);
          }
          if (typeof parsed.human_desc === 'string') {
            humanDesc = parsed.human_desc;
          }
        }
      } catch {
        const colonIdx = content.indexOf(':');
        toolName = colonIdx >= 0 ? content.substring(0, colonIdx).trim() : content.trim();
        argsRaw = colonIdx >= 0 ? content.substring(colonIdx + 1).trim() : '{}';
      }

      const toolId = crypto.randomUUID();
      const reasoning = tabPendingReasoning.get(activeTabId) || undefined;
      const taskSwitch = tabPendingTaskSwitch.get(activeTabId);
      tabPendingReasoning.delete(activeTabId);
      tabPendingTaskSwitch.delete(activeTabId);

      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'tool_call',
        toolName: toolName,
        toolId: toolId,
        index: 0,
        reasoning,
        taskSwitch,
        humanDescription: humanDesc,
      }));
      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'tool_call_update',
        toolId: toolId,
        // Pass clean JSON args string so getToolDescription can parse it
        partialInput: argsRaw
      }));
    } else if (kind === 'reasoning') {
      const text = (content || '').trim();
      if (text) {
        tabPendingReasoning.set(activeTabId, text);
        eventListeners.forEach(fn => fn(activeTabId, {
          type: 'reasoning_step',
          text,
        }));
      }
    } else if (kind === 'task_switch') {
      try {
        const parsed = JSON.parse(content || '{}');
        const taskSwitch = {
          previousTask: String(parsed.previous_task || parsed.previousTask || 'Previous step'),
          newTask: String(parsed.new_task || parsed.newTask || 'Next step'),
          reason: String(parsed.reason || 'The model switched capability to continue.'),
          necessary: Boolean(parsed.necessary),
        };
        tabPendingTaskSwitch.set(activeTabId, taskSwitch);
        eventListeners.forEach(fn => fn(activeTabId, {
          type: 'task_switch',
          ...taskSwitch,
        }));
      } catch {
        const fallback = {
          previousTask: 'Previous step',
          newTask: 'Next step',
          reason: content || 'The model switched capability to continue.',
          necessary: true,
        };
        tabPendingTaskSwitch.set(activeTabId, fallback);
      }
    } else if (kind === 'tool_result') {
      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'tool_call_complete',
        index: 0,
        output: content,
      }));
    } else if (kind === 'message') {
      upsertSession(sessionId, (prev) => {
        const nowIso = new Date().toISOString();
        const prompt = tabPendingPrompt.get(activeTabId) || prev?.firstMessage || null;
        return {
          sessionId,
          firstMessage: prev?.firstMessage || prompt,
          lastTimestamp: nowIso,
          size: (prev?.size || 0) + content.length,
          projectPath: prev?.projectPath,
          messages: [...(prev?.messages || []), { role: 'assistant', content, timestamp: Date.now() }],
        };
      });

      // Send text chunks
      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'text_chunk',
        text: content
      }));
      // End the task and flush the message
      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'task_update',
        message: {
          model: 'imprint',
          id: crypto.randomUUID(),
          role: 'assistant',
          content: [{ type: 'text', text: content }],
          stop_reason: null,
          usage: {}
        }
      }));
    } else if (kind === 'done') {
      tabPendingPrompt.delete(activeTabId);
      tabPendingReasoning.delete(activeTabId);
      tabPendingTaskSwitch.delete(activeTabId);
      statusListeners.forEach(fn => fn(activeTabId, 'completed', 'running'));
      eventListeners.forEach(fn => fn(activeTabId, {
        type: 'task_complete',
        result: 'Task completed.',
        costUsd: 0,
        durationMs: 0,
        numTurns: 1,
        usage: {},
        sessionId: 'local'
      }));
    } else if (kind === 'error') {
      tabPendingReasoning.delete(activeTabId);
      tabPendingTaskSwitch.delete(activeTabId);
      statusListeners.forEach(fn => fn(activeTabId, 'failed', 'running'));
      errorListeners.forEach(fn => fn(activeTabId, {
        message: content,
        stderrTail: [],
        exitCode: 1,
        elapsedMs: 0,
        toolCallCount: 0
      }));
    }
  });
}

;(window as any).clui = {
  // ─── Lifecycle / info ───
  start: async () => ({
    version: '1.0.0',
    auth: { email: null, subscriptionType: null },
    projectPath: '~',
    homePath: '~',
  }),
  isVisible: async () => true,

  // ─── Tabs ───
  createTab: async () => ({ tabId: crypto.randomUUID() }),
  closeTab: asyncNoop,
  resetTabSession: noop,
  stopTab: async () => {
    try {
      await invoke('interrupt_agent');
    } catch (err) {
      console.error(err);
    }
  },

  // ─── Prompts / sessions ───
  prompt: async (tabId: string, _requestId: string, opts: any) => {
    (window as any)._clui_last_tab_id = tabId;
    ensureListener();

    _autoHideRunDepth += 1;
    if (_autoHideRunDepth === 1) {
      try {
        const win = getCurrentWindow();
        await win.minimize();
      } catch {
        try {
          await getCurrentWindow().hide();
        } catch {}
      }
    }

    try {

      const sessionId = tabToSession.get(tabId) || crypto.randomUUID();
      tabToSession.set(tabId, sessionId);
      tabPendingPrompt.set(tabId, opts.prompt || '');

      upsertSession(sessionId, (prev) => {
        const nowIso = new Date().toISOString();
        const userMsg = String(opts.prompt || '');
        return {
          sessionId,
          firstMessage: prev?.firstMessage || userMsg || null,
          lastTimestamp: nowIso,
          size: (prev?.size || 0) + userMsg.length,
          projectPath: opts.projectPath,
          messages: [...(prev?.messages || []), { role: 'user', content: userMsg, timestamp: Date.now() }],
        };
      });

      eventListeners.forEach(fn => fn(tabId, {
        type: 'session_init',
        sessionId,
        tools: [],
        model: 'gemini-3.1-pro-preview',
        mcpServers: [],
        skills: [],
        version: '1.0',
        isWarmup: false
      }));
      statusListeners.forEach(fn => fn(tabId, 'running', 'idle'));

      const apiKey = localStorage.getItem('imprint_api_key') || "";
      /*
      if (!apiKey) {
        errorListeners.forEach(fn => fn(tabId, {
          message: "API Key is missing. Please enter it in the settings.",
          stderrTail: [], exitCode: 1, elapsedMs: 0, toolCallCount: 0
        }));
        return;
      }
      */

      try {
        const all = loadSessions();
        const session = all[sessionId];
        const history = session ? formatHistoryForGemini(session.messages) : [];

        await invoke('run_agent_command', {
          prompt: opts.prompt,
          history,
          apiKey
        });
      } catch (e) {
        errorListeners.forEach(fn => fn(tabId, {
          message: String(e),
          stderrTail: [], exitCode: 1, elapsedMs: 0, toolCallCount: 0
        }));
      }
    } finally {
      _autoHideRunDepth = Math.max(0, _autoHideRunDepth - 1);
      if (_autoHideRunDepth === 0) {
        try {
          const win = getCurrentWindow();
          await win.unminimize();
          await win.show();
        } catch {
          try {
            await getCurrentWindow().show();
          } catch {}
        }
      }
    }
  },
  respondPermission: asyncNoop,
  loadSession: async (sessionId: string) => {
    const all = loadSessions();
    const s = all[sessionId];
    if (!s) return [];
    return s.messages.map((m) => ({
      role: m.role,
      content: m.content,
      toolName: m.toolName,
      timestamp: m.timestamp,
    }));
  },
  listSessions: async (projectPath?: string) => {
    const all = loadSessions();
    return Object.values(all)
      .filter((s) => !projectPath || !s.projectPath || s.projectPath === projectPath)
      .sort((a, b) => new Date(b.lastTimestamp).getTime() - new Date(a.lastTimestamp).getTime())
      .map((s) => ({
        sessionId: s.sessionId,
        slug: null,
        firstMessage: s.firstMessage,
        lastTimestamp: s.lastTimestamp,
        size: s.size,
      }));
  },
  setPermissionMode: noop,

  // ─── Theme ───
  getTheme: async () => ({ isDark: true }),
  onThemeChange: noopUnsub,

  // ─── Window manipulation ───
  setIgnoreMouseEvents: async (ignore: boolean) => {
    try {
      _ignoring = ignore;
      if (getCurrentWindow().setIgnoreCursorEvents) {
        await getCurrentWindow().setIgnoreCursorEvents(ignore);
      }
    } catch (e) {
      console.error(e);
    }
  },
  startWindowDrag: asyncNoop,
  startDraggingNative: async () => {
    _dragging = true;
    try {
      await getCurrentWindow().startDragging();
    } catch (e) {
      console.error(e);
    }
    // Fallback: clear drag flag after 2s in case mouseup doesn't fire in webview
    setTimeout(() => { _dragging = false; }, 2000);
  },
  resetWindowPosition: async () => {
    try {
      const win = getCurrentWindow();
      const monitor = await currentMonitor();
      if (monitor) {
        const sf = monitor.scaleFactor || 1;
        const outer = await win.outerSize();
        const winWidth = outer.width / sf;
        const winHeight = outer.height / sf;

        // Use OS-reported available work area so we stay above taskbar/dock.
        const screenAny = window.screen as Screen & { availLeft?: number; availTop?: number };
        const availLeft = typeof screenAny.availLeft === 'number' ? screenAny.availLeft : 0;
        const availTop = typeof screenAny.availTop === 'number' ? screenAny.availTop : 0;
        const availWidth = typeof screenAny.availWidth === 'number' ? screenAny.availWidth : screenAny.width;
        const availHeight = typeof screenAny.availHeight === 'number' ? screenAny.availHeight : screenAny.height;

        const bottomGap = 4;
        const x = availLeft + (availWidth - winWidth) / 2;
        const y = availTop + availHeight - winHeight - bottomGap;

        await win.setPosition(new LogicalPosition(x, y));
      }
    } catch (e) {
      console.error(e);
    }
  },
  resizeWindow: async (width: number, height: number) => {
    try {
      const win = getCurrentWindow();
      await win.setSize(new LogicalSize(width, height));
      // Re-anchor to bottom-center after resize
      const monitor = await currentMonitor();
      if (monitor) {
        const sf = monitor.scaleFactor || 1;
        const outer = await win.outerSize();
        const winWidth = outer.width / sf;
        const winHeight = outer.height / sf;

        const screenAny = window.screen as Screen & { availLeft?: number; availTop?: number };
        const availLeft = typeof screenAny.availLeft === 'number' ? screenAny.availLeft : 0;
        const availTop = typeof screenAny.availTop === 'number' ? screenAny.availTop : 0;
        const availWidth = typeof screenAny.availWidth === 'number' ? screenAny.availWidth : screenAny.width;
        const availHeight = typeof screenAny.availHeight === 'number' ? screenAny.availHeight : screenAny.height;

        const bottomGap = 4;
        const x = availLeft + (availWidth - winWidth) / 2;
        const y = availTop + availHeight - winHeight - bottomGap;

        await win.setPosition(new LogicalPosition(x, y));
      }
    } catch (e) {
      console.error(e);
    }
  },
  hideWindow: async () => {
    try {
      await getCurrentWindow().hide();
    } catch (e) {
      console.error(e);
    }
  },
  onWindowShown: noopUnsub,

  // ─── File / attachment helpers ───
  takeScreenshot: async () => {
    try {
      const win = getCurrentWindow();
      await win.hide();
      // Give compositor time to hide app before capture UX opens.
      await new Promise((r) => setTimeout(r, 180));
      const result = await invoke<any>('take_screenshot_command');
      await win.show();
      await win.setFocus();
      return normalizeAttachment(result) || null;
    } catch (e) {
      try {
        const win = getCurrentWindow();
        await win.show();
        await win.setFocus();
      } catch {}
      console.error(e);
      return null;
    }
  },
  attachFiles: async () => {
    try {
      const result = await invoke<any[]>('attach_files_command');
      const normalized = Array.isArray(result)
        ? result
        .map(normalizeAttachment)
        .filter((a): a is NonNullable<ReturnType<typeof normalizeAttachment>> => a !== null)
        .filter((a) => a.path || a.name)
        : [];
      if (normalized.length > 0) return normalized;
      // Some environments can return an empty native payload even after a file is chosen.
      // Fall back to the browser picker to keep attachment UX reliable.
      return pickFilesFallback();
    } catch (e) {
      console.error(e);
      return pickFilesFallback();
    }
  },
  pasteImage: async (dataUrl: string) => {
    try {
      if (!dataUrl || !dataUrl.startsWith('data:image/')) return null;
      const mime = dataUrl.substring(5, dataUrl.indexOf(';')) || 'image/png';
      const b64 = dataUrl.split(',')[1] || '';
      const size = Math.floor((b64.length * 3) / 4);
      const ext = mime.split('/')[1] || 'png';
      return {
        id: crypto.randomUUID(),
        type: 'image',
        name: `pasted-image.${ext}`,
        path: `clipboard://pasted-image-${Date.now()}.${ext}`,
        mimeType: mime,
        dataUrl,
        size,
      };
    } catch {
      return null;
    }
  },
  selectDirectory: async () => {
    try {
      const result = await invoke<string | null>('select_directory_command');
      return result || null;
    } catch {
      return null;
    }
  },

  // ─── External ───
  openExternal: async (url: string) => {
    try {
      await invoke('open_external_command', { url });
    } catch (e) {
      console.error(e);
    }
  },
  openInTerminal: async (_sessionId: string | null, path: string) => {
    try {
      await invoke('open_in_terminal_command', { path });
    } catch (e) {
      console.error(e);
    }
  },
  transcribeAudio: async (audioBase64: string) => {
    try {
      const result = await invoke<{ transcript?: string; error?: string }>('transcribe_audio_command', {
        audioBase64,
      });
      return {
        transcript: result?.transcript || '',
        error: result?.error || undefined,
      };
    } catch (e) {
      return {
        transcript: '',
        error: String(e),
      };
    }
  },

  // ─── Marketplace ───
  fetchMarketplace: async () => ({
    plugins: [
      {
        id: 'local/filesystem-automation',
        name: 'Filesystem Automation',
        description: 'Automate file and folder tasks across platforms.',
        version: '1.0.0',
        author: 'Imprint',
        marketplace: 'Local',
        repo: 'local/imprint',
        sourcePath: 'skills/filesystem',
        installName: 'filesystem-automation',
        category: 'Agent Skills',
        tags: ['Automation', 'Files'],
        isSkillMd: true,
      },
      {
        id: 'local/terminal-automation',
        name: 'Terminal Automation',
        description: 'Run and orchestrate shell tasks safely.',
        version: '1.0.0',
        author: 'Imprint',
        marketplace: 'Local',
        repo: 'local/imprint',
        sourcePath: 'skills/terminal',
        installName: 'terminal-automation',
        category: 'Agent Skills',
        tags: ['Automation', 'Terminal'],
        isSkillMd: true,
      },
    ],
    error: null,
  }),
  listInstalledPlugins: async () => getInstalledPlugins(),
  installPlugin: async (_repo: string, installName: string) => {
    const list = getInstalledPlugins();
    if (!list.includes(installName)) setInstalledPlugins([...list, installName]);
    return { ok: true };
  },
  uninstallPlugin: async (installName: string) => {
    const list = getInstalledPlugins().filter((n) => n !== installName);
    setInstalledPlugins(list);
    return { ok: true };
  },

  // ─── Events ───
  onEvent: (fn: any) => {
    eventListeners.push(fn);
    return () => { eventListeners = eventListeners.filter(l => l !== fn); };
  },
  onTabStatusChange: (fn: any) => {
    statusListeners.push(fn);
    return () => { statusListeners = statusListeners.filter(l => l !== fn); };
  },
  onError: (fn: any) => {
    errorListeners.push(fn);
    return () => { errorListeners = errorListeners.filter(l => l !== fn); };
  },
  onSkillStatus: noopUnsub,
  tabHealth: async () => ({}),
}

// ─── Click-through polling ───
// The Tauri window is wider than the visible UI. Transparent areas would normally
// swallow mouse events. We fix this by setting setIgnoreCursorEvents(true) when
// the cursor is over empty/transparent space, and false when it's over UI.
// We poll cursor position via the Tauri API because once setIgnoreCursorEvents(true)
// is active, the webview stops receiving DOM mouse events entirely.
;(async function initClickThrough() {
  const win = getCurrentWindow();

  // Keep window bounds cached so the poll only makes one IPC call per tick
  const updateBounds = async () => {
    try {
      const monitor = await currentMonitor();
      _cachedSf = monitor?.scaleFactor ?? 1;
      const pos = await win.outerPosition();
      _cachedWinX = pos.x;
      _cachedWinY = pos.y;
    } catch {}
  };

  await updateBounds();
  win.listen('tauri://move', updateBounds);
  win.listen('tauri://resize', updateBounds);

  // Clear drag flag when mouse button is released
  window.addEventListener('mouseup', () => { _dragging = false; });

  const poll = async () => {
    try {
      const cursor = await cursorPosition(); // PhysicalPosition
      const sf = _cachedSf;

      // Convert to logical CSS pixels relative to the window top-left
      const relX = (cursor.x - _cachedWinX) / sf;
      const relY = (cursor.y - _cachedWinY) / sf;

      const el = document.elementFromPoint(relX, relY);
      const isOverUI = !!(el?.closest('[data-clui-ui]'));

      if (isOverUI && _ignoring && !_dragging) {
        _ignoring = false;
        await win.setIgnoreCursorEvents(false);
      } else if (!isOverUI && !_ignoring && !_dragging) {
        _ignoring = true;
        await win.setIgnoreCursorEvents(true);
      }
    } catch {}
    setTimeout(poll, 50);
  };

  // Start with click-through enabled so transparent areas are immediately passable
  _ignoring = true;
  await win.setIgnoreCursorEvents(true);
  poll();
})().catch(() => {});
