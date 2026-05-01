import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useDaveStore } from '../state/store';

export function Composer() {
  const [text, setText] = useState('');
  const ref = useRef<HTMLTextAreaElement>(null);
  const send = useDaveStore((s) => s.send);

  useEffect(() => {
    const ta = ref.current;
    if (!ta) return;
    ta.style.height = 'auto';
    const next = Math.min(220, ta.scrollHeight);
    ta.style.height = next + 'px';
  }, [text]);

  function handleKey(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      // Composer is always available — no isStreaming gate. Dave can
      // refuse, delay, or be mid-stream; the user can always type.
      if (text.trim()) {
        const out = text;
        setText('');
        send(out);
      }
    }
  }

  return (
    <div
      className="px-12 py-6"
      style={{ borderTop: '1px solid var(--border-subtle)' }}
    >
      <div className="max-w-2xl mx-auto">
        <textarea
          ref={ref}
          className="composer-input"
          rows={1}
          placeholder="write to him..."
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKey}
          autoFocus
          spellCheck={false}
        />
      </div>
    </div>
  );
}
