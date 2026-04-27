export function formatStatusDate(d: Date = new Date()): string {
  const weekday = d.toLocaleDateString(undefined, { weekday: 'long' }).toLowerCase();
  const month = d.toLocaleDateString(undefined, { month: 'long' }).toLowerCase();
  const day = d.getDate();
  return `${weekday}, ${month} ${day}`;
}

export function formatJournalDate(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const weekday = d.toLocaleDateString(undefined, { weekday: 'short' }).toLowerCase();
  const month = d.toLocaleDateString(undefined, { month: 'short' }).toLowerCase();
  const day = d.getDate();
  let hours = d.getHours();
  const minutes = d.getMinutes();
  const ampm = hours < 12 ? 'am' : 'pm';
  hours = hours % 12 || 12;
  const mm = String(minutes).padStart(2, '0');
  return `${weekday} ${month} ${day}, ${hours}:${mm}${ampm}`;
}
