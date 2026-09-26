import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AccountDto,
  AddAccountPasswordRequest,
  AppError,
  ListMessagesRequest,
  ListMessagesResponse,
  MailboxDto,
  MessageDetailDto,
  ProviderPreset,
  SendMessageRequest,
  SetFlagsRequest,
  SyncProgressEvent,
  SyncResult,
} from "./types";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw {
      code: "not_tauri",
      message: "NovaMail API is only available inside the Tauri shell.",
    } satisfies AppError;
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    const appError = error as AppError;
    throw appError;
  }
}

export const api = {
  providerPresets: () => call<ProviderPreset[]>("provider_presets"),
  accountsList: () => call<AccountDto[]>("accounts_list"),
  accountsAddPassword: (request: AddAccountPasswordRequest) =>
    call<AccountDto>("accounts_add_password", { request }),
  accountsRemove: (accountId: string) =>
    call<void>("accounts_remove", { accountId }),
  mailboxesList: (accountId?: string | null) =>
    call<MailboxDto[]>("mailboxes_list", { accountId: accountId ?? null }),
  messagesList: (request: ListMessagesRequest) =>
    call<ListMessagesResponse>("messages_list", { request }),
  messagesGet: (messageId: string) =>
    call<MessageDetailDto>("messages_get", { messageId }),
  messagesSetFlags: (request: SetFlagsRequest) =>
    call<void>("messages_set_flags", { request }),
  messagesSend: (request: SendMessageRequest) =>
    call<void>("messages_send", { request }),
  mailSync: (accountId?: string | null) =>
    call<SyncResult[]>("mail_sync", { request: { accountId: accountId ?? null } }),
  onSyncProgress: async (
    handler: (event: SyncProgressEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<SyncProgressEvent>("sync://progress", (event) => {
      handler(event.payload);
    });
  },
};

/** Demo data used when the UI is opened in a plain browser for design work. */
export const demoMessages: ListMessagesResponse = {
  total: 3,
  messages: [
    {
      id: "demo-1",
      accountId: "acc-1",
      mailboxId: "mb-1",
      threadId: "th-1",
      subject: "Welcome to NovaMail",
      from: { name: "NovaMail", email: "hello@novamail.app" },
      to: [{ email: "you@example.com" }],
      date: Math.floor(Date.now() / 1000) - 3600,
      snippet: "A modern open-source mail client for Ubuntu.",
      unread: true,
      starred: false,
      hasAttachments: false,
      accountEmail: "you@example.com",
    },
    {
      id: "demo-2",
      accountId: "acc-1",
      mailboxId: "mb-1",
      threadId: "th-2",
      subject: "Quarterly planning notes",
      from: { name: "Alex Rivera", email: "alex@company.com" },
      to: [{ email: "you@example.com" }],
      date: Math.floor(Date.now() / 1000) - 7200,
      snippet: "Here are the talking points for Monday.",
      unread: true,
      starred: true,
      hasAttachments: true,
      accountEmail: "you@example.com",
    },
    {
      id: "demo-3",
      accountId: "acc-2",
      mailboxId: "mb-2",
      threadId: "th-3",
      subject: "Your receipt from Bookstore",
      from: { name: "Bookstore", email: "orders@bookstore.example" },
      to: [{ email: "personal@example.com" }],
      date: Math.floor(Date.now() / 1000) - 86400,
      snippet: "Thanks for your purchase.",
      unread: false,
      starred: false,
      hasAttachments: false,
      accountEmail: "personal@example.com",
    },
  ],
};
