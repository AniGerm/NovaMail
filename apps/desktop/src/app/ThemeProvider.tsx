import { useEffect } from "react";

import { applyColorScheme } from "@/shared/theme/schemes";
import { useUiStore } from "@/shared/store/uiStore";

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const theme = useUiStore((s) => s.theme);
  const colorScheme = useUiStore((s) => s.colorScheme);
  const highContrast = useUiStore((s) => s.highContrast);
  const density = useUiStore((s) => s.density);

  useEffect(() => {
    const root = document.documentElement;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      root.classList.toggle("dark", dark);
      root.classList.toggle("hc", highContrast);
      root.dataset.density = density;
      applyColorScheme(colorScheme, dark ? "dark" : "light", root);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme, colorScheme, highContrast, density]);

  return children;
}
