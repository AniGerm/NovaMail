import { useCallback, useEffect, useMemo, useState } from "react";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { Button } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, MessageDetailDto, MessageSummaryDto } from "@/shared/api/types";
import { displayName, formatRelative } from "@/shared/lib/format";

interface QuickTriageProps {
  open: boolean;
  messages: MessageSummaryDto[];
  onClose: () => void;
  onChanged: () => void;
}

export function QuickTriage({ open, messages, onClose, onChanged }: QuickTriageProps) {
  const queue = useMemo(
    () => messages.filter((m) => m.unread || true),
    [messages],
  );
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
      setError((err as AppError).message || "Keep failed");
    } finally {
      setBusy(false);
    }
  }, [advance, busy, current, onChanged]);

  const handleDelete = useCallback(async () => {
    if (!current || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.messagesDelete(current.id);
      onChanged();
      advance();
    } catch (err) {
      setError((err as AppError).message || "Delete failed");
    } finally {
      setBusy(false);
    }
  }, [advance, busy, current, onChanged]);

  const shortcuts = useMemo(
    () => ({
      k: () => {
        void handleKeep();
      },
      d: () => {
        void handleDelete();
      },
      " ": () => setPreviewOpen((v) => !v),
      enter: () => setPreviewOpen(true),
      escape: () => {
        if (previewOpen) setPreviewOpen(false);
        else onClose();
      },
    }),
    [handleDelete, handleKeep, onClose, previewOpen],
  );
  useKeyboardShortcuts(open ? shortcuts : {});

  if (!open) return null;

  const done = !current;

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
              Quick Sort
            </h2>
            <p className="text-sm text-[var(--nova-ink-muted)]">
              Hotkeys: K keep · D delete · Space preview · Esc close
            </p>
          </div>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            Close
          </Button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          {done ? (
            <p className="text-[var(--nova-ink-muted)]">
              Inbox sorted. No more messages in this queue.
            </p>
          ) : (
            <>
              <p className="mb-2 text-xs uppercase tracking-wide text-[var(--nova-ink-muted)]">
                {index + 1} / {queue.length}
              </p>
              <h3 className="font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
                {current.subject || "(no subject)"}
              </h3>
              <p className="mt-2 text-sm text-[var(--nova-ink-muted)]">
                {displayName(current.from)} &lt;{current.from.email}&gt; ·{" "}
                {formatRelative(current.date)}
              </p>
              <p className="mt-4 text-[15px] leading-7 text-[var(--nova-ink)]">
                {current.snippet}
              </p>
              {current.hasAttachments ? (
                <p className="mt-3 text-sm text-[var(--nova-accent)]">Has attachments</p>
              ) : null}

              {previewOpen ? (
                <div className="mt-5 max-h-[40vh] overflow-y-auto rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_85%,var(--nova-bg))] p-4">
                  {preview?.bodyHtml ? (
                    <div
                      className="prose text-[14px] leading-6"
                      dangerouslySetInnerHTML={{ __html: preview.bodyHtml }}
                    />
                  ) : (
                    <pre className="whitespace-pre-wrap text-[14px] leading-6">
                      {preview?.bodyText || current.snippet}
                    </pre>
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
              onClick={() => setPreviewOpen((v) => !v)}
            >
              {previewOpen ? "Hide preview" : "Preview"}
            </Button>
            <Button type="button" variant="secondary" disabled={busy} onClick={handleKeep}>
              Keep in inbox
            </Button>
            <Button type="button" disabled={busy} onClick={handleDelete}>
              Delete
            </Button>
          </footer>
        ) : null}
      </div>
    </div>
  );
}
