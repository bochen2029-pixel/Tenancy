import { useEffect, useState } from 'react';
import { useDaveStore } from '../state/store';
import { formatStatusDate } from '../lib/time';
import { memoryIndicator, memoryPercent } from '../lib/memory';

export function StatusBar() {
  const isStreaming = useDaveStore((s) => s.isStreaming);
  const ready = useDaveStore((s) => s.ready);
  const messages = useDaveStore((s) => s.messages);
  const bufferSize = useDaveStore((s) => s.bufferSize);
  const [date, setDate] = useState(() => formatStatusDate());

  useEffect(() => {
    const id = setInterval(() => setDate(formatStatusDate()), 60_000);
    return () => clearInterval(id);
  }, []);

  const indicator = memoryIndicator(messages.length, bufferSize);
  const pct = memoryPercent(messages.length, bufferSize);
  const dotClass = isStreaming ? 'streaming' : ready ? '' : 'waiting';

  const toggleSettings = useDaveStore((s) => s.toggleSettingsPanel);

  return (
    <div className="status-bar flex items-center justify-between px-6 py-3 select-none">
      <div className="flex items-center gap-3">
        <button
          className="gear-button"
          title="settings"
          aria-label="settings"
          onClick={toggleSettings}
        >
          {'\u2699'}
        </button>
        <div className={`presence-dot ${dotClass}`} />
        <span>{date}</span>
      </div>
      <span title={`${pct}%`}>{indicator}</span>
    </div>
  );
}
