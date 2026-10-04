import { useEffect, useMemo, useState } from "react";
import { cn } from "./utils";
import { Input } from "./Input";
import { Overlay } from "./Overlay";

export type CommandItem = {
  id: string;
  label: string;
  hint?: string;
  /** Extra text used only for fuzzy matching */
  keywords?: string;
  group?: string;
  onSelect: () => void;
};

function fuzzyScore(haystack: string, needle: string): number {
  if (!needle) return 1;
  const h = haystack.toLowerCase();
  const n = needle.toLowerCase();
  if (h === n) return 1000;
  const idx = h.indexOf(n);
  if (idx === 0) return 800 - Math.min(h.length, 40);
  if (idx > 0) return 600 - idx;
  // subsequence match
  let hi = 0;
  for (let ni = 0; ni < n.length; ni += 1) {
    const ch = n[ni]!;
    const found = h.indexOf(ch, hi);
    if (found < 0) return 0;
    hi = found + 1;
  }
  return 200 - Math.min(h.length, 100);
}

export function CommandPalette({
  open,
  items,
  onClose,
  placeholder = "Type a command…",
  emptyLabel = "No matches",
  ariaLabel = "Command palette",
}: {
  open: boolean;
  items: CommandItem[];
  onClose: () => void;
  placeholder?: string;
  emptyLabel?: string;
  ariaLabel?: string;
}) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const filtered = useMemo(() => {
    const q = query.trim();
    if (!q) return items;
    return items
      .map((item) => {
        const hay = `${item.label} ${item.keywords ?? ""} ${item.group ?? ""}`;
        return { item, score: fuzzyScore(hay, q) };
      })
      .filter((row) => row.score > 0)
      .sort((a, b) => b.score - a.score)
      .map((row) => row.item);
  }, [items, query]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setActive(0);
    }
  }, [open]);

  useEffect(() => {
    setActive(0);
  }, [query]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        setActive((i) => Math.min(i + 1, Math.max(filtered.length - 1, 0)));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setActive((i) => Math.max(i - 1, 0));
      } else if (event.key === "Enter") {
        event.preventDefault();
        filtered[active]?.onSelect();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, active, onClose]);

  if (!open) return null;

  let lastGroup: string | undefined;

  return (
    <Overlay
      align="start"
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="nova-fade-in max-h-full w-full max-w-xl overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[var(--nova-shadow)]">
        <div className="border-b border-[var(--nova-border)] p-3">
          <Input
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            placeholder={placeholder}
            aria-label={placeholder}
          />
        </div>
        <ul className="max-h-80 overflow-y-auto p-2" role="listbox">
          {filtered.length === 0 ? (
            <li className="px-3 py-4 text-sm text-[var(--nova-ink-muted)]">
              {emptyLabel}
            </li>
          ) : (
            filtered.map((item, index) => {
              const showGroup = Boolean(item.group && item.group !== lastGroup);
              if (item.group) lastGroup = item.group;
              return (
                <li key={item.id}>
                  {showGroup ? (
                    <p className="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
                      {item.group}
                    </p>
                  ) : null}
                  <button
                    type="button"
                    role="option"
                    aria-selected={index === active}
                    className={cn(
                      "flex w-full items-center justify-between rounded-[var(--nova-radius-md)] px-3 py-3 text-left text-sm",
                      index === active
                        ? "bg-[var(--nova-accent-soft)] text-[var(--nova-accent)]"
                        : "hover:bg-[var(--nova-surface-2)]",
                    )}
                    onMouseEnter={() => setActive(index)}
                    onClick={() => {
                      item.onSelect();
                      onClose();
                    }}
                  >
                    <span>{item.label}</span>
                    {item.hint ? (
                      <span className="text-xs text-[var(--nova-ink-muted)]">
                        {item.hint}
                      </span>
                    ) : null}
                  </button>
                </li>
              );
            })
          )}
        </ul>
      </div>
    </Overlay>
  );
}
