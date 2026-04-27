import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import { ipc, type OutreachDrop } from '../lib/tauri';
import { formatJournalDate } from '../lib/time';

// Forensic panel for outreach drops. Hidden surface — only opens via
// Ctrl+Shift+J. Shows recent drops with their discriminator scores so the
// operator can spot-check whether the filter is calibrated correctly.
// Never shown to the user during normal use; spell stays intact.

export function DropsPanel() {
  const open = useDaveStore((s) => s.dropsPanelOpen);
  const close = useDaveStore((s) => s.closeDropsPanel);
  const [entries, setEntries] = useState<OutreachDrop[]>([]);

  useEffect(() => {
    if (open) {
      ipc.loadOutreachDrops(100).then(setEntries).catch(() => {});
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
          width: '520px',
          borderLeft: '1px solid var(--border-medium)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="status-bar mb-6" style={{ opacity: 0.6 }}>
          outreach drops · forensic
        </div>
        {entries.length === 0 ? (
          <p className="status-bar" style={{ opacity: 0.5 }}>
            no drops yet.
          </p>
        ) : (
          entries.map((d) => (
            <div key={d.id} className="mb-8">
              <div className="status-bar mb-2 flex justify-between" style={{ gap: 12 }}>
                <span>
                  {formatJournalDate(d.generated_at)} &middot; {d.drop_reason}
                  {d.history_shape ? ` · ${d.history_shape}` : ''}
                </span>
                <span style={{ opacity: 0.7 }}>
                  {d.heuristic_pass ? 'h✓' : 'h✗'}
                  {d.llm_score !== null ? ` · score ${d.llm_score}` : ''}
                </span>
              </div>
              <p
                className="journal-body"
                style={{ fontSize: 14, fontStyle: 'normal', color: 'var(--text-secondary)' }}
              >
                {d.content || <em style={{ opacity: 0.5 }}>(empty)</em>}
              </p>
            </div>
          ))
        )}
      </aside>
    </div>
  );
}
