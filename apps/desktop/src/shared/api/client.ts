import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AccountDto,
  AddAccountPasswordRequest,
  UpdateAccountRequest,
  AppError,
  AttachmentDto,
  CardDavServerStatus,
  ContactDto,
  ContactsBookSettings,
  ContactsShareStatus,
  SetContactsShareModeRequest,
  RecipientSuggestion,
  SpellcheckStatus,
  SpellDictionaryDto,
  ExportBackupResponse,
  ImportBackupResult,
  AiInstallOllamaResponse,
  AiInstallProgressEvent,
  AiPullModelResponse,
  AiPullProgressEvent,
  AiRuntimeStatus,
  AiSettings,
  MessageAiInsights,
  SuggestRepliesMessageResponse,
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
  FolderPoliciesDto,
  OfflineMailboxAccountPolicy,
  OfflineMailboxMode,
  OfflineMailboxSettingsDto,
  OfflineOffloadReport,
  OfflinePromptEvent,
  AccountQuotaDto,
  CalendarAccountDto,
  CalendarCollectionDto,
  CalendarEventDto,
  CalendarInvitationDto,
  CalendarReminderDto,
  CalendarTaskDto,
  InvitationResponse,
  JobsTickReport,
  OutboundQueueItemDto,
  PgpDecryptResult,
  PgpKeyDto,
  PgpVerifyResult,
  PlannedSummaryDto,
  SendLaterRequest,
  SnoozeRequest,
  SnoozedMessageDto,
  RuleDto,
  SaveDraftRequest,
  SendMessageRequest,
  SetFlagsRequest,
  SignatureDto,
  SpamScoreDto,
  SpamSettingsDto,
  SuggestReplyMessageResponse,
  SummarizeMessageResponse,
  SyncProgressEvent,
  SyncResult,
  UpsertContactRequest,
  AppVersionInfo,
  UpdateActionResult,
  UpdateCheckResult,
  UpdateStatusEvent,
} from "./types";

export type { SendMessageRequest, OutgoingAttachment };

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function formatApiError(error: unknown, fallback = "Request failed"): string {
  if (typeof error === "string" && error.trim()) return error;
  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    if (typeof record.message === "string" && record.message.trim()) {
      return record.message;
    }
    if (typeof record.error === "string" && record.error.trim()) {
      return record.error;
    }
    // Tauri sometimes nests the payload.
    if (record.message && typeof record.message === "object") {
      return formatApiError(record.message, fallback);
    }
  }
  try {
    const raw = JSON.stringify(error);
    if (raw && raw !== "{}") return raw;
  } catch {
    /* ignore */
  }
  return fallback;
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
    throw {
      code: "invoke",
      message: formatApiError(error, `Command ${command} failed`),
    } satisfies AppError;
  }
}

export const api = {
  providerPresets: () => call<ProviderPreset[]>("provider_presets"),
  accountsList: () => call<AccountDto[]>("accounts_list"),
  accountsAddPassword: (request: AddAccountPasswordRequest) =>
    call<AccountDto>("accounts_add_password", { request }),
  accountsAddOAuth: (request: {
    name: string;
    label?: string;
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
  accountsUpdate: (request: UpdateAccountRequest) =>
    call<AccountDto>("accounts_update", { request }),
  mailboxesList: (accountId?: string | null) =>
    call<MailboxDto[]>("mailboxes_list", { accountId: accountId ?? null }),
  messagesList: (request: ListMessagesRequest) =>
    call<ListMessagesResponse>("messages_list", { request }),
  messagesListIds: (request: ListMessagesRequest) =>
    call<string[]>("messages_list_ids", { request }),
  threadsList: (request: ListMessagesRequest) =>
    call<ListThreadsResponse>("threads_list", { request }),
  messagesListByThread: (threadId: string) =>
    call<MessageSummaryDto[]>("messages_list_by_thread", { threadId }),
  messagesGet: (messageId: string) =>
    call<MessageDetailDto>("messages_get", { messageId }),
  messagesSetFlags: (request: SetFlagsRequest) =>
    call<void>("messages_set_flags", { request }),
  messagesSetFlagsMany: (
    messageIds: string[],
    flags: { unread?: boolean; starred?: boolean },
  ) =>
    call<void>("messages_set_flags_many", {
      messageIds,
      unread: flags.unread ?? null,
      starred: flags.starred ?? null,
    }),
  messagesSend: (request: SendMessageRequest) =>
    call<void>("messages_send", { request }),
  messagesSaveDraft: (request: SaveDraftRequest) =>
    call<MessageDetailDto>("messages_save_draft", { request }),
  messagesDelete: (messageId: string) =>
    call<void>("messages_delete", { messageId }),
  mailSync: (accountId?: string | null) =>
    call<SyncResult[]>("mail_sync", { request: { accountId: accountId ?? null } }),
  aiSummarizeMessage: (messageId: string, preferredLanguage?: string | null) =>
    call<SummarizeMessageResponse>("ai_summarize_message", {
      request: { messageId, preferredLanguage: preferredLanguage ?? null },
    }),
  aiSuggestReply: (
    messageId: string,
    facts?: string | null,
    preferredLanguage?: string | null,
  ) =>
    call<SuggestReplyMessageResponse>("ai_suggest_reply", {
      request: {
        messageId,
        facts: facts ?? null,
        preferredLanguage: preferredLanguage ?? null,
      },
    }),
  aiSuggestReplies: (
    messageId: string,
    facts?: string | null,
    preferredLanguage?: string | null,
  ) =>
    call<SuggestRepliesMessageResponse>("ai_suggest_replies", {
      request: {
        messageId,
        facts: facts ?? null,
        preferredLanguage: preferredLanguage ?? null,
      },
    }),
  aiMessageInsights: (messageId: string) =>
    call<MessageAiInsights>("ai_message_insights", { messageId }),
  aiGetSettings: () => call<AiSettings>("ai_get_settings"),
  aiSetSettings: (settings: AiSettings) =>
    call<AiSettings>("ai_set_settings", { settings }),
  aiRuntimeStatus: () => call<AiRuntimeStatus>("ai_runtime_status"),
  aiPullModel: (model: string) =>
    call<AiPullModelResponse>("ai_pull_model", { request: { model } }),
  onAiPullProgress: async (
    handler: (event: AiPullProgressEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<AiPullProgressEvent>("ai://pull-progress", (event) => {
      handler(event.payload);
    });
  },
  aiInstallOllama: (mode: "user" | "system") =>
    call<AiInstallOllamaResponse>("ai_install_ollama", { request: { mode } }),
  aiStartOllama: () => call<AiInstallOllamaResponse>("ai_start_ollama"),
  onAiInstallProgress: async (
    handler: (event: AiInstallProgressEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<AiInstallProgressEvent>("ai://install-progress", (event) => {
      handler(event.payload);
    });
  },
  messagesArchive: (messageId: string) =>
    call<void>("messages_archive", { messageId }),
  messagesForwardDraft: (messageId: string) =>
    call<SendMessageRequest>("messages_forward_draft", { messageId }),
  attachmentsList: (messageId: string) =>
    call<AttachmentDto[]>("attachments_list", { messageId }),
  attachmentsOpenPath: (
    attachmentId: string,
    messageId?: string | null,
    filename?: string | null,
  ) =>
    call<string>("attachments_open_path", {
      attachmentId,
      messageId: messageId ?? null,
      filename: filename ?? null,
    }),
  /** Opens the attachment with the system handler (Rust-side xdg-open). */
  attachmentsOpen: (
    attachmentId: string,
    messageId?: string | null,
    filename?: string | null,
  ) =>
    call<string>("attachments_open_path", {
      attachmentId,
      messageId: messageId ?? null,
      filename: filename ?? null,
    }),
  attachmentsReveal: (
    attachmentId: string,
    messageId?: string | null,
    filename?: string | null,
  ) =>
    call<string>("attachments_reveal", {
      attachmentId,
      messageId: messageId ?? null,
      filename: filename ?? null,
    }),
  messagesExportPdf: (messageId: string) =>
    call<string>("messages_export_pdf", { messageId }),
  messagesExportHtml: (messageId: string) =>
    call<string>("messages_export_html", { messageId }),
  contactsList: (query?: string | null) =>
    call<ContactDto[]>("contacts_list", { query: query ?? null }),
  recipientsSuggest: (query: string, limit = 12) =>
    call<RecipientSuggestion[]>("recipients_suggest", { query, limit }),
  spellcheckStatus: () => call<SpellcheckStatus>("spellcheck_status"),
  spellcheckSuggest: (word: string, lang: string) =>
    call<{
      word: string;
      correct: boolean;
      suggestions: string[];
      autocorrect?: string | null;
    }>("spellcheck_suggest", { word, lang }),
  spellcheckLearnWord: (word: string, lang: string) =>
    call<void>("spellcheck_learn_word", { word, lang }),
  spellcheckInstall: (code: string) =>
    call<SpellDictionaryDto>("spellcheck_install", { code }),
  spellcheckEnsureForLocale: (locale: string) =>
    call<SpellDictionaryDto>("spellcheck_ensure_for_locale", { locale }),
  spellcheckSetLanguages: (languages: string[]) =>
    call<void>("spellcheck_set_languages", { languages }),
  aiOptimizeDraft: (request: {
    subject: string;
    bodyText: string;
    preferredLanguage?: string | null;
  }) =>
    call<{ suggestion: string; provider: string }>("ai_optimize_draft", {
      request,
    }),
  contactsUpsert: (request: UpsertContactRequest) =>
    call<ContactDto>("contacts_upsert", { request }),
  contactsDelete: (contactId: string) =>
    call<void>("contacts_delete", { contactId }),
  carddavStart: () => call<CardDavServerStatus>("carddav_start"),
  carddavStop: () => call<CardDavServerStatus>("carddav_stop"),
  carddavStatus: () => call<CardDavServerStatus>("carddav_status"),
  contactsShareStatus: () => call<ContactsShareStatus>("contacts_share_status"),
  contactsSetShareMode: (request: SetContactsShareModeRequest) =>
    call<ContactsShareStatus>("contacts_set_share_mode", {
      mode: request.mode,
      clientUrl: request.clientUrl ?? null,
      clientBindDn: request.clientBindDn ?? null,
      clientPassword: request.clientPassword ?? null,
      clientBaseDn: request.clientBaseDn ?? null,
    }),
  contactsClientSync: () => call<LdapSyncResult>("contacts_client_sync"),
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
  messagesSetLabels: (request: { messageId: string; labelIds: string[] }) =>
    call<void>("messages_set_labels", { request }),
  messagesListLabels: (messageId: string) =>
    call<LabelDto[]>("messages_list_labels", { messageId }),
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
  messagesMove: (request: { messageId: string; target: string }) =>
    call<void>("messages_move", { request }),
  messagesMarkSpam: (messageId: string) =>
    call<void>("messages_mark_spam", { messageId }),
  messagesMarkNotSpam: (messageId: string) =>
    call<void>("messages_mark_not_spam", { messageId }),
  spamGetSettings: () => call<SpamSettingsDto>("spam_get_settings"),
  spamSetSettings: (settings: SpamSettingsDto) =>
    call<SpamSettingsDto>("spam_set_settings", { settings }),
  spamScoreMessage: (messageId: string) =>
    call<SpamScoreDto>("spam_score_message", { messageId }),
  folderPoliciesGet: () => call<FolderPoliciesDto>("folder_policies_get"),
  folderPoliciesSet: (policies: FolderPoliciesDto) =>
    call<FolderPoliciesDto>("folder_policies_set", { policies }),
  folderPoliciesApply: () => call<number>("folder_policies_apply"),
  offlineMailboxGet: () =>
    call<OfflineMailboxSettingsDto>("offline_mailbox_get"),
  offlineMailboxSet: (settings: OfflineMailboxSettingsDto) =>
    call<OfflineMailboxSettingsDto>("offline_mailbox_set", { settings }),
  offlineMailboxSetPolicy: (policy: OfflineMailboxAccountPolicy) =>
    call<OfflineMailboxAccountPolicy>("offline_mailbox_set_policy", {
      policy,
    }),
  offlineMailboxQuota: (accountId: string) =>
    call<AccountQuotaDto>("offline_mailbox_quota", { accountId }),
  offlineMailboxLocalCount: (accountId?: string | null) =>
    call<number>("offline_mailbox_local_count", {
      accountId: accountId ?? null,
    }),
  offlineMailboxRun: (accountId: string, force = false) =>
    call<OfflineOffloadReport>("offline_mailbox_run", { accountId, force }),
  offlineMailboxDismissPrompt: (accountId: string) =>
    call<void>("offline_mailbox_dismiss_prompt", { accountId }),
  offlineMailboxEnableFromPrompt: (
    accountId: string,
    mode: OfflineMailboxMode = "threshold",
  ) =>
    call<OfflineMailboxAccountPolicy>("offline_mailbox_enable_from_prompt", {
      accountId,
      mode,
    }),
  onOfflinePrompt: async (
    handler: (event: OfflinePromptEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<OfflinePromptEvent>("offline://prompt", (event) => {
      handler(event.payload);
    });
  },
  messagesSnooze: (request: SnoozeRequest) =>
    call<number>("messages_snooze", { request }),
  messagesUnsnooze: (messageId: string) =>
    call<void>("messages_unsnooze", { messageId }),
  messagesListSnoozed: (limit = 200) =>
    call<SnoozedMessageDto[]>("messages_list_snoozed", { limit }),
  messagesSendLater: (request: SendLaterRequest) =>
    call<string>("messages_send_later", { request }),
  outboundList: (limit = 200) =>
    call<OutboundQueueItemDto[]>("outbound_list", { limit }),
  outboundCancel: (id: string) => call<void>("outbound_cancel", { id }),
  plannedSummary: () => call<PlannedSummaryDto>("planned_summary"),
  jobsTick: () => call<JobsTickReport>("jobs_tick"),
  onJobsTick: async (
    handler: (event: JobsTickReport) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<JobsTickReport>("jobs://tick", (event) => {
      handler(event.payload);
    });
  },
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
  backupExport: (passphrase: string) =>
    call<ExportBackupResponse>("backup_export", { request: { passphrase } }),
  backupImport: (passphrase: string, dataBase64: string) =>
    call<ImportBackupResult>("backup_import", {
      request: { passphrase, dataBase64 },
    }),
  pgpListKeys: () => call<PgpKeyDto[]>("pgp_list_keys"),
  pgpGenerate: (userId: string) =>
    call<PgpKeyDto>("pgp_generate", { request: { userId } }),
  pgpImport: (armored: string) =>
    call<PgpKeyDto>("pgp_import", { request: { armored } }),
  pgpDelete: (fingerprint: string) => call<void>("pgp_delete", { fingerprint }),
  pgpExportPublic: (fingerprint: string) =>
    call<string>("pgp_export_public", { fingerprint }),
  pgpDecryptText: (armored: string) =>
    call<PgpDecryptResult>("pgp_decrypt_text", { armored }),
  pgpVerifyText: (armored: string) =>
    call<PgpVerifyResult>("pgp_verify_text", { armored }),
  pgpInspectMessage: (messageId: string) =>
    call<PgpDecryptResult | null>("pgp_inspect_message", { messageId }),
  calendarAccountsList: () =>
    call<CalendarAccountDto[]>("calendar_accounts_list"),
  calendarAccountsUpsert: (request: {
    id?: string | null;
    name: string;
    caldavUrl: string;
    username: string;
    password?: string | null;
    collections?: { href: string; displayName: string }[];
  }) => call<CalendarAccountDto>("calendar_accounts_upsert", { request }),
  calendarAccountsDelete: (id: string) =>
    call<void>("calendar_accounts_delete", { id }),
  calendarAccountsSync: (id: string) =>
    call<[number, number]>("calendar_accounts_sync", { id }),
  calendarDiscover: (request: {
    caldavUrl: string;
    username: string;
    password: string;
  }) =>
    call<{ href: string; displayName: string }[]>("calendar_discover", {
      request,
    }),
  calendarCollectionsList: () =>
    call<CalendarCollectionDto[]>("calendar_collections_list"),
  calendarCollectionsUpsert: (request: {
    id?: string | null;
    calendarAccountId?: string | null;
    href?: string | null;
    displayName: string;
    color: string;
    isVisible?: boolean;
    isDefault?: boolean;
  }) => call<CalendarCollectionDto>("calendar_collections_upsert", { request }),
  calendarCollectionsSetDefault: (id: string) =>
    call<CalendarCollectionDto>("calendar_collections_set_default", { id }),
  calendarCollectionsDelete: (id: string) =>
    call<void>("calendar_collections_delete", { id }),
  calendarEventsList: (from: number, to: number) =>
    call<CalendarEventDto[]>("calendar_events_list", { request: { from, to } }),
  calendarEventsUpsert: (request: {
    id?: string | null;
    calendarAccountId?: string | null;
    collectionId?: string | null;
    title: string;
    startsAt: number;
    endsAt?: number | null;
    location?: string | null;
    description?: string | null;
    allDay?: boolean;
    sourceMessageId?: string | null;
    reminders?: CalendarReminderDto[];
    status?: string | null;
  }) => call<CalendarEventDto>("calendar_events_upsert", { request }),
  calendarEventsDelete: (id: string) =>
    call<void>("calendar_events_delete", { id }),
  calendarTasksList: (includeCompleted = true) =>
    call<CalendarTaskDto[]>("calendar_tasks_list", { includeCompleted }),
  calendarTasksUpsert: (request: {
    id?: string | null;
    calendarAccountId?: string | null;
    title: string;
    dueAt?: number | null;
    completed?: boolean;
    notes?: string;
    sourceMessageId?: string | null;
  }) => call<CalendarTaskDto>("calendar_tasks_upsert", { request }),
  calendarTasksDelete: (id: string) =>
    call<void>("calendar_tasks_delete", { id }),
  calendarEventFromMessage: (
    messageId: string,
    startsAt: number,
    endsAt?: number | null,
    calendarAccountId?: string | null,
  ) =>
    call<CalendarEventDto>("calendar_event_from_message", {
      messageId,
      startsAt,
      endsAt: endsAt ?? null,
      calendarAccountId: calendarAccountId ?? null,
    }),
  calendarTaskFromMessage: (messageId: string, dueAt?: number | null) =>
    call<CalendarTaskDto>("calendar_task_from_message", {
      messageId,
      dueAt: dueAt ?? null,
    }),
  calendarInvitationsList: (pendingOnly = true) =>
    call<CalendarInvitationDto[]>("calendar_invitations_list", {
      pendingOnly,
    }),
  calendarInvitationsRespond: (id: string, response: InvitationResponse) =>
    call<CalendarInvitationDto>("calendar_invitations_respond", {
      request: { id, response },
    }),
  appVersion: () => call<AppVersionInfo>("app_version"),
  logsPath: () => call<string>("logs_path"),
  shellGetPrefs: () =>
    call<{ closeToTray: boolean; autostart: boolean; uiLocale: string }>(
      "shell_get_prefs",
    ),
  shellSetCloseToTray: (enabled: boolean) =>
    call<{ closeToTray: boolean; autostart: boolean; uiLocale: string }>(
      "shell_set_close_to_tray",
      { enabled },
    ),
  shellSetAutostart: (enabled: boolean) =>
    call<{ closeToTray: boolean; autostart: boolean; uiLocale: string }>(
      "shell_set_autostart",
      { enabled },
    ),
  shellSetUiLocale: (locale: string) =>
    call<{ closeToTray: boolean; autostart: boolean; uiLocale: string }>(
      "shell_set_ui_locale",
      { locale },
    ),
  updatesCheck: () => call<UpdateCheckResult>("updates_check"),
  updatesDownload: () => call<UpdateActionResult>("updates_download"),
  updatesInstall: () => call<UpdateActionResult>("updates_install"),
  onUpdateStatus: async (
    handler: (event: UpdateStatusEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<UpdateStatusEvent>("update://status", (event) => {
      handler(event.payload);
    });
  },
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
  onMailNew: async (handler: (count: number) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<number>("mail://new", (event) => {
      handler(event.payload);
    });
  },
  onSyncCycle: async (handler: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => undefined;
    return listen<boolean>("sync://cycle", () => {
      handler();
    });
  },
};

export function isDesktopShell(): boolean {
  return isTauri();
}
