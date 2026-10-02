import { useEffect, useState } from "react";
import { Button, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AccountDto, AppError, MailProvider } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

export function AccountsPanel({
  accounts,
  onChanged,
  onAddAccount,
}: {
  accounts: AccountDto[];
  onChanged: () => void;
  onAddAccount: () => void;
}) {
  const t = useT();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [label, setLabel] = useState("");
  const [email, setEmail] = useState("");
  const [provider, setProvider] = useState<MailProvider>("generic");
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [imapTls, setImapTls] = useState(true);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(465);
  const [smtpTls, setSmtpTls] = useState(true);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const editing = accounts.find((a) => a.id === editingId) ?? null;

  useEffect(() => {
    if (!editing) return;
    setName(editing.name);
    setLabel(editing.label ?? "");
    setEmail(editing.email);
    setProvider(editing.provider);
    setImapHost(editing.imapHost);
    setImapPort(editing.imapPort);
    setImapTls(editing.imapTls);
    setSmtpHost(editing.smtpHost);
    setSmtpPort(editing.smtpPort);
    setSmtpTls(editing.smtpTls);
    setPassword("");
    setError(null);
    setStatus(null);
  }, [editing]);

  function startEdit(account: AccountDto) {
    setEditingId(account.id);
  }

  function cancelEdit() {
    setEditingId(null);
    setPassword("");
    setError(null);
    setStatus(null);
  }

  async function handleSave() {
    if (!editingId) return;
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      await api.accountsUpdate({
        id: editingId,
        name: name.trim() || email.trim(),
        label: label.trim(),
        email: email.trim(),
        provider,
        imapHost: imapHost.trim(),
        imapPort,
        imapTls,
        smtpHost: smtpHost.trim(),
        smtpPort,
        smtpTls,
        password: password.trim() ? password : null,
      });
      setStatus(t("accountSaved"));
      setEditingId(null);
      setPassword("");
      onChanged();
    } catch (err) {
      setError((err as AppError).message || t("accountSaveFailed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleRemove(account: AccountDto) {
    if (!window.confirm(t("accountRemoveConfirm", { email: account.email }))) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.accountsRemove(account.id);
      if (editingId === account.id) cancelEdit();
      setStatus(t("accountRemoved"));
      onChanged();
    } catch (err) {
      setError((err as AppError).message || t("accountRemoveFailed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-medium">{t("settingsTabAccounts")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("accountsSettingsDescription")}
          </p>
        </div>
        <Button type="button" size="sm" variant="secondary" onClick={onAddAccount}>
          {t("addAccount")}
        </Button>
      </div>

      {accounts.length === 0 ? (
        <p className="text-sm text-[var(--nova-ink-muted)]">{t("noAccountsYet")}</p>
      ) : (
        <ul className="grid gap-2">
          {accounts.map((account) => (
            <li
              key={account.id}
              className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="min-w-0">
                  <p className="truncate font-medium">
                    {(account.label || account.name).trim() || account.email}
                  </p>
                  <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                    {account.email}
                    {account.label && account.label !== account.name
                      ? ` · ${account.name}`
                      : ""}{" "}
                    · {account.provider} · {account.authType}
                  </p>
                </div>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    disabled={busy}
                    onClick={() => startEdit(account)}
                  >
                    {t("editAccount")}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="danger"
                    disabled={busy}
                    onClick={() => void handleRemove(account)}
                  >
                    {t("removeAccount")}
                  </Button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}

      {editing ? (
        <form
          className="grid gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3"
          onSubmit={(e) => {
            e.preventDefault();
            void handleSave();
          }}
        >
          <h4 className="font-medium">
            {t("editAccountTitle", { email: editing.email })}
          </h4>
          <label className="grid gap-1">
            <span>{t("displayName")}</span>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="grid gap-1">
            <span>{t("sidebarName")}</span>
            <Input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={t("sidebarNameHint")}
            />
            <span className="text-xs text-[var(--nova-ink-muted)]">
              {t("sidebarNameHint")}
            </span>
          </label>
          <label className="grid gap-1">
            <span>{t("email")}</span>
            <Input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </label>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="grid gap-1">
              <span>{t("imapHost")}</span>
              <Input
                value={imapHost}
                onChange={(e) => setImapHost(e.target.value)}
                required
              />
            </label>
            <label className="grid gap-1">
              <span>{t("imapPort")}</span>
              <Input
                type="number"
                value={imapPort}
                onChange={(e) => setImapPort(Number(e.target.value) || 993)}
              />
            </label>
            <label className="grid gap-1">
              <span>{t("smtpHost")}</span>
              <Input
                value={smtpHost}
                onChange={(e) => setSmtpHost(e.target.value)}
                required
              />
            </label>
            <label className="grid gap-1">
              <span>{t("smtpPort")}</span>
              <Input
                type="number"
                value={smtpPort}
                onChange={(e) => setSmtpPort(Number(e.target.value) || 465)}
              />
            </label>
          </div>
          <div className="flex flex-wrap gap-4">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={imapTls}
                onChange={(e) => setImapTls(e.target.checked)}
              />
              {t("imapTls")}
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={smtpTls}
                onChange={(e) => setSmtpTls(e.target.checked)}
              />
              {t("smtpTls")}
            </label>
          </div>
          {editing.authType === "password" ? (
            <label className="grid gap-1">
              <span>{t("passwordOptional")}</span>
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t("passwordKeepPlaceholder")}
                autoComplete="new-password"
              />
              <span className="text-xs text-[var(--nova-ink-muted)]">
                {t("accountEditTestHint")}
              </span>
            </label>
          ) : (
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("accountOauthEditHint")}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            <Button type="submit" disabled={busy}>
              {busy ? t("working") : t("saveAccount")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              onClick={cancelEdit}
            >
              {t("cancel")}
            </Button>
          </div>
        </form>
      ) : null}

      {status ? (
        <p className="text-sm text-[var(--nova-accent)]" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="text-sm text-[var(--nova-danger)]" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
