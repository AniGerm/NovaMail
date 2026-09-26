import { useEffect } from "react";

import { applyColorScheme } from "@/shared/theme/schemes";
import { resolveIsDark } from "@/shared/theme/resolveTheme";
import { useUiStore } from "@/shared/store/uiStore";

function systemPreferenceSupported(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  // Spec: unsupported queries serialize to "not all".
  return mq.media !== "not all";
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const theme = useUiStore((s) => s.theme);
  const colorScheme = useUiStore((s) => s.colorScheme);
  const highContrast = useUiStore((s) => s.highContrast);
  const density = useUiStore((s) => s.density);

  useEffect(() => {
    const root = document.documentElement;
    const media = systemPreferenceSupported()
      ? window.matchMedia("(prefers-color-scheme: dark)")
      : null;

    const apply = () => {
      const dark = resolveIsDark(theme, new Date(), media);
      root.classList.toggle("dark", dark);
      root.classList.toggle("hc", highContrast);
      root.dataset.density = density;
      applyColorScheme(colorScheme, dark ? "dark" : "light", root);
    };

    apply();

    const onMedia = () => apply();
    media?.addEventListener("change", onMedia);

    // Re-evaluate evening-hours fallback around sunset/sunrise boundaries.
    const timer =
      theme === "system" && !media
        ? window.setInterval(apply, 60_000)
        : undefined;

    return () => {
      media?.removeEventListener("change", onMedia);
      if (timer !== undefined) window.clearInterval(timer);
    };
  }, [theme, colorScheme, highContrast, density]);

  return children;
}
