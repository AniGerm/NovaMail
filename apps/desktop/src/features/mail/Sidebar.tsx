import { useEffect, useMemo, useState } from "react";
import {
  Inbox,
  PenSquare,
  RefreshCw,
  Settings2,
  Moon,
  Sun,
  Monitor,
  BookUser,
  FilePenLine,
  HardDrive,
  Clock3,
  CalendarDays,
  ShieldAlert,
  Zap,
  ChevronDown,
  ChevronRight,
  Send,
  Trash2,
  Folder,
} from "lucide-react";
import { Badge, Button, IconButton } from "@novamail/ui";

import type { AccountDto, MailboxDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import type { ThemeMode } from "@/shared/theme/resolveTheme";

interface SidebarProps {
  accounts: AccountDto[];
  mailboxes: MailboxDto[];
  selectedAccountId: string | null;
  selectedMailboxId: string | null;
  draftsSelected?: boolean;
  spamSelected?: boolean;
  offlineSelected?: boolean;
  plannedSelected?: boolean;
  plannedCount?: number;
  calendarSelected?: boolean;
  /** Pending invites + due/overdue tasks. */
  calendarCount?: number;
  syncStatus: string | null;
  themeMode: ThemeMode;
  onSelectUnified: () => void;
  onSelectAccount: (accountId: string) => void;
  onSelectMailbox: (accountId: string, mailboxId: string) => void;
  onSelectDrafts: () => void;
  onSelectSpam: () => void;
  onSelectOffline: () => void;
  onSelectPlanned: () => void;
  onSelectCalendar: () => void;
  onCompose: () => void;
  onSync: () => void;
  onAddAccount: () => void;
  onToggleTheme: () => void;
  onOpenSettings: () => void;
  onOpenContacts: () => void;
  onOpenTriage: () => void;
  /** Optional column width in px (resizable main layout). */
  width?: number;
}

const ROLE_ORDER = ["inbox", "sent", "drafts", "junk", "trash"];

function mailboxSortKey(mb: MailboxDto): [number, string] {
  const role = (mb.role ?? "").toLowerCase();
  const idx = ROLE_ORDER.indexOf(role);
  return [idx === -1 ? 100 : idx, mb.name.toLowerCase()];
}

function MailboxIcon({ role }: { role?: string | null }) {
  switch ((role ?? "").toLowerCase()) {
    case "inbox":
      return <Inbox size={15} />;
    case "sent":
      return <Send size={15} />;
    case "drafts":
      return <FilePenLine size={15} />;
    case "junk":
      return <ShieldAlert size={15} />;
    case "trash":
      return <Trash2 size={15} />;
    default:
      return <Folder size={15} />;
  }
}

export function Sidebar({
  accounts,
  mailboxes,
  selectedAccountId,
  selectedMailboxId,
  draftsSelected = false,
  spamSelected = false,
  offlineSelected = false,
  plannedSelected = false,
  plannedCount = 0,
  calendarSelected = false,
  calendarCount = 0,
  syncStatus,
  themeMode,
  onSelectUnified,
  onSelectAccount,
  onSelectMailbox,
  onSelectDrafts,
  onSelectSpam,
  onSelectOffline,
  onSelectPlanned,
  onSelectCalendar,
  onCompose,
  onSync,
  onAddAccount,
  onToggleTheme,
  onOpenSettings,
  onOpenContacts,
  onOpenTriage,
  width,
}: SidebarProps) {
  const t = useT();
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (!selectedAccountId) return;
    setExpanded((prev) =>
      prev[selectedAccountId] ? prev : { ...prev, [selectedAccountId]: true },
    );
  }, [selectedAccountId]);

  const mailboxesByAccount = useMemo(() => {
    const map = new Map<string, MailboxDto[]>();
    for (const mb of mailboxes) {
      const list = map.get(mb.accountId) ?? [];
      list.push(mb);
      map.set(mb.accountId, list);
    }
    for (const list of map.values()) {
      list.sort((a, b) => {
        const [ai, an] = mailboxSortKey(a);
        const [bi, bn] = mailboxSortKey(b);
        return ai - bi || an.localeCompare(bn);
      });
    }
    return map;
  }, [mailboxes]);

  function mailboxLabel(mb: MailboxDto): string {
    switch ((mb.role ?? "").toLowerCase()) {
      case "inbox":
        return t("folderInbox");
      case "sent":
        return t("folderSent");
      case "drafts":
        return t("drafts");
      case "junk":
        return t("spam");
      case "trash":
        return t("trash");
      default:
        return mb.name;
    }
  }

  function toggleAccount(accountId: string) {
    setExpanded((prev) => ({ ...prev, [accountId]: !prev[accountId] }));
  }

  function handleAccountClick(accountId: string) {
    setExpanded((prev) => ({ ...prev, [accountId]: true }));
    onSelectAccount(accountId);
  }

  const themeLabel =
    themeMode === "light"
      ? t("switchToDark")
      : themeMode === "dark"
        ? t("switchToAuto")
        : t("switchToLight");
  const themeIcon =
    themeMode === "light" ? (
      <Moon size={18} />
    ) : themeMode === "dark" ? (
      <Monitor size={18} />
    ) : (
      <Sun size={18} />
    );

  const specialView =
    draftsSelected ||
    spamSelected ||
    offlineSelected ||
    plannedSelected ||
    calendarSelected;

  return (
    <aside
      aria-label={t("navigation")}
      className="nova-slide-in flex h-full shrink-0 flex-col border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_70%,transparent)] backdrop-blur-[var(--nova-blur)]"
      style={{ width: width ?? 240 }}
    >
      <div className="px-4 pb-2 pt-5">
        <div className="mb-4 flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2.5">
            <img
              src="/novamail-mark.png"
              alt=""
              width={36}
              height={36}
              className="h-9 w-9 shrink-0 rounded-[10px] shadow-[0_1px_2px_color-mix(in_srgb,var(--nova-ink)_12%,transparent)]"
            />
            <div className="min-w-0">
              <p className="font-[family-name:var(--nova-font-display)] text-xl tracking-tight">
                {t("appName")}
              </p>
              <p className="text-xs text-[var(--nova-ink-muted)]">{t("tagline")}</p>
            </div>
          </div>
          <IconButton label={themeLabel} onClick={onToggleTheme}>
            {themeIcon}
          </IconButton>
        </div>
        <Button className="w-full" onClick={onCompose}>
          <PenSquare size={16} />
          {t("compose")}
        </Button>
      </div>

      <nav className="mt-4 flex flex-1 flex-col gap-1 overflow-y-auto px-2 pb-2">
        <button
          type="button"
          onClick={onSelectUnified}
          className={
            selectedAccountId === null && !specialView
              ? "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <Inbox size={18} />
          {t("unifiedInbox")}
        </button>
        <button
          type="button"
          onClick={onSelectDrafts}
          className={
            draftsSelected
              ? "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <FilePenLine size={18} />
          {t("drafts")}
        </button>
        <button
          type="button"
          onClick={onSelectSpam}
          className={
            spamSelected
              ? "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <ShieldAlert size={18} />
          {t("spam")}
        </button>
        <button
          type="button"
          onClick={onSelectOffline}
          className={
            offlineSelected
              ? "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <HardDrive size={18} />
          {t("offlineMailbox")}
        </button>
        <button
          type="button"
          onClick={onSelectPlanned}
          className={
            plannedSelected
              ? "flex h-11 items-center justify-between gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center justify-between gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <span className="flex items-center gap-3">
            <Clock3 size={18} />
            {t("planned")}
          </span>
          {plannedCount > 0 ? <Badge>{plannedCount}</Badge> : null}
        </button>
        <button
          type="button"
          onClick={onSelectCalendar}
          className={
            calendarSelected
              ? "flex h-11 items-center justify-between gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
              : "flex h-11 items-center justify-between gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          }
        >
          <span className="flex items-center gap-3">
            <CalendarDays size={18} />
            {t("calendar")}
          </span>
          {calendarCount > 0 ? <Badge>{calendarCount}</Badge> : null}
        </button>
        <button
          type="button"
          className="flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          onClick={onOpenTriage}
        >
          <Zap size={18} />
          {t("quickSort")}
        </button>
        <button
          type="button"
          className="flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] px-3 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
          onClick={onOpenContacts}
        >
          <BookUser size={18} />
          {t("contacts")}
        </button>

        <div className="mt-6 px-1">
          <div className="mb-2 flex items-center justify-between px-1">
            <p className="text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              {t("accounts")}
            </p>
            <button
              type="button"
              className="text-xs text-[var(--nova-accent)]"
              onClick={onAddAccount}
            >
              {t("add")}
            </button>
          </div>
          <ul className="space-y-1">
            {accounts.length === 0 ? (
              <li className="px-1 text-sm text-[var(--nova-ink-muted)]">
                {t("noAccountsYet")}
              </li>
            ) : (
              accounts.map((account) => {
                const isOpen = Boolean(expanded[account.id]);
                const accountMailboxes = mailboxesByAccount.get(account.id) ?? [];
                const accountActive =
                  selectedAccountId === account.id && !specialView;
                return (
                  <li key={account.id}>
                    <div className="flex items-stretch gap-0.5">
                      <button
                        type="button"
                        aria-label={
                          isOpen ? t("collapseFolders") : t("expandFolders")
                        }
                        aria-expanded={isOpen}
                        onClick={() => toggleAccount(account.id)}
                        className="flex h-9 w-7 shrink-0 items-center justify-center rounded-[var(--nova-radius-sm)] text-[var(--nova-ink-muted)] hover:bg-[var(--nova-surface-2)]"
                      >
                        {isOpen ? (
                          <ChevronDown size={16} />
                        ) : (
                          <ChevronRight size={16} />
                        )}
                      </button>
                      <button
                        type="button"
                        onClick={() => handleAccountClick(account.id)}
                        className={
                          accountActive && !selectedMailboxId
                            ? "flex min-w-0 flex-1 items-center justify-between rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-2 py-1.5 text-left text-sm text-[var(--nova-accent)]"
                            : "flex min-w-0 flex-1 items-center justify-between rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-left text-sm hover:bg-[var(--nova-surface-2)]"
                        }
                      >
                        <span className="truncate">
                          {(account.label || account.name).trim() ||
                            account.email}
                        </span>
                      </button>
                    </div>
                    {isOpen ? (
                      <ul className="mb-1 ml-3 mt-0.5 space-y-0.5 border-l border-[var(--nova-border)] pl-2">
                        {accountMailboxes.length === 0 ? (
                          <li className="px-2 py-1 text-xs text-[var(--nova-ink-muted)]">
                            {t("noFoldersYet")}
                          </li>
                        ) : (
                          accountMailboxes.map((mb) => {
                            const active =
                              accountActive && selectedMailboxId === mb.id;
                            return (
                              <li key={mb.id}>
                                <button
                                  type="button"
                                  onClick={() =>
                                    onSelectMailbox(account.id, mb.id)
                                  }
                                  className={
                                    active
                                      ? "flex w-full items-center justify-between gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-2 py-1.5 text-left text-xs font-medium text-[var(--nova-accent)]"
                                      : "flex w-full items-center justify-between gap-2 rounded-[var(--nova-radius-sm)] px-2 py-1.5 text-left text-xs hover:bg-[var(--nova-surface-2)]"
                                  }
                                >
                                  <span className="flex min-w-0 items-center gap-2">
                                    <MailboxIcon role={mb.role} />
                                    <span className="truncate">
                                      {mailboxLabel(mb)}
                                    </span>
                                  </span>
                                  {mb.unreadCount > 0 ? (
                                    <Badge>{mb.unreadCount}</Badge>
                                  ) : null}
                                </button>
                              </li>
                            );
                          })
                        )}
                      </ul>
                    ) : null}
                  </li>
                );
              })
            )}
          </ul>
        </div>
      </nav>

      <div className="space-y-2 border-t border-[var(--nova-border)] p-3">
        {syncStatus ? (
          <p className="px-1 text-xs text-[var(--nova-ink-muted)]" role="status">
            {syncStatus}
          </p>
        ) : null}
        <div className="flex gap-2">
          <Button variant="secondary" className="flex-1" onClick={onSync}>
            <RefreshCw size={16} />
            {t("sync")}
          </Button>
          <IconButton label={t("settings")} onClick={onOpenSettings}>
            <Settings2 size={18} />
          </IconButton>
        </div>
      </div>
    </aside>
  );
}
