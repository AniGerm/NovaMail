import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AccountDto,
  AddAccountPasswordRequest,
  AppError,
  AttachmentDto,
  CardDavServerStatus,
  ContactDto,
  ContactsBookSettings,
  LabelDto,
  LdapSearchRequest,
  LdapSyncRequest,
  LdapSyncResult,
  LdapSyncSettings,
  ListMessagesRequest,
  ListMessagesResponse,
  ListThreadsResponse,
  MailboxDto,
  MessageDetailDto,
  MessageSummaryDto,
  OutgoingAttachment,
  ProviderPreset,
  RuleDto,
  SendMessageRequest,
  SetFlagsRequest,
  SignatureDto,
  SuggestReplyMessageResponse,
  SummarizeMessageResponse,
  SyncProgressEvent,
  SyncResult,
  UpsertContactRequest,
} from "./types";

export type { SendMessageRequest, OutgoingAttachment };

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
  threadsList: (request: ListMessagesRequest) =>
    call<ListThreadsResponse>("threads_list", { request }),
  messagesListByThread: (threadId: string) =>
    call<MessageSummaryDto[]>("messages_list_by_thread", { threadId }),
  messagesGet: (messageId: string) =>
    call<MessageDetailDto>("messages_get", { messageId }),
  messagesSetFlags: (request: SetFlagsRequest) =>
    call<void>("messages_set_flags", { request }),
  messagesSend: (request: SendMessageRequest) =>
    call<void>("messages_send", { request }),
  messagesDelete: (messageId: string) =>
    call<void>("messages_delete", { messageId }),
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
  attachmentsList: (messageId: string) =>
    call<AttachmentDto[]>("attachments_list", { messageId }),
  attachmentsOpenPath: (attachmentId: string) =>
    call<string>("attachments_open_path", { attachmentId }),
  contactsList: (query?: string | null) =>
    call<ContactDto[]>("contacts_list", { query: query ?? null }),
  contactsUpsert: (request: UpsertContactRequest) =>
    call<ContactDto>("contacts_upsert", { request }),
  contactsDelete: (contactId: string) =>
    call<void>("contacts_delete", { contactId }),
  carddavStart: () => call<CardDavServerStatus>("carddav_start"),
  carddavStop: () => call<CardDavServerStatus>("carddav_stop"),
  carddavStatus: () => call<CardDavServerStatus>("carddav_status"),
  ldapSearch: (request: LdapSearchRequest) =>
    call<ContactDto[]>("ldap_search", { request }),
  ldapGetSettings: () => call<LdapSyncSettings>("ldap_get_settings"),
  ldapSync: (request: LdapSyncRequest) =>
    call<LdapSyncResult>("ldap_sync", { request }),
  contactsBookSettings: () =>
    call<ContactsBookSettings>("contacts_book_settings"),
  contactsSetBookSettings: (settings: ContactsBookSettings) =>
    call<ContactsBookSettings>("contacts_set_book_settings", { settings }),
  labelsList: (accountId?: string | null) =>
    call<LabelDto[]>("labels_list", { accountId: accountId ?? null }),
  labelsUpsert: (request: {
    id?: string | null;
    accountId: string;
    name: string;
    color: string;
  }) => call<LabelDto>("labels_upsert", { request }),
  labelsDelete: (labelId: string) => call<void>("labels_delete", { labelId }),
  rulesList: () => call<RuleDto[]>("rules_list"),
  rulesUpsert: (request: {
    id?: string | null;
    accountId?: string | null;
    name: string;
    enabled: boolean;
    predicateJson: string;
    actionJson: string;
  }) => call<RuleDto>("rules_upsert", { request }),
  rulesDelete: (ruleId: string) => call<void>("rules_delete", { ruleId }),
  signaturesList: (accountId?: string | null) =>
    call<SignatureDto[]>("signatures_list", { accountId: accountId ?? null }),
  signaturesUpsert: (request: {
    id?: string | null;
    accountId?: string | null;
    name: string;
    bodyText: string;
    isDefault: boolean;
  }) => call<SignatureDto>("signatures_upsert", { request }),
  signaturesDelete: (signatureId: string) =>
    call<void>("signatures_delete", { signatureId }),
  pop3Test: (args: {
    host: string;
    port: number;
    useTls: boolean;
    user: string;
    password: string;
  }) => call<number>("pop3_test", args),
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
