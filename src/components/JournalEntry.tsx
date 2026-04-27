type Props = { content: string };

function stripThink(text: string): string {
  return text.replace(/<think\b[^>]*>[\s\S]*?<\/think>\s*/gi, '').trim();
}

export function JournalEntry({ content }: Props) {
  return (
    <div className="journal-block">
      <div className="journal-label">while you were gone</div>
      <p className="journal-body">{stripThink(content)}</p>
    </div>
  );
}
