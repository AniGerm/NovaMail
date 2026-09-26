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

export interface ListMessagesRequest {
  mailboxId?: string | null;
  accountId?: string | null;
  unified: boolean;
  limit: number;
  offset: number;
  query?: string | null;
}

export interface ListMessagesResponse {
  messages: MessageSummaryDto[];
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

export interface ContactDto {
  id: string;
  displayName: string;
  emails: string[];
  phones: string[];
  notes: string;
  updatedAt: number;
}

export interface UpsertContactRequest {
  id?: string | null;
  displayName: string;
  emails: string[];
  phones: string[];
  notes: string;
}

export interface CardDavServerStatus {
  running: boolean;
  listenUrl: string;
  addressbookUrl: string;
  contactCount: number;
}

export interface LdapSearchRequest {
  url: string;
  bindDn?: string | null;
  password?: string | null;
  baseDn: string;
  filter: string;
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

export interface SignatureDto {
  id: string;
  accountId?: string | null;
  name: string;
  bodyText: string;
  isDefault: boolean;
}
