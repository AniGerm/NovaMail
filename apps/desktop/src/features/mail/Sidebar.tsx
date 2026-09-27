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
  ShieldAlert,
  Zap,
} from "lucide-react";
import { Badge, Button, IconButton } from "@novamail/ui";

import type { AccountDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import type { ThemeMode } from "@/shared/theme/resolveTheme";

interface SidebarProps {
  accounts: AccountDto[];
  selectedAccountId: string | null;
  draftsSelected?: boolean;
  spamSelected?: boolean;
  offlineSelected?: boolean;
  plannedSelected?: boolean;
  plannedCount?: number;
  syncStatus: string | null;
  themeMode: ThemeMode;
  onSelectUnified: () => void;
  onSelectAccount: (accountId: string) => void;
  onSelectDrafts: () => void;
  onSelectSpam: () => void;
  onSelectOffline: () => void;
  onSelectPlanned: () => void;
  onCompose: () => void;
  onSync: () => void;
  onAddAccount: () => void;
  onToggleTheme: () => void;
  onOpenSettings: () => void;
  onOpenContacts: () => void;
  onOpenTriage: () => void;
}

export function Sidebar({
  accounts,
  selectedAccountId,
  draftsSelected = false,
  spamSelected = false,
  offlineSelected = false,
  plannedSelected = false,
  plannedCount = 0,
  syncStatus,
  themeMode,
  onSelectUnified,
  onSelectAccount,
  onSelectDrafts,
  onSelectSpam,
  onSelectOffline,
  onSelectPlanned,
  onCompose,
  onSync,
  onAddAccount,
  onToggleTheme,
  onOpenSettings,
  onOpenContacts,
  onOpenTriage,
}: SidebarProps) {
  const t = useT();
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

  return (
    <aside
      aria-label={t("navigation")}
      className="nova-slide-in flex h-full w-[240px] shrink-0 flex-col border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_70%,transparent)] backdrop-blur-[var(--nova-blur)]"
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

      <nav className="mt-4 flex flex-1 flex-col gap-1 px-2">
        <button
          type="button"
          onClick={onSelectUnified}
          className={
            selectedAccountId === null &&
            !draftsSelected &&
            !spamSelected &&
            !offlineSelected &&
            !plannedSelected
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

        <div className="mt-6 px-2">
          <div className="mb-2 flex items-center justify-between">
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
              accounts.map((account) => (
                <li key={account.id}>
                  <button
                    type="button"
                    onClick={() => onSelectAccount(account.id)}
                    className={
                      selectedAccountId === account.id &&
                      !draftsSelected &&
                      !spamSelected &&
                      !offlineSelected &&
                      !plannedSelected
                        ? "flex w-full items-center justify-between rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-2 py-2 text-left text-sm text-[var(--nova-accent)]"
                        : "flex w-full items-center justify-between rounded-[var(--nova-radius-sm)] px-2 py-2 text-left text-sm hover:bg-[var(--nova-surface-2)]"
                    }
                  >
                    <span className="truncate">{account.name}</span>
                    <Badge>{account.provider}</Badge>
                  </button>
                </li>
              ))
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
