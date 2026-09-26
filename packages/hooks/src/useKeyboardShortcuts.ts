import { useEffect } from "react";

export type ShortcutMap = Record<string, (event: KeyboardEvent) => void>;

/**
 * Registers global keyboard shortcuts. Ignores events originating from
 * editable fields unless the key starts with "mod+".
 */
export function useKeyboardShortcuts(map: ShortcutMap) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      const editable =
        tag === "input" ||
        tag === "textarea" ||
        target?.isContentEditable === true;

      const parts: string[] = [];
      if (event.metaKey || event.ctrlKey) parts.push("mod");
      if (event.shiftKey) parts.push("shift");
      if (event.altKey) parts.push("alt");
      parts.push(event.key.toLowerCase());
      const combo = parts.join("+");

      const action = map[combo] ?? map[event.key.toLowerCase()];
      if (!action) return;
      if (editable && !combo.startsWith("mod+")) return;

      event.preventDefault();
      action(event);
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [map]);
}
