/** Id that should take the preview after `removedId` leaves an ordered list. */
export function nextIdAfterRemoval(
  ids: readonly string[],
  removedId: string,
): string | null {
  const index = ids.indexOf(removedId);
  if (index < 0) return null;
  return ids[index + 1] ?? ids[index - 1] ?? null;
}
