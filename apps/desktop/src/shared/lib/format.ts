import { format, formatDistanceToNowStrict, isToday, isYesterday } from "date-fns";
import { de, enUS } from "date-fns/locale";

import type { Locale } from "@/shared/i18n";
import { t } from "@/shared/i18n";

function dateLocale(locale: Locale) {
  return locale === "de" ? de : enUS;
}

export function formatMessageDate(epochSeconds: number, locale: Locale = "de"): string {
  const date = new Date(epochSeconds * 1000);
  if (isToday(date)) return format(date, "HH:mm");
  if (isYesterday(date)) return t(locale, "yesterday");
  return format(date, locale === "de" ? "d. MMM" : "MMM d", {
    locale: dateLocale(locale),
  });
}

export function formatRelative(epochSeconds: number, locale: Locale = "de"): string {
  return formatDistanceToNowStrict(new Date(epochSeconds * 1000), {
    addSuffix: true,
    locale: dateLocale(locale),
  });
}

export function displayName(from: { name?: string | null; email: string }): string {
  return from.name?.trim() || from.email;
}
