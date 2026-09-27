import { useEffect, useRef, useState } from "react";
import { open as openPath } from "@tauri-apps/plugin-shell";
import { Reply, Star, Forward, Sparkles, Paperclip } from "lucide-react";
import { Button, EmptyState, IconButton } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, MessageDetailDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { displayName, formatRelative } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

type ReplyVariant = "a" | "b" | "own";

interface ReadingPaneProps {
  message?: MessageDetailDto | null;
  aiEnabled?: boolean;
  onReply: () => void;
  onForward?: () => void;
  onToggleStar: () => void;
  onReplySent?: () => void;
}

export function ReadingPane({
  message,
  aiEnabled = true,
  onReply,
  onForward,
  onToggleStar,
  onReplySent,
}: ReadingPaneProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const [summary, setSummary] = useState<string | null>(null);
  const [variantA, setVariantA] = useState<string | null>(null);
  const [variantB, setVariantB] = useState<string | null>(null);
  const [activeVariant, setActiveVariant] = useState<ReplyVariant>("a");
  const [draft, setDraft] = useState("");
  const [aiBusy, setAiBusy] = useState(false);
  const [sendBusy, setSendBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const draftRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setSummary(null);
    setVariantA(null);
    setVariantB(null);
    setActiveVariant("a");
    setDraft("");
    setAiError(null);
    const id = message?.summary.id;
    if (!id) return;
    let cancelled = false;
    void api
      .aiMessageInsights(id)
      .then((insights) => {
        if (cancelled) return;
        if (insights.summary) setSummary(insights.summary);
        const a = insights.replyA ?? insights.replySuggestion ?? null;
        const b = insights.replyB ?? null;
        setVariantA(a);
        setVariantB(b);
        if (a) {
          setActiveVariant("a");
          setDraft(a);
        }
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

  async function selectVariant(which: "a" | "b") {
    const variants = await ensureVariants();
    if (!variants) return;
    setActiveVariant(which);
    setDraft(which === "a" ? variants.a : variants.b);
    requestAnimationFrame(() => draftRef.current?.focus());
  }

  function selectOwn() {
    setActiveVariant("own");
    setDraft("");
    requestAnimationFrame(() => draftRef.current?.focus());
  }

  async function handleSendReply() {
    const text = draft.trim();
    if (!text) {
      setAiError(t("replyDraftEmpty"));
      return;
    }
    setSendBusy(true);
    setAiError(null);
    try {
      const subject = current.summary.subject.startsWith("Re:")
        ? current.summary.subject
        : `Re: ${current.summary.subject || t("noSubject")}`;
      await api.messagesSend({
        accountId: current.summary.accountId,
        to: [{ email: current.summary.from.email, name: current.summary.from.name }],
        cc: [],
        bcc: [],
        subject,
        bodyText: text,
        bodyHtml: null,
        inReplyTo: current.messageId ?? null,
        references: [
          ...current.references,
          current.messageId ?? "",
        ].filter(Boolean),
        attachments: [],
      });
      onReplySent?.();
      if (activeVariant === "a") setVariantA(text);
      if (activeVariant === "b") setVariantB(text);
    } catch (error) {
      setAiError((error as AppError).message || t("sendFailed"));
    } finally {
      setSendBusy(false);
    }
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
            <IconButton label={t("reply")} onClick={onReply}>
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
        {aiError ? (
          <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
            {aiError}
          </p>
        ) : null}
      </header>

      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-8 py-6">
        {attachments.length > 0 ? (
          <section aria-label={t("attachments")}>
            <p className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
              <Paperclip size={14} />
              {t("attachments")}
            </p>
            <ul className="flex flex-wrap gap-2">
              {attachments.map((attachment) => (
                <li key={attachment.id}>
                  <button
                    type="button"
                    className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-1.5 text-sm hover:bg-[var(--nova-accent-soft)]"
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
          </section>
        ) : null}

        <section
          aria-label={t("messageBody")}
          className="rounded-[var(--nova-radius-md)] border-2 border-[#7aa7d4] bg-[color-mix(in_srgb,var(--nova-accent-soft)_35%,var(--nova-surface))] px-5 py-5 shadow-[0_0_0_3px_color-mix(in_srgb,#7aa7d4_22%,transparent)]"
        >
          <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
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

        {aiEnabled ? (
          <section
            aria-label={t("replyAssistTitle")}
            className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-bg)_55%,var(--nova-surface))] px-4 py-4"
          >
            <div className="mb-2 flex items-center justify-between gap-3">
              <p className="text-sm font-medium">{t("replyAssistTitle")}</p>
              {!summary ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={aiBusy}
                  onClick={() => void handleSummarize()}
                >
                  <Sparkles size={14} />
                  {aiBusy ? t("working") : t("summarize")}
                </Button>
              ) : null}
            </div>
            {summary ? (
              <p className="mb-3 text-sm leading-6 text-[var(--nova-ink-muted)]">
                <span className="font-medium text-[var(--nova-accent)]">{t("summary")}: </span>
                {summary}
              </p>
            ) : null}
            <p className="mb-3 text-xs text-[var(--nova-ink-muted)]">
              {t("replyAssistSimpleHint")}
            </p>
            <div className="mb-3 flex flex-wrap gap-2">
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "a" ? "primary" : "secondary"}
                disabled={aiBusy || sendBusy}
                onClick={() => void selectVariant("a")}
              >
                {t("replyVariantAShort")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "b" ? "primary" : "secondary"}
                disabled={aiBusy || sendBusy}
                onClick={() => void selectVariant("b")}
              >
                {t("replyVariantBShort")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "own" ? "primary" : "ghost"}
                disabled={aiBusy || sendBusy}
                onClick={selectOwn}
              >
                {t("replyOwn")}
              </Button>
            </div>
            <label className="grid gap-1.5">
              <span className="sr-only">{t("replyDraftPlaceholder")}</span>
              <textarea
                ref={draftRef}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                placeholder={t("replyDraftPlaceholder")}
                rows={6}
                className="w-full resize-y rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2.5 text-[15px] leading-6 text-[var(--nova-ink)] outline-none focus:border-[var(--nova-accent)] focus:ring-2 focus:ring-[color-mix(in_srgb,var(--nova-accent)_25%,transparent)]"
              />
            </label>
            <div className="mt-3 flex justify-end">
              <Button
                type="button"
                disabled={aiBusy || sendBusy || !draft.trim()}
                onClick={() => void handleSendReply()}
              >
                {sendBusy ? t("working") : t("send")}
              </Button>
            </div>
          </section>
        ) : (
          <p className="text-sm text-[var(--nova-ink-muted)]">{t("aiDisabledHint")}</p>
        )}
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
