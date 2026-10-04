import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ChevronDown,
  ChevronRight,
  ListTree,
  List,
  Mail,
  MailOpen,
  Paperclip,
  Star,
  StarOff,
} from "lucide-react";
import { Button, cn, Select } from "@novamail/ui";

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
  /** Virtual folder: currently snoozed messages */
  snoozedOnly: boolean;
  sortBy: MessageSortBy;
  sortDir: SortDirection;
  viewMode: InboxViewMode;
}

export type BulkFlagAction =
  | "read"
  | "unread"
  | "star"
  | "unstar";

interface MessageListProps {
  messages: MessageSummaryDto[];
  threads: ThreadListItemDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onOpenFocus?: (id: string) => void;
  onToggleStar?: (messageId: string, starred: boolean) => void;
  onBulkFlags?: (messageIds: string[], action: BulkFlagAction) => void | Promise<void>;
  total: number;
  filters: InboxFilters;
  onFiltersChange: (next: InboxFilters) => void;
  accounts: AccountDto[];
  mailboxes: MailboxDto[];
  /** Used by select-all to load every matching mailbox id (not just the page). */
  searchQuery?: string;
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
  onOpenFocus,
  onToggleStar,
  onBulkFlags,
  total,
  filters,
  onFiltersChange,
  accounts,
  mailboxes,
  searchQuery = "",
}: MessageListProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const parentRef = useRef<HTMLDivElement>(null);
  const [expandedThreads, setExpandedThreads] = useState<Set<string>>(new Set());
  const [threadMessages, setThreadMessages] = useState<
    Record<string, MessageSummaryDto[]>
  >({});
  const [loadingThread, setLoadingThread] = useState<string | null>(null);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());
  const [bulkBusy, setBulkBusy] = useState(false);
  const lastClickedId = useRef<string | null>(null);

  useEffect(() => {
    setExpandedThreads(new Set());
    setThreadMessages({});
    setCheckedIds(new Set());
    lastClickedId.current = null;
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

  const selectableMessageIds = useMemo(() => {
    if (filters.viewMode === "flat") {
      return messages.map((m) => m.id);
    }
    const ids: string[] = [];
    for (const thread of threads) {
      const children = threadMessages[thread.id];
      if (children) {
        for (const m of children) ids.push(m.id);
      }
    }
    return ids;
  }, [filters.viewMode, messages, threadMessages, threads]);

  const allVisibleSelected =
    selectableMessageIds.length > 0 &&
    selectableMessageIds.every((id) => checkedIds.has(id));

  const toggleChecked = (id: string, shiftKey: boolean) => {
    setCheckedIds((prev) => {
      const next = new Set(prev);
      if (shiftKey && lastClickedId.current) {
        const order = selectableMessageIds;
        const a = order.indexOf(lastClickedId.current);
        const b = order.indexOf(id);
        if (a >= 0 && b >= 0) {
          const [from, to] = a < b ? [a, b] : [b, a];
          for (let i = from; i <= to; i += 1) {
            const mid = order[i];
            if (mid) next.add(mid);
          }
          lastClickedId.current = id;
          return next;
        }
      }
      if (next.has(id)) next.delete(id);
      else next.add(id);
      lastClickedId.current = id;
      return next;
    });
  };

  const ensureThreadMessages = async (threadId: string) => {
    if (threadMessages[threadId]) return threadMessages[threadId];
    const items = await api.messagesListByThread(threadId);
    setThreadMessages((prev) => ({ ...prev, [threadId]: items }));
    return items;
  };

  const toggleThreadChecked = async (threadId: string) => {
    const items = await ensureThreadMessages(threadId);
    setCheckedIds((prev) => {
      const next = new Set(prev);
      const allSelected =
        items.length > 0 && items.every((m) => next.has(m.id));
      if (allSelected) {
        for (const m of items) next.delete(m.id);
      } else {
        for (const m of items) next.add(m.id);
      }
      return next;
    });
  };

  const selectAllVisible = async () => {
    // Entire mailbox matching current filters — not just the visible page (~200).
    try {
      const request: ListMessagesRequest = {
        ...buildListRequest(filters, searchQuery),
        limit: 20000,
        offset: 0,
      };
      const ids = await api.messagesListIds(request);
      if (ids.length > 0) {
        setCheckedIds(new Set(ids));
        return;
      }
    } catch {
      /* fall back to visible rows */
    }
    if (filters.viewMode === "flat") {
      setCheckedIds(new Set(messages.map((m) => m.id)));
      return;
    }
    const ids = new Set<string>();
    await Promise.all(
      threads.map(async (thread) => {
        const items = await ensureThreadMessages(thread.id);
        for (const m of items) ids.add(m.id);
      }),
    );
    setCheckedIds(ids);
  };

  const clearSelection = () => {
    setCheckedIds(new Set());
    lastClickedId.current = null;
  };

  const runBulk = async (action: BulkFlagAction) => {
    if (!onBulkFlags || checkedIds.size === 0 || bulkBusy) return;
    setBulkBusy(true);
    try {
      await onBulkFlags([...checkedIds], action);
      clearSelection();
    } finally {
      setBulkBusy(false);
    }
  };

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

  const checkedCount = checkedIds.size;

  return (
    <section
      aria-label={t("messageList")}
      className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]"
    >
      <header className="shrink-0 space-y-3 border-b border-[var(--nova-border)] px-4 py-3">
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

        <div className="flex flex-wrap items-center gap-1.5">
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

        {checkedCount > 0 ? (
          <div
            className="flex flex-wrap items-center gap-1.5 rounded-[var(--nova-radius-md)] border border-[var(--nova-accent)]/30 bg-[var(--nova-accent-soft)] px-2.5 py-2"
            role="toolbar"
            aria-label={t("bulkActions")}
          >
            <span className="mr-1 text-xs font-medium text-[var(--nova-accent)]">
              {t("selectedCount", { count: checkedCount })}
            </span>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bulkBusy}
              onClick={() => void runBulk("read")}
            >
              <MailOpen className="h-3.5 w-3.5" />
              {t("markAsRead")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bulkBusy}
              onClick={() => void runBulk("unread")}
            >
              <Mail className="h-3.5 w-3.5" />
              {t("markAsUnread")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bulkBusy}
              onClick={() => void runBulk("star")}
            >
              <Star className="h-3.5 w-3.5" />
              {t("markAsStarred")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bulkBusy}
              onClick={() => void runBulk("unstar")}
            >
              <StarOff className="h-3.5 w-3.5" />
              {t("markAsUnstarred")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bulkBusy}
              onClick={clearSelection}
            >
              {t("clearSelection")}
            </Button>
          </div>
        ) : null}

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
                  {(account.label || account.name).trim() || account.email}
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

      <div className="flex min-h-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center gap-1.5 border-b border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface-2)_55%,transparent)] px-2 py-1.5">
          <label className="flex shrink-0 items-center">
            <input
              type="checkbox"
              className="mt-0"
              checked={allVisibleSelected && checkedCount > 0}
              ref={(el) => {
                if (el) {
                  el.indeterminate =
                    checkedCount > 0 && !allVisibleSelected;
                }
              }}
              aria-label={t("selectAll")}
              title={
                allVisibleSelected && checkedCount > 0
                  ? t("clearSelection")
                  : t("selectAll")
              }
              onChange={() => {
                if (allVisibleSelected && checkedCount > 0) clearSelection();
                else void selectAllVisible();
              }}
            />
          </label>
          <span className="text-[11px] font-medium text-[var(--nova-ink-muted)]">
            {checkedCount > 0
              ? t("selectedCount", { count: checkedCount })
              : t("selectAll")}
          </span>
        </div>

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
              const children = threadMessages[thread.id] ?? [];
              const threadChecked =
                children.length > 0 &&
                children.every((m) => checkedIds.has(m.id));
              const threadPartial =
                !threadChecked &&
                children.some((m) => checkedIds.has(m.id));
              return (
                <div
                  key={`thread-${thread.id}`}
                  className={cn(
                    "absolute left-0 right-0 border-b border-[var(--nova-border)]",
                    unread &&
                      "bg-[color-mix(in_srgb,var(--nova-accent-soft)_28%,transparent)] shadow-[inset_3px_0_0_0_var(--nova-accent),inset_0_0_0_1px_color-mix(in_srgb,var(--nova-accent)_42%,transparent)]",
                  )}
                  style={{
                    height: `${item.size}px`,
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  <div
                    className={cn(
                      "flex h-full w-full items-stretch gap-1 px-2 py-2.5 transition-colors",
                      !unread && "hover:bg-[var(--nova-surface-2)]",
                      (threadChecked || threadPartial) &&
                        "bg-[color-mix(in_srgb,var(--nova-accent-soft)_55%,transparent)]",
                    )}
                  >
                    <label
                      className="flex shrink-0 items-start pt-1"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <input
                        type="checkbox"
                        className="mt-0.5"
                        checked={threadChecked}
                        ref={(el) => {
                          if (el) el.indeterminate = threadPartial;
                        }}
                        aria-label={t("selectThread")}
                        onChange={() => {
                          void toggleThreadChecked(thread.id);
                        }}
                      />
                    </label>
                    <button
                      type="button"
                      onClick={() => {
                        void toggleThread(thread.id);
                      }}
                      className="flex min-w-0 flex-1 flex-col gap-1 text-left"
                    >
                    <div className="flex items-center gap-1.5">
                      {expanded ? (
                        <ChevronDown className="h-3.5 w-3.5 shrink-0 text-[var(--nova-ink-muted)]" />
                      ) : (
                        <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[var(--nova-ink-muted)]" />
                      )}
                      {unread ? (
                        <span
                          aria-hidden
                          className="h-2 w-2 shrink-0 rounded-full bg-[var(--nova-accent)]"
                        />
                      ) : null}
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
            const checked = checkedIds.has(message.id);

            return (
              <div
                key={message.id}
                className={cn(
                  "absolute left-0 right-0 flex border-b border-[var(--nova-border)] transition-colors",
                  indented ? "py-2 pl-10 pr-3" : "px-2 py-3",
                  selected
                    ? "bg-[var(--nova-accent-soft)]"
                    : !message.unread && "hover:bg-[var(--nova-surface-2)]",
                  checked &&
                    "bg-[color-mix(in_srgb,var(--nova-accent-soft)_60%,transparent)]",
                  message.unread &&
                    "bg-[color-mix(in_srgb,var(--nova-accent-soft)_28%,transparent)] shadow-[inset_3px_0_0_0_var(--nova-accent),inset_0_0_0_1px_color-mix(in_srgb,var(--nova-accent)_42%,transparent)]",
                  indented &&
                    !message.unread &&
                    "border-l-[3px] border-l-[var(--nova-accent)]",
                )}
                style={{
                  height: `${item.size}px`,
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <label
                  className="flex shrink-0 items-start pt-1 pr-1.5"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    type="checkbox"
                    className="mt-0.5"
                    checked={checked}
                    aria-label={t("selectMessage")}
                    onChange={(e) => {
                      const shiftKey =
                        (e.nativeEvent as MouseEvent).shiftKey === true;
                      toggleChecked(message.id, shiftKey);
                    }}
                    onClick={(e) => {
                      // Support shift-click range without relying on change event.
                      if (e.shiftKey) {
                        e.preventDefault();
                        toggleChecked(message.id, true);
                      }
                    }}
                  />
                </label>
                <button
                  type="button"
                  onClick={() => onSelect(message.id)}
                  onDoubleClick={() => {
                    onSelect(message.id);
                    onOpenFocus?.(message.id);
                  }}
                  className="flex min-w-0 flex-1 flex-col gap-1 text-left"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span
                      className={cn(
                        "flex min-w-0 items-center gap-1.5 truncate text-sm",
                        message.unread ? "font-semibold" : "font-medium",
                      )}
                    >
                      {indented ? (
                        <span
                          aria-hidden
                          className="shrink-0 font-mono text-[var(--nova-ink-muted)]"
                        >
                          ↳
                        </span>
                      ) : null}
                      {message.unread ? (
                        <span
                          aria-hidden
                          className="h-2 w-2 shrink-0 rounded-full bg-[var(--nova-accent)]"
                        />
                      ) : null}
                      <span className="truncate">
                        {displayName(message.from)}
                      </span>
                    </span>
                    <span className="text-xs text-[var(--nova-ink-muted)]">
                      {formatMessageDate(message.date, locale)}
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
                    {message.snoozedUntil ? (
                      <span className="shrink-0 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--nova-ink-muted)]">
                        {t("snoozedBadge")}
                      </span>
                    ) : null}
                  </div>
                  {!indented ? (
                    <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                      {message.snippet}
                    </p>
                  ) : null}
                </button>
                <button
                  type="button"
                  aria-label={t("starMessage")}
                  aria-pressed={message.starred}
                  className="mt-1 shrink-0 self-start rounded p-0.5 text-[var(--nova-ink-muted)] hover:bg-[var(--nova-surface)] hover:text-[var(--nova-warning)]"
                  onClick={() => onToggleStar?.(message.id, !message.starred)}
                >
                  <Star
                    className={cn(
                      "h-3.5 w-3.5",
                      message.starred &&
                        "fill-[var(--nova-warning)] text-[var(--nova-warning)]",
                    )}
                  />
                </button>
              </div>
            );
          })}
        </div>
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
  const snoozedSelected = filters.snoozedOnly;
  return {
    mailboxId: offlineSelected || snoozedSelected ? null : filters.mailboxId,
    accountId: mailboxSelected ? null : filters.accountId,
    unified:
      !mailboxSelected && !roleSelected && !offlineSelected && !snoozedSelected,
    mailboxRole: offlineSelected || snoozedSelected ? null : filters.mailboxRole,
    localOnly: offlineSelected,
    snoozedOnly: snoozedSelected,
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
  snoozedOnly: false,
  sortBy: "date",
  sortDir: "desc",
  viewMode: "threads",
};
