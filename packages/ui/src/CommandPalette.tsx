import { useEffect, useMemo, useState } from "react";
import { cn } from "./utils";
import { Input } from "./Input";

export type CommandItem = {
  id: string;
  label: string;
  hint?: string;
  onSelect: () => void;
};

export function CommandPalette({
  open,
  items,
  onClose,
}: {
  open: boolean;
  items: CommandItem[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((item) => item.label.toLowerCase().includes(q));
  }, [items, query]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setActive(0);
    }
  }, [open]);

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

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-[rgba(14,17,20,0.35)] p-6 pt-[12vh] backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="nova-fade-in w-full max-w-xl overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[var(--nova-shadow)]">
        <div className="border-b border-[var(--nova-border)] p-3">
          <Input
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            placeholder="Type a command…"
            aria-label="Filter commands"
          />
        </div>
        <ul className="max-h-80 overflow-y-auto p-2" role="listbox">
          {filtered.length === 0 ? (
            <li className="px-3 py-4 text-sm text-[var(--nova-ink-muted)]">No matches</li>
          ) : (
            filtered.map((item, index) => (
              <li key={item.id}>
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
                    <span className="text-xs text-[var(--nova-ink-muted)]">{item.hint}</span>
                  ) : null}
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  );
}
