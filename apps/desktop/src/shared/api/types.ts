export type AuthType = "password" | "oauth2";

export type MailProvider =
  | "generic"
  | "gmail"
  | "microsoft365"
  | "yahoo"
  | "protonBridge";

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
}
