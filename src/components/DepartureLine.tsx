type Props = { content: string };

function stripThink(text: string): string {
  return text.replace(/<think\b[^>]*>[\s\S]*?<\/think>\s*/gi, '').trim();
}

export function DepartureLine({ content }: Props) {
  return <p className="departure-line">{stripThink(content)}</p>;
}
