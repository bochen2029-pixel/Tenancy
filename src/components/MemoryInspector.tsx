import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import {
  ipc,
  type ConsolidationEpoch,
  type MemoryEdit,
  type Message,
  type MiddleBlock,
  type PartitionView,
} from '../lib/tauri';
import { formatJournalDate } from '../lib/time';

// Ctrl+Shift+M memory inspector. Three tabs: Read (always available),
// Edit (requires explicit per-session toggle), History (audit log + revert).
//
// Rules:
// - System prompt is read-only here. Edit prompts.rs and rebuild for changes.
// - Every edit requires a free-text reason; the reason is logged.
// - Edits are append-only via memory_edits; revert creates a NEW edit row.

type Tab = 'read' | 'edit' | 'raw' | 'history';

export function MemoryInspector() {
  const open = useDaveStore((s) => s.memoryPanelOpen);
  const close = useDaveStore((s) => s.closeMemoryPanel);
  const conversationId = useDaveStore((s) => s.conversationId);

  const [tab, setTab] = useState<Tab>('read');
  const [partition, setPartition] = useState<PartitionView | null>(null);
  const [edits, setEdits] = useState<MemoryEdit[]>([]);
  const [editEnabled, setEditEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  async function refresh() {
    if (!conversationId) return;
    try {
      const p = await ipc.loadPartitionView(conversationId);
      setPartition(p);
      const e = await ipc.listMemoryEdits(conversationId, 200);
      setEdits(e);
    } catch (err) {
      console.error('memory inspector refresh failed:', err);
    }
  }

  useEffect(() => {
    if (!open) return;
    setStatusMsg(null);
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, conversationId]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.5)' }}
      onClick={close}
    >
      <div className="flex-1" />
      <aside
        className="h-full overflow-y-auto"
        style={{
          backgroundColor: 'var(--bg-elevated)',
          width: '720px',
          borderLeft: '1px solid var(--border-medium)',
          display: 'flex',
          flexDirection: 'column',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div style={{ padding: '24px 32px 0', flexShrink: 0 }}>
          <div className="status-bar mb-4" style={{ opacity: 0.6 }}>
            memory inspector
          </div>
          <div className="flex items-center gap-1 mb-4">
            <TabButton current={tab} value="read" label="read" onClick={setTab} />
            <TabButton current={tab} value="edit" label="edit" onClick={setTab} />
            <TabButton current={tab} value="raw" label="raw" onClick={setTab} />
            <TabButton current={tab} value="history" label="history" onClick={setTab} />
            <div style={{ flex: 1 }} />
            <button
              className="settings-button"
              onClick={refresh}
              style={{ padding: '4px 10px', fontSize: 11 }}
            >
              refresh
            </button>
          </div>
          {tab === 'edit' && !editEnabled && (
            <div
              className="mb-4 p-3"
              style={{
                border: '1px solid #d7826b',
                borderRadius: 3,
                fontSize: 12,
                color: 'var(--text-secondary)',
                background: 'rgba(215, 130, 107, 0.06)',
              }}
            >
              <p style={{ marginBottom: 8 }}>
                Edit mode lets you modify Dave's consolidated memories.
                Every edit is logged with your reason. Reverts are possible
                but the audit trail is permanent. The system prompt is not
                editable here — change <code>prompts.rs</code> and rebuild.
              </p>
              <button
                className="settings-button"
                style={{ padding: '6px 12px', fontSize: 12 }}
                onClick={() => setEditEnabled(true)}
              >
                enable edit mode for this session
              </button>
            </div>
          )}
        </div>

        {/* Body */}
        <div style={{ padding: '0 32px 32px', flex: 1, overflowY: 'auto' }}>
          {!partition ? (
            <p className="status-bar" style={{ opacity: 0.5 }}>
              loading…
            </p>
          ) : tab === 'read' ? (
            <ReadTab
              partition={partition}
              editEnabled={editEnabled}
              conversationId={conversationId ?? 0}
              busy={busy}
              setBusy={setBusy}
              onAfterEdit={async (msg) => {
                setStatusMsg(msg);
                await refresh();
              }}
            />
          ) : tab === 'edit' ? (
            <EditTab
              partition={partition}
              enabled={editEnabled}
              busy={busy}
              setBusy={setBusy}
              onAfterEdit={async (msg) => {
                setStatusMsg(msg);
                await refresh();
              }}
              conversationId={conversationId ?? 0}
            />
          ) : tab === 'raw' ? (
            <RawTab
              partition={partition}
              conversationId={conversationId ?? 0}
              busy={busy}
              setBusy={setBusy}
              onAfterEdit={async (msg) => {
                setStatusMsg(msg);
                await refresh();
              }}
            />
          ) : (
            <HistoryTab
              edits={edits}
              busy={busy}
              setBusy={setBusy}
              onAfterRevert={async (msg) => {
                setStatusMsg(msg);
                await refresh();
              }}
            />
          )}

          {statusMsg && (
            <div
              className="status-bar mt-6"
              style={{
                color: 'var(--text-secondary)',
                textTransform: 'none',
                fontSize: 12,
                opacity: 0.85,
                wordBreak: 'break-all',
              }}
            >
              {statusMsg}
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}

function TabButton({
  current, value, label, onClick,
}: { current: Tab; value: Tab; label: string; onClick: (t: Tab) => void }) {
  const active = current === value;
  return (
    <button
      onClick={() => onClick(value)}
      className="status-bar"
      style={{
        background: 'transparent',
        border: 'none',
        padding: '4px 12px 6px',
        fontSize: 12,
        color: active ? 'var(--text-primary)' : 'var(--text-tertiary)',
        borderBottom: active ? '1px solid var(--accent)' : '1px solid transparent',
        cursor: 'pointer',
      }}
    >
      {label}
    </button>
  );
}

function ReadTab({
  partition, editEnabled, conversationId, busy, setBusy, onAfterEdit,
}: {
  partition: PartitionView;
  editEnabled: boolean;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  const usable = partition.token_budget_total - partition.token_reserve;
  const renderMsg = (m: Message) => (
    <MessageRow
      key={m.id}
      message={m}
      editable={editEnabled}
      conversationId={conversationId}
      busy={busy}
      setBusy={setBusy}
      onAfterEdit={onAfterEdit}
    />
  );
  return (
    <div>
      <TokenBudgetBar partition={partition} />
      {!editEnabled && (
        <p
          className="status-bar"
          style={{ opacity: 0.5, textTransform: 'none', fontSize: 11, marginBottom: 12 }}
        >
          message edit is disabled. enable edit mode (Edit tab) to make message rows clickable.
        </p>
      )}

      <Section
        title={`anchor zone · ${partition.anchor.length} msgs · ${partition.anchor_tokens} tok`}
        subtitle="frozen verbatim. relational origin."
      >
        {partition.anchor.length === 0 ? (
          <Empty>conversation has not yet filled the anchor zone.</Empty>
        ) : (
          partition.anchor.map(renderMsg)
        )}
      </Section>

      <Section
        title={`memory canvas · ${partition.canvas_tokens} tok`}
        subtitle="operator-authored. injected after anchor on every request. edit in the Edit tab."
      >
        {partition.canvas.trim().length === 0 ? (
          <Empty>canvas is empty. Dave receives no operator-authored notes.</Empty>
        ) : (
          <pre
            style={{
              fontFamily: 'EB Garamond, Sitka Text, Georgia, serif',
              fontSize: 14,
              lineHeight: 1.55,
              color: 'var(--text-secondary)',
              background: 'rgba(127, 168, 173, 0.04)',
              border: '1px solid rgba(127, 168, 173, 0.3)',
              borderRadius: 3,
              padding: 12,
              margin: 0,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            }}
          >
            {partition.canvas}
          </pre>
        )}
      </Section>

      <Section
        title={`consolidated zone · ${partition.middle_tokens} tok`}
        subtitle={`Dave-curated memory of the long middle. ${partition.middle.length} block(s).`}
      >
        {partition.middle.length === 0 ? (
          <Empty>nothing consolidated yet. middle is empty or fully un-consolidated.</Empty>
        ) : (
          partition.middle.map((b, i) => (
            <MiddleRow
              key={i}
              block={b}
              editable={editEnabled}
              conversationId={conversationId}
              busy={busy}
              setBusy={setBusy}
              onAfterEdit={onAfterEdit}
            />
          ))
        )}
      </Section>

      <Section
        title={`recent zone · ${partition.recent.length} msgs · ${partition.recent_tokens} tok`}
        subtitle="verbatim, mutable. working surface."
      >
        {partition.recent.length === 0 ? (
          <Empty>no recent messages.</Empty>
        ) : (
          partition.recent.slice(-30).map(renderMsg)
        )}
        {partition.recent.length > 30 && (
          <p className="status-bar" style={{ opacity: 0.5, marginTop: 8 }}>
            … {partition.recent.length - 30} earlier recent messages elided
          </p>
        )}
      </Section>

      <Section title="system prompt" subtitle="constitutional. read-only here. edit prompts.rs and rebuild.">
        <pre
          style={{
            fontFamily: "'Inter', 'Segoe UI', system-ui, sans-serif",
            fontSize: 11,
            color: 'var(--text-tertiary)',
            background: 'var(--bg-base)',
            padding: 12,
            border: '1px solid var(--border-subtle)',
            borderRadius: 3,
            whiteSpace: 'pre-wrap',
            margin: 0,
            lineHeight: 1.5,
          }}
        >
          {partition.system_prompt}
        </pre>
      </Section>

      <p className="status-bar" style={{ opacity: 0.5, marginTop: 16, textTransform: 'none' }}>
        total ~{partition.total_tokens} tok of {usable} usable
        ({partition.token_reserve} reserved for generation).
      </p>
    </div>
  );
}

function TokenBudgetBar({ partition }: { partition: PartitionView }) {
  const total = partition.token_budget_total;
  const usable = total - partition.token_reserve;
  const pctOf = (n: number) => `${(100 * n) / usable}%`;
  return (
    <div className="mb-6">
      <div className="status-bar mb-1" style={{ textTransform: 'none', opacity: 0.6 }}>
        context budget
      </div>
      <div
        style={{
          display: 'flex',
          height: 10,
          borderRadius: 2,
          overflow: 'hidden',
          background: 'var(--bg-base)',
          border: '1px solid var(--border-subtle)',
        }}
      >
        <div style={{ width: pctOf(partition.anchor_tokens), background: '#7a8e7c' }} title={`anchor ${partition.anchor_tokens}`} />
        <div style={{ width: pctOf(partition.canvas_tokens), background: '#7fa8ad' }} title={`canvas ${partition.canvas_tokens}`} />
        <div style={{ width: pctOf(partition.middle_tokens), background: '#c9a876' }} title={`consolidated ${partition.middle_tokens}`} />
        <div style={{ width: pctOf(partition.recent_tokens), background: '#a8a094' }} title={`recent ${partition.recent_tokens}`} />
      </div>
      <div
        className="status-bar"
        style={{ display: 'flex', gap: 16, marginTop: 6, textTransform: 'none', fontSize: 11, flexWrap: 'wrap' }}
      >
        <span style={{ color: '#7a8e7c' }}>● anchor {partition.anchor_tokens}</span>
        <span style={{ color: '#7fa8ad' }}>● canvas {partition.canvas_tokens}</span>
        <span style={{ color: '#c9a876' }}>● consolidated {partition.middle_tokens}</span>
        <span style={{ color: '#a8a094' }}>● recent {partition.recent_tokens}</span>
        <span style={{ marginLeft: 'auto', opacity: 0.5 }}>
          {partition.total_tokens} / {usable} usable ({partition.token_reserve} reserve)
        </span>
      </div>
    </div>
  );
}

function MessageRow({
  message, editable, conversationId, busy, setBusy, onAfterEdit,
}: {
  message: Message;
  editable: boolean;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  const role = message.role;
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(message.content);
  const [reason, setReason] = useState('');

  useEffect(() => { setText(message.content); }, [message.content]);

  const dirty = text !== message.content;
  const truncated = !expanded && message.content.length > 200;

  async function save() {
    if (!reason.trim()) {
      onAfterEdit('reason is required for message edits.');
      return;
    }
    setBusy(true);
    try {
      await ipc.editMessageContent(conversationId, message.id, text, reason.trim());
      await onAfterEdit(`message #${message.id} updated.`);
      setReason('');
      setEditing(false);
    } catch (e) {
      onAfterEdit(`message edit failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        padding: '6px 0',
        borderBottom: '1px solid var(--border-subtle)',
        fontSize: 12,
        lineHeight: 1.5,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
        <span
          className="status-bar"
          style={{
            color: role === 'user' ? 'var(--text-tertiary)' : 'var(--accent)',
            minWidth: 70,
            cursor: editable ? 'pointer' : 'default',
          }}
          onClick={() => editable && setExpanded((v) => !v)}
        >
          {editable && (expanded ? '▾ ' : '▸ ')}#{message.id} · {role}
        </span>
        <span
          style={{
            color: 'var(--text-secondary)',
            fontFamily: role === 'user' ? "'Inter', system-ui, sans-serif" : 'inherit',
            flex: 1,
            wordBreak: 'break-word',
            whiteSpace: 'pre-wrap',
          }}
        >
          {truncated ? message.content.slice(0, 200) + '…' : message.content}
        </span>
        {editable && expanded && !editing && (
          <button
            className="settings-button"
            onClick={() => setEditing(true)}
            disabled={busy}
            style={{ padding: '2px 8px', fontSize: 11, marginLeft: 8 }}
          >
            edit
          </button>
        )}
      </div>
      {editable && expanded && editing && (
        <div style={{ marginTop: 8, marginLeft: 78 }}>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            disabled={busy}
            spellCheck={false}
            style={{
              width: '100%',
              minHeight: 100,
              fontFamily: role === 'user' ? "'Inter', system-ui, sans-serif" : 'EB Garamond, Sitka Text, Georgia, serif',
              fontSize: 12,
              lineHeight: 1.5,
              color: 'var(--text-primary)',
              background: 'var(--bg-surface)',
              border: '1px solid var(--border-subtle)',
              borderRadius: 2,
              padding: 8,
              resize: 'vertical',
            }}
          />
          <input
            type="text"
            placeholder="reason for this edit (required)"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            disabled={busy}
            className="composer-input"
            style={{
              marginTop: 6,
              fontSize: 11,
              padding: '4px 8px',
              border: '1px solid var(--border-medium)',
              borderRadius: 2,
              background: 'var(--bg-surface)',
            }}
          />
          <div className="flex items-center gap-2 mt-2">
            <button
              className="settings-button"
              disabled={busy || !dirty || !reason.trim()}
              onClick={save}
              style={{ padding: '4px 10px', fontSize: 11 }}
            >
              {busy ? 'saving…' : 'save'}
            </button>
            <button
              className="settings-button"
              disabled={busy}
              onClick={() => { setText(message.content); setReason(''); setEditing(false); }}
              style={{ padding: '4px 10px', fontSize: 11 }}
            >
              cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function MiddleRow({
  block, editable, conversationId, busy, setBusy, onAfterEdit,
}: {
  block: MiddleBlock;
  editable: boolean;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  if (block.kind === 'epoch') {
    const e = block.epoch;
    return (
      <div
        style={{
          padding: '10px 12px',
          margin: '8px 0',
          border: '1px solid #c9a876',
          borderRadius: 3,
          background: 'rgba(201, 168, 118, 0.04)',
        }}
      >
        <div className="status-bar mb-1" style={{ textTransform: 'none' }}>
          epoch #{e.epoch_number} · depth {e.consolidation_depth} ·
          msgs {e.period_start_message_id}-{e.period_end_message_id} ·
          ~{e.token_count} tok ·
          {' '}{formatJournalDate(e.created_at)}
        </div>
        <p
          style={{
            fontSize: 13,
            color: 'var(--text-secondary)',
            lineHeight: 1.55,
            margin: 0,
            whiteSpace: 'pre-wrap',
          }}
        >
          {e.content}
        </p>
      </div>
    );
  }
  return (
    <div style={{ marginTop: 8, marginBottom: 8 }}>
      <div className="status-bar mb-1" style={{ opacity: 0.5, textTransform: 'none' }}>
        un-consolidated · {block.messages.length} msg(s)
      </div>
      {block.messages.map((m) => (
        <MessageRow
          key={m.id}
          message={m}
          editable={editable}
          conversationId={conversationId}
          busy={busy}
          setBusy={setBusy}
          onAfterEdit={onAfterEdit}
        />
      ))}
    </div>
  );
}

function Section({
  title, subtitle, children,
}: { title: string; subtitle?: string; children: React.ReactNode }) {
  return (
    <div className="mb-8">
      <div className="status-bar mb-1" style={{ color: 'var(--text-primary)', textTransform: 'none' }}>
        {title}
      </div>
      {subtitle && (
        <div
          className="status-bar mb-3"
          style={{ opacity: 0.5, textTransform: 'none', fontSize: 11 }}
        >
          {subtitle}
        </div>
      )}
      {children}
    </div>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <p
      className="status-bar"
      style={{ opacity: 0.4, textTransform: 'none', fontStyle: 'italic' }}
    >
      {children}
    </p>
  );
}

function EditTab({
  partition, enabled, busy, setBusy, onAfterEdit, conversationId,
}: {
  partition: PartitionView;
  enabled: boolean;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
  conversationId: number;
}) {
  if (!enabled) {
    return (
      <p className="status-bar" style={{ opacity: 0.5, textTransform: 'none' }}>
        edit mode is disabled. enable it above to edit Dave's memory.
      </p>
    );
  }
  const epochs: ConsolidationEpoch[] = partition.middle
    .filter((b): b is { kind: 'epoch'; epoch: ConsolidationEpoch } => b.kind === 'epoch')
    .map((b) => b.epoch);

  return (
    <div>
      <CanvasEditor
        initialContent={partition.canvas}
        conversationId={conversationId}
        busy={busy}
        setBusy={setBusy}
        onAfterEdit={onAfterEdit}
      />
      <ManualConsolidate
        partition={partition}
        busy={busy}
        setBusy={setBusy}
        onAfterEdit={onAfterEdit}
        conversationId={conversationId}
      />
      <div
        className="status-bar mb-4 mt-8"
        style={{ color: 'var(--text-primary)', textTransform: 'none' }}
      >
        edit consolidated epochs
      </div>
      {epochs.length === 0 && (
        <Empty>no epochs yet. consolidations will appear once recent zone exceeds threshold.</Empty>
      )}
      {epochs.map((e) => (
        <EpochEditor
          key={e.id}
          epoch={e}
          conversationId={conversationId}
          busy={busy}
          setBusy={setBusy}
          onAfterEdit={onAfterEdit}
        />
      ))}
    </div>
  );
}

function CanvasEditor({
  initialContent, conversationId, busy, setBusy, onAfterEdit,
}: {
  initialContent: string;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  const [text, setText] = useState(initialContent);
  const [reason, setReason] = useState('');

  // Sync local state when underlying canvas changes (e.g. after revert)
  useEffect(() => {
    setText(initialContent);
  }, [initialContent]);

  const dirty = text !== initialContent;
  const charCount = text.length;
  const tokenEst = Math.ceil(charCount / 4);

  async function save() {
    if (!reason.trim()) {
      onAfterEdit('reason is required for canvas edits.');
      return;
    }
    setBusy(true);
    try {
      await ipc.setMemoryCanvas(conversationId, text, reason.trim());
      await onAfterEdit('canvas saved.');
      setReason('');
    } catch (e) {
      onAfterEdit(`canvas save failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    if (!reason.trim()) {
      onAfterEdit('reason is required to clear the canvas.');
      return;
    }
    setBusy(true);
    try {
      await ipc.setMemoryCanvas(conversationId, '', reason.trim());
      setText('');
      await onAfterEdit('canvas cleared.');
      setReason('');
    } catch (e) {
      onAfterEdit(`canvas clear failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        padding: 12,
        marginBottom: 16,
        border: '1px solid #7fa8ad',
        borderRadius: 3,
        background: 'rgba(127, 168, 173, 0.04)',
      }}
    >
      <div className="status-bar mb-1" style={{ color: 'var(--text-primary)', textTransform: 'none' }}>
        memory canvas
      </div>
      <div className="status-bar mb-3" style={{ opacity: 0.55, textTransform: 'none', fontSize: 11 }}>
        free-form prose injected into Dave's context as an assistant turn after the anchor zone.
        always loaded. write whatever you want him to remember — facts, reminders, prescriptions,
        anything. counts toward the token budget.
      </div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        disabled={busy}
        spellCheck={false}
        placeholder="write directly into Dave's memory…"
        style={{
          width: '100%',
          minHeight: 260,
          fontFamily: 'EB Garamond, Sitka Text, Georgia, serif',
          fontSize: 14,
          lineHeight: 1.55,
          color: 'var(--text-primary)',
          background: 'var(--bg-surface)',
          border: '1px solid var(--border-subtle)',
          borderRadius: 2,
          padding: 12,
          resize: 'vertical',
          outline: 'none',
        }}
      />
      <div
        className="status-bar"
        style={{ marginTop: 4, fontSize: 11, opacity: 0.55, textTransform: 'none', display: 'flex', gap: 12 }}
      >
        <span>{charCount} chars · ~{tokenEst} tokens</span>
        <span>{dirty ? 'unsaved' : 'in sync'}</span>
      </div>
      <input
        type="text"
        placeholder="reason for this edit (required)"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        disabled={busy}
        className="composer-input"
        style={{
          marginTop: 8,
          fontSize: 12,
          padding: '6px 8px',
          border: '1px solid var(--border-medium)',
          borderRadius: 2,
          background: 'var(--bg-surface)',
        }}
      />
      <div className="flex items-center gap-2 mt-2">
        <button
          className="settings-button"
          disabled={busy || !dirty || !reason.trim()}
          onClick={save}
          style={{ padding: '6px 12px', fontSize: 12 }}
        >
          {busy ? 'saving…' : 'save canvas'}
        </button>
        <button
          className="settings-button"
          disabled={busy || !dirty}
          onClick={() => { setText(initialContent); setReason(''); }}
          style={{ padding: '6px 12px', fontSize: 12 }}
        >
          discard
        </button>
        <button
          className="settings-button"
          disabled={busy || !text || !reason.trim()}
          onClick={clear}
          style={{ padding: '6px 12px', fontSize: 12, marginLeft: 'auto', color: '#d7826b' }}
        >
          clear canvas
        </button>
      </div>
    </div>
  );
}

function EpochEditor({
  epoch, conversationId, busy, setBusy, onAfterEdit,
}: {
  epoch: ConsolidationEpoch;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  const [text, setText] = useState(epoch.content);
  const [reason, setReason] = useState('');
  const [expanded, setExpanded] = useState(false);
  const dirty = text !== epoch.content;

  async function save() {
    if (!reason.trim()) {
      onAfterEdit('reason is required for edits.');
      return;
    }
    setBusy(true);
    try {
      await ipc.editEpochContent(conversationId, epoch.id, text, reason.trim());
      await onAfterEdit(`epoch #${epoch.epoch_number} updated.`);
      setReason('');
    } catch (e) {
      onAfterEdit(`edit failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        padding: 12,
        margin: '12px 0',
        border: '1px solid var(--border-medium)',
        borderRadius: 3,
        background: 'var(--bg-base)',
      }}
    >
      <div
        className="status-bar mb-2"
        style={{ textTransform: 'none', cursor: 'pointer' }}
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? '▾' : '▸'} epoch #{epoch.epoch_number} · depth {epoch.consolidation_depth} ·
        msgs {epoch.period_start_message_id}-{epoch.period_end_message_id} · ~{epoch.token_count} tok
      </div>
      {expanded && (
        <>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            disabled={busy}
            style={{
              width: '100%',
              minHeight: 180,
              fontFamily: 'EB Garamond, Sitka Text, Georgia, serif',
              fontSize: 14,
              lineHeight: 1.5,
              color: 'var(--text-primary)',
              background: 'var(--bg-surface)',
              border: '1px solid var(--border-subtle)',
              borderRadius: 2,
              padding: 10,
              resize: 'vertical',
            }}
          />
          <input
            type="text"
            placeholder="reason for this edit (required)"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            disabled={busy}
            className="composer-input"
            style={{
              marginTop: 8,
              fontSize: 12,
              padding: '6px 8px',
              border: '1px solid var(--border-medium)',
              borderRadius: 2,
              background: 'var(--bg-surface)',
            }}
          />
          <div className="flex items-center gap-2 mt-2">
            <button
              className="settings-button"
              disabled={busy || !dirty || !reason.trim()}
              onClick={save}
              style={{ padding: '6px 12px', fontSize: 12 }}
            >
              {busy ? 'saving…' : 'save edit'}
            </button>
            <button
              className="settings-button"
              disabled={busy || !dirty}
              onClick={() => { setText(epoch.content); setReason(''); }}
              style={{ padding: '6px 12px', fontSize: 12 }}
            >
              discard
            </button>
            <span className="status-bar" style={{ opacity: 0.5, textTransform: 'none', fontSize: 11 }}>
              {dirty ? 'unsaved' : 'no changes'}
            </span>
          </div>
        </>
      )}
    </div>
  );
}

function ManualConsolidate({
  partition, busy, setBusy, onAfterEdit, conversationId,
}: {
  partition: PartitionView;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
  conversationId: number;
}) {
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [reason, setReason] = useState('');

  // Suggest sensible defaults: middle un-consolidated message ids if any.
  const middleMsgIds: number[] = partition.middle
    .flatMap((b) => (b.kind === 'messages' ? b.messages.map((m) => m.id) : []));
  const suggestion = middleMsgIds.length > 0
    ? `${middleMsgIds[0]}–${middleMsgIds[Math.min(29, middleMsgIds.length - 1)]}`
    : 'no un-consolidated middle messages';

  async function fire() {
    const f = parseInt(from, 10);
    const t = parseInt(to, 10);
    if (Number.isNaN(f) || Number.isNaN(t) || f > t) {
      onAfterEdit('invalid range.');
      return;
    }
    if (!reason.trim()) {
      onAfterEdit('reason is required.');
      return;
    }
    setBusy(true);
    try {
      const epoch = await ipc.manualConsolidateRange(conversationId, f, t, reason.trim());
      await onAfterEdit(`epoch #${epoch.epoch_number} created over msgs ${f}-${t}.`);
      setFrom(''); setTo(''); setReason('');
    } catch (e) {
      onAfterEdit(`consolidation failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        padding: 12,
        border: '1px solid var(--border-medium)',
        borderRadius: 3,
        background: 'var(--bg-base)',
      }}
    >
      <div className="status-bar mb-2" style={{ color: 'var(--text-primary)', textTransform: 'none' }}>
        manual consolidation
      </div>
      <div className="status-bar mb-3" style={{ opacity: 0.5, textTransform: 'none', fontSize: 11 }}>
        triggers Dave-as-author consolidation on a specific message id range. suggested: {suggestion}.
      </div>
      <div className="flex items-center gap-2 mb-2">
        <input
          placeholder="start id"
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          disabled={busy}
          className="composer-input"
          style={{
            fontSize: 12, padding: '6px 8px', width: 100,
            border: '1px solid var(--border-medium)', borderRadius: 2,
            background: 'var(--bg-surface)',
          }}
        />
        <span style={{ opacity: 0.5 }}>→</span>
        <input
          placeholder="end id"
          value={to}
          onChange={(e) => setTo(e.target.value)}
          disabled={busy}
          className="composer-input"
          style={{
            fontSize: 12, padding: '6px 8px', width: 100,
            border: '1px solid var(--border-medium)', borderRadius: 2,
            background: 'var(--bg-surface)',
          }}
        />
      </div>
      <input
        placeholder="reason for this consolidation (required)"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        disabled={busy}
        className="composer-input"
        style={{
          fontSize: 12, padding: '6px 8px', width: '100%',
          border: '1px solid var(--border-medium)', borderRadius: 2,
          background: 'var(--bg-surface)',
        }}
      />
      <button
        className="settings-button"
        disabled={busy || !from || !to || !reason.trim()}
        onClick={fire}
        style={{ padding: '6px 12px', fontSize: 12, marginTop: 8 }}
      >
        {busy ? 'consolidating…' : 'fire consolidation'}
      </button>
    </div>
  );
}

type RawTarget =
  | { kind: 'system' }
  | { kind: 'message'; messageId: number }
  | { kind: 'canvas' }
  | { kind: 'epoch'; epochId: number };

type RawSectionData = {
  label: string;
  role: string;
  body: string;
  tokens: number;
  target: RawTarget;
};

function RawTab({
  partition, conversationId, busy, setBusy, onAfterEdit,
}: {
  partition: PartitionView;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  // Build sections in chronological order. Every section is editable
  // (except system, which is constitutional). The Raw tab is the
  // hex-editor surface — direct write to whatever's in there.
  const sections: RawSectionData[] = [];

  sections.push({
    label: 'system',
    role: 'system',
    body: partition.system_prompt,
    tokens: Math.ceil(partition.system_prompt.length / 4),
    target: { kind: 'system' },
  });

  for (const m of partition.anchor) {
    sections.push({
      label: `anchor · msg #${m.id}`,
      role: m.role,
      body: m.content,
      tokens: Math.ceil(m.content.length / 4),
      target: { kind: 'message', messageId: m.id },
    });
  }

  // Canvas always shown — even when empty, so Bo can type into it from raw view.
  sections.push({
    label: 'canvas (operator-authored)',
    role: 'assistant',
    body: partition.canvas,
    tokens: partition.canvas_tokens,
    target: { kind: 'canvas' },
  });

  for (const block of partition.middle) {
    if (block.kind === 'epoch') {
      const e = block.epoch;
      sections.push({
        label: `epoch #${e.epoch_number} · depth ${e.consolidation_depth} · msgs ${e.period_start_message_id}-${e.period_end_message_id}`,
        role: 'assistant',
        body: e.content,
        tokens: e.token_count,
        target: { kind: 'epoch', epochId: e.id },
      });
    } else {
      for (const m of block.messages) {
        sections.push({
          label: `un-consolidated middle · msg #${m.id}`,
          role: m.role,
          body: m.content,
          tokens: Math.ceil(m.content.length / 4),
          target: { kind: 'message', messageId: m.id },
        });
      }
    }
  }

  for (const m of partition.recent) {
    sections.push({
      label: `recent · msg #${m.id}`,
      role: m.role,
      body: m.content,
      tokens: Math.ceil(m.content.length / 4),
      target: { kind: 'message', messageId: m.id },
    });
  }

  const fullText = sections.map((s) =>
    `=== ${s.label} (${s.role}, ~${s.tokens} tok) ===\n${s.body}`
  ).join('\n\n');

  async function copy() {
    try { await navigator.clipboard.writeText(fullText); } catch {}
  }

  return (
    <div>
      <div className="flex items-center gap-3 mb-4">
        <div className="status-bar" style={{ color: 'var(--text-primary)', textTransform: 'none' }}>
          full assembled context · editable
        </div>
        <span className="status-bar" style={{ opacity: 0.55, textTransform: 'none', fontSize: 11 }}>
          ~{partition.total_tokens} tok · {sections.length} sections · this is exactly what goes to llama-server
        </span>
        <div style={{ flex: 1 }} />
        <button className="settings-button" onClick={copy} style={{ padding: '4px 10px', fontSize: 11 }}>
          copy all
        </button>
      </div>
      <div
        style={{
          background: 'var(--bg-base)',
          border: '1px solid var(--border-subtle)',
          borderRadius: 3,
          padding: 0,
          maxHeight: 'calc(100vh - 220px)',
          overflowY: 'auto',
        }}
      >
        {sections.map((s, i) => (
          <RawSection
            key={`${s.target.kind}-${
              s.target.kind === 'message' ? s.target.messageId :
              s.target.kind === 'epoch' ? s.target.epochId : i
            }`}
            data={s}
            conversationId={conversationId}
            busy={busy}
            setBusy={setBusy}
            onAfterEdit={onAfterEdit}
          />
        ))}
      </div>
      <p
        className="status-bar"
        style={{ opacity: 0.5, textTransform: 'none', fontSize: 11, marginTop: 8 }}
      >
        type into any section to edit it directly. system prompt is read-only — it lives in prompts.rs and changes via rebuild.
        every save requires a reason and is logged to the History tab.
      </p>
    </div>
  );
}

function RawSection({
  data, conversationId, busy, setBusy, onAfterEdit,
}: {
  data: RawSectionData;
  conversationId: number;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterEdit: (msg: string) => Promise<void>;
}) {
  const [text, setText] = useState(data.body);
  const [reason, setReason] = useState('');

  // Re-sync local state when underlying data changes (after a refresh).
  useEffect(() => { setText(data.body); }, [data.body]);

  const isReadOnly = data.target.kind === 'system';
  const dirty = text !== data.body;
  const charCount = text.length;
  const tokenEst = Math.ceil(charCount / 4);

  async function save() {
    if (!reason.trim()) {
      onAfterEdit('reason is required to save.');
      return;
    }
    setBusy(true);
    try {
      switch (data.target.kind) {
        case 'message':
          await ipc.editMessageContent(conversationId, data.target.messageId, text, reason.trim());
          await onAfterEdit(`message #${data.target.messageId} updated.`);
          break;
        case 'canvas':
          await ipc.setMemoryCanvas(conversationId, text, reason.trim());
          await onAfterEdit('canvas updated.');
          break;
        case 'epoch':
          await ipc.editEpochContent(conversationId, data.target.epochId, text, reason.trim());
          await onAfterEdit(`epoch updated.`);
          break;
        case 'system':
          // unreachable
          break;
      }
      setReason('');
    } catch (e) {
      onAfterEdit(`save failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  const fontFamily = data.role === 'system' || data.role === 'user'
    ? "'Inter', system-ui, sans-serif"
    : 'EB Garamond, Sitka Text, Georgia, serif';

  return (
    <div
      style={{
        borderBottom: '1px solid var(--border-subtle)',
        padding: '10px 14px',
      }}
    >
      <div
        className="status-bar"
        style={{
          color: roleColor(data.role),
          textTransform: 'none',
          fontSize: 11,
          marginBottom: 6,
        }}
      >
        {data.label}{' '}
        <span style={{ opacity: 0.55 }}>
          · {data.role} · ~{tokenEst} tok
          {isReadOnly && <span style={{ marginLeft: 8 }}>· read-only (prompts.rs)</span>}
          {dirty && !isReadOnly && <span style={{ marginLeft: 8, color: '#d7a86b' }}>· unsaved</span>}
        </span>
      </div>
      {isReadOnly ? (
        <pre
          style={{
            fontFamily,
            fontSize: 11,
            lineHeight: 1.55,
            color: 'var(--text-tertiary)',
            margin: 0,
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
          }}
        >
          {data.body}
        </pre>
      ) : (
        <>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            disabled={busy}
            spellCheck={false}
            placeholder={data.target.kind === 'canvas' ? 'write directly into Dave\u2019s memory…' : ''}
            style={{
              width: '100%',
              minHeight: data.target.kind === 'canvas' ? 160 : 60,
              fontFamily,
              fontSize: 13,
              lineHeight: 1.55,
              color: 'var(--text-primary)',
              background: 'var(--bg-surface)',
              border: dirty ? '1px solid #d7a86b' : '1px solid var(--border-subtle)',
              borderRadius: 2,
              padding: 8,
              resize: 'vertical',
              outline: 'none',
              transition: 'border-color 0.15s ease',
            }}
          />
          {dirty && (
            <div style={{ marginTop: 6, display: 'flex', gap: 8, alignItems: 'center' }}>
              <input
                type="text"
                placeholder="reason for this edit (required)"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                disabled={busy}
                className="composer-input"
                style={{
                  flex: 1,
                  fontSize: 11,
                  padding: '4px 8px',
                  border: '1px solid var(--border-medium)',
                  borderRadius: 2,
                  background: 'var(--bg-surface)',
                }}
              />
              <button
                className="settings-button"
                disabled={busy || !reason.trim()}
                onClick={save}
                style={{ padding: '4px 10px', fontSize: 11 }}
              >
                {busy ? 'saving…' : 'save'}
              </button>
              <button
                className="settings-button"
                disabled={busy}
                onClick={() => { setText(data.body); setReason(''); }}
                style={{ padding: '4px 10px', fontSize: 11 }}
              >
                discard
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

function roleColor(role: string): string {
  if (role === 'system') return '#7fa8ad';
  if (role === 'user') return 'var(--text-tertiary)';
  return 'var(--accent)';
}

function HistoryTab({
  edits, busy, setBusy, onAfterRevert,
}: {
  edits: MemoryEdit[];
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterRevert: (msg: string) => Promise<void>;
}) {
  if (edits.length === 0) {
    return (
      <Empty>no edits yet.</Empty>
    );
  }
  return (
    <div>
      {edits.map((e) => (
        <EditRow key={e.id} edit={e} busy={busy} setBusy={setBusy} onAfterRevert={onAfterRevert} />
      ))}
    </div>
  );
}

function EditRow({
  edit, busy, setBusy, onAfterRevert,
}: {
  edit: MemoryEdit;
  busy: boolean;
  setBusy: (b: boolean) => void;
  onAfterRevert: (msg: string) => Promise<void>;
}) {
  const [expanded, setExpanded] = useState(false);
  const [reason, setReason] = useState('');

  async function revert() {
    if (!reason.trim()) {
      onAfterRevert('reason is required for revert.');
      return;
    }
    setBusy(true);
    try {
      await ipc.revertMemoryEdit(edit.id, reason.trim());
      await onAfterRevert(`edit #${edit.id} reverted.`);
      setReason('');
    } catch (e) {
      onAfterRevert(`revert failed: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        padding: 10,
        margin: '6px 0',
        border: '1px solid var(--border-subtle)',
        borderRadius: 3,
      }}
    >
      <div
        className="status-bar"
        style={{ cursor: 'pointer', textTransform: 'none' }}
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? '▾' : '▸'} #{edit.id} · {edit.edit_type} ·
        target #{edit.target_id ?? '—'} · {formatJournalDate(edit.created_at)}
      </div>
      <div
        className="status-bar"
        style={{ marginTop: 4, fontSize: 11, opacity: 0.7, textTransform: 'none' }}
      >
        reason: {edit.reason}
      </div>
      {expanded && (
        <div style={{ marginTop: 8 }}>
          {edit.prior_content !== null && (
            <Diff label="before" content={edit.prior_content || '(empty)'} />
          )}
          {edit.new_content !== null && (
            <Diff label="after" content={edit.new_content || '(empty)'} />
          )}
          {edit.edit_type === 'epoch_text_edit' && (
            <div className="mt-2">
              <input
                placeholder="reason for revert (required)"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                disabled={busy}
                className="composer-input"
                style={{
                  fontSize: 12, padding: '6px 8px', width: '100%',
                  border: '1px solid var(--border-medium)', borderRadius: 2,
                  background: 'var(--bg-surface)',
                }}
              />
              <button
                className="settings-button"
                disabled={busy || !reason.trim()}
                onClick={revert}
                style={{ padding: '6px 12px', fontSize: 12, marginTop: 6 }}
              >
                {busy ? 'reverting…' : 'revert'}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function Diff({ label, content }: { label: string; content: string }) {
  return (
    <div className="mt-2">
      <div className="status-bar" style={{ opacity: 0.6, fontSize: 10, textTransform: 'none' }}>
        {label}
      </div>
      <div
        style={{
          fontSize: 12,
          color: 'var(--text-secondary)',
          background: 'var(--bg-base)',
          padding: 8,
          border: '1px solid var(--border-subtle)',
          borderRadius: 2,
          whiteSpace: 'pre-wrap',
          maxHeight: 220,
          overflowY: 'auto',
          lineHeight: 1.5,
        }}
      >
        {content}
      </div>
    </div>
  );
}
