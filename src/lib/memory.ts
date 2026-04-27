// Opacity reflects truncation pressure, not chronological age. When the
// conversation fits comfortably in the buffer (no messages have been or
// are about to be dropped), every message renders at full opacity. Fade
// only applies once messages start falling out of the active context
// window — and only to the oldest ones still inside the buffer.
//
// Past spec §7.1 (literal): "position 0 of buffer → 0.30, ramp to 1.00
// across first 30%, 1.00 for the rest." That formula is right when the
// buffer is at or above capacity. Applying it to a 2-message conversation
// fades the brand-new user message because position 0 = "oldest in
// buffer" — technically correct, semantically wrong.
export function opacityForMessage(
  messageIndex: number,
  totalLen: number,
  bufferSize: number,
): number {
  // No truncation pressure: everything's fresh in Dave's mind.
  if (totalLen <= bufferSize) return 1.0;

  const bufferStart = totalLen - bufferSize;
  if (messageIndex < bufferStart) return 0.10; // dropped off the back

  const positionInBuffer = messageIndex - bufferStart;
  const fadeRegion = Math.max(1, Math.floor(bufferSize * 0.3));
  if (positionInBuffer >= fadeRegion) return 1.0;
  return 0.30 + (positionInBuffer / fadeRegion) * 0.70;
}

export function memoryIndicator(messageCount: number, bufferSize: number): string {
  if (messageCount > bufferSize) return '\u25CC';
  if (messageCount / bufferSize > 0.8) return '\u2198\u2198';
  return '\u2198';
}

export function memoryPercent(messageCount: number, bufferSize: number): number {
  return Math.min(100, Math.round((messageCount / bufferSize) * 100));
}
