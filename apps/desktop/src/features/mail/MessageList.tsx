import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Paperclip, Star } from "lucide-react";
import { cn } from "@novamail/ui";

import type { MessageSummaryDto } from "@/shared/api/types";
import { displayName, formatMessageDate } from "@/shared/lib/format";

interface MessageListProps {
  messages: MessageSummaryDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  total: number;
}

export function MessageList({ messages, selectedId, onSelect, total }: MessageListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 76,
    overscan: 12,
  });

  return (
    <section
      aria-label="Message list"
      className="flex h-full min-w-0 flex-col border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]"
    >
      <header className="flex items-center justify-between px-4 py-3">
        <div>
          <h1 className="text-sm font-semibold tracking-wide text-[var(--nova-ink)]">
            Unified Inbox
          </h1>
          <p className="text-xs text-[var(--nova-ink-muted)]">{total} messages</p>
        </div>
      </header>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        <div
          style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const message = messages[item.index];
            const selected = message.id === selectedId;
            return (
              <button
                key={message.id}
                type="button"
                onClick={() => onSelect(message.id)}
                className={cn(
                  "absolute left-0 right-0 flex w-full flex-col gap-1 border-b border-[var(--nova-border)] px-4 py-3 text-left transition-colors",
                  selected
                    ? "bg-[var(--nova-accent-soft)]"
                    : "hover:bg-[var(--nova-surface-2)]",
                )}
                style={{
                  height: `${item.size}px`,
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <div className="flex items-center justify-between gap-2">
                  <span
                    className={cn(
                      "truncate text-sm",
                      message.unread ? "font-semibold" : "font-medium",
                    )}
                  >
                    {displayName(message.from)}
                  </span>
                  <span className="shrink-0 text-xs text-[var(--nova-ink-muted)]">
                    {formatMessageDate(message.date)}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      "truncate text-sm",
                      message.unread
                        ? "text-[var(--nova-ink)]"
                        : "text-[var(--nova-ink-muted)]",
                    )}
                  >
                    {message.subject || "(no subject)"}
                  </span>
                  {message.starred ? (
                    <Star className="h-3.5 w-3.5 fill-[var(--nova-warning)] text-[var(--nova-warning)]" />
                  ) : null}
                  {message.hasAttachments ? (
                    <Paperclip className="h-3.5 w-3.5 text-[var(--nova-ink-muted)]" />
                  ) : null}
                </div>
                <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                  {message.snippet}
                </p>
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}
