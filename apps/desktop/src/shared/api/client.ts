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

export function isDesktopShell(): boolean {
  return isTauri();
}
