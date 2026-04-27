// Variable-delay scheduler. The model emits tokens at ~80 tok/s, which renders
// as a firehose. This wraps the stream with per-character delays at punctuation
// boundaries, simulating cadence. Constants are here, not in a settings UI.

export type PacedRendererOptions = {
  onChar: (char: string) => void;
  onComplete: () => void;
};

export type PacedRenderer = {
  push(text: string): void;
  closeInput(): void;
  isActive(): boolean;
};

const PARAGRAPH_PAUSE_MIN = 600;
const PARAGRAPH_PAUSE_VAR = 600;
const SENTENCE_PAUSE_MIN = 200;
const SENTENCE_PAUSE_VAR = 200;
const CLAUSE_PAUSE_MIN = 80;
const CLAUSE_PAUSE_VAR = 70;
const CHAR_DELAY_MIN = 12;
const CHAR_DELAY_VAR = 18;

function delayFor(char: string, prevChar: string | undefined): number {
  if (char === '\n' && prevChar === '\n') {
    return PARAGRAPH_PAUSE_MIN + Math.random() * PARAGRAPH_PAUSE_VAR;
  }
  if (prevChar === '.' || prevChar === '!' || prevChar === '?') {
    return SENTENCE_PAUSE_MIN + Math.random() * SENTENCE_PAUSE_VAR;
  }
  if (prevChar === ',' || prevChar === ';' || prevChar === ':') {
    return CLAUSE_PAUSE_MIN + Math.random() * CLAUSE_PAUSE_VAR;
  }
  return CHAR_DELAY_MIN + Math.random() * CHAR_DELAY_VAR;
}

export function createPacedRenderer({ onChar, onComplete }: PacedRendererOptions): PacedRenderer {
  const queue: string[] = [];
  let streaming = false;
  let inputClosed = false;

  async function loop() {
    streaming = true;
    let prevChar: string | undefined;
    while (queue.length > 0 || !inputClosed) {
      if (queue.length === 0) {
        await new Promise((r) => setTimeout(r, 30));
        continue;
      }
      const char = queue.shift()!;
      onChar(char);
      const delay = delayFor(char, prevChar);
      prevChar = char;
      await new Promise((r) => setTimeout(r, delay));
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
