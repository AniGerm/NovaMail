type TriageMessage = { id: string; unread: boolean };

/**
 * New mail first, then everything else, keeping the server order inside each group.
 */
export function orderTriageQueue<T extends TriageMessage>(
  messages: readonly T[],
): T[] {
  const seen = new Set<string>();
  const unread: T[] = [];
  const older: T[] = [];
  for (const message of messages) {
    if (seen.has(message.id)) continue;
    seen.add(message.id);
    if (message.unread) unread.push(message);
    else older.push(message);
  }
  return [...unread, ...older];
}

/** How many leading items are still the "new" section. */
export function triageNewCount(queue: readonly { unread: boolean }[]): number {
  let count = 0;
  for (const message of queue) {
    if (!message.unread) break;
    count += 1;
  }
  return count;
}
