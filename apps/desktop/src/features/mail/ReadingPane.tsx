import { Reply, Star, Forward } from "lucide-react";
import { Button, EmptyState, IconButton } from "@novamail/ui";

import type { MessageDetailDto } from "@/shared/api/types";
import { displayName, formatRelative } from "@/shared/lib/format";

interface ReadingPaneProps {
  message?: MessageDetailDto | null;
  onReply: () => void;
  onToggleStar: () => void;
}

export function ReadingPane({ message, onReply, onToggleStar }: ReadingPaneProps) {
  if (!message) {
    return (
      <EmptyState
        title="Select a message"
        description="Choose a conversation from the unified inbox to read it here."
      />
    );
  }

  const body =
    message.bodyText?.trim() ||
    stripHtml(message.bodyHtml ?? "") ||
    message.summary.snippet;

  return (
    <article
      aria-label="Reading pane"
      className="nova-fade-in flex h-full min-w-0 flex-col"
    >
      <header className="border-b border-[var(--nova-border)] px-8 py-5">
        <div className="mb-3 flex items-start justify-between gap-4">
          <h2 className="max-w-3xl font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
            {message.summary.subject || "(no subject)"}
          </h2>
          <div className="flex items-center gap-1">
            <IconButton label="Star message" onClick={onToggleStar}>
              <Star
                className={
                  message.summary.starred
                    ? "fill-[var(--nova-warning)] text-[var(--nova-warning)]"
                    : ""
                }
              />
            </IconButton>
            <IconButton label="Reply" onClick={onReply}>
              <Reply />
            </IconButton>
            <IconButton label="Forward">
              <Forward />
            </IconButton>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-[var(--nova-ink-muted)]">
          <span className="font-medium text-[var(--nova-ink)]">
            {displayName(message.summary.from)}
          </span>
          <span>&lt;{message.summary.from.email}&gt;</span>
          <span>·</span>
          <span>{formatRelative(message.summary.date)}</span>
          <span>·</span>
          <span>{message.summary.accountEmail}</span>
        </div>
        <div className="mt-4 flex gap-2">
          <Button size="sm" onClick={onReply}>
            Reply
          </Button>
          <Button size="sm" variant="secondary">
            Forward
          </Button>
        </div>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
        <div className="mx-auto max-w-[720px] whitespace-pre-wrap text-[15px] leading-7 text-[var(--nova-ink)]">
          {body}
        </div>
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
