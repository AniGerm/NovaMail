import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { CommandPalette, EmptyState, Input, VisuallyHidden } from "@novamail/ui";

import { AccountSetup } from "@/features/accounts/AccountSetup";
import { Composer } from "@/features/composer/Composer";
import { MessageList } from "@/features/mail/MessageList";
import { ReadingPane } from "@/features/mail/ReadingPane";
import { Sidebar } from "@/features/mail/Sidebar";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { api, isDesktopShell } from "@/shared/api/client";
import type { AccountDto, AppError, MessageDetailDto } from "@/shared/api/types";
import { useUiStore } from "@/shared/store/uiStore";

export function AppShell() {
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
  const desktop = isDesktopShell();

  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    enabled: desktop,
    queryFn: () => api.accountsList(),
  });

  const messagesQuery = useQuery({
    queryKey: ["messages", "unified", searchQuery],
    enabled: desktop && (accountsQuery.data?.length ?? 0) > 0,
    queryFn: () =>
      api.messagesList({
        unified: true,
        limit: 200,
        offset: 0,
        query: searchQuery || null,
        mailboxId: null,
        accountId: null,
      }),
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
          setSyncStatus(`Sync error in ${event.mailboxName}: ${event.error}`);
          return;
        }
        setSyncStatus(
          event.done
            ? `Synced ${event.mailboxName}`
            : `Syncing ${event.mailboxName}… ${event.fetched}`,
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
      queryClient.invalidateQueries({ queryKey: ["messages"] }),
    ]);
  }, [queryClient]);

  const handleSync = useCallback(async () => {
    if (!desktop) {
      setSyncStatus("Start NovaMail with pnpm dev to sync mail");
      return;
    }
    setSyncStatus("Starting sync…");
    try {
      const results = await api.mailSync(null);
      const total = results.reduce((sum, item) => sum + item.messagesFetched, 0);
      setSyncStatus(`Synced ${total} messages`);
      await refresh();
    } catch (error) {
      setSyncStatus((error as AppError).message);
    }
  }, [desktop, refresh, setSyncStatus]);

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

  const handleForward = useCallback(async () => {
    if (!selectedMessageId || !desktop) return;
    const draft = await api.messagesForwardDraft(selectedMessageId);
    setReplyTo(null);
    setComposerSubject(draft.subject);
    setComposerBody(draft.bodyText);
    setComposerOpen(true);
  }, [desktop, selectedMessageId, setComposerOpen]);

  const messages = messagesQuery.data?.messages ?? [];
  const accounts = accountsQuery.data ?? [];

  const navigateList = useCallback(
    (delta: number) => {
      if (messages.length === 0) return;
      const index = messages.findIndex((m) => m.id === selectedMessageId);
      const next = index < 0 ? 0 : Math.min(Math.max(index + delta, 0), messages.length - 1);
      selectMessage(messages[next].id);
    },
    [messages, selectMessage, selectedMessageId],
  );

  const themeDark =
    theme === "dark" ||
    (theme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);

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
      handleForward,
      messageQuery.data,
      navigateList,
      setCommandPaletteOpen,
      setComposerOpen,
      setSettingsOpen,
    ],
  );
  useKeyboardShortcuts(shortcuts);

  const commandItems = useMemo(
    () => [
      {
        id: "compose",
        label: "Compose message",
        hint: "C",
        onSelect: () => {
          setReplyTo(null);
          setComposerBody("");
          setComposerSubject(undefined);
          setComposerOpen(true);
        },
      },
      {
        id: "sync",
        label: "Sync all accounts",
        hint: "",
        onSelect: () => {
          void handleSync();
        },
      },
      {
        id: "add-account",
        label: "Add account",
        onSelect: () => setAccountSetupOpen(true),
      },
      {
        id: "settings",
        label: "Open settings",
        hint: ",",
        onSelect: () => setSettingsOpen(true),
      },
      {
        id: "archive",
        label: "Archive selected message",
        hint: "E",
        onSelect: () => {
          void handleArchive();
        },
      },
      {
        id: "forward",
        label: "Forward selected message",
        hint: "F",
        onSelect: () => {
          void handleForward();
        },
      },
      {
        id: "theme",
        label: themeDark ? "Switch to light mode" : "Switch to dark mode",
        onSelect: () => setTheme(themeDark ? "light" : "dark"),
      },
    ],
    [
      handleArchive,
      handleForward,
      handleSync,
      setAccountSetupOpen,
      setComposerOpen,
      setSettingsOpen,
      setTheme,
      themeDark,
    ],
  );

  if (!desktop) {
    return (
      <EmptyState
        title="NovaMail"
        description="The mail engine runs inside the Tauri desktop shell. Start it with pnpm dev from the repository root."
      />
    );
  }

  return (
    <div
      className="flex h-full flex-col"
      data-density={density}
    >
      <VisuallyHidden>
        <h1>NovaMail unified inbox</h1>
      </VisuallyHidden>
      <div className="flex items-center gap-3 border-b border-[var(--nova-border)] px-4 py-3">
        <label className="sr-only" htmlFor="global-search">
          Search mail
        </label>
        <Input
          id="global-search"
          placeholder="Search unified inbox"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="max-w-xl"
        />
        <button
          type="button"
          className="text-xs text-[var(--nova-ink-muted)]"
          onClick={() => setCommandPaletteOpen(true)}
        >
          Ctrl/Cmd+K
        </button>
      </div>

      <div className="flex min-h-0 flex-1">
        <Sidebar
          accounts={accounts}
          syncStatus={syncStatus}
          themeDark={themeDark}
          onCompose={() => {
            setReplyTo(null);
            setComposerBody("");
            setComposerSubject(undefined);
            setComposerOpen(true);
          }}
          onSync={handleSync}
          onAddAccount={() => setAccountSetupOpen(true)}
          onToggleTheme={() => setTheme(themeDark ? "light" : "dark")}
          onOpenSettings={() => setSettingsOpen(true)}
        />

        {accounts.length === 0 ? (
          <EmptyState
            title="Welcome to NovaMail"
            description="Add your first account to sync a unified inbox across Gmail, Microsoft 365, Yahoo, Proton Bridge, or any IMAP server."
            action={
              <button
                type="button"
                className="mt-2 text-[var(--nova-accent)]"
                onClick={() => setAccountSetupOpen(true)}
              >
                Add account
              </button>
            }
          />
        ) : (
          <>
            <div className="w-[360px] shrink-0">
              <MessageList
                messages={messages}
                selectedId={selectedMessageId}
                onSelect={selectMessage}
                total={messagesQuery.data?.total ?? messages.length}
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
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
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
  return (
    <VisuallyHidden>
      <p>{accounts.length} accounts configured</p>
    </VisuallyHidden>
  );
}
