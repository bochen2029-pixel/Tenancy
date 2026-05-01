// Visible typing indicator — three pulsing dots in Dave's body register.
//
// Shown only when isStreaming is true AND no tokens have arrived yet
// (pendingAssistant is empty). The moment the first token streams in, the
// streaming Message component takes over and this indicator disappears.
//
// This is the user-visible signal that Dave is actively about to speak.
// Per the design directive: typing indicator only when Dave will actually
// type. Refuse/Delay paths emit no stream_start so this never appears for
// them. Respond path emits stream_start AFTER the natural pause, so this
// appears only when Dave has finished his "reading + composing" beat and
// is about to produce tokens.

export function TypingIndicator() {
  return (
    <div className="typing-indicator" aria-label="Dave is typing">
      <span className="dot">.</span>
      <span className="dot">.</span>
      <span className="dot">.</span>
    </div>
  );
}
