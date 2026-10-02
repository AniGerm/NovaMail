import { useEffect, useRef, useState } from "react";

import { api, isDesktopShell } from "@/shared/api/client";
import { useT } from "@/shared/i18n/useT";
import { useUiStore } from "@/shared/store/uiStore";

interface SpellSuggestBarProps {
  /** Plain text currently being edited (textarea value or plain extraction). */
  text: string;
  /** Caret index in `text` (end of selection). */
  caret: number;
  /** Replace the current word with a suggestion. */
  onApply: (from: number, to: number, replacement: string) => void;
  className?: string;
}

function currentWord(text: string, caret: number): { word: string; from: number; to: number } | null {
  if (!text) return null;
  const safe = Math.max(0, Math.min(caret, text.length));
  let from = safe;
  let to = safe;
  const isWord = (ch: string) => /[\p{L}\p{N}'’-]/u.test(ch);
  while (from > 0 && isWord(text[from - 1] ?? "")) from -= 1;
  while (to < text.length && isWord(text[to] ?? "")) to += 1;
  const word = text.slice(from, to);
  if (word.length < 2) return null;
  // Don't suggest mid-word if user is still typing a short prefix without pause — handled by debounce.
  return { word, from, to };
}

/** Inline suggestion chips (iPhone-style) for the word at the caret. */
export function SpellSuggestBar({
  text,
  caret,
  onApply,
  className,
}: SpellSuggestBarProps) {
  const t = useT();
  const spellcheckLang = useUiStore((s) => s.spellcheckLang);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [activeWord, setActiveWord] = useState<{
    word: string;
    from: number;
    to: number;
  } | null>(null);
  const seq = useRef(0);

  useEffect(() => {
    if (!isDesktopShell()) {
      setSuggestions([]);
      setActiveWord(null);
      return;
    }
    const found = currentWord(text, caret);
    if (!found) {
      setSuggestions([]);
      setActiveWord(null);
      return;
    }
    // Skip while the word ends with incomplete typing of very short tokens.
    if (found.word.length < 3) {
      setSuggestions([]);
      setActiveWord(null);
      return;
    }
    const id = ++seq.current;
    const timer = window.setTimeout(() => {
      void api
        .spellcheckSuggest(found.word, spellcheckLang)
        .then((result) => {
          if (id !== seq.current) return;
          if (result.correct) {
            setSuggestions([]);
            setActiveWord(null);
            return;
          }
          setActiveWord(found);
          setSuggestions(result.suggestions.slice(0, 5));
        })
        .catch(() => {
          if (id === seq.current) {
            setSuggestions([]);
            setActiveWord(null);
          }
        });
    }, 280);
    return () => window.clearTimeout(timer);
  }, [caret, spellcheckLang, text]);

  if (!activeWord || suggestions.length === 0) return null;

  return (
    <div
      className={
        className ??
        "flex flex-wrap items-center gap-1.5 rounded-[var(--nova-radius-sm)] border border-[var(--nova-border)] bg-[var(--nova-surface-2)] px-2 py-1.5"
      }
      role="listbox"
      aria-label={t("spellSuggestions")}
    >
      <span className="mr-1 text-[11px] font-medium uppercase tracking-wide text-[var(--nova-ink-muted)]">
        {t("spellSuggestionsFor", { word: activeWord.word })}
      </span>
      {suggestions.map((s) => (
        <button
          key={s}
          type="button"
          role="option"
          className="rounded-full border border-[var(--nova-border)] bg-[var(--nova-surface)] px-2.5 py-0.5 text-sm text-[var(--nova-ink)] hover:border-[var(--nova-accent)] hover:text-[var(--nova-accent)]"
          onMouseDown={(e) => {
            // Prevent blur before apply.
            e.preventDefault();
            onApply(activeWord.from, activeWord.to, s);
          }}
        >
          {s}
        </button>
      ))}
    </div>
  );
}
