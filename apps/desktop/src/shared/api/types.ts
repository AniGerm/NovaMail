export type AuthType = "password" | "oauth2";

export type MailProvider =
  | "generic"
  | "gmail"
  | "microsoft365"
  | "yahoo"
  | "protonBridge"
  | "icloud";

export interface AccountDto {
  id: string;
  name: string;
  email: string;
  provider: MailProvider;
  authType: AuthType;
  imapHost: string;
  imapPort: number;
  imapTls: boolean;
  smtpHost: string;
  smtpPort: number;
  smtpTls: boolean;
  createdAt: number;
}

export interface ProviderPreset {
  provider: MailProvider;
  label: string;
  imapHost: string;
  imapPort: number;
  imapTls: boolean;
  smtpHost: string;
  smtpPort: number;
  smtpTls: boolean;
  authType: AuthType;
  oauthAuthorizeUrl?: string | null;
}

export interface AddressDto {
  name?: string | null;
  email: string;
}

export interface AttachmentDto {
  id: string;
  messageId: string;
  filename: string;
  mime: string;
  size: number;
  path: string;
}

export interface OutgoingAttachment {
  filename: string;
  mime: string;
  dataBase64: string;
}

export interface MessageSummaryDto {
  id: string;
  accountId: string;
  mailboxId: string;
  threadId: string;
  subject: string;
  from: AddressDto;
  to: AddressDto[];
  date: number;
  snippet: string;
  unread: boolean;
  starred: boolean;
  hasAttachments: boolean;
  accountEmail: string;
  /** True when the message was offloaded from IMAP and exists only locally. */
  localOnly?: boolean;
  /** Unix seconds when a snooze ends; absent when not snoozed. */
  snoozedUntil?: number | null;
}

export interface MessageDetailDto {
  summary: MessageSummaryDto;
  bodyText?: string | null;
  bodyHtml?: string | null;
  messageId?: string | null;
  inReplyTo?: string | null;
  references: string[];
  attachments: AttachmentDto[];
}

export interface MailboxDto {
  id: string;
  accountId: string;
  name: string;
  role?: string | null;
  unreadCount: number;
  totalCount: number;
}

export type MessageSortBy = "date" | "subject" | "from" | "attachments";
export type SortDirection = "asc" | "desc";

export interface ListMessagesRequest {
  mailboxId?: string | null;
  accountId?: string | null;
  unified: boolean;
  mailboxRole?: string | null;
  /** When true, only messages offloaded from IMAP (`localOnly`). */
  localOnly?: boolean;
  /** When true, only currently snoozed messages. */
  snoozedOnly?: boolean;
  limit: number;
  offset: number;
  query?: string | null;
  unreadOnly?: boolean;
  starredOnly?: boolean;
  hasAttachments?: boolean;
  sortBy?: MessageSortBy;
  sortDir?: SortDirection;
}

export type OfflineMailboxMode = "off" | "threshold" | "overflow" | "alwaysPurge";

export type QuotaSource = "server" | "estimate" | "unknown";

export interface OfflineMailboxAccountPolicy {
  accountId: string;
  mode: OfflineMailboxMode;
  promptThresholdPercent: number;
  activeThresholdPercent: number;
  keepStarredOnImap: boolean;
  minAgeDays: number;
  batchLimit: number;
  promptDismissed?: boolean;
}

export interface OfflineMailboxSettingsDto {
  accounts: OfflineMailboxAccountPolicy[];
}

export interface AccountQuotaDto {
  accountId: string;
  usedBytes: number;
  limitBytes?: number | null;
  percent?: number | null;
  source: QuotaSource;
}

export interface OfflineOffloadReport {
  accountId: string;
  candidates: number;
  offloaded: number;
  skippedIncomplete: number;
  skippedStarred: number;
  freedBytes: number;
  errors: number;
}

export interface OfflinePromptEvent {
  accountId: string;
  accountEmail: string;
  percent: number;
  usedBytes: number;
  limitBytes?: number | null;
}

export interface ListMessagesResponse {
  messages: MessageSummaryDto[];
  total: number;
}

export interface ThreadListItemDto {
  id: string;
  accountId: string;
  subject: string;
  snippet: string;
  lastMessageAt: number;
  messageCount: number;
  unreadCount: number;
  hasAttachments: boolean;
  participants: AddressDto[];
  latestFrom: AddressDto;
  accountEmail: string;
}

export interface ListThreadsResponse {
  threads: ThreadListItemDto[];
  total: number;
}

export interface AddAccountPasswordRequest {
  name: string;
  email: string;
  password: string;
  provider: MailProvider;
  imapHost: string;
  imapPort: number;
  imapTls: boolean;
  smtpHost: string;
  smtpPort: number;
  smtpTls: boolean;
}

export interface SyncProgressEvent {
  accountId: string;
  mailboxName: string;
  fetched: number;
  totalEstimate?: number | null;
  done: boolean;
  error?: string | null;
}

export interface SyncResult {
  accountId: string;
  mailboxesSynced: number;
  messagesFetched: number;
}

export interface AppError {
  code: string;
  message: string;
  details?: string | null;
}

export interface SetFlagsRequest {
  messageId: string;
  unread?: boolean | null;
  starred?: boolean | null;
}

export interface SendMessageRequest {
  accountId: string;
  to: AddressDto[];
  cc: AddressDto[];
  bcc: AddressDto[];
  subject: string;
  bodyText: string;
  bodyHtml?: string | null;
  inReplyTo?: string | null;
  references: string[];
  attachments: OutgoingAttachment[];
  draftId?: string | null;
}

export type SnoozePreset =
  | "laterToday"
  | "tomorrowMorning"
  | "nextMonday"
  | "custom";

export interface SnoozeRequest {
  messageId: string;
  preset: SnoozePreset;
  wakeAt?: number | null;
}

export interface SnoozedMessageDto {
  messageId: string;
  accountId: string;
  wakeAt: number;
  subject: string;
  fromEmail: string;
  accountEmail: string;
}

export type OutboundStatus =
  | "pending"
  | "sending"
  | "sent"
  | "failed"
  | "cancelled";

export interface SendLaterRequest {
  sendAt: number;
  message: SendMessageRequest;
}

export interface OutboundQueueItemDto {
  id: string;
  accountId: string;
  accountEmail: string;
  subject: string;
  toSummary: string;
  sendAt: number;
  status: OutboundStatus;
  lastError?: string | null;
  createdAt: number;
}

export interface JobsTickReport {
  wokeSnoozes: number;
  sentLater: number;
  failedLater: number;
}

export interface PlannedSummaryDto {
  snoozedCount: number;
  outboundPendingCount: number;
}

export interface SaveDraftRequest {
  id?: string | null;
  accountId: string;
  to: AddressDto[];
  cc?: AddressDto[];
  subject: string;
  bodyText: string;
  bodyHtml?: string | null;
  inReplyTo?: string | null;
  references?: string[];
}

export interface SummarizeMessageResponse {
  messageId: string;
  summary: string;
  provider: string;
}

export interface SuggestReplyMessageResponse {
  messageId: string;
  suggestion: string;
  provider: string;
}

export interface ContactAddress {
  label: string;
  street: string;
  city: string;
  region: string;
  postalCode: string;
  country: string;
}

export interface ContactCustomField {
  label: string;
  value: string;
}

export interface ContactDto {
  id: string;
  displayName: string;
  givenName: string;
  familyName: string;
  emails: string[];
  phones: string[];
  faxes: string[];
  organization: string;
  jobTitle: string;
  addresses: ContactAddress[];
  customFields: ContactCustomField[];
  photoBase64?: string | null;
  ldapDn?: string | null;
  notes: string;
  updatedAt: number;
}

export interface RecipientSuggestion {
  email: string;
  name?: string | null;
  source: string;
  inContacts: boolean;
  contactId?: string | null;
}

export interface SpellDictionaryDto {
  code: string;
  name: string;
  installed: boolean;
  source: string;
}

export interface SpellcheckStatus {
  dictionaries: SpellDictionaryDto[];
  userDictDir: string;
}

export type ContactPrefill = {
  displayName?: string;
  givenName?: string;
  familyName?: string;
  emails?: string[];
  notes?: string;
};

export interface UpsertContactRequest {
  id?: string | null;
  displayName: string;
  givenName?: string;
  familyName?: string;
  emails: string[];
  phones: string[];
  faxes?: string[];
  organization?: string;
  jobTitle?: string;
  addresses?: ContactAddress[];
  customFields?: ContactCustomField[];
  photoBase64?: string | null;
  ldapDn?: string | null;
  notes: string;
}

export type ContactNameOrder = "givenFamily" | "familyGiven";
export type ContactSortBy =
  | "familyName"
  | "givenName"
  | "displayName"
  | "organization";

export interface ContactsBookSettings {
  nameOrder: ContactNameOrder;
  sortBy: ContactSortBy;
  sortAscending: boolean;
}

export interface CardDavServerStatus {
  running: boolean;
  listenUrl: string;
  addressbookUrl: string;
  contactCount: number;
  username: string;
  password: string;
}

export type ContactsShareMode = "local" | "server" | "client";

export interface LdapServerStatus {
  running: boolean;
  listenUrl: string;
  baseDn: string;
  bindDn: string;
  username: string;
  password: string;
  contactCount: number;
}

export interface ContactsShareStatus {
  mode: ContactsShareMode;
  carddav: CardDavServerStatus;
  ldapServer: LdapServerStatus;
  client?: LdapSyncSettings | null;
}

export interface SetContactsShareModeRequest {
  mode: ContactsShareMode;
  clientUrl?: string | null;
  clientBindDn?: string | null;
  clientPassword?: string | null;
  clientBaseDn?: string | null;
}

export interface LdapSearchRequest {
  url: string;
  bindDn?: string | null;
  password?: string | null;
  baseDn: string;
  filter: string;
}

export interface LdapSyncRequest {
  url: string;
  bindDn?: string | null;
  password?: string | null;
  baseDn: string;
  filter: string;
  saveSettings?: boolean;
}

export interface LdapSyncSettings {
  url: string;
  bindDn?: string | null;
  password?: string | null;
  baseDn: string;
  filter: string;
}

export interface LdapSyncResult {
  imported: number;
  updated: number;
  total: number;
}

export interface LabelDto {
  id: string;
  accountId: string;
  name: string;
  color: string;
}

export interface RuleDto {
  id: string;
  accountId?: string | null;
  name: string;
  enabled: boolean;
  predicateJson: string;
  actionJson: string;
}

export interface SpamSettingsDto {
  enabled: boolean;
  autoMove: boolean;
  threshold: number;
  /** Sonderzeichen, Schriftmischung, Obfuskation, URL-Dichte */
  strictHeuristics: boolean;
  trainedSpam: number;
  trainedHam: number;
}

export interface SpamScoreDto {
  messageId: string;
  score: number;
  isSpam: boolean;
  reasons: string[];
}

export type RetentionModeDto = "keep" | "deleteAfterDays";

export interface FolderPolicyDto {
  role: string;
  mode: RetentionModeDto;
  days: number;
}

export interface FolderPoliciesDto {
  policies: FolderPolicyDto[];
}

export interface SignatureDto {
  id: string;
  accountId?: string | null;
  name: string;
  bodyText: string;
  isDefault: boolean;
}

export interface MessageAiInsights {
  messageId: string;
  summary?: string | null;
  replyA?: string | null;
  replyB?: string | null;
  replySuggestion?: string | null;
  provider?: string | null;
}

export interface AiSettings {
  enabled: boolean;
  model: string;
  baseUrl: string;
  onboardingCompleted: boolean;
}

export interface AiRuntimeStatus {
  ollamaReachable: boolean;
  ollamaInstalled: boolean;
  ollamaBinary?: string | null;
  canInstallUser: boolean;
  canInstallSystem: boolean;
  nvidiaGpu: boolean;
  models: string[];
  recommendedModel: string;
  allowModelPick: boolean;
}

export interface AiInstallOllamaResponse {
  binaryPath?: string | null;
  reachable: boolean;
}

export interface AiInstallProgressEvent {
  status: string;
  done: boolean;
}

export interface AiPullModelResponse {
  model: string;
}

export interface AiPullProgressEvent {
  model: string;
  status: string;
  digest?: string | null;
  total?: number | null;
  completed?: number | null;
  done: boolean;
}

export interface SuggestRepliesMessageResponse {
  messageId: string;
  variants: string[];
  provider: string;
}

export interface ExportBackupRequest {
  passphrase: string;
}

export interface ExportBackupResponse {
  filename: string;
  dataBase64: string;
  accounts: number;
  contacts: number;
}

export interface ImportBackupRequest {
  passphrase: string;
  dataBase64: string;
}

export interface ImportBackupResult {
  accountsImported: number;
  accountsUpdated: number;
  contactsImported: number;
  contactsUpdated: number;
  contactsSkipped: number;
  labelsImported: number;
  rulesImported: number;
  signaturesImported: number;
}
