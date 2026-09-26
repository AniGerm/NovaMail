import { create } from "zustand";

type ThemeMode = "light" | "dark" | "system";

interface UiState {
  theme: ThemeMode;
  highContrast: boolean;
  selectedMessageId: string | null;
  composerOpen: boolean;
  accountSetupOpen: boolean;
  searchQuery: string;
  syncStatus: string | null;
  setTheme: (theme: ThemeMode) => void;
  setHighContrast: (value: boolean) => void;
  selectMessage: (id: string | null) => void;
  setComposerOpen: (open: boolean) => void;
  setAccountSetupOpen: (open: boolean) => void;
  setSearchQuery: (query: string) => void;
  setSyncStatus: (status: string | null) => void;
}

export const useUiStore = create<UiState>((set) => ({
  theme: "system",
  highContrast: false,
  selectedMessageId: null,
  composerOpen: false,
  accountSetupOpen: false,
  searchQuery: "",
  syncStatus: null,
  setTheme: (theme) => set({ theme }),
  setHighContrast: (highContrast) => set({ highContrast }),
  selectMessage: (selectedMessageId) => set({ selectedMessageId }),
  setComposerOpen: (composerOpen) => set({ composerOpen }),
  setAccountSetupOpen: (accountSetupOpen) => set({ accountSetupOpen }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setSyncStatus: (syncStatus) => set({ syncStatus }),
}));
