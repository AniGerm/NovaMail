import { useEffect, useState } from "react";
import { Reply, Star, Forward, Sparkles } from "lucide-react";
import { Button, EmptyState, IconButton } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, MessageDetailDto } from "@/shared/api/types";
import { displayName, formatRelative } from "@/shared/lib/format";

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
  const [summary, setSummary] = useState<string | null>(null);
  const [summaryProvider, setSummaryProvider] = useState<string | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    setSummaryProvider(null);
    setAiError(null);
  }, [message?.summary.id]);

  if (!message) {
    return (
      <EmptyState
        title="Select a message"
        description="Choose a conversation from the unified inbox to read it here."
      />
    );
  }

  const current = message;
  const body =
    current.bodyText?.trim() ||
    stripHtml(current.bodyHtml ?? "") ||
    current.summary.snippet;

  async function handleSummarize() {
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSummarizeMessage(current.summary.id);
      setSummary(result.summary);
      setSummaryProvider(result.provider);
    } catch (error) {
      setAiError((error as AppError).message || "Summarize failed");
    } finally {
      setAiBusy(false);
    }
  }

  async function handleSuggestReply() {
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSuggestReply(current.summary.id);
      onUseSuggestedReply?.(result.suggestion);
      onReply();
    } catch (error) {
      setAiError((error as AppError).message || "Suggest reply failed");
    } finally {
      setAiBusy(false);
    }
  }

  return (
    <article
      aria-label="Reading pane"
      className="nova-fade-in flex h-full min-w-0 flex-col"
    >
      <header className="border-b border-[var(--nova-border)] px-8 py-5">
        <div className="mb-3 flex items-start justify-between gap-4">
          <h2 className="max-w-3xl font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
            {current.summary.subject || "(no subject)"}
          </h2>
          <div className="flex items-center gap-1">
            <IconButton label="Star message" onClick={onToggleStar}>
              <Star
                className={
                  current.summary.starred
                    ? "fill-[var(--nova-warning)] text-[var(--nova-warning)]"
                    : ""
                }
              />
            </IconButton>
            <IconButton label="Reply" onClick={onReply}>
              <Reply />
            </IconButton>
            <IconButton label="Forward" onClick={onForward}>
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
          <span>{formatRelative(current.summary.date)}</span>
          <span>·</span>
          <span>{current.summary.accountEmail}</span>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button size="sm" onClick={onReply}>
            Reply
          </Button>
          <Button size="sm" variant="secondary" onClick={onForward}>
            Forward
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={aiBusy}
            onClick={handleSummarize}
          >
            <Sparkles size={14} />
            {aiBusy ? "Working…" : "Summarize"}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={aiBusy}
            onClick={handleSuggestReply}
          >
            Suggest reply
          </Button>
        </div>
        {aiError ? (
          <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
            {aiError}
          </p>
        ) : null}
        {summary ? (
          <div className="mt-4 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-4 py-3 text-sm text-[var(--nova-ink)]">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
              Summary{summaryProvider ? ` · ${summaryProvider}` : ""}
            </p>
            <p className="leading-6">{summary}</p>
          </div>
        ) : null}
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
        {current.bodyHtml ? (
          <div
            className="prose mx-auto max-w-[720px] text-[15px] leading-7 text-[var(--nova-ink)]"
            // HTML is sanitized in novamail-core before IPC.
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
