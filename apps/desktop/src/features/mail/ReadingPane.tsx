import { useEffect, useState } from "react";
import { open as openPath } from "@tauri-apps/plugin-shell";
import { Reply, Star, Forward, Sparkles, Paperclip } from "lucide-react";
import { Button, EmptyState, IconButton } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, MessageDetailDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { displayName, formatRelative } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

interface ReadingPaneProps {
  message?: MessageDetailDto | null;
  onReply: () => void;
  onForward?: () => void;
  onToggleStar: () => void;
  onUseSuggestedReply?: (suggestion: string) => void;
}

export function ReadingPane({
  message,
  onReply,
  onForward,
  onToggleStar,
  onUseSuggestedReply,
}: ReadingPaneProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const [summary, setSummary] = useState<string | null>(null);
  const [variantA, setVariantA] = useState<string | null>(null);
  const [variantB, setVariantB] = useState<string | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    setVariantA(null);
    setVariantB(null);
    setAiError(null);
    const id = message?.summary.id;
    if (!id) return;
    let cancelled = false;
    void api
      .aiMessageInsights(id)
      .then((insights) => {
        if (cancelled) return;
        if (insights.summary) setSummary(insights.summary);
        setVariantA(insights.replyA ?? insights.replySuggestion ?? null);
        setVariantB(insights.replyB ?? null);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [message?.summary.id]);

  if (!message) {
    return (
      <EmptyState
        title={t("selectMessageTitle")}
        description={t("selectMessageDescription")}
      />
    );
  }

  const current = message;
  const body =
    current.bodyText?.trim() ||
    stripHtml(current.bodyHtml ?? "") ||
    current.summary.snippet;
  const attachments = current.attachments ?? [];

  async function handleSummarize() {
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSummarizeMessage(current.summary.id);
      setSummary(result.summary);
    } catch (error) {
      setAiError((error as AppError).message || t("summarizeFailed"));
    } finally {
      setAiBusy(false);
    }
  }

  async function ensureVariants(): Promise<{ a: string; b: string } | null> {
    if (variantA && variantB) {
      return { a: variantA, b: variantB };
    }
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSuggestReplies(current.summary.id);
      const a = result.variants[0] ?? "";
      const b = result.variants[1] ?? result.variants[0] ?? "";
      setVariantA(a || null);
      setVariantB(b || null);
      if (!a) return null;
      return { a, b: b || a };
    } catch (error) {
      setAiError((error as AppError).message || t("suggestFailed"));
      return null;
    } finally {
      setAiBusy(false);
    }
  }

  async function openVariant(which: "a" | "b") {
    const variants = await ensureVariants();
    if (!variants) return;
    const text = which === "a" ? variants.a : variants.b;
    onUseSuggestedReply?.(text);
    onReply();
  }

  function openOwnReply() {
    onUseSuggestedReply?.("");
    onReply();
  }

  async function openAttachment(id: string) {
    try {
      const path = await api.attachmentsOpenPath(id);
      await openPath(path);
    } catch (error) {
      setAiError((error as AppError).message || t("openAttachmentFailed"));
    }
  }

  return (
    <article
      aria-label={t("readingPane")}
      className="nova-fade-in flex h-full min-w-0 flex-col"
    >
      <header className="border-b border-[var(--nova-border)] px-8 py-5">
        <div className="mb-3 flex items-start justify-between gap-4">
          <h2 className="max-w-3xl font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
            {current.summary.subject || t("noSubject")}
          </h2>
          <div className="flex items-center gap-1">
            <IconButton label={t("starMessage")} onClick={onToggleStar}>
              <Star
                className={
                  current.summary.starred
                    ? "fill-[var(--nova-warning)] text-[var(--nova-warning)]"
                    : ""
                }
              />
            </IconButton>
            <IconButton label={t("reply")} onClick={openOwnReply}>
              <Reply />
            </IconButton>
            <IconButton label={t("forward")} onClick={onForward}>
              <Forward />
            </IconButton>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-[var(--nova-ink-muted)]">
          <span className="font-medium text-[var(--nova-ink)]">
            {displayName(current.summary.from)}
          </span>
          <span>&lt;{current.summary.from.email}&gt;</span>
          <span>·</span>
          <span>{formatRelative(current.summary.date, locale)}</span>
          <span>·</span>
          <span>{current.summary.accountEmail}</span>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button size="sm" onClick={openOwnReply}>
            {t("reply")}
          </Button>
          <Button size="sm" variant="secondary" onClick={onForward}>
            {t("forward")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={aiBusy}
            onClick={() => void handleSummarize()}
          >
            <Sparkles size={14} />
            {aiBusy ? t("working") : t("summarize")}
          </Button>
        </div>
        {attachments.length > 0 ? (
          <div className="mt-4">
            <p className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
              <Paperclip size={14} />
              {t("attachments")}
            </p>
            <ul className="flex flex-wrap gap-2">
              {attachments.map((attachment) => (
                <li key={attachment.id}>
                  <button
                    type="button"
                    className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-1.5 text-sm hover:bg-[var(--nova-accent-soft)]"
                    onClick={() => void openAttachment(attachment.id)}
                  >
                    {attachment.filename}{" "}
                    <span className="text-[var(--nova-ink-muted)]">
                      ({formatBytes(attachment.size)})
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        {aiError ? (
          <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
            {aiError}
          </p>
        ) : null}
      </header>

      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-8 py-6">
        <section
          aria-label={t("messageBody")}
          className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-5 py-5 shadow-[0_1px_0_color-mix(in_srgb,var(--nova-ink)_5%,transparent)]"
        >
          <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
            {t("messageBody")}
          </p>
          {current.bodyHtml ? (
            <div
              className="prose max-w-none text-[15px] leading-7 text-[var(--nova-ink)]"
              dangerouslySetInnerHTML={{ __html: current.bodyHtml }}
            />
          ) : (
            <div className="whitespace-pre-wrap text-[15px] leading-7 text-[var(--nova-ink)]">
              {body}
            </div>
          )}
        </section>

        <section
          aria-label={t("replyAssistTitle")}
          className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-bg)_55%,var(--nova-surface))] px-4 py-4"
        >
          <p className="mb-1 text-sm font-medium">{t("replyAssistTitle")}</p>
          {summary ? (
            <p className="mb-3 text-sm leading-6 text-[var(--nova-ink-muted)]">
              <span className="font-medium text-[var(--nova-accent)]">{t("summary")}: </span>
              {summary}
            </p>
          ) : null}
          <p className="mb-3 text-xs text-[var(--nova-ink-muted)]">
            {t("replyAssistSimpleHint")}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              disabled={aiBusy}
              onClick={() => void openVariant("a")}
            >
              {t("replyVariantAShort")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={aiBusy}
              onClick={() => void openVariant("b")}
            >
              {t("replyVariantBShort")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={openOwnReply}
            >
              {t("replyOwn")}
            </Button>
          </div>
        </section>
      </div>
    </article>
  );
}

function stripHtml(html: string): string {
  return html
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}
