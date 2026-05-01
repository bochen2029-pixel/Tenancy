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

/// Per-user-message delivery state. Telegram-style two-checkmark indicator.
/// `delivered` fires when the harness has confirmed llama-server is reachable
/// (real connection check, not fake). `read` fires after the harness has
/// completed reading the message into Dave's pipeline (real triage decision,
/// gated by a length-proportional read delay). `read` implies `delivered`.
export type MessageDeliveryState = {
  delivered: boolean;
  read: boolean;
};

interface DaveState {
  ready: boolean;
  initError: string | null;
  conversationId: number | null;
  bufferSize: number;

  messages: Message[];
  /// Map of message id → delivery state. Only populated for user messages.
  /// Backend fires dave:message_delivered + dave:message_read against the
  /// user_msg.id; the streamConsumer flips entries in this map.
  ///
  /// Optimistically-rendered user messages (negative ids before the backend
  /// returns the persisted id) carry no entry — they render with no
  /// checkmark at all until the backend assigns a real id and emits the
  /// delivered event for it.
  deliveryByMessageId: Record<number, MessageDeliveryState>;

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
  reconcileOptimisticUserId: (realId: number, content: string) => void;
  markMessageDelivered: (messageId: number) => void;
  markMessageRead: (messageId: number) => void;
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
  deliveryByMessageId: {},
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
    // Composer is ALWAYS available — Dave can refuse on his side, but he
    // cannot prevent the user from sending. The only gate is "do we have a
    // conversation to send to."
    if (!state.conversationId) return;
    const trimmed = text.trim();
    if (!trimmed) return;

    const optimisticUser: Message = {
      id: -Date.now(),
      conversation_id: state.conversationId,
      role: 'user',
      content: trimmed,
      created_at: Math.floor(Date.now() / 1000),
    };

    // NOTE: do NOT flip isStreaming=true here. The TypingIndicator should
    // only appear when Dave is actually about to type — that happens via the
    // dave:stream_start event (which only fires on Respond/ForcedRespond,
    // AFTER the read delay and triage decision). Setting isStreaming=true at
    // send time would show the indicator during the read-pending phase,
    // which is wrong: Dave hasn't even decided to respond yet.
    set({
      messages: [...state.messages, optimisticUser],
    });

    ipc.reportUserPresent().catch(() => {});

    try {
      await ipc.sendToDave(state.conversationId, trimmed);
    } catch (e) {
      console.error('send failed:', e);
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

  reconcileOptimisticUserId: (realId, content) =>
    set((state) => {
      // Find the most recent optimistic user message (negative id) whose
      // content matches. Replace its id with the real DB id so subsequent
      // delivered/read events can target it.
      const idx = [...state.messages]
        .reverse()
        .findIndex((m) => m.role === 'user' && m.id < 0 && m.content === content);
      if (idx < 0) return state;
      const realIdx = state.messages.length - 1 - idx;
      const updated = state.messages.slice();
      updated[realIdx] = { ...updated[realIdx], id: realId };
      return { messages: updated };
    }),

  markMessageDelivered: (messageId) =>
    set((state) => {
      const prior = state.deliveryByMessageId[messageId];
      // Idempotent: if already delivered or read, no-op.
      if (prior?.delivered) return state;
      return {
        deliveryByMessageId: {
          ...state.deliveryByMessageId,
          [messageId]: { delivered: true, read: prior?.read ?? false },
        },
      };
    }),

  markMessageRead: (messageId) =>
    set((state) => {
      const prior = state.deliveryByMessageId[messageId];
      if (prior?.read) return state;
      // Read implies delivered. If somehow read fires before delivered
      // (shouldn't happen but defensive), set both true.
      return {
        deliveryByMessageId: {
          ...state.deliveryByMessageId,
          [messageId]: { delivered: true, read: true },
        },
      };
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
