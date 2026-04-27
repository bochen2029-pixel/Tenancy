import { memo } from 'react';

type Props = {
  role: 'user' | 'assistant';
  content: string;
  opacity: number;
  streaming?: boolean;
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

function MessageInner({ role, content, opacity }: Props) {
  if (role === 'user') {
    return (
      <p className="user-line message-fade" style={{ opacity }}>
        {content}
      </p>
    );
  }
  const cleaned = stripThink(content);
  const flat = flattenBullets(cleaned);
  const paragraphs = paragraphsOf(flat);
  return (
    <div className="dave-body message-fade" style={{ opacity }}>
      {paragraphs.map((p, i) => (
        <p key={i}>{p}</p>
      ))}
    </div>
  );
}

export const Message = memo(MessageInner);
