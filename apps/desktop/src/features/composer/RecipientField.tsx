import { useEffect, useId, useRef, useState } from "react";
import { BookPlus } from "lucide-react";
import { Input } from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type { ContactPrefill, RecipientSuggestion } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

interface RecipientFieldProps {
  value: string;
  onChange: (value: string) => void;
  onAddToContacts?: (prefill: ContactPrefill) => void;
}

function parseNameEmail(token: string): { name?: string; email: string } {
  const trimmed = token.trim();
  const match = trimmed.match(/^(.*?)\s*<([^>]+)>$/);
  if (match) {
    return {
      name: match[1]?.trim() || undefined,
      email: (match[2] ?? "").trim().toLowerCase(),
    };
  }
  return { email: trimmed.toLowerCase() };
}

function formatRecipient(suggestion: RecipientSuggestion): string {
  if (suggestion.name) {
    return `${suggestion.name} <${suggestion.email}>`;
  }
  return suggestion.email;
}

export function RecipientField({
  value,
  onChange,
  onAddToContacts,
}: RecipientFieldProps) {
  const t = useT();
  const listId = useId();
  const blurTimer = useRef<number | null>(null);
  const [suggestions, setSuggestions] = useState<RecipientSuggestion[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [resolved, setResolved] = useState<RecipientSuggestion | null>(null);

  const tokens = value
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
  const lastToken = value.includes(",")
    ? value.slice(value.lastIndexOf(",") + 1).trimStart()
    : value;
  const completed = tokens.map(parseNameEmail).filter((t) => t.email.includes("@"));
  const primary = completed[completed.length - 1] ?? null;

  useEffect(() => {
    if (!isDesktopShell()) return;
    const q = lastToken.trim();
    if (q.length < 2) {
      setSuggestions([]);
      setOpen(false);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(() => {
      void api
        .recipientsSuggest(q, 10)
        .then((items) => {
          if (cancelled) return;
          setSuggestions(items);
          setActiveIndex(0);
          setOpen(items.length > 0);
        })
        .catch(() => {
          if (!cancelled) {
            setSuggestions([]);
            setOpen(false);
          }
        });
    }, 160);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [lastToken]);

  useEffect(() => {
    if (!primary || !isDesktopShell()) {
      setResolved(null);
      return;
    }
    let cancelled = false;
    void api
      .recipientsSuggest(primary.email, 8)
      .then((items) => {
        if (cancelled) return;
        const hit =
          items.find(
            (item) => item.email.toLowerCase() === primary.email.toLowerCase(),
          ) ?? null;
        setResolved(
          hit ?? {
            email: primary.email,
            name: primary.name ?? null,
            source: "typed",
            inContacts: false,
            contactId: null,
          },
        );
      })
      .catch(() => {
        if (!cancelled) {
          setResolved({
            email: primary.email,
            name: primary.name ?? null,
            source: "typed",
            inContacts: false,
            contactId: null,
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [primary?.email, primary?.name]);

  function applySuggestion(suggestion: RecipientSuggestion) {
    const before = value.includes(",")
      ? value.slice(0, value.lastIndexOf(",") + 1).trimEnd() + " "
      : "";
    onChange(`${before}${formatRecipient(suggestion)}`);
    setOpen(false);
    setSuggestions([]);
    setResolved(suggestion);
  }

  const showAdd =
    Boolean(primary?.email.includes("@")) &&
    resolved != null &&
    !resolved.inContacts &&
    Boolean(onAddToContacts);

  return (
    <label className="relative grid gap-1 text-sm">
      <span>{t("to")}</span>
      <div className="flex items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <Input
            required
            role="combobox"
            aria-expanded={open}
            aria-controls={listId}
            aria-autocomplete="list"
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onFocus={() => {
              if (suggestions.length > 0) setOpen(true);
            }}
            onBlur={() => {
              blurTimer.current = window.setTimeout(() => setOpen(false), 120);
            }}
            onKeyDown={(e) => {
              if (!open || suggestions.length === 0) return;
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActiveIndex((i) => (i + 1) % suggestions.length);
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActiveIndex(
                  (i) => (i - 1 + suggestions.length) % suggestions.length,
                );
              } else if (e.key === "Enter" && suggestions[activeIndex]) {
                e.preventDefault();
                applySuggestion(suggestions[activeIndex]!);
              } else if (e.key === "Escape") {
                setOpen(false);
              }
            }}
          />
          {open ? (
            <ul
              id={listId}
              role="listbox"
              aria-label={t("recipientSuggestions")}
              className="absolute left-0 right-0 top-[calc(100%+4px)] z-20 max-h-56 overflow-y-auto rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
              onMouseDown={(e) => e.preventDefault()}
            >
              {suggestions.map((item, index) => (
                <li key={`${item.email}-${item.source}`}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={index === activeIndex}
                    className={
                      index === activeIndex
                        ? "flex w-full flex-col items-start gap-0.5 bg-[var(--nova-accent-soft)] px-3 py-2 text-left"
                        : "flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-[var(--nova-surface-2)]"
                    }
                    onMouseEnter={() => setActiveIndex(index)}
                    onClick={() => applySuggestion(item)}
                  >
                    <span className="text-sm font-medium">
                      {item.name || item.email}
                    </span>
                    <span className="text-xs text-[var(--nova-ink-muted)]">
                      {item.name ? item.email : ""}
                      {item.name ? " · " : ""}
                      {item.inContacts
                        ? t("recipientFromContacts")
                        : t("recipientFromHistory")}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
        {showAdd ? (
          <button
            type="button"
            title={t("addRecipientToContacts")}
            aria-label={t("addRecipientToContacts")}
            className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] text-[var(--nova-accent)] hover:bg-[var(--nova-accent-soft)]"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              if (!primary) return;
              const name =
                resolved?.name ||
                primary.name ||
                primary.email.split("@")[0] ||
                primary.email;
              const parts = name.trim().split(/\s+/);
              onAddToContacts?.({
                displayName: name,
                givenName: parts[0] ?? "",
                familyName: parts.slice(1).join(" "),
                emails: [primary.email],
                notes: "",
              });
            }}
          >
            <BookPlus size={18} />
          </button>
        ) : null}
      </div>
    </label>
  );
}
