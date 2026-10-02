import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useKeyboardShortcuts } from "@novamail/hooks";
import {
  CommandPalette,
  EmptyState,
  Input,
  VisuallyHidden,
  type CommandItem,
} from "@novamail/ui";

import { AccountSetup } from "@/features/accounts/AccountSetup";
import { AiSetupDialog } from "@/features/ai/AiSetupDialog";
import { Composer } from "@/features/composer/Composer";
import { ContactsDialog } from "@/features/contacts/ContactsDialog";
import {
  MessageList,
  buildListRequest,
  defaultInboxFilters,
  type BulkFlagAction,
  type InboxFilters,
} from "@/features/mail/MessageList";
import { QuickTriage } from "@/features/mail/QuickTriage";
import { OfflinePromptDialog } from "@/features/mail/OfflinePromptDialog";
import { CalendarPanel } from "@/features/calendar/CalendarPanel";
import { PlannedPanel } from "@/features/mail/PlannedPanel";
import { MessageFocusDialog } from "@/features/mail/MessageFocusDialog";
import { ReadingPane } from "@/features/mail/ReadingPane";
import { Sidebar } from "@/features/mail/Sidebar";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { api, isDesktopShell } from "@/shared/api/client";
import type {
  AccountDto,
  AiSettings,
  AppError,
  ContactPrefill,
  LabelDto,
  MessageDetailDto,
  OfflinePromptEvent,
  PlannedSummaryDto,
  RecipientSuggestion,
  SnoozePreset,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { useUiStore } from "@/shared/store/uiStore";
import { nextThemeMode, type ThemeMode } from "@/shared/theme/resolveTheme";

export function AppShell() {
  const t = useT();
  const queryClient = useQueryClient();
  const {
    selectedMessageId,
    selectMessage,
    composerOpen,
    setComposerOpen,
    accountSetupOpen,
    setAccountSetupOpen,
    settingsOpen,
    setSettingsOpen,
    contactsOpen,
    setContactsOpen,
    triageOpen,
    setTriageOpen,
    commandPaletteOpen,
    setCommandPaletteOpen,
    searchQuery,
    setSearchQuery,
    syncStatus,
    setSyncStatus,
    theme,
    setTheme,
    density,
    inboxViewMode,
    setInboxViewMode,
  } = useUiStore();

  const [replyTo, setReplyTo] = useState<MessageDetailDto | null>(null);
  const [editingDraft, setEditingDraft] = useState<MessageDetailDto | null>(
    null,
  );
  const [composerBody, setComposerBody] = useState("");
  const [composerSubject, setComposerSubject] = useState<string | undefined>();
  const [composerTo, setComposerTo] = useState<string | undefined>();
  const [contactPrefill, setContactPrefill] = useState<ContactPrefill | null>(
    null,
  );
  const contactQueueRef = useRef<ContactPrefill[]>([]);
  const [messageFocusOpen, setMessageFocusOpen] = useState(false);
  const [paletteLabels, setPaletteLabels] = useState<LabelDto[]>([]);
  const [palettePeople, setPalettePeople] = useState<RecipientSuggestion[]>([]);
  const clearContactPrefill = useCallback(() => setContactPrefill(null), []);
  const openContactPrefills = useCallback(
    (prefill: ContactPrefill | ContactPrefill[]) => {
      const list = (Array.isArray(prefill) ? prefill : [prefill]).filter(
        (item) => (item.emails?.length ?? 0) > 0 || Boolean(item.displayName),
      );
      if (list.length === 0) return;
      contactQueueRef.current = list.slice(1);
      setContactPrefill(list[0] ?? null);
      setContactsOpen(true);
    },
    [setContactsOpen],
  );
  const advanceContactQueue = useCallback(() => {
    const queue = contactQueueRef.current;
    if (queue.length === 0) return;
    const [next, ...rest] = queue;
    contactQueueRef.current = rest;
    window.setTimeout(() => {
      setContactPrefill(next ?? null);
    }, 0);
  }, []);
  const [inboxFilters, setInboxFilters] = useState<InboxFilters>(() => ({
    ...defaultInboxFilters,
    viewMode: useUiStore.getState().inboxViewMode,
  }));
  const [offlinePrompt, setOfflinePrompt] =
    useState<OfflinePromptEvent | null>(null);
  const [plannedOpen, setPlannedOpen] = useState(false);
  const [calendarOpen, setCalendarOpen] = useState(false);
  const [plannedSummary, setPlannedSummary] =
    useState<PlannedSummaryDto | null>(null);
  const desktop = isDesktopShell();
  const locale = useUiStore((s) => s.locale);
  const spellcheckLang = useUiStore((s) => s.spellcheckLang);
  const autoCheckUpdates = useUiStore((s) => s.autoCheckUpdates);

  useEffect(() => {
    if (!desktop) return;
    void api.spellcheckEnsureForLocale(locale).catch(() => undefined);
  }, [desktop, locale]);

  useEffect(() => {
    if (!desktop || !autoCheckUpdates) return;
    let cancelled = false;
    const run = () => {
      if (cancelled) return;
      void api.updatesCheck().catch(() => undefined);
    };
    const start = window.setTimeout(run, 10_000);
    const interval = window.setInterval(run, 4 * 60 * 60 * 1000);
    return () => {
      cancelled = true;
      window.clearTimeout(start);
      window.clearInterval(interval);
    };
  }, [desktop, autoCheckUpdates]);

  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    enabled: desktop,
    queryFn: () => api.accountsList(),
  });

  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", "all"],
    enabled: desktop && (accountsQuery.data?.length ?? 0) > 0,
    queryFn: () => api.mailboxesList(null),
  });

  const listRequest = useMemo(
    () => buildListRequest(inboxFilters, searchQuery),
    [inboxFilters, searchQuery],
  );

  const messagesQuery = useQuery({
    queryKey: ["messages", "flat", listRequest],
    enabled:
      desktop &&
      (accountsQuery.data?.length ?? 0) > 0 &&
      (inboxFilters.viewMode === "flat" || triageOpen),
    queryFn: () => api.messagesList(listRequest),
  });

  const threadsQuery = useQuery({
    queryKey: ["messages", "threads", listRequest],
    enabled:
      desktop &&
      (accountsQuery.data?.length ?? 0) > 0 &&
      inboxFilters.viewMode === "threads",
    queryFn: () => api.threadsList(listRequest),
  });

  const messageQuery = useQuery({
    queryKey: ["message", selectedMessageId],
    enabled: desktop && Boolean(selectedMessageId),
    queryFn: () => api.messagesGet(selectedMessageId!),
  });

  const messages = messagesQuery.data?.messages ?? [];
  const threads = threadsQuery.data?.threads ?? [];
  const accounts = accountsQuery.data ?? [];
  const mailboxes = mailboxesQuery.data ?? [];

  const aiSettingsQuery = useQuery({
    queryKey: ["ai-settings"],
    enabled: desktop,
    queryFn: () => api.aiGetSettings(),
  });

  // Debounce list refreshes from background sync so large mailboxes don't thrash.
  // New-mail events use a shorter delay so the inbox feels live.
  const scheduleMailRefresh = useCallback((delayMs = 400) => {
    const w = window as Window & { __novaMailRefreshTimer?: number };
    if (w.__novaMailRefreshTimer != null) {
      window.clearTimeout(w.__novaMailRefreshTimer);
    }
    w.__novaMailRefreshTimer = window.setTimeout(() => {
      w.__novaMailRefreshTimer = undefined;
      void queryClient.invalidateQueries({ queryKey: ["messages"] });
      void queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
      void queryClient.invalidateQueries({ queryKey: ["threads"] });
    }, delayMs);
  }, [queryClient]);

  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    api
      .onSyncProgress((event) => {
        if (event.error) {
          setSyncStatus(`Sync-Fehler in ${event.mailboxName}: ${event.error}`);
          return;
        }
        setSyncStatus(
          event.done
            ? `${event.mailboxName} synchronisiert`
            : `Synchronisiere ${event.mailboxName}… ${event.fetched}`,
        );
        if (event.done) {
          scheduleMailRefresh();
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop, scheduleMailRefresh, setSyncStatus]);

  // Refresh list + sidebar badges as soon as background sync finds new mail.
  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    api
      .onMailNew(() => {
        scheduleMailRefresh(150);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop, scheduleMailRefresh]);

  // Every scheduled IMAP cycle — keep inbox live even when no "new UID" event fired.
  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    api
      .onSyncCycle(() => {
        scheduleMailRefresh(600);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop, scheduleMailRefresh]);

  // Keep WebKit spellcheck language in sync with settings (active language only).
  useEffect(() => {
    if (!desktop) return;
    const code = spellcheckLang || "de_DE";
    const short = code.split("_")[0] ?? code;
    const languages =
      short === "en" ? [code, short] : [code, short];
    void api.spellcheckSetLanguages(languages).catch(() => undefined);
  }, [desktop, spellcheckLang]);

  // Persist UI locale for GTK/WebKit context-menu translations (applies on next launch).
  useEffect(() => {
    if (!desktop) return;
    void api.shellSetUiLocale(locale).catch(() => undefined);
  }, [desktop, locale]);

  // Auto-preselect may leave the first mail unread (blue dot). The first *user*
  // click on that preview — or fullscreen — clears it. Do not require a second
  // click when the detail query is still loading (that was the remaining bug).
  const autoPreviewIdRef = useRef<string | null>(null);

  const markMessageReadLocally = useCallback(
    (id: string, mailboxId?: string | null) => {
      queryClient.setQueryData(
        ["message", id],
        (old: MessageDetailDto | undefined) =>
          old
            ? { ...old, summary: { ...old.summary, unread: false } }
            : old,
      );
      queryClient.setQueriesData(
        { queryKey: ["messages", "flat"] },
        (old: unknown) => {
          if (!old || typeof old !== "object") return old;
          const data = old as {
            messages?: Array<{ id: string; unread: boolean }>;
          };
          if (!Array.isArray(data.messages)) return old;
          return {
            ...data,
            messages: data.messages.map((m) =>
              m.id === id ? { ...m, unread: false } : m,
            ),
          };
        },
      );
      if (mailboxId) {
        queryClient.setQueryData(
          ["mailboxes", "all"],
          (old: Array<{ id: string; unreadCount: number }> | undefined) => {
            if (!old) return old;
            return old.map((m) =>
              m.id === mailboxId
                ? { ...m, unreadCount: Math.max(0, (m.unreadCount ?? 0) - 1) }
                : m,
            );
          },
        );
      }
      void api
        .messagesSetFlags({ messageId: id, unread: false })
        .then(() => {
          void queryClient.invalidateQueries({ queryKey: ["messages", "threads"] });
        })
        .catch(() => {
          void queryClient.invalidateQueries({ queryKey: ["message", id] });
          void queryClient.invalidateQueries({ queryKey: ["messages"] });
          void queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
        });
    },
    [queryClient],
  );

  const mailboxIdFor = useCallback(
    (id: string) => {
      if (messageQuery.data?.summary.id === id) {
        return messageQuery.data.summary.mailboxId;
      }
      return messages.find((m) => m.id === id)?.mailboxId ?? null;
    },
    [messageQuery.data, messages],
  );

  const handleSelectMessage = useCallback(
    (id: string | null) => {
      if (id) {
        // Any user-driven select (incl. first click on the auto-previewed
        // top mail) clears unread. Auto-preselect uses selectMessage()
        // directly so the blue dot can stay until that click.
        markMessageReadLocally(id, mailboxIdFor(id));
        autoPreviewIdRef.current = null;
      }
      selectMessage(id);
    },
    [mailboxIdFor, markMessageReadLocally, selectMessage],
  );

  const openFocusForMessage = useCallback(
    (id: string) => {
      autoPreviewIdRef.current = null;
      selectMessage(id);
      // Fullscreen counts as "opened for real" → clear unread blue dot.
      markMessageReadLocally(id, mailboxIdFor(id));
      setMessageFocusOpen(true);
    },
    [mailboxIdFor, markMessageReadLocally, selectMessage],
  );

  // Preselect first mail so preview shows with unread dot still visible.
  useEffect(() => {
    if (!desktop || selectedMessageId) return;
    if (inboxFilters.viewMode === "flat" && messages[0]?.id) {
      autoPreviewIdRef.current = messages[0].id;
      selectMessage(messages[0].id);
      return;
    }
    if (inboxFilters.viewMode === "threads" && threads[0]?.id) {
      let cancelled = false;
      void api
        .messagesListByThread(threads[0].id)
        .then((items) => {
          if (cancelled || !items[0]?.id || selectedMessageId) return;
          autoPreviewIdRef.current = items[0].id;
          selectMessage(items[0].id);
        })
        .catch(() => undefined);
      return () => {
        cancelled = true;
      };
    }
  }, [
    desktop,
    inboxFilters.viewMode,
    messages,
    selectMessage,
    selectedMessageId,
    threads,
  ]);

  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    api
      .onOfflinePrompt((event) => {
        setOfflinePrompt(event);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop]);

  const refreshPlannedSummary = useCallback(() => {
    if (!desktop) return;
    void api
      .plannedSummary()
      .then(setPlannedSummary)
      .catch(() => setPlannedSummary(null));
  }, [desktop]);

  useEffect(() => {
    refreshPlannedSummary();
  }, [refreshPlannedSummary]);

  useEffect(() => {
    if (!desktop || !commandPaletteOpen) return;
    void api
      .labelsList(inboxFilters.accountId)
      .then(setPaletteLabels)
      .catch(() => setPaletteLabels([]));
    void api
      .recipientsSuggest("", 40)
      .then(setPalettePeople)
      .catch(() => setPalettePeople([]));
  }, [commandPaletteOpen, desktop, inboxFilters.accountId]);

  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    api
      .onJobsTick((report) => {
        if (report.wokeSnoozes > 0 || report.sentLater > 0) {
          void queryClient.invalidateQueries({ queryKey: ["messages"] });
          refreshPlannedSummary();
          if (report.wokeSnoozes > 0) {
            setSyncStatus(t("snoozeWoke", { count: report.wokeSnoozes }));
          } else if (report.sentLater > 0) {
            setSyncStatus(t("sendLaterSent", { count: report.sentLater }));
          }
        }
        if ((report.calendarReminders ?? 0) > 0) {
          setSyncStatus(
            t("calendarReminderDue", { count: report.calendarReminders ?? 0 }),
          );
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop, queryClient, refreshPlannedSummary, setSyncStatus, t]);

  const refresh = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["accounts"] }),
      queryClient.invalidateQueries({ queryKey: ["mailboxes"] }),
      queryClient.invalidateQueries({ queryKey: ["messages"] }),
    ]);
  }, [queryClient]);

  const handleSync = useCallback(async () => {
    if (!desktop) {
      setSyncStatus(t("syncStartHint"));
      return;
    }
    setSyncStatus(t("syncStarting"));
    try {
      const results = await api.mailSync(null);
      const total = results.reduce((sum, item) => sum + item.messagesFetched, 0);
      setSyncStatus(t("syncedMessages", { count: total }));
      await refresh();
    } catch (error) {
      setSyncStatus((error as AppError).message);
    }
  }, [desktop, refresh, setSyncStatus, t]);

  const handleToggleStar = useCallback(
    async (messageId?: string, starred?: boolean) => {
      if (!desktop) return;
      const id = messageId ?? messageQuery.data?.summary.id;
      if (!id) return;
      const nextStarred =
        starred ??
        (messageQuery.data?.summary.id === id
          ? !messageQuery.data.summary.starred
          : true);
      await api.messagesSetFlags({
        messageId: id,
        starred: nextStarred,
      });
      await queryClient.invalidateQueries({
        queryKey: ["message", id],
      });
      await queryClient.invalidateQueries({ queryKey: ["messages"] });
      await queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
    },
    [desktop, messageQuery.data, queryClient],
  );

  const handleBulkFlags = useCallback(
    async (messageIds: string[], action: BulkFlagAction) => {
      if (!desktop || messageIds.length === 0) return;
      const flags =
        action === "read"
          ? { unread: false as boolean | undefined, starred: undefined as boolean | undefined }
          : action === "unread"
            ? { unread: true as boolean | undefined, starred: undefined as boolean | undefined }
            : action === "star"
              ? { unread: undefined as boolean | undefined, starred: true as boolean | undefined }
              : { unread: undefined as boolean | undefined, starred: false as boolean | undefined };
      // One batched IMAP STORE per mailbox — Yahoo-friendly.
      await api.messagesSetFlagsMany(messageIds, {
        unread: flags.unread,
        starred: flags.starred,
      });
      await queryClient.invalidateQueries({ queryKey: ["messages"] });
      await queryClient.invalidateQueries({ queryKey: ["threads"] });
      await queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
      if (selectedMessageId && messageIds.includes(selectedMessageId)) {
        await queryClient.invalidateQueries({
          queryKey: ["message", selectedMessageId],
        });
      }
      setSyncStatus(
        action === "read"
          ? t("bulkMarkedRead", { count: messageIds.length })
          : action === "unread"
            ? t("bulkMarkedUnread", { count: messageIds.length })
            : action === "star"
              ? t("bulkMarkedStarred", { count: messageIds.length })
              : t("bulkMarkedUnstarred", { count: messageIds.length }),
      );
    },
    [desktop, queryClient, selectedMessageId, setSyncStatus, t],
  );

  const handleArchive = useCallback(async () => {
    if (!selectedMessageId || !desktop) return;
    await api.messagesArchive(selectedMessageId);
    selectMessage(null);
    await refresh();
  }, [desktop, refresh, selectMessage, selectedMessageId]);

  const handleDelete = useCallback(async () => {
    if (!selectedMessageId || !desktop) return;
    await api.messagesDelete(selectedMessageId);
    selectMessage(null);
    await refresh();
  }, [desktop, refresh, selectMessage, selectedMessageId]);

  const handleForward = useCallback(async () => {
    if (!selectedMessageId || !desktop) return;
    const draft = await api.messagesForwardDraft(selectedMessageId);
    setReplyTo(null);
    setEditingDraft(null);
    setComposerSubject(draft.subject);
    setComposerBody(draft.bodyText);
    setComposerOpen(true);
  }, [desktop, selectedMessageId, setComposerOpen]);

  const listTotal =
    inboxFilters.viewMode === "threads"
      ? (threadsQuery.data?.total ?? threads.length)
      : (messagesQuery.data?.total ?? messages.length);
  const selectedMailboxId =
    inboxFilters.mailboxId ?? messageQuery.data?.summary.mailboxId ?? null;
  const selectedMailboxRole = selectedMailboxId
    ? (mailboxes.find((m) => m.id === selectedMailboxId)?.role ?? null)
    : inboxFilters.mailboxRole;
  const inSentOrDrafts =
    selectedMailboxRole === "sent" ||
    selectedMailboxRole === "drafts" ||
    inboxFilters.mailboxRole === "drafts";
  const aiReplyEnabled =
    Boolean(aiSettingsQuery.data?.enabled) && !inSentOrDrafts;

  const navigateList = useCallback(
    (delta: number) => {
      if (messages.length === 0) return;
      const index = messages.findIndex((m) => m.id === selectedMessageId);
      const next = index < 0 ? 0 : Math.min(Math.max(index + delta, 0), messages.length - 1);
      handleSelectMessage(messages[next].id);
    },
    [handleSelectMessage, messages, selectedMessageId],
  );

  const handleSelectAccountFilter = useCallback(
    (accountId: string | null) => {
      setPlannedOpen(false);
      setCalendarOpen(false);
      setInboxFilters((prev) => ({
        ...prev,
        accountId,
        mailboxId: null,
        mailboxRole: null,
        localOnly: false,
        snoozedOnly: false,
        viewMode: inboxViewMode,
      }));
    },
    [inboxViewMode],
  );

  const handleSelectMailbox = useCallback(
    (accountId: string, mailboxId: string) => {
      selectMessage(null);
      setPlannedOpen(false);
      setCalendarOpen(false);
      setInboxFilters((prev) => ({
        ...prev,
        accountId,
        mailboxId,
        mailboxRole: null,
        localOnly: false,
        snoozedOnly: false,
        viewMode: inboxViewMode,
      }));
    },
    [inboxViewMode, selectMessage],
  );

  const handleSelectDrafts = useCallback(() => {
    selectMessage(null);
    setPlannedOpen(false);
    setCalendarOpen(false);
    setInboxFilters((prev) => ({
      ...prev,
      mailboxId: null,
      mailboxRole: "drafts",
      localOnly: false,
      snoozedOnly: false,
      viewMode: "flat",
    }));
  }, [selectMessage]);

  const handleSelectSpam = useCallback(() => {
    selectMessage(null);
    setPlannedOpen(false);
    setCalendarOpen(false);
    setInboxFilters((prev) => ({
      ...prev,
      mailboxId: null,
      mailboxRole: "junk",
      localOnly: false,
      snoozedOnly: false,
      viewMode: "flat",
    }));
  }, [selectMessage]);

  const handleSelectOffline = useCallback(() => {
    selectMessage(null);
    setPlannedOpen(false);
    setCalendarOpen(false);
    setInboxFilters((prev) => ({
      ...prev,
      mailboxId: null,
      mailboxRole: null,
      localOnly: true,
      snoozedOnly: false,
      viewMode: "flat",
    }));
  }, [selectMessage]);

  const handleSelectPlanned = useCallback(() => {
    selectMessage(null);
    setCalendarOpen(false);
    setPlannedOpen(true);
    setInboxFilters((prev) => ({
      ...prev,
      mailboxId: null,
      mailboxRole: null,
      localOnly: false,
      snoozedOnly: true,
      viewMode: "flat",
    }));
    refreshPlannedSummary();
  }, [refreshPlannedSummary, selectMessage]);

  const handleInboxFiltersChange = useCallback(
    (next: InboxFilters) => {
      setInboxFilters(next);
      // Persist user preference only when choosing flat/threads in normal mail.
      if (
        !next.mailboxRole &&
        !next.localOnly &&
        !next.snoozedOnly &&
        next.viewMode !== inboxViewMode
      ) {
        setInboxViewMode(next.viewMode);
      }
    },
    [inboxViewMode, setInboxViewMode],
  );

  const handleSelectCalendar = useCallback(() => {
    selectMessage(null);
    setPlannedOpen(false);
    setCalendarOpen(true);
    setInboxFilters((prev) => ({
      ...prev,
      mailboxId: null,
      mailboxRole: null,
      localOnly: false,
      snoozedOnly: false,
    }));
  }, [selectMessage]);

  const refreshMailQueries = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ["messages"] });
    void queryClient.invalidateQueries({ queryKey: ["message"] });
    void queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
  }, [queryClient]);

  const handleSnooze = useCallback(
    async (preset: SnoozePreset) => {
      if (!selectedMessageId || !desktop) return;
      await api.messagesSnooze({ messageId: selectedMessageId, preset });
      selectMessage(null);
      refreshMailQueries();
      refreshPlannedSummary();
      setSyncStatus(t("snoozeDone"));
    },
    [
      desktop,
      refreshMailQueries,
      refreshPlannedSummary,
      selectMessage,
      selectedMessageId,
      setSyncStatus,
      t,
    ],
  );

  const handleMarkSpam = useCallback(async () => {
    if (!selectedMessageId) return;
    await api.messagesMarkSpam(selectedMessageId);
    selectMessage(null);
    refreshMailQueries();
  }, [refreshMailQueries, selectMessage, selectedMessageId]);

  const handleMarkNotSpam = useCallback(async () => {
    if (!selectedMessageId) return;
    await api.messagesMarkNotSpam(selectedMessageId);
    selectMessage(null);
    refreshMailQueries();
  }, [refreshMailQueries, selectMessage, selectedMessageId]);

  const openDraftInComposer = useCallback(
    async (messageId: string) => {
      if (!desktop) return;
      const detail = await api.messagesGet(messageId);
      setReplyTo(null);
      setEditingDraft(detail);
      setComposerBody("");
      setComposerSubject(undefined);
      setComposerOpen(true);
    },
    [desktop, setComposerOpen],
  );

  const cycleTheme = useCallback(() => {
    setTheme(nextThemeMode(theme as ThemeMode));
  }, [setTheme, theme]);

  const openComposer = useCallback(
    (opts?: { to?: string }) => {
      setReplyTo(null);
      setEditingDraft(null);
      setComposerBody("");
      setComposerSubject(undefined);
      setComposerTo(opts?.to);
      setComposerOpen(true);
    },
    [setComposerOpen],
  );

  const shortcuts = useMemo(
    () => ({
      c: () => openComposer(),
      r: () => {
        if (messageQuery.data) {
          setReplyTo(messageQuery.data);
          setEditingDraft(null);
          setComposerBody("");
          setComposerSubject(undefined);
          setComposerTo(undefined);
          setComposerOpen(true);
        }
      },
      f: () => {
        void handleForward();
      },
      e: () => {
        void handleArchive();
      },
      backspace: () => {
        void handleDelete();
      },
      delete: () => {
        void handleDelete();
      },
      t: () => setTriageOpen(true),
      j: () => navigateList(1),
      k: () => navigateList(-1),
      "/": () => {
        document.getElementById("global-search")?.focus();
      },
      "mod+k": () => setCommandPaletteOpen(true),
      ",": () => setSettingsOpen(true),
      h: () => {
        void handleSnooze("laterToday");
      },
    }),
    [
      handleArchive,
      handleDelete,
      handleForward,
      handleSnooze,
      messageQuery.data,
      navigateList,
      openComposer,
      setCommandPaletteOpen,
      setComposerOpen,
      setSettingsOpen,
      setTriageOpen,
    ],
  );
  useKeyboardShortcuts(triageOpen ? {} : shortcuts);

  const commandItems = useMemo(() => {
    const items: CommandItem[] = [
      {
        id: "compose",
        label: t("cmdCompose"),
        hint: "C",
        group: t("cmdGroupNavigate"),
        onSelect: () => openComposer(),
      },
      {
        id: "triage",
        label: t("cmdTriage"),
        hint: "T",
        group: t("cmdGroupNavigate"),
        onSelect: () => setTriageOpen(true),
      },
      {
        id: "drafts",
        label: t("drafts"),
        group: t("cmdGroupNavigate"),
        keywords: "entwürfe drafts",
        onSelect: () => handleSelectDrafts(),
      },
      {
        id: "planned",
        label: t("cmdPlanned"),
        group: t("cmdGroupNavigate"),
        keywords: "snooze geplant planned later",
        onSelect: () => handleSelectPlanned(),
      },
      {
        id: "offline",
        label: t("cmdOffline"),
        group: t("cmdGroupNavigate"),
        keywords: "offline local only",
        onSelect: () => handleSelectOffline(),
      },
      {
        id: "calendar",
        label: t("calendar"),
        group: t("cmdGroupNavigate"),
        keywords: "calendar kalender tasks aufgaben",
        onSelect: () => handleSelectCalendar(),
      },
      {
        id: "contacts",
        label: t("cmdContacts"),
        group: t("cmdGroupNavigate"),
        onSelect: () => setContactsOpen(true),
      },
      {
        id: "sync",
        label: t("cmdSync"),
        group: t("cmdGroupNavigate"),
        onSelect: () => {
          void handleSync();
        },
      },
      {
        id: "add-account",
        label: t("cmdAddAccount"),
        group: t("cmdGroupNavigate"),
        onSelect: () => setAccountSetupOpen(true),
      },
      {
        id: "settings",
        label: t("cmdSettings"),
        hint: ",",
        group: t("cmdGroupNavigate"),
        onSelect: () => setSettingsOpen(true),
      },
      {
        id: "theme",
        label:
          theme === "light"
            ? t("switchToDark")
            : theme === "dark"
              ? t("switchToAuto")
              : t("switchToLight"),
        group: t("cmdGroupNavigate"),
        onSelect: () => cycleTheme(),
      },
      {
        id: "archive",
        label: t("cmdArchive"),
        hint: "E",
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleArchive();
        },
      },
      {
        id: "delete",
        label: t("cmdDelete"),
        hint: "⌫",
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleDelete();
        },
      },
      {
        id: "forward",
        label: t("cmdForward"),
        hint: "F",
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleForward();
        },
      },
      {
        id: "star",
        label: t("cmdStar"),
        group: t("cmdGroupActions"),
        keywords: "favorite favourit star",
        onSelect: () => {
          void handleToggleStar();
        },
      },
      {
        id: "mark-spam",
        label: t("cmdMarkSpam"),
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleMarkSpam();
        },
      },
      {
        id: "snooze-later-today",
        label: t("cmdSnoozeLaterToday"),
        hint: "H",
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleSnooze("laterToday");
        },
      },
      {
        id: "snooze-tomorrow",
        label: t("cmdSnoozeTomorrow"),
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleSnooze("tomorrowMorning");
        },
      },
      {
        id: "snooze-monday",
        label: t("cmdSnoozeMonday"),
        group: t("cmdGroupActions"),
        onSelect: () => {
          void handleSnooze("nextMonday");
        },
      },
      {
        id: "filter-unread",
        label: t("cmdFilterUnread"),
        group: t("cmdGroupFilters"),
        onSelect: () => {
          setPlannedOpen(false);
          setInboxFilters((prev) => ({
            ...prev,
            unreadOnly: true,
            starredOnly: false,
            localOnly: false,
            snoozedOnly: false,
            mailboxRole: null,
          }));
        },
      },
      {
        id: "filter-starred",
        label: t("cmdFilterStarred"),
        group: t("cmdGroupFilters"),
        onSelect: () => {
          setPlannedOpen(false);
          setInboxFilters((prev) => ({
            ...prev,
            starredOnly: true,
            unreadOnly: false,
            localOnly: false,
            snoozedOnly: false,
            mailboxRole: null,
          }));
        },
      },
      {
        id: "filter-clear",
        label: t("cmdFilterClear"),
        group: t("cmdGroupFilters"),
        onSelect: () => {
          setPlannedOpen(false);
          setInboxFilters(defaultInboxFilters);
        },
      },
    ];

    for (const account of accounts) {
      const sideLabel =
        (account.label || account.name).trim() || account.email;
      items.push({
        id: `account-${account.id}`,
        label: t("cmdAccount", { name: sideLabel }),
        group: t("cmdGroupFilters"),
        keywords: `${sideLabel} ${account.name} ${account.email}`,
        onSelect: () => handleSelectAccountFilter(account.id),
      });
    }

    if (selectedMessageId) {
      for (const label of paletteLabels) {
        items.push({
          id: `label-${label.id}`,
          label: t("cmdLabel", { name: label.name }),
          group: t("cmdGroupActions"),
          keywords: `label ${label.name}`,
          onSelect: () => {
            void api
              .messagesListLabels(selectedMessageId)
              .then((current) => {
                const ids = new Set(current.map((l) => l.id));
                if (ids.has(label.id)) ids.delete(label.id);
                else ids.add(label.id);
                return api.messagesSetLabels({
                  messageId: selectedMessageId,
                  labelIds: [...ids],
                });
              })
              .then(() => refreshMailQueries())
              .catch((err) => setSyncStatus((err as AppError).message));
          },
        });
      }
    }

    for (const person of palettePeople) {
      const name = person.name?.trim() || person.email;
      const to = person.name?.trim()
        ? `${person.name.trim()} <${person.email}>`
        : person.email;
      items.push({
        id: `person-${person.source}-${person.email}`,
        label: t("cmdComposeTo", { name }),
        group: t("cmdGroupPeople"),
        keywords: `${person.email} ${person.name ?? ""} contact recipient`,
        hint: person.inContacts ? t("contacts") : person.source,
        onSelect: () => openComposer({ to }),
      });
    }

    return items;
  }, [
    accounts,
    cycleTheme,
    handleArchive,
    handleDelete,
    handleForward,
    handleMarkSpam,
    handleSelectAccountFilter,
    handleSelectCalendar,
    handleSelectDrafts,
    handleSelectOffline,
    handleSelectPlanned,
    handleSnooze,
    handleSync,
    handleToggleStar,
    openComposer,
    paletteLabels,
    palettePeople,
    refreshMailQueries,
    selectedMessageId,
    setAccountSetupOpen,
    setContactsOpen,
    setSettingsOpen,
    setSyncStatus,
    setTriageOpen,
    t,
    theme,
  ]);

  if (!desktop) {
    return (
      <EmptyState
        title={t("shellOnlyTitle")}
        description={t("shellOnlyDescription")}
      />
    );
  }

  return (
    <div
      className="flex h-full flex-col"
      data-density={density}
    >
      <VisuallyHidden>
        <h1>{t("unifiedInbox")}</h1>
      </VisuallyHidden>
      <div className="flex items-center gap-3 border-b border-[var(--nova-border)] px-4 py-3">
        <label className="sr-only" htmlFor="global-search">
          {t("searchPlaceholder")}
        </label>
        <Input
          id="global-search"
          placeholder={t("searchPlaceholder")}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="max-w-xl"
        />
        <button
          type="button"
          className="text-xs text-[var(--nova-ink-muted)]"
          onClick={() => setCommandPaletteOpen(true)}
        >
          {t("commandHint")}
        </button>
      </div>

      <div className="flex min-h-0 flex-1">
        <Sidebar
          accounts={accounts}
          mailboxes={mailboxes}
          selectedAccountId={inboxFilters.accountId}
          selectedMailboxId={inboxFilters.mailboxId}
          draftsSelected={inboxFilters.mailboxRole === "drafts"}
          spamSelected={inboxFilters.mailboxRole === "junk"}
          offlineSelected={inboxFilters.localOnly}
          plannedSelected={plannedOpen}
          plannedCount={
            (plannedSummary?.snoozedCount ?? 0) +
            (plannedSummary?.outboundPendingCount ?? 0)
          }
          calendarSelected={calendarOpen}
          syncStatus={syncStatus}
          themeMode={theme}
          onSelectUnified={() => handleSelectAccountFilter(null)}
          onSelectAccount={handleSelectAccountFilter}
          onSelectMailbox={handleSelectMailbox}
          onSelectDrafts={handleSelectDrafts}
          onSelectSpam={handleSelectSpam}
          onSelectOffline={handleSelectOffline}
          onSelectPlanned={handleSelectPlanned}
          onSelectCalendar={handleSelectCalendar}
          onCompose={() => openComposer()}
          onSync={handleSync}
          onAddAccount={() => setAccountSetupOpen(true)}
          onToggleTheme={cycleTheme}
          onOpenSettings={() => setSettingsOpen(true)}
          onOpenContacts={() => setContactsOpen(true)}
          onOpenTriage={() => setTriageOpen(true)}
        />

        {calendarOpen ? (
          <div className="min-w-0 flex-1 bg-[color-mix(in_srgb,var(--nova-surface)_92%,transparent)]">
            <CalendarPanel />
          </div>
        ) : plannedOpen ? (
          <div className="min-w-0 flex-1 bg-[color-mix(in_srgb,var(--nova-surface)_92%,transparent)]">
            <PlannedPanel
              onOpenMessage={(id) => {
                setPlannedOpen(false);
                setInboxFilters((prev) => ({
                  ...prev,
                  snoozedOnly: false,
                }));
                handleSelectMessage(id);
              }}
              onChanged={() => {
                refreshMailQueries();
                refreshPlannedSummary();
              }}
            />
          </div>
        ) : accounts.length === 0 ? (
          <div className="flex min-h-0 min-w-0 flex-1 items-center justify-center">
            <EmptyState
              className="w-full"
              title={t("welcomeTitle")}
              description={t("welcomeDescription")}
              action={
                <button
                  type="button"
                  className="mt-2 text-[var(--nova-accent)]"
                  onClick={() => setAccountSetupOpen(true)}
                >
                  {t("addAccount")}
                </button>
              }
            />
          </div>
        ) : (
          <>
            <div className="w-[380px] shrink-0">
              <MessageList
                messages={messages}
                threads={threads}
                selectedId={selectedMessageId}
                onSelect={(id) => {
                  handleSelectMessage(id);
                  if (inboxFilters.mailboxRole === "drafts") {
                    void openDraftInComposer(id);
                  }
                }}
                onOpenFocus={(id) => {
                  openFocusForMessage(id);
                }}
                onToggleStar={(messageId, starred) => {
                  void handleToggleStar(messageId, starred);
                }}
                onBulkFlags={(ids, action) => {
                  void handleBulkFlags(ids, action);
                }}
                total={listTotal}
                filters={inboxFilters}
                onFiltersChange={handleInboxFiltersChange}
                accounts={accounts}
                mailboxes={mailboxes}
                searchQuery={searchQuery}
              />
            </div>
            <div className="min-w-0 flex-1 bg-[color-mix(in_srgb,var(--nova-surface)_92%,transparent)]">
              <ReadingPane
                message={messageQuery.data}
                aiEnabled={aiReplyEnabled}
                inSpamFolder={inboxFilters.mailboxRole === "junk"}
                onOpenFocus={() => {
                  if (selectedMessageId) openFocusForMessage(selectedMessageId);
                  else setMessageFocusOpen(true);
                }}
                onToggleFocus={() => {
                  if (messageFocusOpen) {
                    setMessageFocusOpen(false);
                  } else if (selectedMessageId) {
                    openFocusForMessage(selectedMessageId);
                  }
                }}
                onDelete={() => {
                  void handleDelete();
                }}
                onArchive={() => {
                  void handleArchive();
                }}
                onReply={() => {
                  setMessageFocusOpen(false);
                  setComposerBody("");
                  if (messageQuery.data) {
                    setReplyTo(messageQuery.data);
                    setEditingDraft(null);
                    setComposerSubject(undefined);
                    setComposerOpen(true);
                  }
                }}
                onForward={() => {
                  setMessageFocusOpen(false);
                  void handleForward();
                }}
                onToggleStar={() => {
                  void handleToggleStar();
                }}
                onMarkSpam={() => {
                  void handleMarkSpam();
                }}
                onMarkNotSpam={() => {
                  void handleMarkNotSpam();
                }}
                onSnooze={(preset) => {
                  void handleSnooze(preset);
                }}
                onCreateEvent={(suggestion) => {
                  if (!selectedMessageId) return;
                  const startsAt =
                    suggestion?.startsAt ??
                    Math.floor(Date.now() / 1000) + 3600;
                  const endsAt =
                    suggestion?.endsAt ?? startsAt + 3600;
                  void api
                    .calendarEventFromMessage(
                      selectedMessageId,
                      startsAt,
                      endsAt,
                    )
                    .then(async (created) => {
                      if (suggestion?.location) {
                        await api.calendarEventsUpsert({
                          id: created.id,
                          collectionId: created.collectionId,
                          calendarAccountId: created.calendarAccountId,
                          title: created.title,
                          startsAt: created.startsAt,
                          endsAt: created.endsAt,
                          location: suggestion.location,
                          description: created.description,
                          allDay: created.allDay,
                          sourceMessageId: created.sourceMessageId,
                          reminders: created.reminders ?? [{ minutes: 15 }],
                          status: created.status ?? "confirmed",
                        });
                      }
                      setMessageFocusOpen(false);
                      setCalendarOpen(true);
                      setPlannedOpen(false);
                      setSyncStatus(t("eventCreated"));
                    })
                    .catch((err) =>
                      setSyncStatus((err as AppError).message),
                    );
                }}
                onCreateTask={() => {
                  if (!selectedMessageId) return;
                  void api
                    .calendarTaskFromMessage(
                      selectedMessageId,
                      Math.floor(Date.now() / 1000) + 86400,
                    )
                    .then(() => {
                      setMessageFocusOpen(false);
                      setCalendarOpen(true);
                      setPlannedOpen(false);
                      setSyncStatus(t("taskCreated"));
                    })
                    .catch((err) =>
                      setSyncStatus((err as AppError).message),
                    );
                }}
                onReplySent={() => {
                  void refresh();
                }}
              />
            </div>
          </>
        )}
      </div>

      <AiSetupDialog
        open={
          desktop &&
          Boolean(aiSettingsQuery.data) &&
          !aiSettingsQuery.data?.onboardingCompleted
        }
        onCompleted={(settings: AiSettings) => {
          queryClient.setQueryData(["ai-settings"], settings);
          void queryClient.invalidateQueries({ queryKey: ["ai-settings"] });
        }}
      />
      <AccountSetup
        open={accountSetupOpen}
        onClose={() => setAccountSetupOpen(false)}
        onCreated={async () => {
          await refresh();
          await handleSync();
        }}
      />
      <Composer
        open={composerOpen}
        accounts={accounts}
        replyTo={replyTo}
        draft={editingDraft}
        initialBody={composerBody}
        initialSubject={composerSubject}
        initialTo={composerTo}
        onClose={() => {
          setComposerOpen(false);
          setComposerBody("");
          setComposerSubject(undefined);
          setComposerTo(undefined);
          setEditingDraft(null);
        }}
        onSent={async (sentDraftId) => {
          // Draft was deleted server-side — clear selection so the pane doesn't
          // fetch a missing id and look like a blank / white screen.
          if (sentDraftId && selectedMessageId === sentDraftId) {
            selectMessage(null);
          }
          setSyncStatus(t("messageSent"));
          await refresh();
          refreshPlannedSummary();
        }}
        onDraftSaved={refresh}
        onAddToContacts={openContactPrefills}
      />
      <MessageFocusDialog
        open={messageFocusOpen && Boolean(messageQuery.data)}
        message={messageQuery.data}
        aiEnabled={aiReplyEnabled}
        inSpamFolder={inboxFilters.mailboxRole === "junk"}
        onClose={() => setMessageFocusOpen(false)}
        onDelete={() => {
          setMessageFocusOpen(false);
          void handleDelete();
        }}
        onArchive={() => {
          setMessageFocusOpen(false);
          void handleArchive();
        }}
        onReply={() => {
          setMessageFocusOpen(false);
          setComposerBody("");
          if (messageQuery.data) {
            setReplyTo(messageQuery.data);
            setEditingDraft(null);
            setComposerSubject(undefined);
            setComposerOpen(true);
          }
        }}
        onForward={() => {
          setMessageFocusOpen(false);
          void handleForward();
        }}
        onToggleStar={() => {
          void handleToggleStar();
        }}
        onMarkSpam={() => {
          void handleMarkSpam();
        }}
        onMarkNotSpam={() => {
          void handleMarkNotSpam();
        }}
        onSnooze={(preset) => {
          setMessageFocusOpen(false);
          void handleSnooze(preset);
        }}
        onReplySent={() => {
          void refresh();
        }}
      />
      <SettingsDialog
        open={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
          void queryClient.invalidateQueries({ queryKey: ["ai-settings"] });
        }}
        accounts={accounts}
        onAccountsChanged={() => {
          void queryClient.invalidateQueries({ queryKey: ["accounts"] });
          void queryClient.invalidateQueries({ queryKey: ["mailboxes"] });
        }}
        onAddAccount={() => {
          setSettingsOpen(false);
          setAccountSetupOpen(true);
        }}
      />
      <ContactsDialog
        open={contactsOpen}
        prefill={contactPrefill}
        onPrefillConsumed={clearContactPrefill}
        onContactSaved={advanceContactQueue}
        onClose={() => {
          setContactsOpen(false);
          setContactPrefill(null);
          contactQueueRef.current = [];
        }}
      />
      <QuickTriage
        open={triageOpen}
        messages={messages}
        onClose={() => setTriageOpen(false)}
        onChanged={refresh}
      />
      <OfflinePromptDialog
        prompt={offlinePrompt}
        onDismiss={() => {
          const accountId = offlinePrompt?.accountId;
          setOfflinePrompt(null);
          if (accountId) {
            void api.offlineMailboxDismissPrompt(accountId);
          }
        }}
        onEnable={() => {
          const accountId = offlinePrompt?.accountId;
          setOfflinePrompt(null);
          if (!accountId) return;
          void api
            .offlineMailboxEnableFromPrompt(accountId, "threshold")
            .then(() => api.offlineMailboxRun(accountId, true))
            .then(() => {
              setSettingsOpen(true);
              void queryClient.invalidateQueries({ queryKey: ["messages"] });
            })
            .catch((err) => setSyncStatus((err as AppError).message));
        }}
      />
      <CommandPalette
        open={commandPaletteOpen}
        items={commandItems}
        onClose={() => setCommandPaletteOpen(false)}
        placeholder={t("cmdPalettePlaceholder")}
        emptyLabel={t("cmdPaletteEmpty")}
        ariaLabel={t("commandHint")}
      />
      <div aria-live="polite" className="sr-only">
        {syncStatus}
      </div>
      <AccountCountAnnouncer accounts={accounts} />
    </div>
  );
}

function AccountCountAnnouncer({ accounts }: { accounts: AccountDto[] }) {
  const t = useT();
  return (
    <VisuallyHidden>
      <p>{t("accountsConfigured", { count: accounts.length })}</p>
    </VisuallyHidden>
  );
}
