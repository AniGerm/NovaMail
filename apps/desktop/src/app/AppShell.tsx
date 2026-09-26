import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { CommandPalette, EmptyState, Input, VisuallyHidden } from "@novamail/ui";

import { AccountSetup } from "@/features/accounts/AccountSetup";
import { Composer } from "@/features/composer/Composer";
import { ContactsDialog } from "@/features/contacts/ContactsDialog";
import {
  MessageList,
  buildListRequest,
  defaultInboxFilters,
  type InboxFilters,
} from "@/features/mail/MessageList";
import { QuickTriage } from "@/features/mail/QuickTriage";
import { ReadingPane } from "@/features/mail/ReadingPane";
import { Sidebar } from "@/features/mail/Sidebar";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { api, isDesktopShell } from "@/shared/api/client";
import type { AccountDto, AppError, MessageDetailDto } from "@/shared/api/types";
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
  } = useUiStore();

  const [replyTo, setReplyTo] = useState<MessageDetailDto | null>(null);
  const [composerBody, setComposerBody] = useState("");
  const [composerSubject, setComposerSubject] = useState<string | undefined>();
  const [inboxFilters, setInboxFilters] =
    useState<InboxFilters>(defaultInboxFilters);
  const desktop = isDesktopShell();

  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    enabled: desktop,
    queryFn: () => api.accountsList(),
  });

  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", inboxFilters.accountId],
    enabled: desktop && (accountsQuery.data?.length ?? 0) > 0,
    queryFn: () => api.mailboxesList(inboxFilters.accountId),
  });

  const listRequest = useMemo(
    () => buildListRequest(inboxFilters, searchQuery),
    [inboxFilters, searchQuery],
  );

  const messagesQuery = useQuery({
    queryKey: ["messages", "flat", listRequest],
    enabled: desktop && (accountsQuery.data?.length ?? 0) > 0,
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
          void queryClient.invalidateQueries({ queryKey: ["messages"] });
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [desktop, queryClient, setSyncStatus]);

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

  const handleToggleStar = useCallback(async () => {
    if (!messageQuery.data || !desktop) return;
    await api.messagesSetFlags({
      messageId: messageQuery.data.summary.id,
      starred: !messageQuery.data.summary.starred,
    });
    await queryClient.invalidateQueries({
      queryKey: ["message", messageQuery.data.summary.id],
    });
    await queryClient.invalidateQueries({ queryKey: ["messages"] });
  }, [desktop, messageQuery.data, queryClient]);

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
    setComposerSubject(draft.subject);
    setComposerBody(draft.bodyText);
    setComposerOpen(true);
  }, [desktop, selectedMessageId, setComposerOpen]);

  const messages = messagesQuery.data?.messages ?? [];
  const threads = threadsQuery.data?.threads ?? [];
  const accounts = accountsQuery.data ?? [];
  const mailboxes = mailboxesQuery.data ?? [];
  const listTotal =
    inboxFilters.viewMode === "threads"
      ? (threadsQuery.data?.total ?? threads.length)
      : (messagesQuery.data?.total ?? messages.length);

  const navigateList = useCallback(
    (delta: number) => {
      if (messages.length === 0) return;
      const index = messages.findIndex((m) => m.id === selectedMessageId);
      const next = index < 0 ? 0 : Math.min(Math.max(index + delta, 0), messages.length - 1);
      selectMessage(messages[next].id);
    },
    [messages, selectMessage, selectedMessageId],
  );

  const handleSelectAccountFilter = useCallback((accountId: string | null) => {
    setInboxFilters((prev) => ({
      ...prev,
      accountId,
      mailboxId: null,
    }));
  }, []);

  const cycleTheme = useCallback(() => {
    setTheme(nextThemeMode(theme as ThemeMode));
  }, [setTheme, theme]);

  const shortcuts = useMemo(
    () => ({
      c: () => {
        setReplyTo(null);
        setComposerBody("");
        setComposerSubject(undefined);
        setComposerOpen(true);
      },
      r: () => {
        if (messageQuery.data) {
          setReplyTo(messageQuery.data);
          setComposerBody("");
          setComposerSubject(undefined);
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
    }),
    [
      handleArchive,
      handleDelete,
      handleForward,
      messageQuery.data,
      navigateList,
      setCommandPaletteOpen,
      setComposerOpen,
      setSettingsOpen,
      setTriageOpen,
    ],
  );
  useKeyboardShortcuts(triageOpen ? {} : shortcuts);

  const commandItems = useMemo(
    () => [
      {
        id: "compose",
        label: t("cmdCompose"),
        hint: "C",
        onSelect: () => {
          setReplyTo(null);
          setComposerBody("");
          setComposerSubject(undefined);
          setComposerOpen(true);
        },
      },
      {
        id: "triage",
        label: t("cmdTriage"),
        hint: "T",
        onSelect: () => setTriageOpen(true),
      },
      {
        id: "contacts",
        label: t("cmdContacts"),
        onSelect: () => setContactsOpen(true),
      },
      {
        id: "sync",
        label: t("cmdSync"),
        hint: "",
        onSelect: () => {
          void handleSync();
        },
      },
      {
        id: "add-account",
        label: t("cmdAddAccount"),
        onSelect: () => setAccountSetupOpen(true),
      },
      {
        id: "settings",
        label: t("cmdSettings"),
        hint: ",",
        onSelect: () => setSettingsOpen(true),
      },
      {
        id: "archive",
        label: t("cmdArchive"),
        hint: "E",
        onSelect: () => {
          void handleArchive();
        },
      },
      {
        id: "delete",
        label: t("cmdDelete"),
        hint: "⌫",
        onSelect: () => {
          void handleDelete();
        },
      },
      {
        id: "forward",
        label: t("cmdForward"),
        hint: "F",
        onSelect: () => {
          void handleForward();
        },
      },
      {
        id: "theme",
        label:
          theme === "light"
            ? t("switchToDark")
            : theme === "dark"
              ? t("switchToAuto")
              : t("switchToLight"),
        onSelect: () => cycleTheme(),
      },
    ],
    [
      cycleTheme,
      handleArchive,
      handleDelete,
      handleForward,
      handleSync,
      setAccountSetupOpen,
      setComposerOpen,
      setContactsOpen,
      setSettingsOpen,
      setTriageOpen,
      t,
      theme,
    ],
  );

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
          selectedAccountId={inboxFilters.accountId}
          syncStatus={syncStatus}
          themeMode={theme}
          onSelectUnified={() => handleSelectAccountFilter(null)}
          onSelectAccount={handleSelectAccountFilter}
          onCompose={() => {
            setReplyTo(null);
            setComposerBody("");
            setComposerSubject(undefined);
            setComposerOpen(true);
          }}
          onSync={handleSync}
          onAddAccount={() => setAccountSetupOpen(true)}
          onToggleTheme={cycleTheme}
          onOpenSettings={() => setSettingsOpen(true)}
          onOpenContacts={() => setContactsOpen(true)}
          onOpenTriage={() => setTriageOpen(true)}
        />

        {accounts.length === 0 ? (
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
                onSelect={selectMessage}
                total={listTotal}
                filters={inboxFilters}
                onFiltersChange={setInboxFilters}
                accounts={accounts}
                mailboxes={mailboxes}
              />
            </div>
            <div className="min-w-0 flex-1 bg-[color-mix(in_srgb,var(--nova-surface)_92%,transparent)]">
              <ReadingPane
                message={messageQuery.data}
                onReply={() => {
                  if (messageQuery.data) {
                    setReplyTo(messageQuery.data);
                    setComposerSubject(undefined);
                    setComposerOpen(true);
                  }
                }}
                onForward={() => {
                  void handleForward();
                }}
                onToggleStar={handleToggleStar}
                onUseSuggestedReply={(suggestion) => {
                  setComposerBody(suggestion);
                  if (messageQuery.data) {
                    setReplyTo(messageQuery.data);
                  }
                }}
              />
            </div>
          </>
        )}
      </div>

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
        initialBody={composerBody}
        initialSubject={composerSubject}
        onClose={() => {
          setComposerOpen(false);
          setComposerBody("");
          setComposerSubject(undefined);
        }}
        onSent={refresh}
      />
      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        accounts={accounts}
      />
      <ContactsDialog open={contactsOpen} onClose={() => setContactsOpen(false)} />
      <QuickTriage
        open={triageOpen}
        messages={messages}
        onClose={() => setTriageOpen(false)}
        onChanged={refresh}
      />
      <CommandPalette
        open={commandPaletteOpen}
        items={commandItems}
        onClose={() => setCommandPaletteOpen(false)}
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
