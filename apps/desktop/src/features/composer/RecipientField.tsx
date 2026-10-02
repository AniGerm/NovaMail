import { useEffect, useId, useMemo, useRef, useState } from "react";
import { BookPlus } from "lucide-react";
import { Button, Input } from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type { ContactPrefill, RecipientSuggestion } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

interface RecipientFieldProps {
  value: string;
  onChange: (value: string) => void;
  onAddToContacts?: (prefill: ContactPrefill | ContactPrefill[]) => void;
}

type ParsedRecipient = { name?: string; email: string };

function parseNameEmail(token: string): ParsedRecipient {
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

function toPrefill(recipient: ParsedRecipient): ContactPrefill {
  const name =
    recipient.name || recipient.email.split("@")[0] || recipient.email;
  const parts = name.trim().split(/\s+/);
  return {
    displayName: name,
    givenName: parts[0] ?? "",
    familyName: parts.slice(1).join(" "),
    emails: [recipient.email],
    notes: "",
  };
}

export function RecipientField({
  value,
  onChange,
  onAddToContacts,
}: RecipientFieldProps) {
  const t = useT();
  const listId = useId();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const blurTimer = useRef<number | null>(null);
  const [suggestions, setSuggestions] = useState<RecipientSuggestion[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [pickerOpen, setPickerOpen] = useState(false);
  /** email → known contact status (true = already in book) */
  const [contactStatus, setContactStatus] = useState<Record<string, boolean>>(
    {},
  );

  const lastToken = value.includes(",")
    ? value.slice(value.lastIndexOf(",") + 1).trimStart()
    : value;
  const completed = useMemo(() => {
    return value
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean)
      .map(parseNameEmail)
      .filter((item) => item.email.includes("@"));
  }, [value]);

  const missingRecipients = useMemo(() => {
    const seen = new Set<string>();
    const out: ParsedRecipient[] = [];
    for (const item of completed) {
      if (seen.has(item.email)) continue;
      seen.add(item.email);
      if (contactStatus[item.email] === false) out.push(item);
      // Unknown status: treat as candidate until proven in contacts.
      if (contactStatus[item.email] === undefined) out.push(item);
    }
    return out;
  }, [completed, contactStatus]);

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

  // Resolve contact-book status for every completed recipient.
  useEffect(() => {
    if (!isDesktopShell() || completed.length === 0) {
      setContactStatus({});
      return;
    }
    let cancelled = false;
    void Promise.all(
      completed.map(async (item) => {
        try {
          const items = await api.recipientsSuggest(item.email, 8);
          const hit = items.find(
            (s) => s.email.toLowerCase() === item.email.toLowerCase(),
          );
          return [item.email, Boolean(hit?.inContacts)] as const;
        } catch {
          return [item.email, false] as const;
        }
      }),
    ).then((entries) => {
      if (cancelled) return;
      const next: Record<string, boolean> = {};
      for (const [email, inContacts] of entries) next[email] = inContacts;
      setContactStatus(next);
    });
    return () => {
      cancelled = true;
    };
  }, [completed.map((c) => c.email).join("|")]);

  function applySuggestion(suggestion: RecipientSuggestion) {
    const before = value.includes(",")
      ? `${value.slice(0, value.lastIndexOf(",") + 1).trimEnd()} `
      : "";
    // Trailing comma so the next recipient can be typed immediately.
    const next = `${before}${formatRecipient(suggestion)}, `;
    onChange(next);
    setOpen(false);
    setSuggestions([]);
    window.requestAnimationFrame(() => {
      const el = inputRef.current;
      if (!el) return;
      el.focus();
      const end = next.length;
      el.setSelectionRange(end, end);
    });
  }

  function addPrefills(prefills: ContactPrefill[]) {
    if (!onAddToContacts || prefills.length === 0) return;
    setPickerOpen(false);
    onAddToContacts(prefills.length === 1 ? prefills[0]! : prefills);
  }

  const showAdd =
    missingRecipients.some((r) => contactStatus[r.email] !== true) &&
    Boolean(onAddToContacts);
  const candidates = missingRecipients.filter(
    (r) => contactStatus[r.email] !== true,
  );

  return (
    <label className="relative grid gap-1 text-sm">
      <span>{t("to")}</span>
      <div className="flex items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <Input
            ref={inputRef}
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
              if (candidates.length <= 1) {
                const only = candidates[0];
                if (only) addPrefills([toPrefill(only)]);
                return;
              }
              setPickerOpen(true);
            }}
          >
            <BookPlus size={18} />
          </button>
        ) : null}
      </div>

      {pickerOpen && candidates.length > 1 ? (
        <div
          className="absolute right-0 top-[calc(100%+6px)] z-30 w-[min(100%,20rem)] rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-3 shadow-[var(--nova-shadow)]"
          role="dialog"
          aria-label={t("addRecipientPickerTitle")}
          onMouseDown={(e) => e.preventDefault()}
        >
          <p className="text-sm font-medium text-[var(--nova-ink)]">
            {t("addRecipientPickerTitle")}
          </p>
          <p className="mt-0.5 text-xs text-[var(--nova-ink-muted)]">
            {t("addRecipientPickerHint")}
          </p>
          <ul className="mt-2 max-h-48 space-y-1 overflow-y-auto">
            {candidates.map((item) => (
              <li key={item.email}>
                <button
                  type="button"
                  className="flex w-full flex-col items-start rounded-[var(--nova-radius-md)] px-2.5 py-2 text-left hover:bg-[var(--nova-accent-soft)]"
                  onClick={() => addPrefills([toPrefill(item)])}
                >
                  <span className="text-sm font-medium">
                    {item.name || item.email}
                  </span>
                  {item.name ? (
                    <span className="text-xs text-[var(--nova-ink-muted)]">
                      {item.email}
                    </span>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
          <div className="mt-2 flex gap-2 border-t border-[var(--nova-border)] pt-2">
            <Button
              type="button"
              size="sm"
              className="flex-1"
              onClick={() => addPrefills(candidates.map(toPrefill))}
            >
              {t("addAllRecipientsToContacts")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => setPickerOpen(false)}
            >
              {t("cancel")}
            </Button>
          </div>
        </div>
      ) : null}
    </label>
  );
}
