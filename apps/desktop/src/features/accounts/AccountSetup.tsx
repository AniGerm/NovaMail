import { useEffect, useMemo, useState } from "react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { Button, Input, Select } from "@novamail/ui";

import { api } from "@/shared/api/client";
import { addAccountPasswordSchema } from "@/shared/api/schemas";
import type {
  AddAccountPasswordRequest,
  AppError,
  MailProvider,
  ProviderPreset,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

interface AccountSetupProps {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}

export function AccountSetup({ open, onClose, onCreated }: AccountSetupProps) {
  const t = useT();
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [provider, setProvider] = useState<MailProvider>("generic");
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(465);
  const [imapTls, setImapTls] = useState(true);
  const [smtpTls, setSmtpTls] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    api
      .providerPresets()
      .then((items) => {
        setPresets(items);
        const generic = items.find((p) => p.provider === "generic") ?? items[0];
        if (generic) applyPreset(generic);
      })
      .catch(() => {
        const fallback: ProviderPreset = {
          provider: "generic",
          label: t("otherImapSmtp"),
          imapHost: "",
          imapPort: 993,
          imapTls: true,
          smtpHost: "",
          smtpPort: 465,
          smtpTls: true,
          authType: "password",
        };
        setPresets([fallback]);
        applyPreset(fallback);
      });
  }, [open, t]);

  const selected = useMemo(
    () => presets.find((p) => p.provider === provider),
    [presets, provider],
  );

  function applyPreset(preset: ProviderPreset) {
    setProvider(preset.provider);
    setImapHost(preset.imapHost);
    setImapPort(preset.imapPort);
    setImapTls(preset.imapTls);
    setSmtpHost(preset.smtpHost);
    setSmtpPort(preset.smtpPort);
    setSmtpTls(preset.smtpTls);
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    // Enter in the email field must not start a password IMAP login for OAuth presets.
    if (selected?.authType === "oauth2") {
      if (!email.trim() || busy) return;
      await handleOAuth();
      return;
    }
    setBusy(true);
    setError(null);
    const request: AddAccountPasswordRequest = {
      name: name || email,
      email,
      password,
      provider,
      imapHost,
      imapPort,
      imapTls,
      smtpHost,
      smtpPort,
      smtpTls,
    };
    const parsed = addAccountPasswordSchema.safeParse(request);
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? t("invalidAccount"));
      setBusy(false);
      return;
    }
    try {
      await api.accountsAddPassword(parsed.data);
      onCreated();
      onClose();
      setPassword("");
    } catch (err) {
      const appError = err as AppError;
      setError(appError.message || t("failedAddAccount"));
    } finally {
      setBusy(false);
    }
  }

  async function handleOAuth() {
    setBusy(true);
    setError(null);
    try {
      const url = await api.oauthAuthorizeUrl(provider);
      const wait = api.oauthWaitCallback(180);
      await openUrl(url);
      const callback = await wait;
      const tokens = await api.oauthExchangeCode(provider, callback.code);
      await api.accountsAddOAuth({
        name: name || email,
        email,
        provider,
        accessToken: tokens.tokens.accessToken,
        refreshToken: tokens.tokens.refreshToken ?? null,
        expiresAt: tokens.tokens.expiresAt ?? null,
        imapHost,
        imapPort,
        imapTls,
        smtpHost,
        smtpPort,
        smtpTls,
      });
      onCreated();
      onClose();
    } catch (err) {
      setError((err as AppError).message || t("oauthFailed"));
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[rgba(14,17,20,0.45)] p-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="account-setup-title"
    >
      <form
        onSubmit={handleSubmit}
        className="nova-fade-in w-full max-w-xl rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-6 shadow-[var(--nova-shadow)]"
      >
        <h2
          id="account-setup-title"
          className="font-[family-name:var(--nova-font-display)] text-2xl"
        >
          {t("addAccountTitle")}
        </h2>
        <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
          {t("addAccountDescription")}
        </p>

        <div className="mt-5 grid gap-3">
          <label className="grid gap-1 text-sm">
            <span>{t("provider")}</span>
            <Select
              className="h-11"
              value={provider}
              onChange={(e) => {
                const next = presets.find((p) => p.provider === e.target.value);
                if (next) applyPreset(next);
              }}
            >
              {presets.map((preset) => (
                <option key={preset.provider} value={preset.provider}>
                  {preset.label}
                </option>
              ))}
            </Select>
          </label>

          <label className="grid gap-1 text-sm">
            <span>{t("displayName")}</span>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </label>

          <label className="grid gap-1 text-sm">
            <span>{t("email")}</span>
            <Input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </label>

          {selected?.authType !== "oauth2" ? (
            <label className="grid gap-1 text-sm">
              <span>{t("passwordAppPassword")}</span>
              <Input
                type="password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </label>
          ) : null}

          {provider === "icloud" ? (
            <p className="rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 py-2 text-sm text-[var(--nova-ink)]">
              {t("icloudHint")}
            </p>
          ) : null}

          {provider === "yahoo" && selected?.authType !== "oauth2" ? (
            <p className="rounded-[var(--nova-radius-md)] bg-[var(--nova-accent-soft)] px-3 py-2 text-sm text-[var(--nova-ink)]">
              {t("yahooHint")}
            </p>
          ) : null}

          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-1 text-sm">
              <span>{t("imapHost")}</span>
              <Input
                required
                value={imapHost}
                onChange={(e) => setImapHost(e.target.value)}
              />
            </label>
            <label className="grid gap-1 text-sm">
              <span>{t("imapPort")}</span>
              <Input
                type="number"
                required
                value={imapPort}
                onChange={(e) => setImapPort(Number(e.target.value))}
              />
            </label>
            <label className="grid gap-1 text-sm">
              <span>{t("smtpHost")}</span>
              <Input
                required
                value={smtpHost}
                onChange={(e) => setSmtpHost(e.target.value)}
              />
            </label>
            <label className="grid gap-1 text-sm">
              <span>{t("smtpPort")}</span>
              <Input
                type="number"
                required
                value={smtpPort}
                onChange={(e) => setSmtpPort(Number(e.target.value))}
              />
            </label>
          </div>

          <div className="flex gap-4 text-sm text-[var(--nova-ink-muted)]">
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

          {error ? (
            <p className="text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        <div className="mt-6 flex flex-wrap justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("cancel")}
          </Button>
          {selected?.authType === "oauth2" ? (
            <Button type="button" disabled={busy || !email} onClick={handleOAuth}>
              {busy ? t("waitingBrowser") : t("signInOAuth")}
            </Button>
          ) : (
            <Button type="submit" disabled={busy}>
              {busy ? t("connecting") : t("connectSync")}
            </Button>
          )}
        </div>
      </form>
    </div>
  );
}
