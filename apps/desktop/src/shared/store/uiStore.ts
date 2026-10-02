import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { Locale } from "@/shared/i18n";
import type { ColorSchemeId } from "@/shared/theme/schemes";
import type { ThemeMode } from "@/shared/theme/resolveTheme";

type Density = "comfortable" | "compact";
export type InboxViewModePref = "threads" | "flat";

export type { ThemeMode };

function defaultSpellLang(locale: Locale): string {
  return locale === "de" ? "de_DE" : "en_US";
}

interface UiState {
  theme: ThemeMode;
  colorScheme: ColorSchemeId;
  locale: Locale;
  /** Enchant / WebKit spellcheck language tag, e.g. de_DE */
  spellcheckLang: string;
  highContrast: boolean;
  density: Density;
  autoCheckUpdates: boolean;
  /** Persist flat vs conversation list preference. */
  inboxViewMode: InboxViewModePref;
  selectedMessageId: string | null;
  composerOpen: boolean;
  accountSetupOpen: boolean;
  settingsOpen: boolean;
  contactsOpen: boolean;
  triageOpen: boolean;
  commandPaletteOpen: boolean;
  searchQuery: string;
  syncStatus: string | null;
  setTheme: (theme: ThemeMode) => void;
  setColorScheme: (scheme: ColorSchemeId) => void;
  setLocale: (locale: Locale) => void;
  setSpellcheckLang: (lang: string) => void;
  setHighContrast: (value: boolean) => void;
  setDensity: (density: Density) => void;
  setAutoCheckUpdates: (value: boolean) => void;
  setInboxViewMode: (mode: InboxViewModePref) => void;
  selectMessage: (id: string | null) => void;
  setComposerOpen: (open: boolean) => void;
  setAccountSetupOpen: (open: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
  setContactsOpen: (open: boolean) => void;
  setTriageOpen: (open: boolean) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setSearchQuery: (query: string) => void;
  setSyncStatus: (status: string | null) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      theme: "system",
      colorScheme: "navy",
      locale: "de",
      spellcheckLang: "de_DE",
      highContrast: false,
      density: "comfortable",
      autoCheckUpdates: true,
      inboxViewMode: "threads",
      selectedMessageId: null,
      composerOpen: false,
      accountSetupOpen: false,
      settingsOpen: false,
      contactsOpen: false,
      triageOpen: false,
      commandPaletteOpen: false,
      searchQuery: "",
      syncStatus: null,
      setTheme: (theme) => set({ theme }),
      setColorScheme: (colorScheme) => set({ colorScheme }),
      setLocale: (locale) =>
        set({ locale, spellcheckLang: defaultSpellLang(locale) }),
      setSpellcheckLang: (spellcheckLang) => set({ spellcheckLang }),
      setHighContrast: (highContrast) => set({ highContrast }),
      setDensity: (density) => set({ density }),
      setAutoCheckUpdates: (autoCheckUpdates) => set({ autoCheckUpdates }),
      setInboxViewMode: (inboxViewMode) => set({ inboxViewMode }),
      selectMessage: (selectedMessageId) => set({ selectedMessageId }),
      setComposerOpen: (composerOpen) => set({ composerOpen }),
      setAccountSetupOpen: (accountSetupOpen) => set({ accountSetupOpen }),
      setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
      setContactsOpen: (contactsOpen) => set({ contactsOpen }),
      setTriageOpen: (triageOpen) => set({ triageOpen }),
      setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
      setSearchQuery: (searchQuery) => set({ searchQuery }),
      setSyncStatus: (syncStatus) => set({ syncStatus }),
    }),
    {
      name: "novamail-ui",
      partialize: (state) => ({
        theme: state.theme,
        colorScheme: state.colorScheme,
        locale: state.locale,
        spellcheckLang: state.spellcheckLang,
        highContrast: state.highContrast,
        density: state.density,
        autoCheckUpdates: state.autoCheckUpdates,
        inboxViewMode: state.inboxViewMode,
      }),
    },
  ),
);
