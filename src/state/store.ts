import { create } from 'zustand';
import { ipc, type JournalEntry, type Message } from '../lib/tauri';

// Mirrors src-tauri/src/leak.rs. Last line of defense.
function isHarnessLeak(text: string): boolean {
  return /^\s*\[(pass|meta|outreach|decision)/i.test(text);
}

// Mirrors src-tauri/src/think_strip.rs. Defense-in-depth — backend strips
// at the SDK boundary, but if a <think>...</think> block ever reaches here
// we strip it before persistence/render.
function stripThink(text: string): string {
  return text.replace(/<think\b[^>]*>[\s\S]*?<\/think>\s*/gi, '').trim();
}

export type StoreMessage = Message;
export type StoreJournal = JournalEntry;

interface DaveState {
  ready: boolean;
  initError: string | null;
  conversationId: number | null;
  bufferSize: number;

  messages: Message[];

  isStreaming: boolean;
  pendingAssistant: string;

  inlineJournals: JournalEntry[];
  departure: JournalEntry | null;
  startupEntry: JournalEntry | null;
  journalPanelOpen: boolean;
  dropsPanelOpen: boolean;
  settingsPanelOpen: boolean;
  memoryPanelOpen: boolean;

  init: () => Promise<void>;
  setReady: (v: boolean) => void;
  setInitError: (msg: string) => void;
  send: (text: string) => Promise<void>;
  appendChar: (char: string) => void;
  finalizeAssistant: () => void;
  receiveInlineJournal: (entry: JournalEntry) => void;
  toggleJournalPanel: () => void;
  closeJournalPanel: () => void;
  toggleDropsPanel: () => void;
  closeDropsPanel: () => void;
  toggleSettingsPanel: () => void;
  closeSettingsPanel: () => void;
  toggleMemoryPanel: () => void;
  closeMemoryPanel: () => void;
  reloadAfterDbReset: () => Promise<void>;
}

export const useDaveStore = create<DaveState>((set, get) => ({
  ready: false,
  initError: null,
  conversationId: null,
  bufferSize: 60,
  messages: [],
  isStreaming: false,
  pendingAssistant: '',
  inlineJournals: [],
  departure: null,
  startupEntry: null,
  journalPanelOpen: false,
  dropsPanelOpen: false,
  settingsPanelOpen: false,
  memoryPanelOpen: false,

  setReady: (v) => set({ ready: v }),
  setInitError: (msg) => set({ initError: msg }),

  init: async () => {
    let conversationId: number | null = null;
    const start = Date.now();
    while (Date.now() - start < 180_000) {
      if (get().initError) {
        set({ ready: true });
        return;
      }
      try {
        conversationId = await ipc.latestOrNewConversation();
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 500));
      }
    }
    if (conversationId === null) {
      set({
        initError: 'Backend did not become ready. Check that the model file is in place.',
        ready: true,
      });
      return;
    }

    try {
      const bufferSize = await ipc.bufferSize().catch(() => 60);
      const messages = await ipc.loadRecentMessages(conversationId);
      const departure = await ipc.departureEntry();
      const unread = await ipc.loadUnreadJournal();
      const inlineJournals = unread.filter((j) => j.type === 'idle');

      let startupEntry: JournalEntry | null = null;
      if (!departure && inlineJournals.length === 0) {
        try {
          startupEntry = await ipc.ensureStartupEntry();
        } catch {
          startupEntry = null;
        }
      }

      if (departure) {
        ipc.markJournalSurfaced(departure.id).catch(() => {});
      }
      for (const j of inlineJournals) {
        ipc.markJournalSurfaced(j.id).catch(() => {});
      }
      if (startupEntry) {
        ipc.markJournalSurfaced(startupEntry.id).catch(() => {});
      }

      set({
        bufferSize,
        conversationId,
        messages,
        departure,
        inlineJournals,
        startupEntry,
        ready: true,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ initError: msg, ready: true });
    }
  },

  send: async (text) => {
    const state = get();
    if (state.isStreaming || !state.conversationId) return;
    const trimmed = text.trim();
    if (!trimmed) return;

    const optimisticUser: Message = {
      id: -Date.now(),
      conversation_id: state.conversationId,
      role: 'user',
      content: trimmed,
      created_at: Math.floor(Date.now() / 1000),
    };

    set({
      messages: [...state.messages, optimisticUser],
      isStreaming: true,
      pendingAssistant: '',
      // departure + startupEntry stay visible at the top of the column
      // for the whole session; they are the opening of the conversation,
      // not transient notifications. They do NOT re-show on next launch
      // (markJournalSurfaced was called at init time).
    });

    ipc.reportUserPresent().catch(() => {});

    try {
      await ipc.sendToDave(state.conversationId, trimmed);
    } catch (e) {
      console.error('send failed:', e);
      set({ isStreaming: false, pendingAssistant: '' });
    }
  },

  appendChar: (char) =>
    set((state) => ({ pendingAssistant: state.pendingAssistant + char })),

  finalizeAssistant: () =>
    set((state) => {
      if (!state.isStreaming) return state;
      // Defense-in-depth strip of any leaked <think>...</think> blocks
      // before harness-leak check / persistence / render.
      const text = stripThink(state.pendingAssistant);
      // Defense-in-depth: drop harness leaks at finalize. Backend already
      // filters via leak::is_harness_leak; this is the last line of defense
      // if anything slips through.
      if (!text || isHarnessLeak(text)) {
        return { isStreaming: false, pendingAssistant: '' };
      }
      const conversationId = state.conversationId ?? 0;
      const newMsg: Message = {
        id: Date.now(),
        conversation_id: conversationId,
        role: 'assistant',
        content: text,
        created_at: Math.floor(Date.now() / 1000),
      };
      return {
        isStreaming: false,
        pendingAssistant: '',
        messages: [...state.messages, newMsg],
      };
    }),

  receiveInlineJournal: (entry) =>
    set((state) => {
      ipc.markJournalSurfaced(entry.id).catch(() => {});
      return { inlineJournals: [...state.inlineJournals, entry] };
    }),

  toggleJournalPanel: () =>
    set((state) => ({ journalPanelOpen: !state.journalPanelOpen })),

  closeJournalPanel: () => set({ journalPanelOpen: false }),

  toggleDropsPanel: () =>
    set((state) => ({ dropsPanelOpen: !state.dropsPanelOpen })),

  closeDropsPanel: () => set({ dropsPanelOpen: false }),

  toggleSettingsPanel: () =>
    set((state) => ({ settingsPanelOpen: !state.settingsPanelOpen })),

  closeSettingsPanel: () => set({ settingsPanelOpen: false }),

  toggleMemoryPanel: () =>
    set((state) => ({ memoryPanelOpen: !state.memoryPanelOpen })),

  closeMemoryPanel: () => set({ memoryPanelOpen: false }),

  reloadAfterDbReset: async () => {
    // Re-init the store from the new DB state. Used by SettingsPanel after
    // inject-test-conversation or clear-all-data so the user sees the change
    // immediately without restarting the app.
    try {
      const conversationId = await ipc.latestOrNewConversation();
      const messages = await ipc.loadRecentMessages(conversationId);
      const departure = await ipc.departureEntry();
      const unread = await ipc.loadUnreadJournal();
      const inlineJournals = unread.filter((j) => j.type === 'idle');
      set({
        conversationId,
        messages,
        departure,
        inlineJournals,
        startupEntry: null,
        isStreaming: false,
        pendingAssistant: '',
      });
    } catch (e) {
      console.error('reload after db reset failed:', e);
    }
  },
}));
