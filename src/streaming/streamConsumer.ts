import { events, type Unlisten } from '../lib/tauri';
import { createPacedRenderer, type PacedRenderer } from './pacedRenderer';
import { useDaveStore } from '../state/store';

// Single stream contract. Backend emits dave:stream_start before any tokens
// (both user-initiated send_to_dave and backend-initiated outreach), tokens
// via dave:token, and either dave:stream_end (commit + persist) or
// dave:stream_aborted (drop entirely without persisting). Frontend listens
// in exactly one place.

let activeRenderer: PacedRenderer | null = null;
const unlistens: Unlisten[] = [];

function abortStream() {
  activeRenderer = null;
  useDaveStore.setState({ isStreaming: false, pendingAssistant: '' });
}

function ensureRenderer(): PacedRenderer {
  if (activeRenderer) return activeRenderer;
  activeRenderer = createPacedRenderer({
    onChar: (c) => useDaveStore.getState().appendChar(c),
    onComplete: () => {
      useDaveStore.getState().finalizeAssistant();
      activeRenderer = null;
    },
  });
  return activeRenderer;
}

export async function setupStreamConsumer() {
  if (unlistens.length > 0) return;

  unlistens.push(
    await events.onStreamStart(() => {
      useDaveStore.setState({ isStreaming: true, pendingAssistant: '' });
      ensureRenderer();
    }),
  );

  unlistens.push(
    await events.onToken((chunk) => {
      if (!useDaveStore.getState().isStreaming) {
        useDaveStore.setState({ isStreaming: true, pendingAssistant: '' });
      }
      ensureRenderer().push(chunk);
    }),
  );

  unlistens.push(
    await events.onStreamEnd(() => {
      if (activeRenderer) {
        activeRenderer.closeInput();
      } else {
        useDaveStore.getState().finalizeAssistant();
      }
    }),
  );

  unlistens.push(
    await events.onStreamAborted(() => {
      // Backend dropped the response (harness leak or empty). Discard
      // whatever was streamed without finalizing into messages.
      abortStream();
    }),
  );

  unlistens.push(
    await events.onJournalArrived((entry) => {
      useDaveStore.getState().receiveInlineJournal(entry);
    }),
  );
}

export function teardownStreamConsumer() {
  for (const u of unlistens) u();
  unlistens.length = 0;
  activeRenderer = null;
}
