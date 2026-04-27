import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import { ipc, type JournalEntry } from '../lib/tauri';
import { formatJournalDate } from '../lib/time';

export function JournalPanel() {
  const open = useDaveStore((s) => s.journalPanelOpen);
  const close = useDaveStore((s) => s.closeJournalPanel);
  const [entries, setEntries] = useState<JournalEntry[]>([]);

  useEffect(() => {
    if (open) {
      ipc.loadAllJournal().then(setEntries).catch(() => {});
    }
  }, [open]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.4)' }}
      onClick={close}
    >
      <div className="flex-1" />
      <aside
        className="h-full overflow-y-auto p-10"
        style={{
          backgroundColor: 'var(--bg-elevated)',
          width: '420px',
          borderLeft: '1px solid var(--border-medium)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {entries.length === 0 ? (
          <p className="status-bar" style={{ opacity: 0.5 }}>
            nothing yet.
          </p>
        ) : (
          entries.map((e) => (
            <div key={e.id} className="mb-10">
              <div className="status-bar mb-2">
                {formatJournalDate(e.created_at)} &middot; {e.type}
              </div>
              <p className="journal-body" style={{ fontSize: 15 }}>
                {e.content}
              </p>
            </div>
          ))
        )}
      </aside>
    </div>
  );
}
