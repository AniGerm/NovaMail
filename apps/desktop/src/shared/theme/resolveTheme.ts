export type ThemeMode = "light" | "dark" | "system";

type MediaLike = {
  matches: boolean;
  media?: string;
};

/**
 * Resolve whether the UI should render dark.
 * - light / dark: explicit
 * - system: prefer prefers-color-scheme; if unavailable, use local evening hours
 *   (dark roughly 19:00–07:00 as a sunset/sunrise stand-in).
 */
export function resolveIsDark(
  theme: ThemeMode,
  now: Date = new Date(),
  media: MediaLike | null = typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null,
): boolean {
  if (theme === "dark") return true;
  if (theme === "light") return false;

  if (media && media.media !== "not all") {
    return media.matches;
  }

  return isEveningHours(now);
}

/** Approximate dark period when system theme is unavailable. */
export function isEveningHours(now: Date = new Date()): boolean {
  const hour = now.getHours();
  return hour >= 19 || hour < 7;
}

export function nextThemeMode(current: ThemeMode): ThemeMode {
  if (current === "light") return "dark";
  if (current === "dark") return "system";
  return "light";
}
