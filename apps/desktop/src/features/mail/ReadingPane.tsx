import { useEffect, useState } from "react";
import { open as openPath } from "@tauri-apps/plugin-shell";
import { Reply, Star, Forward, Sparkles, Paperclip } from "lucide-react";
import { Button, EmptyState, IconButton, Input } from "@novamail/ui";

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
  const [summaryProvider, setSummaryProvider] = useState<string | null>(null);
  const [variantA, setVariantA] = useState<string | null>(null);
  const [variantB, setVariantB] = useState<string | null>(null);
  const [selectedVariant, setSelectedVariant] = useState<"a" | "b">("a");
  const [draft, setDraft] = useState("");
  const [facts, setFacts] = useState("");
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    setSummaryProvider(null);
    setVariantA(null);
    setVariantB(null);
    setSelectedVariant("a");
    setDraft("");
    setFacts("");
    setAiError(null);
    const id = message?.summary.id;
    if (!id) return;
    let cancelled = false;
    void api
      .aiMessageInsights(id)
      .then((insights) => {
        if (cancelled) return;
        if (insights.summary) {
          setSummary(insights.summary);
          setSummaryProvider(insights.provider ?? "cache");
        }
        const a = insights.replyA ?? insights.replySuggestion ?? null;
        const b = insights.replyB ?? null;
        if (a) {
          setVariantA(a);
          setDraft(a);
        }
        if (b) setVariantB(b);
      })
      .catch(() => {
        /* insights optional */
      });
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
      setSummaryProvider(result.provider);
    } catch (error) {
      setAiError((error as AppError).message || t("summarizeFailed"));
    } finally {
      setAiBusy(false);
    }
  }

  async function loadReplyVariants(withFacts?: string) {
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSuggestReplies(
        current.summary.id,
        withFacts?.trim() || null,
      );
      const a = result.variants[0] ?? "";
      const b = result.variants[1] ?? result.variants[0] ?? "";
      setVariantA(a || null);
      setVariantB(b || null);
      setSelectedVariant("a");
      setDraft(a);
      setSummaryProvider(result.provider);
    } catch (error) {
      setAiError((error as AppError).message || t("suggestFailed"));
    } finally {
      setAiBusy(false);
    }
  }

  function pickVariant(which: "a" | "b") {
    setSelectedVariant(which);
    const text = which === "a" ? variantA : variantB;
    if (text) setDraft(text);
  }

  function useDraftInReply() {
    if (!draft.trim()) return;
    onUseSuggestedReply?.(draft);
    onReply();
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
        <div className="mt-4 flex flex-wrap gap-2">
          <Button size="sm" onClick={onReply}>
            {t("reply")}
          </Button>
          <Button size="sm" variant="secondary" onClick={onForward}>
            {t("forward")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={aiBusy}
            onClick={() => void handleSummarize()}
          >
            <Sparkles size={14} />
            {aiBusy ? t("working") : t("summarize")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={aiBusy}
            onClick={() => void loadReplyVariants()}
          >
            {t("suggestReply")}
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
                    onClick={() => {
                      void (async () => {
                        try {
                          const path = await api.attachmentsOpenPath(attachment.id);
                          await openPath(path);
                        } catch (error) {
                          setAiError(
                            (error as AppError).message || t("openAttachmentFailed"),
                          );
                        }
                      })();
                    }}
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
        {summary ? (
          <div className="mt-4 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-4 py-3 text-sm text-[var(--nova-ink)]">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
              {t("summary")}
              {summaryProvider ? ` · ${summaryProvider}` : ""}
            </p>
            <p className="leading-6">{summary}</p>
          </div>
        ) : null}

        <section className="mt-4 grid gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-4 py-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-medium">{t("replyAssistTitle")}</h3>
            <span className="text-xs text-[var(--nova-ink-muted)]">
              {t("replyAssistHint")}
            </span>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              variant={selectedVariant === "a" ? "primary" : "secondary"}
              disabled={!variantA || aiBusy}
              onClick={() => pickVariant("a")}
            >
              {t("replyVariantA")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant={selectedVariant === "b" ? "primary" : "secondary"}
              disabled={!variantB || aiBusy}
              onClick={() => pickVariant("b")}
            >
              {t("replyVariantB")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={aiBusy}
              onClick={() => void loadReplyVariants()}
            >
              {aiBusy ? t("working") : t("generateReplyVariants")}
            </Button>
          </div>
          <textarea
            className="min-h-[120px] w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2 text-sm leading-6"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={t("replyDraftPlaceholder")}
          />
          <label className="grid gap-1 text-xs">
            <span>{t("replyFactsLabel")}</span>
            <Input
              value={facts}
              onChange={(e) => setFacts(e.target.value)}
              placeholder={t("replyFactsPlaceholder")}
            />
          </label>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={aiBusy || !facts.trim()}
              onClick={() => void loadReplyVariants(facts)}
            >
              {t("rewriteWithFacts")}
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={!draft.trim()}
              onClick={useDraftInReply}
            >
              {t("useReplyDraft")}
            </Button>
          </div>
        </section>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
        {current.bodyHtml ? (
          <div
            className="prose mx-auto max-w-[720px] text-[15px] leading-7 text-[var(--nova-ink)]"
            dangerouslySetInnerHTML={{ __html: current.bodyHtml }}
          />
        ) : (
          <div className="mx-auto max-w-[720px] whitespace-pre-wrap text-[15px] leading-7 text-[var(--nova-ink)]">
            {body}
          </div>
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
