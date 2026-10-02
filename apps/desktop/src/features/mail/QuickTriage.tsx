import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Archive, Eye, EyeOff, Inbox, Paperclip, Sparkles, Trash2, X } from "lucide-react";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { Button, IconButton } from "@novamail/ui";

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
  const [queue, setQueue] = useState<MessageSummaryDto[]>([]);
  const [index, setIndex] = useState(0);
  const [preview, setPreview] = useState<MessageDetailDto | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [aiSummary, setAiSummary] = useState<string | null>(null);
  const [aiLoading, setAiLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingRef = useRef(0);
  const refreshTimer = useRef<number | null>(null);

  const current = queue[index] ?? null;
  const progress = queue.length === 0 ? 0 : Math.min((index + (current ? 0 : 1)) / queue.length, 1);

  const scheduleRefresh = useCallback(() => {
    if (refreshTimer.current != null) {
      window.clearTimeout(refreshTimer.current);
    }
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      onChanged();
    }, 400);
  }, [onChanged]);

  // Snapshot the queue only when the dialog opens so background refreshes
  // don't reset progress mid-triage.
  useEffect(() => {
    if (!open) return;
    const unread = messages.filter((m) => m.unread);
    setQueue(unread.length > 0 ? unread : messages);
    setIndex(0);
    setPreview(null);
    setPreviewOpen(false);
    setAiSummary(null);
    setAiLoading(false);
    setError(null);
    pendingRef.current = 0;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- snapshot on open only
  }, [open]);

  useEffect(() => {
    return () => {
      if (refreshTimer.current != null) {
        window.clearTimeout(refreshTimer.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!open || !current) return;
    let cancelled = false;
    setAiSummary(null);
    setAiLoading(true);
    void api
      .aiMessageInsights(current.id)
      .then((insights) => {
        if (!cancelled) {
          setAiSummary(insights.summary || null);
          setAiLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) setAiLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, current]);

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
    setAiSummary(null);
    setIndex((i) => i + 1);
  }, []);

  const handleKeep = useCallback(() => {
    if (!current) return;
    const id = current.id;
    advance();
    pendingRef.current += 1;
    void api
      .messagesSetFlags({ messageId: id, unread: false })
      .catch((err) => {
        setError((err as AppError).message || t("keepFailed"));
      })
      .finally(() => {
        pendingRef.current = Math.max(0, pendingRef.current - 1);
        scheduleRefresh();
      });
  }, [advance, current, scheduleRefresh, t]);

  const handleDelete = useCallback(() => {
    if (!current) return;
    const id = current.id;
    advance();
    pendingRef.current += 1;
    void api
      .messagesDelete(id)
      .catch((err) => {
        setError((err as AppError).message || t("deleteFailed"));
      })
      .finally(() => {
        pendingRef.current = Math.max(0, pendingRef.current - 1);
        scheduleRefresh();
      });
  }, [advance, current, scheduleRefresh, t]);

  const shortcuts = useMemo(() => {
    const map: Record<string, () => void> = {
      b: () => {
        handleKeep();
      },
      k: () => {
        handleKeep();
      },
      l: () => {
        handleDelete();
      },
      d: () => {
        handleDelete();
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
  const shown = Math.min(index + (done ? 0 : 1), queue.length);

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-[color-mix(in_srgb,var(--nova-ink)_55%,transparent)] p-4 backdrop-blur-md"
      role="dialog"
      aria-modal="true"
      aria-labelledby="triage-title"
    >
      <div className="nova-fade-in flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[linear-gradient(180deg,color-mix(in_srgb,var(--nova-accent-soft)_55%,var(--nova-surface)),var(--nova-surface)_38%)] shadow-[var(--nova-shadow)]">
        <div
          className="h-1.5 w-full bg-[var(--nova-surface-2)]"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={queue.length}
          aria-valuenow={shown}
        >
          <div
            className="h-full bg-[var(--nova-accent)] transition-[width] duration-200"
            style={{ width: `${Math.max(progress, done ? 1 : 0) * 100}%` }}
          />
        </div>

        <header className="flex items-start justify-between gap-4 px-6 pb-4 pt-5">
          <div className="flex items-start gap-3">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-[var(--nova-radius-md)] bg-[var(--nova-accent)] text-white">
              <Archive size={20} />
            </div>
            <div>
              <h2
                id="triage-title"
                className="font-[family-name:var(--nova-font-display)] text-2xl leading-tight"
              >
                {t("triageTitle")}
              </h2>
              <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">{t("triageHotkeys")}</p>
            </div>
          </div>
          <IconButton label={t("close")} onClick={onClose}>
            <X size={18} />
          </IconButton>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-2">
          {done ? (
            <div className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-5 py-8 text-center">
              <Inbox className="mx-auto mb-3 text-[var(--nova-accent)]" size={28} />
              <p className="text-[var(--nova-ink)]">{t("triageDone")}</p>
            </div>
          ) : (
            <div className="overflow-hidden rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[0_1px_0_color-mix(in_srgb,var(--nova-ink)_6%,transparent)]">
              <div className="flex items-center justify-between gap-3 border-b border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-accent-soft)_40%,var(--nova-surface))] px-5 py-3">
                <span className="rounded-full bg-[var(--nova-accent)] px-3 py-1 text-xs font-semibold text-white">
                  {index + 1} / {queue.length}
                </span>
                {current.hasAttachments ? (
                  <span className="inline-flex items-center gap-1.5 rounded-full border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-1 text-xs font-medium text-[var(--nova-accent)]">
                    <Paperclip size={13} />
                    {t("hasAttachments")}
                  </span>
                ) : null}
              </div>

              <div className="px-5 py-5">
                <h3 className="font-[family-name:var(--nova-font-display)] text-[1.65rem] leading-tight text-[var(--nova-ink)]">
                  {current.subject || t("noSubject")}
                </h3>
                <div className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-[var(--nova-ink-muted)]">
                  <span className="font-medium text-[var(--nova-ink)]">
                    {displayName(current.from)}
                  </span>
                  <span>&lt;{current.from.email}&gt;</span>
                  <span aria-hidden>·</span>
                  <span>{formatRelative(current.date, locale)}</span>
                </div>

                {aiSummary ? (
                  <div className="mt-5 rounded-[var(--nova-radius-md)] border border-[var(--nova-accent)]/25 bg-[var(--nova-accent-soft)] px-4 py-3">
                    <p className="mb-1 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-accent)]">
                      <Sparkles size={13} aria-hidden />
                      {t("aiPreviewSummary")}
                    </p>
                    <p className="text-[15px] leading-6 text-[var(--nova-ink)]">
                      {aiSummary}
                    </p>
                  </div>
                ) : aiLoading ? (
                  <div className="mt-5 flex items-center gap-2 rounded-[var(--nova-radius-md)] border border-dashed border-[var(--nova-border)] px-4 py-3 text-xs text-[var(--nova-ink-muted)]">
                    <Sparkles size={13} className="animate-pulse" aria-hidden />
                    {t("working")}
                  </div>
                ) : null}

                <div className="mt-4 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-bg)_70%,var(--nova-surface))] px-4 py-4">
                  <p className="text-[15px] leading-7 text-[var(--nova-ink)]">
                    {plainPreview(current.snippet)}
                  </p>
                </div>

                {previewOpen ? (
                  <div className="mt-4 max-h-[36vh] overflow-y-auto rounded-[var(--nova-radius-md)] border border-[var(--nova-accent)]/30 bg-[var(--nova-accent-soft)]/35 px-4 py-4">
                    <p className="mb-2 text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-accent)]">
                      {t("preview")}
                    </p>
                    {aiSummary ? (
                      <p className="mb-3 text-sm font-medium leading-6 text-[var(--nova-ink)]">
                        {aiSummary}
                      </p>
                    ) : null}
                    {preview ? (
                      <div className="whitespace-pre-wrap text-[14px] leading-6 text-[var(--nova-ink)]">
                        {previewText || plainPreview(current.snippet)}
                      </div>
                    ) : (
                      <p className="text-sm text-[var(--nova-ink-muted)]">
                        {t("loadingPreview")}
                      </p>
                    )}
                  </div>
                ) : null}
              </div>
            </div>
          )}
          {error ? (
            <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        {!done ? (
          <footer className="grid gap-3 border-t border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_92%,var(--nova-bg))] px-6 py-4 sm:grid-cols-[auto_1fr_1fr]">
            <Button
              type="button"
              variant="secondary"
              size="lg"
              className="min-w-[140px]"
              onClick={() => setPreviewOpen((value) => !value)}
            >
              {previewOpen ? <EyeOff size={18} /> : <Eye size={18} />}
              <span>{previewOpen ? t("hidePreview") : t("preview")}</span>
              <kbd className="ml-1 rounded border border-[var(--nova-border)] bg-[var(--nova-surface)] px-1.5 py-0.5 text-[11px] text-[var(--nova-ink-muted)]">
                V
              </kbd>
            </Button>
            <Button
              type="button"
              variant="primary"
              size="lg"
              className="w-full"
              onClick={handleKeep}
            >
              <Inbox size={18} />
              <span>{t("keepInInbox")}</span>
              <kbd className="ml-1 rounded border border-white/30 bg-white/15 px-1.5 py-0.5 text-[11px]">
                B
              </kbd>
            </Button>
            <Button
              type="button"
              variant="danger"
              size="lg"
              className="w-full"
              onClick={handleDelete}
            >
              <Trash2 size={18} />
              <span>{t("delete")}</span>
              <kbd className="ml-1 rounded border border-white/30 bg-white/15 px-1.5 py-0.5 text-[11px]">
                L
              </kbd>
            </Button>
          </footer>
        ) : (
          <footer className="flex justify-end border-t border-[var(--nova-border)] px-6 py-4">
            <Button type="button" onClick={onClose}>
              {t("done")}
            </Button>
          </footer>
        )}
      </div>
    </div>
  );
}
