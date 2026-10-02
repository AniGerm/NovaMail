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

function isWordChar(ch: string) {
  return /[\p{L}\p{N}'’-]/u.test(ch);
}

function currentWord(
  text: string,
  caret: number,
): { word: string; from: number; to: number } | null {
  if (!text) return null;
  const safe = Math.max(0, Math.min(caret, text.length));
  let from = safe;
  let to = safe;
  while (from > 0 && isWordChar(text[from - 1] ?? "")) from -= 1;
  while (to < text.length && isWordChar(text[to] ?? "")) to += 1;
  const word = text.slice(from, to);
  if (word.length < 2) return null;
  return { word, from, to };
}

/** Word immediately before caret when caret sits on a boundary (space/punct). */
function completedWordBeforeCaret(
  text: string,
  caret: number,
): { word: string; from: number; to: number } | null {
  if (!text || caret <= 0) return null;
  const safe = Math.max(0, Math.min(caret, text.length));
  const boundary = text[safe - 1] ?? "";
  if (isWordChar(boundary)) return null;
  let to = safe - 1;
  while (to > 0 && !isWordChar(text[to - 1] ?? "") && !isWordChar(text[to] ?? "")) {
    to -= 1;
  }
  if (to <= 0 || !isWordChar(text[to - 1] ?? "")) return null;
  let from = to;
  while (from > 0 && isWordChar(text[from - 1] ?? "")) from -= 1;
  const word = text.slice(from, to);
  if (word.length < 3) return null;
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
  const [learning, setLearning] = useState(false);
  const seq = useRef(0);
  const lastAutocorrect = useRef<string>("");

  // Autocorrect the previous word when the user types a boundary character.
  useEffect(() => {
    if (!isDesktopShell()) return;
    const completed = completedWordBeforeCaret(text, caret);
    if (!completed) return;
    const key = `${completed.from}:${completed.word}`;
    if (lastAutocorrect.current === key) return;
    const id = ++seq.current;
    const timer = window.setTimeout(() => {
      void api
        .spellcheckSuggest(completed.word, spellcheckLang)
        .then((result) => {
          if (id !== seq.current) return;
          if (result.correct || !result.autocorrect) return;
          // Guard against re-applying the same correction in a loop.
          if (result.autocorrect === completed.word) return;
          lastAutocorrect.current = key;
          onApply(completed.from, completed.to, result.autocorrect);
        })
        .catch(() => undefined);
    }, 40);
    return () => window.clearTimeout(timer);
  }, [caret, onApply, spellcheckLang, text]);

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

  async function learnWord() {
    if (!activeWord || learning) return;
    setLearning(true);
    try {
      await api.spellcheckLearnWord(activeWord.word, spellcheckLang);
      setSuggestions([]);
      setActiveWord(null);
    } catch {
      /* keep chips; user can retry */
    } finally {
      setLearning(false);
    }
  }

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
      <button
        type="button"
        className="rounded-full border border-dashed border-[var(--nova-border)] px-2.5 py-0.5 text-sm text-[var(--nova-ink-muted)] hover:border-[var(--nova-accent)] hover:text-[var(--nova-accent)] disabled:opacity-50"
        disabled={learning}
        onMouseDown={(e) => {
          e.preventDefault();
          void learnWord();
        }}
      >
        {learning ? t("spellLearnBusy") : t("spellLearn")}
      </button>
    </div>
  );
}
