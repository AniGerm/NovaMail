import { format, formatDistanceToNowStrict, isToday, isYesterday } from "date-fns";

export function formatMessageDate(epochSeconds: number): string {
  const date = new Date(epochSeconds * 1000);
  if (isToday(date)) return format(date, "HH:mm");
  if (isYesterday(date)) return "Yesterday";
  return format(date, "MMM d");
}

export function formatRelative(epochSeconds: number): string {
  return formatDistanceToNowStrict(new Date(epochSeconds * 1000), {
    addSuffix: true,
  });
}

export function displayName(from: { name?: string | null; email: string }): string {
  return from.name?.trim() || from.email;
}
