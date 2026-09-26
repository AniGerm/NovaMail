import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useKeyboardShortcuts } from "@novamail/hooks";
import { EmptyState, Input } from "@novamail/ui";

import { AccountSetup } from "@/features/accounts/AccountSetup";
import { Composer } from "@/features/composer/Composer";
import { MessageList } from "@/features/mail/MessageList";
import { ReadingPane } from "@/features/mail/ReadingPane";
import { Sidebar } from "@/features/mail/Sidebar";
import { api, demoMessages } from "@/shared/api/client";
import type { AppError, MessageDetailDto } from "@/shared/api/types";
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
    searchQuery,
    setSearchQuery,
    syncStatus,
    setSyncStatus,
    theme,
    setTheme,
  } = useUiStore();

  const [replyTo, setReplyTo] = useState<MessageDetailDto | null>(null);
  const [demoMode, setDemoMode] = useState(false);

  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    queryFn: async () => {
      try {
        return await api.accountsList();
      } catch (error) {
        if ((error as AppError).code === "not_tauri") {
          setDemoMode(true);
          return [];
        }
        throw error;
      }
    },
  });

  const messagesQuery = useQuery({
    queryKey: ["messages", "unified", searchQuery],
    queryFn: async () => {
      if (demoMode) {
        const filtered = demoMessages.messages.filter((message) => {
          if (!searchQuery.trim()) return true;
          const q = searchQuery.toLowerCase();
          return (
            message.subject.toLowerCase().includes(q) ||
            message.snippet.toLowerCase().includes(q) ||
            message.from.email.toLowerCase().includes(q)
          );
        });
        return { messages: filtered, total: filtered.length };
      }
      return api.messagesList({
        unified: true,
        limit: 200,
        offset: 0,
        query: searchQuery || null,
        mailboxId: null,
        accountId: null,
      });
    },
  });

  const messageQuery = useQuery({
    queryKey: ["message", selectedMessageId],
    enabled: Boolean(selectedMessageId),
    queryFn: async () => {
      if (demoMode) {
        const summary = demoMessages.messages.find((m) => m.id === selectedMessageId);
        if (!summary) throw { code: "not_found", message: "Missing demo message" };
        return {
          summary,
          bodyText: `${summary.snippet}\n\nThis is demo content shown outside the Tauri shell.`,
          bodyHtml: null,
          messageId: `<${summary.id}@demo>`,
          inReplyTo: null,
          references: [],
        } satisfies MessageDetailDto;
      }
      return api.messagesGet(selectedMessageId!);
    },
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    api.onSyncProgress((event) => {
      if (event.error) {
        setSyncStatus(`Sync error in ${event.mailboxName}: ${event.error}`);
        return;
      }
      setSyncStatus(
        event.done
          ? `Synced ${event.mailboxName}`
          : `Syncing ${event.mailboxName}… ${event.fetched}`,
      );
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [setSyncStatus]);

  const refresh = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["accounts"] }),
      queryClient.invalidateQueries({ queryKey: ["messages"] }),
    ]);
  }, [queryClient]);

  const handleSync = useCallback(async () => {
    if (demoMode) {
      setSyncStatus("Demo mode — open via Tauri to sync real mail");
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
  }, [demoMode, refresh, setSyncStatus]);

  const handleToggleStar = useCallback(async () => {
    if (!messageQuery.data) return;
    if (demoMode) return;
    await api.messagesSetFlags({
      messageId: messageQuery.data.summary.id,
      starred: !messageQuery.data.summary.starred,
    });
    await queryClient.invalidateQueries({
      queryKey: ["message", messageQuery.data.summary.id],
    });
    await queryClient.invalidateQueries({ queryKey: ["messages"] });
  }, [demoMode, messageQuery.data, queryClient]);

  const themeDark =
    theme === "dark" ||
    (theme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);

  const shortcuts = useMemo(
    () => ({
      c: () => {
        setReplyTo(null);
        setComposerOpen(true);
      },
      r: () => {
        if (messageQuery.data) {
          setReplyTo(messageQuery.data);
          setComposerOpen(true);
        }
      },
      "/": () => {
        document.getElementById("global-search")?.focus();
      },
      "mod+k": () => {
        document.getElementById("global-search")?.focus();
      },
    }),
    [messageQuery.data, setComposerOpen],
  );
  useKeyboardShortcuts(shortcuts);

  const messages = messagesQuery.data?.messages ?? [];
  const accounts = accountsQuery.data ?? [];

  return (
    <div className="flex h-full flex-col">
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
        {demoMode ? (
          <span className="text-xs text-[var(--nova-ink-muted)]">
            Browser preview · demo data
          </span>
        ) : null}
      </div>

      <div className="flex min-h-0 flex-1">
        <Sidebar
          accounts={accounts}
          syncStatus={syncStatus}
          themeDark={themeDark}
          onCompose={() => {
            setReplyTo(null);
            setComposerOpen(true);
          }}
          onSync={handleSync}
          onAddAccount={() => setAccountSetupOpen(true)}
          onToggleTheme={() => setTheme(themeDark ? "light" : "dark")}
        />

        {accounts.length === 0 && !demoMode ? (
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
                    setComposerOpen(true);
                  }
                }}
                onToggleStar={handleToggleStar}
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
        onClose={() => setComposerOpen(false)}
        onSent={refresh}
      />
    </div>
  );
}
