import { useCallback, useEffect, useMemo, useState } from "react";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { Button } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, MessageDetailDto, MessageSummaryDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { displayName, formatRelative } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

interface QuickTriageProps {
  open: boolean;
  messages: MessageSummaryDto[];
  onClose: () => void;
  onChanged: () => void;
}

function plainPreview(htmlOrText: string): string {
  return htmlOrText
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n\n")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/\{[^}]*\}/g, " ")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .replace(/[ \t]{2,}/g, " ")
    .trim();
}

export function QuickTriage({ open, messages, onClose, onChanged }: QuickTriageProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const queue = useMemo(() => messages, [messages]);
  const [index, setIndex] = useState(0);
  const [preview, setPreview] = useState<MessageDetailDto | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const current = queue[index] ?? null;

  useEffect(() => {
    if (!open) return;
    setIndex(0);
    setPreview(null);
    setPreviewOpen(false);
    setError(null);
  }, [open]);

  useEffect(() => {
    if (!open || !current || !previewOpen) return;
    let cancelled = false;
    api
      .messagesGet(current.id)
      .then((detail) => {
        if (!cancelled) setPreview(detail);
      })
      .catch((err) => {
        if (!cancelled) setError((err as AppError).message);
      });
    return () => {
      cancelled = true;
    };
  }, [open, current, previewOpen]);

  const advance = useCallback(() => {
    setPreviewOpen(false);
    setPreview(null);
    setIndex((i) => i + 1);
  }, []);

  const handleKeep = useCallback(async () => {
    if (!current || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.messagesSetFlags({ messageId: current.id, unread: false });
      onChanged();
      advance();
    } catch (err) {
      setError((err as AppError).message || t("keepFailed"));
    } finally {
      setBusy(false);
    }
  }, [advance, busy, current, onChanged, t]);

  const handleDelete = useCallback(async () => {
    if (!current || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.messagesDelete(current.id);
      onChanged();
      advance();
    } catch (err) {
      setError((err as AppError).message || t("deleteFailed"));
    } finally {
      setBusy(false);
    }
  }, [advance, busy, current, onChanged, t]);

  const shortcuts = useMemo(() => {
    const map: Record<string, () => void> = {
      // German-friendly + English keep/delete; V for preview (never Space / braces)
      b: () => {
        void handleKeep();
      },
      k: () => {
        void handleKeep();
      },
      l: () => {
        void handleDelete();
      },
      d: () => {
        void handleDelete();
      },
      v: () => setPreviewOpen((value) => !value),
      enter: () => setPreviewOpen(true),
      escape: () => {
        if (previewOpen) setPreviewOpen(false);
        else onClose();
      },
    };
    return map;
  }, [handleDelete, handleKeep, onClose, previewOpen]);
  useKeyboardShortcuts(open ? shortcuts : {});

  if (!open) return null;

  const done = !current;
  const previewText = preview
    ? plainPreview(preview.bodyText || preview.bodyHtml || current?.snippet || "")
    : "";

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-[rgba(14,17,20,0.55)] p-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="triage-title"
    >
      <div className="nova-fade-in flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[var(--nova-shadow)]">
        <header className="flex items-center justify-between border-b border-[var(--nova-border)] px-5 py-4">
          <div>
            <h2
              id="triage-title"
              className="font-[family-name:var(--nova-font-display)] text-xl"
            >
              {t("triageTitle")}
            </h2>
            <p className="text-sm text-[var(--nova-ink-muted)]">{t("triageHotkeys")}</p>
          </div>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            {t("close")}
          </Button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          {done ? (
            <p className="text-[var(--nova-ink-muted)]">{t("triageDone")}</p>
          ) : (
            <>
              <p className="mb-2 text-xs uppercase tracking-wide text-[var(--nova-ink-muted)]">
                {index + 1} / {queue.length}
              </p>
              <h3 className="font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
                {current.subject || t("noSubject")}
              </h3>
              <p className="mt-2 text-sm text-[var(--nova-ink-muted)]">
                {displayName(current.from)} &lt;{current.from.email}&gt; ·{" "}
                {formatRelative(current.date, locale)}
              </p>
              <p className="mt-4 text-[15px] leading-7 text-[var(--nova-ink)]">
                {plainPreview(current.snippet)}
              </p>
              {current.hasAttachments ? (
                <p className="mt-3 text-sm text-[var(--nova-accent)]">
                  {t("hasAttachments")}
                </p>
              ) : null}

              {previewOpen ? (
                <div className="mt-5 max-h-[40vh] overflow-y-auto rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_85%,var(--nova-bg))] p-4">
                  {preview ? (
                    <div className="whitespace-pre-wrap text-[14px] leading-6 text-[var(--nova-ink)]">
                      {previewText || current.snippet}
                    </div>
                  ) : (
                    <p className="text-sm text-[var(--nova-ink-muted)]">
                      {t("loadingPreview")}
                    </p>
                  )}
                </div>
              ) : null}
            </>
          )}
          {error ? (
            <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        {!done ? (
          <footer className="flex flex-wrap justify-end gap-2 border-t border-[var(--nova-border)] px-5 py-4">
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              onClick={() => setPreviewOpen((value) => !value)}
            >
              {previewOpen ? t("hidePreview") : t("preview")}
            </Button>
            <Button type="button" variant="secondary" disabled={busy} onClick={handleKeep}>
              {t("keepInInbox")}
            </Button>
            <Button type="button" variant="danger" disabled={busy} onClick={handleDelete}>
              {t("delete")}
            </Button>
          </footer>
        ) : null}
      </div>
    </div>
  );
}
