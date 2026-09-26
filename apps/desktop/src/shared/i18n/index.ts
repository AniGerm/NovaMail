import { de, type TranslationKey, type Translations } from "./de";
import { en } from "./en";

export type Locale = "de" | "en";

const catalogs: Record<Locale, Translations> = { de, en };

export function t(
  locale: Locale,
  key: TranslationKey,
  vars?: Record<string, string | number>,
): string {
  let value = catalogs[locale][key] ?? catalogs.de[key] ?? key;
  if (vars) {
    for (const [name, raw] of Object.entries(vars)) {
      value = value.replaceAll(`{${name}}`, String(raw));
    }
  }
  return value;
}

export type { TranslationKey, Translations };
export { de, en };
