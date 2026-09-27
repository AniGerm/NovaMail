import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ChevronDown,
  ChevronRight,
  ListTree,
  List,
  Paperclip,
  Star,
} from "lucide-react";
import { cn, Select } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AccountDto,
  ListMessagesRequest,
  MailboxDto,
  MessageSortBy,
  MessageSummaryDto,
  SortDirection,
  ThreadListItemDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { displayName, formatMessageDate } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

export type InboxViewMode = "threads" | "flat";

export interface InboxFilters {
  unreadOnly: boolean;
  starredOnly: boolean;
  hasAttachments: boolean;
  accountId: string | null;
  mailboxId: string | null;
  /** e.g. `"drafts"` — list that mailbox role across accounts */
  mailboxRole: string | null;
  /** Virtual folder: messages kept only locally after IMAP offload */
  localOnly: boolean;
  sortBy: MessageSortBy;
  sortDir: SortDirection;
  viewMode: InboxViewMode;
}

interface MessageListProps {
  messages: MessageSummaryDto[];
  threads: ThreadListItemDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onToggleStar?: (messageId: string, starred: boolean) => void;
  total: number;
  filters: InboxFilters;
  onFiltersChange: (next: InboxFilters) => void;
  accounts: AccountDto[];
  mailboxes: MailboxDto[];
}

type FlatRow =
  | { kind: "message"; message: MessageSummaryDto }
  | {
      kind: "thread";
      thread: ThreadListItemDto;
      expanded: boolean;
    }
  | {
      kind: "thread-child";
      message: MessageSummaryDto;
      threadId: string;
    };

export function MessageList({
  messages,
  threads,
  selectedId,
  onSelect,
  onToggleStar,
  total,
  filters,
  onFiltersChange,
  accounts,
  mailboxes,
}: MessageListProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const parentRef = useRef<HTMLDivElement>(null);
  const [expandedThreads, setExpandedThreads] = useState<Set<string>>(new Set());
  const [threadMessages, setThreadMessages] = useState<
    Record<string, MessageSummaryDto[]>
  >({});
  const [loadingThread, setLoadingThread] = useState<string | null>(null);

  useEffect(() => {
    setExpandedThreads(new Set());
    setThreadMessages({});
  }, [filters.viewMode, filters.accountId, filters.mailboxId, filters.unreadOnly, filters.starredOnly, filters.hasAttachments, filters.sortBy, filters.sortDir]);

  const toggleThread = async (threadId: string) => {
    const next = new Set(expandedThreads);
    if (next.has(threadId)) {
      next.delete(threadId);
      setExpandedThreads(next);
      return;
    }
    next.add(threadId);
    setExpandedThreads(next);
    if (!threadMessages[threadId]) {
      setLoadingThread(threadId);
      try {
        const items = await api.messagesListByThread(threadId);
        setThreadMessages((prev) => ({ ...prev, [threadId]: items }));
        if (items.length > 0 && !selectedId) {
          onSelect(items[items.length - 1].id);
        }
      } finally {
        setLoadingThread(null);
      }
    }
  };

  const rows: FlatRow[] = useMemo(() => {
    if (filters.viewMode === "flat") {
      return messages.map((message) => ({ kind: "message" as const, message }));
    }
    const out: FlatRow[] = [];
    for (const thread of threads) {
      const expanded = expandedThreads.has(thread.id);
      out.push({ kind: "thread", thread, expanded });
      if (expanded) {
        const children = threadMessages[thread.id] ?? [];
        for (const message of children) {
          out.push({ kind: "thread-child", message, threadId: thread.id });
        }
      }
    }
    return out;
  }, [expandedThreads, filters.viewMode, messages, threadMessages, threads]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) => {
      const row = rows[index];
      if (row?.kind === "thread-child") return 56;
      return 78;
    },
    overscan: 12,
  });

  const mailboxOptions = useMemo(() => {
    if (!filters.accountId) return mailboxes;
    return mailboxes.filter((m) => m.accountId === filters.accountId);
  }, [filters.accountId, mailboxes]);

  const patch = (partial: Partial<InboxFilters>) =>
    onFiltersChange({ ...filters, ...partial });

  return (
    <section
      aria-label={t("messageList")}
      className="flex h-full min-w-0 flex-col border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]"
    >
      <header className="space-y-3 border-b border-[var(--nova-border)] px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <div>
            <h1 className="text-sm font-semibold tracking-wide text-[var(--nova-ink)]">
              {t("unifiedInbox")}
            </h1>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {filters.viewMode === "threads"
                ? t("threadsCount", { count: total })
                : t("messagesCount", { count: total })}
            </p>
          </div>
          <div className="flex items-center gap-1 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] p-0.5">
            <button
              type="button"
              title={t("viewThreads")}
              aria-pressed={filters.viewMode === "threads"}
              onClick={() => patch({ viewMode: "threads" })}
              className={cn(
                "rounded-[calc(var(--nova-radius-sm)-2px)] p-1.5 transition-colors",
                filters.viewMode === "threads"
                  ? "bg-[var(--nova-accent-soft)] text-[var(--nova-accent)]"
                  : "text-[var(--nova-ink-muted)] hover:text-[var(--nova-ink)]",
              )}
            >
              <ListTree className="h-4 w-4" />
            </button>
            <button
              type="button"
              title={t("viewFlat")}
              aria-pressed={filters.viewMode === "flat"}
              onClick={() => patch({ viewMode: "flat" })}
              className={cn(
                "rounded-[calc(var(--nova-radius-sm)-2px)] p-1.5 transition-colors",
                filters.viewMode === "flat"
                  ? "bg-[var(--nova-accent-soft)] text-[var(--nova-accent)]"
                  : "text-[var(--nova-ink-muted)] hover:text-[var(--nova-ink)]",
              )}
            >
              <List className="h-4 w-4" />
            </button>
          </div>
        </div>

        <div className="flex flex-wrap gap-1.5">
          <FilterChip
            active={filters.unreadOnly}
            onClick={() => patch({ unreadOnly: !filters.unreadOnly })}
            label={t("filterUnread")}
          />
          <FilterChip
            active={filters.starredOnly}
            onClick={() => patch({ starredOnly: !filters.starredOnly })}
            label={t("filterStarred")}
          />
          <FilterChip
            active={filters.hasAttachments}
            onClick={() => patch({ hasAttachments: !filters.hasAttachments })}
            label={t("filterAttachments")}
          />
        </div>

        <div className="grid grid-cols-2 gap-2">
          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              {t("filterMailbox")}
            </span>
            <Select
              value={filters.mailboxId ?? ""}
              onChange={(e) =>
                patch({
                  mailboxId: e.target.value || null,
                  accountId: e.target.value
                    ? mailboxes.find((m) => m.id === e.target.value)?.accountId ??
                      filters.accountId
                    : filters.accountId,
                })
              }
              className="rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-xs"
            >
              <option value="">{t("allMailboxes")}</option>
              {mailboxOptions.map((mailbox) => (
                <option key={mailbox.id} value={mailbox.id}>
                  {mailbox.name}
                </option>
              ))}
            </Select>
          </label>
          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              {t("filterAccount")}
            </span>
            <Select
              value={filters.accountId ?? ""}
              onChange={(e) =>
                patch({
                  accountId: e.target.value || null,
                  mailboxId: null,
                })
              }
              className="rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-xs"
            >
              <option value="">{t("allAccounts")}</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name}
                </option>
              ))}
            </Select>
          </label>
          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              {t("sortBy")}
            </span>
            <Select
              value={filters.sortBy}
              onChange={(e) =>
                patch({ sortBy: e.target.value as MessageSortBy })
              }
              className="rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-xs"
            >
              <option value="date">{t("sortDate")}</option>
              <option value="subject">{t("sortSubject")}</option>
              <option value="from">{t("sortFrom")}</option>
              <option value="attachments">{t("sortAttachments")}</option>
            </Select>
          </label>
          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              {t("sortDirection")}
            </span>
            <Select
              value={filters.sortDir}
              onChange={(e) =>
                patch({ sortDir: e.target.value as SortDirection })
              }
              className="rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-xs"
            >
              <option value="desc">{t("sortNewestFirst")}</option>
              <option value="asc">{t("sortOldestFirst")}</option>
            </Select>
          </label>
        </div>
      </header>

      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        <div
          style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const row = rows[item.index];
            if (!row) return null;

            if (row.kind === "thread") {
              const { thread, expanded } = row;
              const unread = thread.unreadCount > 0;
              return (
                <div
                  key={`thread-${thread.id}`}
                  className="absolute left-0 right-0 border-b border-[var(--nova-border)]"
                  style={{
                    height: `${item.size}px`,
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  <button
                    type="button"
                    onClick={() => {
                      void toggleThread(thread.id);
                    }}
                    className={cn(
                      "flex h-full w-full flex-col gap-1 px-3 py-2.5 text-left transition-colors",
                      unread
                        ? "bg-[color-mix(in_srgb,var(--nova-accent-soft)_35%,transparent)]"
                        : "hover:bg-[var(--nova-surface-2)]",
                    )}
                  >
                    <div className="flex items-center gap-1.5">
                      {expanded ? (
                        <ChevronDown className="h-3.5 w-3.5 shrink-0 text-[var(--nova-ink-muted)]" />
                      ) : (
                        <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[var(--nova-ink-muted)]" />
                      )}
                      <span
                        className={cn(
                          "min-w-0 flex-1 truncate text-sm",
                          unread ? "font-semibold" : "font-medium",
                        )}
                      >
                        {displayName(thread.latestFrom)}
                      </span>
                      <span className="shrink-0 rounded-full bg-[var(--nova-surface-2)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--nova-ink-muted)]">
                        {thread.messageCount}
                      </span>
                      <span className="shrink-0 text-xs text-[var(--nova-ink-muted)]">
                        {formatMessageDate(thread.lastMessageAt, locale)}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 pl-5">
                      <span
                        className={cn(
                          "truncate text-sm",
                          unread
                            ? "text-[var(--nova-ink)]"
                            : "text-[var(--nova-ink-muted)]",
                        )}
                      >
                        {thread.subject || t("noSubject")}
                      </span>
                      {thread.hasAttachments ? (
                        <Paperclip className="h-3.5 w-3.5 shrink-0 text-[var(--nova-ink-muted)]" />
                      ) : null}
                      {unread ? (
                        <span className="shrink-0 text-[10px] font-semibold text-[var(--nova-accent)]">
                          {t("unreadCountShort", { count: thread.unreadCount })}
                        </span>
                      ) : null}
                    </div>
                    <p className="truncate pl-5 text-xs text-[var(--nova-ink-muted)]">
                      {loadingThread === thread.id
                        ? t("loadingThread")
                        : thread.snippet}
                    </p>
                  </button>
                </div>
              );
            }

            const message =
              row.kind === "message" || row.kind === "thread-child"
                ? row.message
                : null;
            if (!message) return null;
            const selected = message.id === selectedId;
            const indented = row.kind === "thread-child";

            return (
              <button
                key={message.id}
                type="button"
                onClick={() => onSelect(message.id)}
                className={cn(
                  "absolute left-0 right-0 flex w-full flex-col gap-1 border-b border-[var(--nova-border)] text-left transition-colors",
                  indented ? "py-2 pl-8 pr-4" : "px-4 py-3",
                  selected
                    ? "bg-[var(--nova-accent-soft)]"
                    : "hover:bg-[var(--nova-surface-2)]",
                  indented &&
                    "border-l-2 border-l-[var(--nova-accent)] bg-[color-mix(in_srgb,var(--nova-surface-2)_55%,transparent)]",
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
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      aria-label={t("starMessage")}
                      aria-pressed={message.starred}
                      className="rounded p-0.5 text-[var(--nova-ink-muted)] hover:bg-[var(--nova-surface)] hover:text-[var(--nova-warning)]"
                      onClick={(event) => {
                        event.stopPropagation();
                        onToggleStar?.(message.id, !message.starred);
                      }}
                    >
                      <Star
                        className={cn(
                          "h-3.5 w-3.5",
                          message.starred &&
                            "fill-[var(--nova-warning)] text-[var(--nova-warning)]",
                        )}
                      />
                    </button>
                    <span className="text-xs text-[var(--nova-ink-muted)]">
                      {formatMessageDate(message.date, locale)}
                    </span>
                  </div>
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
                    {indented
                      ? message.snippet || message.subject || t("noSubject")
                      : message.subject || t("noSubject")}
                  </span>
                  {message.hasAttachments ? (
                    <Paperclip className="h-3.5 w-3.5 text-[var(--nova-ink-muted)]" />
                  ) : null}
                  {message.localOnly ? (
                    <span className="shrink-0 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--nova-ink-muted)]">
                      {t("localOnlyBadge")}
                    </span>
                  ) : null}
                </div>
                {!indented ? (
                  <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                    {message.snippet}
                  </p>
                ) : null}
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function FilterChip({
  active,
  onClick,
  label,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "rounded-[var(--nova-radius-sm)] border px-2 py-1 text-xs font-medium transition-colors",
        active
          ? "border-[var(--nova-accent)] bg-[var(--nova-accent-soft)] text-[var(--nova-accent)]"
          : "border-[var(--nova-border)] text-[var(--nova-ink-muted)] hover:border-[var(--nova-accent)] hover:text-[var(--nova-ink)]",
      )}
    >
      {label}
    </button>
  );
}

export function buildListRequest(
  filters: InboxFilters,
  searchQuery: string,
): ListMessagesRequest {
  const mailboxSelected = Boolean(filters.mailboxId);
  const roleSelected = Boolean(filters.mailboxRole);
  const offlineSelected = filters.localOnly;
  return {
    mailboxId: offlineSelected ? null : filters.mailboxId,
    accountId: mailboxSelected ? null : filters.accountId,
    unified: !mailboxSelected && !roleSelected && !offlineSelected,
    mailboxRole: offlineSelected ? null : filters.mailboxRole,
    localOnly: offlineSelected,
    limit: 200,
    offset: 0,
    query: searchQuery || null,
    unreadOnly: filters.unreadOnly,
    starredOnly: filters.starredOnly,
    hasAttachments: filters.hasAttachments,
    sortBy: filters.sortBy,
    sortDir: filters.sortDir,
  };
}

export const defaultInboxFilters: InboxFilters = {
  unreadOnly: false,
  starredOnly: false,
  hasAttachments: false,
  accountId: null,
  mailboxId: null,
  mailboxRole: null,
  localOnly: false,
  sortBy: "date",
  sortDir: "desc",
  viewMode: "threads",
};
