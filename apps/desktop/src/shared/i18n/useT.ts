import { useCallback } from "react";

import { useUiStore } from "@/shared/store/uiStore";

import { t, type TranslationKey } from "./index";

export function useT() {
  const locale = useUiStore((s) => s.locale);
  return useCallback(
    (key: TranslationKey, vars?: Record<string, string | number>) =>
      t(locale, key, vars),
    [locale],
  );
}
