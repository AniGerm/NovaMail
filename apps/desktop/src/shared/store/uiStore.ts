import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { Locale } from "@/shared/i18n";
import type { ColorSchemeId } from "@/shared/theme/schemes";

type ThemeMode = "light" | "dark" | "system";
type Density = "comfortable" | "compact";

interface UiState {
  theme: ThemeMode;
  colorScheme: ColorSchemeId;
  locale: Locale;
  highContrast: boolean;
  density: Density;
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
  setHighContrast: (value: boolean) => void;
  setDensity: (density: Density) => void;
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
      highContrast: false,
      density: "comfortable",
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
      setLocale: (locale) => set({ locale }),
      setHighContrast: (highContrast) => set({ highContrast }),
      setDensity: (density) => set({ density }),
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
        highContrast: state.highContrast,
        density: state.density,
      }),
    },
  ),
);
