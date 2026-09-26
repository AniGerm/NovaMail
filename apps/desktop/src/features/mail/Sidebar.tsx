import { Inbox, PenSquare, RefreshCw, Settings2, Moon, Sun } from "lucide-react";
import { Badge, Button, IconButton } from "@novamail/ui";

import type { AccountDto } from "@/shared/api/types";

interface SidebarProps {
  accounts: AccountDto[];
  syncStatus: string | null;
  themeDark: boolean;
  onCompose: () => void;
  onSync: () => void;
  onAddAccount: () => void;
  onToggleTheme: () => void;
  onOpenSettings: () => void;
}

export function Sidebar({
  accounts,
  syncStatus,
  themeDark,
  onCompose,
  onSync,
  onAddAccount,
  onToggleTheme,
  onOpenSettings,
}: SidebarProps) {
  return (
    <aside
      aria-label="Navigation"
      className="nova-slide-in flex h-full w-[240px] shrink-0 flex-col border-r border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_70%,transparent)] backdrop-blur-[var(--nova-blur)]"
    >
      <div className="px-4 pb-2 pt-5">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <p className="font-[family-name:var(--nova-font-display)] text-xl tracking-tight">
              NovaMail
            </p>
            <p className="text-xs text-[var(--nova-ink-muted)]">Local-first mail</p>
          </div>
          <IconButton
            label={themeDark ? "Switch to light mode" : "Switch to dark mode"}
            onClick={onToggleTheme}
          >
            {themeDark ? <Sun size={18} /> : <Moon size={18} />}
          </IconButton>
        </div>
        <Button className="w-full" onClick={onCompose}>
          <PenSquare size={16} />
          Compose
        </Button>
      </div>

      <nav className="mt-4 flex flex-1 flex-col gap-1 px-2">
        <button
          type="button"
          className="flex h-11 items-center gap-3 rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 text-left text-sm font-medium text-[var(--nova-accent)]"
        >
          <Inbox size={18} />
          Unified Inbox
        </button>

        <div className="mt-6 px-2">
          <div className="mb-2 flex items-center justify-between">
            <p className="text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
              Accounts
            </p>
            <button
              type="button"
              className="text-xs text-[var(--nova-accent)]"
              onClick={onAddAccount}
            >
              Add
            </button>
          </div>
          <ul className="space-y-1">
            {accounts.length === 0 ? (
              <li className="px-1 text-sm text-[var(--nova-ink-muted)]">
                No accounts yet
              </li>
            ) : (
              accounts.map((account) => (
                <li
                  key={account.id}
                  className="flex items-center justify-between rounded-[var(--nova-radius-sm)] px-2 py-2 text-sm"
                >
                  <span className="truncate">{account.name}</span>
                  <Badge>{account.provider}</Badge>
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
            Sync
          </Button>
          <IconButton label="Settings" onClick={onOpenSettings}>
            <Settings2 size={18} />
          </IconButton>
        </div>
      </div>
    </aside>
  );
}
