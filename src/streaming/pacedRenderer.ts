// Pass-through renderer. Backend (chat_pacing.rs) owns ALL visual pacing
// now — it computes cadence-aware per-char delays based on response length
// and conversation tempo, then emits dave:token events at the calculated
// pace. This module's job is just to:
//
//   1. Receive chars as they arrive from backend (one per dave:token event)
//   2. Append them to pendingAssistant via onChar
//   3. Call onComplete when input is closed and all chars rendered
//
// No client-side delays. The backend's inter-emit sleeps create the visible
// cadence. This is the single-source-of-truth model: backend has the cadence
// score, the response length, and the typing-speed math; frontend just
// displays.

export type PacedRendererOptions = {
  onChar: (char: string) => void;
  onComplete: () => void;
};

export type PacedRenderer = {
  push(text: string): void;
  closeInput(): void;
  isActive(): boolean;
};

export function createPacedRenderer({ onChar, onComplete }: PacedRendererOptions): PacedRenderer {
  const queue: string[] = [];
  let streaming = false;
  let inputClosed = false;

  async function loop() {
    streaming = true;
    while (queue.length > 0 || !inputClosed) {
      if (queue.length === 0) {
        // Idle wait for either new chars or closeInput. Short interval so
        // we react quickly when backend emits — backend's per-char sleep
        // is the actual pacing source, this just keeps the loop alive.
        await new Promise((r) => setTimeout(r, 16));
        continue;
      }
      const char = queue.shift()!;
      onChar(char);
      // No per-char delay here. Backend controls timing via inter-emit
      // sleeps; chars arrive at the right moments and we just render.
    }
    streaming = false;
    onComplete();
  }

  return {
    push(text: string) {
      for (const ch of text) queue.push(ch);
      if (!streaming) loop();
    },
    closeInput() {
      inputClosed = true;
    },
    isActive() {
      return streaming || queue.length > 0;
    },
  };
}
