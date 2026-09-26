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
  SuggestReplyMessageResponse,
  SummarizeMessageResponse,
  SyncProgressEvent,
  SyncResult,
} from "./types";

// Re-export for consumers that need the send draft shape.
export type { SendMessageRequest };

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
  accountsAddOAuth: (request: {
    name: string;
    email: string;
    provider: string;
    accessToken: string;
    refreshToken?: string | null;
    expiresAt?: number | null;
    imapHost: string;
    imapPort: number;
    imapTls: boolean;
    smtpHost: string;
    smtpPort: number;
    smtpTls: boolean;
  }) => call<AccountDto>("accounts_add_oauth", { request }),
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
  aiSummarizeMessage: (messageId: string) =>
    call<SummarizeMessageResponse>("ai_summarize_message", {
      request: { messageId },
    }),
  aiSuggestReply: (messageId: string) =>
    call<SuggestReplyMessageResponse>("ai_suggest_reply", {
      request: { messageId },
    }),
  messagesArchive: (messageId: string) =>
    call<void>("messages_archive", { messageId }),
  messagesForwardDraft: (messageId: string) =>
    call<SendMessageRequest>("messages_forward_draft", { messageId }),
  oauthAuthorizeUrl: (provider: string) =>
    call<string>("oauth_authorize_url", { provider }),
  oauthWaitCallback: (timeoutSecs = 180) =>
    call<{ code: string; state?: string | null }>("oauth_wait_callback", {
      timeoutSecs,
    }),
  oauthExchangeCode: (provider: string, code: string) =>
    call<{
      tokens: {
        accessToken: string;
        refreshToken?: string | null;
        expiresAt?: number | null;
      };
    }>("oauth_exchange_code", { request: { provider, code } }),
  onSyncProgress: async (
    handler: (event: SyncProgressEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<SyncProgressEvent>("sync://progress", (event) => {
      handler(event.payload);
    });
  },
};

export function isDesktopShell(): boolean {
  return isTauri();
}
