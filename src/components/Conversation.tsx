import { useEffect, useRef, useState } from 'react';
import { useDaveStore } from '../state/store';
import { Message } from './Message';
import { JournalEntry } from './JournalEntry';
import { DepartureLine } from './DepartureLine';
import { opacityForMessage } from '../lib/memory';

export function Conversation() {
  const messages = useDaveStore((s) => s.messages);
  const inlineJournals = useDaveStore((s) => s.inlineJournals);
  const isStreaming = useDaveStore((s) => s.isStreaming);
  const pendingAssistant = useDaveStore((s) => s.pendingAssistant);
  const departure = useDaveStore((s) => s.departure);
  const startupEntry = useDaveStore((s) => s.startupEntry);
  const bufferSize = useDaveStore((s) => s.bufferSize);

  const scrollRef = useRef<HTMLDivElement>(null);
  const [stickToBottom, setStickToBottom] = useState(true);

  useEffect(() => {
    if (stickToBottom && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [
    messages.length,
    isStreaming,
    pendingAssistant.length,
    inlineJournals.length,
    !!departure,
    !!startupEntry,
    stickToBottom,
  ]);

  function onScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    setStickToBottom(atBottom);
  }

  const totalLen = messages.length + (isStreaming ? 1 : 0);
  // Ambient stream (departure + startup) only renders when the
  // conversational stream is empty. Once any turn has happened, the
  // ambient layer gives way. Two streams, never conflated.
  const conversationEmpty = messages.length === 0 && !isStreaming;

  return (
    <div ref={scrollRef} onScroll={onScroll} className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto px-12 py-10">
        {conversationEmpty && departure && (
          <DepartureLine content={departure.content} />
        )}
        {conversationEmpty && startupEntry && (
          <Message
            role="assistant"
            content={startupEntry.content}
            opacity={1.0}
          />
        )}
        {messages.map((m, i) => (
          <Message
            key={m.id}
            role={m.role}
            content={m.content}
            opacity={opacityForMessage(i, totalLen, bufferSize)}
          />
        ))}
        {inlineJournals.map((j) => (
          <JournalEntry key={j.id} content={j.content} />
        ))}
        {isStreaming && (
          <Message
            role="assistant"
            content={pendingAssistant || '\u00A0'}
            opacity={1.0}
            streaming
          />
        )}
      </div>
    </div>
  );
}
