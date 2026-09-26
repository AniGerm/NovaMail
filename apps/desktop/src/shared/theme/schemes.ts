export type ColorSchemeId = "navy" | "forest" | "slate" | "midnight";

export interface ColorScheme {
  id: ColorSchemeId;
  /** CSS custom properties applied to :root / .dark */
  light: Record<string, string>;
  dark: Record<string, string>;
}

/** Selectable accent/atmosphere palettes. Navy is the default (dark blue / white). */
export const COLOR_SCHEMES: Record<ColorSchemeId, ColorScheme> = {
  navy: {
    id: "navy",
    light: {
      "--nova-bg": "#f4f7fb",
      "--nova-bg-accent": "#e8eef6",
      "--nova-surface": "#ffffff",
      "--nova-surface-2": "#eef3f9",
      "--nova-ink": "#0f172a",
      "--nova-ink-muted": "#475569",
      "--nova-border": "#cfd9e6",
      "--nova-accent": "#1e3a5f",
      "--nova-accent-soft": "#d7e4f4",
      "--nova-danger": "#b42318",
      "--nova-warning": "#b54708",
      "--nova-success": "#0f6b4c",
      "--nova-shadow": "0 8px 30px rgba(15, 23, 42, 0.08)",
    },
    dark: {
      "--nova-bg": "#0b1220",
      "--nova-bg-accent": "#111a2b",
      "--nova-surface": "#152033",
      "--nova-surface-2": "#1c2a42",
      "--nova-ink": "#f1f5f9",
      "--nova-ink-muted": "#94a3b8",
      "--nova-border": "#2a3b55",
      "--nova-accent": "#7eb6ff",
      "--nova-accent-soft": "#1a3358",
      "--nova-danger": "#f97066",
      "--nova-warning": "#fdb022",
      "--nova-success": "#32d583",
      "--nova-shadow": "0 12px 40px rgba(0, 0, 0, 0.45)",
    },
  },
  forest: {
    id: "forest",
    light: {
      "--nova-bg": "#f7f6f3",
      "--nova-bg-accent": "#eef0f2",
      "--nova-surface": "#ffffff",
      "--nova-surface-2": "#eef0f2",
      "--nova-ink": "#121417",
      "--nova-ink-muted": "#5c6570",
      "--nova-border": "#d8dde3",
      "--nova-accent": "#0b6e4f",
      "--nova-accent-soft": "#d8f3e7",
      "--nova-danger": "#b42318",
      "--nova-warning": "#b54708",
      "--nova-success": "#067647",
      "--nova-shadow": "0 8px 30px rgba(18, 20, 23, 0.08)",
    },
    dark: {
      "--nova-bg": "#0e1114",
      "--nova-bg-accent": "#171b20",
      "--nova-surface": "#171b20",
      "--nova-surface-2": "#21262d",
      "--nova-ink": "#f2f4f7",
      "--nova-ink-muted": "#9aa3ad",
      "--nova-border": "#2c333c",
      "--nova-accent": "#3dcf9a",
      "--nova-accent-soft": "#12352a",
      "--nova-danger": "#f97066",
      "--nova-warning": "#fdb022",
      "--nova-success": "#32d583",
      "--nova-shadow": "0 12px 40px rgba(0, 0, 0, 0.45)",
    },
  },
  slate: {
    id: "slate",
    light: {
      "--nova-bg": "#f8fafc",
      "--nova-bg-accent": "#eef2f6",
      "--nova-surface": "#ffffff",
      "--nova-surface-2": "#f1f5f9",
      "--nova-ink": "#0f172a",
      "--nova-ink-muted": "#64748b",
      "--nova-border": "#d8dee8",
      "--nova-accent": "#334155",
      "--nova-accent-soft": "#e2e8f0",
      "--nova-danger": "#b42318",
      "--nova-warning": "#b54708",
      "--nova-success": "#0f6b4c",
      "--nova-shadow": "0 8px 30px rgba(15, 23, 42, 0.07)",
    },
    dark: {
      "--nova-bg": "#0f1419",
      "--nova-bg-accent": "#161b22",
      "--nova-surface": "#1c232d",
      "--nova-surface-2": "#252d38",
      "--nova-ink": "#f8fafc",
      "--nova-ink-muted": "#94a3b8",
      "--nova-border": "#334155",
      "--nova-accent": "#cbd5e1",
      "--nova-accent-soft": "#1e293b",
      "--nova-danger": "#f97066",
      "--nova-warning": "#fdb022",
      "--nova-success": "#32d583",
      "--nova-shadow": "0 12px 40px rgba(0, 0, 0, 0.45)",
    },
  },
  midnight: {
    id: "midnight",
    light: {
      "--nova-bg": "#f3f5fb",
      "--nova-bg-accent": "#e6eaf6",
      "--nova-surface": "#ffffff",
      "--nova-surface-2": "#eef0f8",
      "--nova-ink": "#111827",
      "--nova-ink-muted": "#4b5563",
      "--nova-border": "#cfd3e3",
      "--nova-accent": "#312e81",
      "--nova-accent-soft": "#e0e7ff",
      "--nova-danger": "#b42318",
      "--nova-warning": "#b54708",
      "--nova-success": "#0f6b4c",
      "--nova-shadow": "0 8px 30px rgba(17, 24, 39, 0.08)",
    },
    dark: {
      "--nova-bg": "#09090f",
      "--nova-bg-accent": "#11111b",
      "--nova-surface": "#151522",
      "--nova-surface-2": "#1e1e2e",
      "--nova-ink": "#f5f3ff",
      "--nova-ink-muted": "#a5b4c8",
      "--nova-border": "#2e2e44",
      "--nova-accent": "#a5b4fc",
      "--nova-accent-soft": "#23234a",
      "--nova-danger": "#f97066",
      "--nova-warning": "#fdb022",
      "--nova-success": "#32d583",
      "--nova-shadow": "0 12px 40px rgba(0, 0, 0, 0.5)",
    },
  },
};

export const DEFAULT_COLOR_SCHEME: ColorSchemeId = "navy";

export function applyColorScheme(
  schemeId: ColorSchemeId,
  mode: "light" | "dark",
  root: HTMLElement = document.documentElement,
): void {
  const scheme = COLOR_SCHEMES[schemeId] ?? COLOR_SCHEMES.navy;
  const vars = mode === "dark" ? scheme.dark : scheme.light;
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(key, value);
  }
  root.dataset.colorScheme = schemeId;
}
