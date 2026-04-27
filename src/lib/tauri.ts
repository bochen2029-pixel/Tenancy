import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Role = 'user' | 'assistant';

export type Message = {
  id: number;
  conversation_id: number;
  role: Role;
  content: string;
  created_at: number;
};

export type JournalKind = 'idle' | 'departure' | 'startup';

export type JournalEntry = {
  id: number;
  created_at: number;
  type: JournalKind;
  content: string;
  surfaced_at: number | null;
};

export type OutreachDrop = {
  id: number;
  conversation_id: number;
  generated_at: number;
  content: string;
  drop_reason: string;
  heuristic_pass: boolean;
  llm_score: number | null;
  history_shape: string | null;
  last_user_input: number;
};

export type ConsolidationEpoch = {
  id: number;
  conversation_id: number;
  epoch_number: number;
  period_start_message_id: number;
  period_end_message_id: number;
  content: string;
  token_count: number;
  consolidation_depth: number;
  created_at: number;
  superseded_by: number | null;
};

export type MemoryEdit = {
  id: number;
  conversation_id: number;
  edit_type: string;
  target_id: number | null;
  prior_content: string | null;
  new_content: string | null;
  reason: string;
  created_at: number;
};

export type MiddleBlock =
  | { kind: 'epoch'; epoch: ConsolidationEpoch }
  | { kind: 'messages'; messages: Message[] };

export type PartitionView = {
  conversation_id: number;
  system_prompt: string;
  anchor: Message[];
  canvas: string;
  middle: MiddleBlock[];
  recent: Message[];
  anchor_tokens: number;
  canvas_tokens: number;
  middle_tokens: number;
  recent_tokens: number;
  total_tokens: number;
  token_budget_total: number;
  token_reserve: number;
  anchor_message_count: number;
  recent_message_target: number;
  recent_message_trigger: number;
};

export const ipc = {
  sendToDave: (conversationId: number, userText: string) =>
    invoke<void>('send_to_dave', { conversationId, userText }),
  startNewConversation: () => invoke<number>('start_new_conversation'),
  latestOrNewConversation: () => invoke<number>('latest_or_new_conversation'),
  loadRecentMessages: (conversationId: number, limit?: number) =>
    invoke<Message[]>('load_recent_messages', { conversationId, limit }),
  loadUnreadJournal: () => invoke<JournalEntry[]>('load_unread_journal'),
  loadAllJournal: () => invoke<JournalEntry[]>('load_all_journal'),
  markJournalSurfaced: (id: number) =>
    invoke<void>('mark_journal_surfaced', { id }),
  reportUserPresent: () => invoke<void>('report_user_present'),
  departureEntry: () => invoke<JournalEntry | null>('departure_entry'),
  ensureStartupEntry: () => invoke<JournalEntry | null>('ensure_startup_entry'),
  bufferSize: () => invoke<number>('buffer_size'),
  loadOutreachDrops: (limit?: number) =>
    invoke<OutreachDrop[]>('load_outreach_drops', { limit }),
  getSetting: (key: string) =>
    invoke<string | null>('get_setting', { key }),
  setSetting: (key: string, value: string) =>
    invoke<void>('set_setting', { key, value }),
  injectTestConversation: () =>
    invoke<number>('inject_test_conversation'),
  clearAllData: () => invoke<void>('clear_all_data'),
  exportDatabase: () => invoke<string>('export_database'),
  loadPartitionView: (conversationId: number) =>
    invoke<PartitionView>('load_partition_view', { conversationId }),
  listAllEpochs: (conversationId: number) =>
    invoke<ConsolidationEpoch[]>('list_all_epochs_cmd', { conversationId }),
  editEpochContent: (conversationId: number, epochId: number, newContent: string, reason: string) =>
    invoke<void>('edit_epoch_content', { conversationId, epochId, newContent, reason }),
  manualConsolidateRange: (
    conversationId: number,
    rangeStartMessageId: number,
    rangeEndMessageId: number,
    reason: string,
  ) =>
    invoke<ConsolidationEpoch>('manual_consolidate_range', {
      conversationId, rangeStartMessageId, rangeEndMessageId, reason,
    }),
  listMemoryEdits: (conversationId: number, limit?: number) =>
    invoke<MemoryEdit[]>('list_memory_edits_cmd', { conversationId, limit }),
  revertMemoryEdit: (editId: number, reason: string) =>
    invoke<void>('revert_memory_edit', { editId, reason }),
  getMemoryCanvas: (conversationId: number) =>
    invoke<string>('get_memory_canvas', { conversationId }),
  setMemoryCanvas: (conversationId: number, content: string, reason: string) =>
    invoke<void>('set_memory_canvas', { conversationId, content, reason }),
  editMessageContent: (conversationId: number, messageId: number, newContent: string, reason: string) =>
    invoke<void>('edit_message_content', { conversationId, messageId, newContent, reason }),
};

export type Unlisten = UnlistenFn;

export const events = {
  onStreamStart: (cb: () => void) =>
    listen('dave:stream_start', () => cb()),
  onToken: (cb: (chunk: string) => void) =>
    listen<string>('dave:token', (e) => cb(e.payload)),
  onStreamEnd: (cb: () => void) =>
    listen('dave:stream_end', () => cb()),
  onStreamAborted: (cb: () => void) =>
    listen('dave:stream_aborted', () => cb()),
  onJournalArrived: (cb: (entry: JournalEntry) => void) =>
    listen<JournalEntry>('dave:journal_arrived', (e) => cb(e.payload)),
  onReady: (cb: () => void) => listen('dave:ready', () => cb()),
  onInitError: (cb: (msg: string) => void) =>
    listen<string>('dave:init_error', (e) => cb(e.payload)),
  onDbReset: (cb: () => void) => listen('dave:db_reset', () => cb()),
};

export const SETTING_KEY_OUTREACH_THRESHOLD = 'outreach_threshold_seconds';
