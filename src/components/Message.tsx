import { memo } from 'react';

type Props = {
  role: 'user' | 'assistant';
  content: string;
  opacity: number;
  streaming?: boolean;
  /// Delivery state for user messages. Telegram-style two checkmarks:
  /// neither → message is in flight (no llama-server connection yet).
  /// delivered, !read → first check, blue. Harness has llama connection.
  /// delivered + read → both checks, blue. Dave's pipeline ingested it.
  /// Only meaningful for role='user'; ignored for assistant.
  delivered?: boolean;
  read?: boolean;
};

function flattenBullets(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const m = line.match(/^\s*[-*\u2022]\s+(.*)$/);
      return m ? m[1] : line;
    })
    .join('\n');
}

// Render-layer defense in depth for <think>...</think> blocks. Backend
// strips at the SDK boundary now, but legacy DB content from before the
// fix may still contain raw think tags. Strip at display time so old
// messages render clean too.
function stripThink(text: string): string {
  return text.replace(/<think\b[^>]*>[\s\S]*?<\/think>\s*/gi, '').trim();
}

function paragraphsOf(text: string): string[] {
  const parts = text.split(/\n{2,}/);
  return parts.length > 0 ? parts : [text];
}

// Ghost-fade trailing edge config (Bo's directive 2026-05-01).
//
// While Dave is streaming a long response, the most recent N characters
// render with a gradient opacity — the leading edge (newest char) is at
// ~10% opacity ("ghost"), fading up to 100% over the trailing N chars.
// As new chars arrive, older ghost chars settle into full opacity.
//
// Threshold: only kicks in once total content exceeds ~2 sentences worth
// (150 chars). Below that, render normally — short replies don't need
// the apparition effect.
const GHOST_THRESHOLD_CHARS = 150;

// Length of the fading suffix in chars. ~20 chars ≈ a word's worth.
const GHOST_FADE_LEN = 20;

// Min opacity for the leading edge (newest char). 0.10 per Bo.
const GHOST_MIN_OPACITY = 0.10;

function MessageInner({ role, content, opacity, streaming, delivered, read }: Props) {
  if (role === 'user') {
    return (
      <p className="user-line message-fade" style={{ opacity }}>
        {content}
        <DeliveryChecks delivered={!!delivered} read={!!read} />
      </p>
    );
  }
  const cleaned = stripThink(content);
  const flat = flattenBullets(cleaned);
  const paragraphs = paragraphsOf(flat);

  // Apply ghost-fade only while actively streaming AND content has crossed
  // the threshold. The fade applies to the LAST paragraph only (trailing
  // edge of the full content). Earlier paragraphs render normally.
  const applyGhostFade = !!streaming && flat.length > GHOST_THRESHOLD_CHARS;

  return (
    <div className="dave-body message-fade" style={{ opacity }}>
      {paragraphs.map((p, i) => {
        const isLast = i === paragraphs.length - 1;
        if (applyGhostFade && isLast) {
          return <GhostFadeParagraph key={i} text={p} />;
        }
        return <p key={i}>{p}</p>;
      })}
    </div>
  );
}

/// Renders a paragraph with the last GHOST_FADE_LEN chars fading from
/// 1.0 → GHOST_MIN_OPACITY. Stable prefix renders as plain text; only the
/// trailing-edge chars get per-char opacity spans. As `text` grows (new
/// chars appended during streaming) the fade region naturally shifts: the
/// previously-newest char becomes "older" and settles into full opacity.
function GhostFadeParagraph({ text }: { text: string }) {
  if (text.length <= GHOST_FADE_LEN) {
    // Whole paragraph IS the fade region — happens momentarily right after
    // crossing threshold.
    return <p>{renderFading(text, 0)}</p>;
  }
  const splitAt = text.length - GHOST_FADE_LEN;
  const stable = text.slice(0, splitAt);
  const fading = text.slice(splitAt);
  return (
    <p>
      {stable}
      {renderFading(fading, splitAt)}
    </p>
  );
}

function renderFading(fading: string, baseIdx: number) {
  // Opacity ramps from 1.0 at the OLDEST fading char (left) to
  // GHOST_MIN_OPACITY at the NEWEST (right). Linear interp.
  const len = fading.length;
  return [...fading].map((ch, i) => {
    // i=0 is oldest fading char → opacity 1.0
    // i=len-1 is newest char → opacity GHOST_MIN_OPACITY
    const t = len > 1 ? i / (len - 1) : 1;
    const op = 1.0 - t * (1.0 - GHOST_MIN_OPACITY);
    return (
      <span key={baseIdx + i} style={{ opacity: op }}>
        {ch}
      </span>
    );
  });
}

/// Two-checkmark indicator (Telegram-style). Renders inline at the end of
/// the user's message. Both checks gray-faint until `delivered`, then both
/// turn blue at delivered, then both stay blue at read with a subtle
/// brightening. Stacked-with-offset visual: the second check is drawn
/// slightly down-and-right from the first.
function DeliveryChecks({ delivered, read }: { delivered: boolean; read: boolean }) {
  // Single-check (delivered only) renders just the first check, blue.
  // Double-check (read) renders both, blue.
  // Pre-delivered renders both faint-gray to signal "in flight."
  let cls = 'pending';
  if (read) cls = 'read';
  else if (delivered) cls = 'delivered';
  return (
    <span className={`delivery-checks ${cls}`} aria-label={
      read ? 'read' : delivered ? 'delivered' : 'sending'
    }>
      <span className="check check-1">{'\u2713'}</span>
      <span className="check check-2">{'\u2713'}</span>
    </span>
  );
}

export const Message = memo(MessageInner);
