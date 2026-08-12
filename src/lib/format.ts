// Small formatting helpers for the UI.

/** GitKraken-style relative time: "12m ago", "3h ago", "Yesterday", "5d ago". */
export function relativeTime(unixSeconds: number, now = Date.now()): string {
  const diff = Math.max(0, Math.floor(now / 1000) - unixSeconds);
  const min = Math.floor(diff / 60);
  const hr = Math.floor(diff / 3600);
  const day = Math.floor(diff / 86400);

  if (diff < 45) return "just now";
  if (min < 60) return `${min}m ago`;
  if (hr < 24) return `${hr}h ago`;
  if (day === 1) return "Yesterday";
  if (day < 7) return `${day}d ago`;
  if (day < 30) return `${Math.floor(day / 7)}w ago`;
  if (day < 365) return `${Math.floor(day / 30)}mo ago`;
  return `${Math.floor(day / 365)}y ago`;
}

/** Initials for an avatar chip, e.g. "Mathew Chapman" -> "MC". */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}
