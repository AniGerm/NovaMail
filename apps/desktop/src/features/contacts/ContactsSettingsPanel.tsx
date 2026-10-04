import { useEffect, useState } from "react";
import { Button, Input, Select } from "@novamail/ui";

import { api, formatApiError } from "@/shared/api/client";
import type {
  AppError,
  ContactNameOrder,
  ContactSortBy,
  ContactsBookSettings,
  ContactsShareMode,
  ContactsShareStatus,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

const defaultBookSettings = (): ContactsBookSettings => ({
  nameOrder: "givenFamily",
  sortBy: "familyName",
  sortAscending: true,
});

function DirectoryListenStatus({
  share,
  busy,
  onRestart,
}: {
  share: ContactsShareStatus;
  busy: boolean;
  onRestart: () => void;
}) {
  const t = useT();
  const ldap = share.ldapServer;
  const card = share.carddav;
  const urls =
    ldap.running && ldap.listenUrls && ldap.listenUrls.length > 0
      ? ldap.listenUrls
      : ldap.running && ldap.listenUrl
        ? [ldap.listenUrl]
        : [];
  return (
    <div
      className="grid gap-1.5 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface-2)] p-3"
      role="status"
      data-ldap-running={ldap.running ? "yes" : "no"}
      data-carddav-running={card.running ? "yes" : "no"}
    >
      <p
        className={
          ldap.running
            ? "text-sm font-semibold text-[var(--nova-success)]"
            : "text-sm font-semibold text-[var(--nova-danger)]"
        }
      >
        {ldap.running ? t("ldapRunning") : t("ldapDown")}
      </p>
      {urls.map((url) => (
        <p key={url} className="break-all font-mono text-xs">
          {url}
        </p>
      ))}
      {!ldap.running && ldap.lastError ? (
        <p className="text-xs text-[var(--nova-danger)]">{ldap.lastError}</p>
      ) : null}
      <p
        className={
          card.running
            ? "text-sm font-semibold text-[var(--nova-success)]"
            : "text-sm font-semibold text-[var(--nova-danger)]"
        }
      >
        {card.running ? t("cardDavUp") : t("cardDavDown")}
        {card.running && card.addressbookUrl ? ` · ${card.addressbookUrl}` : ""}
      </p>
      {!card.running && card.lastError ? (
        <p className="text-xs text-[var(--nova-danger)]">{card.lastError}</p>
      ) : null}
      {!ldap.running || !card.running ? (
        <Button type="button" size="sm" disabled={busy} onClick={onRestart}>
          {t("directoryRestart")}
        </Button>
      ) : null}
    </div>
  );
}

export function ContactsSettingsPanel({
  onBookSettingsChange,
}: {
  onBookSettingsChange?: (settings: ContactsBookSettings) => void;
}) {
  const t = useT();
  const [bookSettings, setBookSettings] =
    useState<ContactsBookSettings>(defaultBookSettings);
  const [share, setShare] = useState<ContactsShareStatus | null>(null);
  const [clientUrl, setClientUrl] = useState("ldap://192.168.1.10:1389");
  const [clientBind, setClientBind] = useState("cn=novamail,dc=novamail");
  const [clientBase, setClientBase] = useState("ou=people,dc=novamail");
  const [clientPassword, setClientPassword] = useState("");
  const [ldapUrl, setLdapUrl] = useState("ldaps://ldap.example.com");
  const [ldapBase, setLdapBase] = useState("ou=people,dc=example,dc=com");
  const [ldapFilter, setLdapFilter] = useState("(objectClass=inetOrgPerson)");
  const [ldapBind, setLdapBind] = useState("");
  const [ldapPassword, setLdapPassword] = useState("");
  const [statusInfo, setStatusInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    const [shareStatus, ldap, book] = await Promise.all([
      api.contactsShareStatus().catch(() => null),
      api.ldapGetSettings().catch(() => null),
      api.contactsBookSettings().catch(() => defaultBookSettings()),
    ]);
    setBookSettings(book);
    onBookSettingsChange?.(book);
    if (shareStatus) {
      setShare(shareStatus);
      if (shareStatus.client?.url) setClientUrl(shareStatus.client.url);
      if (shareStatus.client?.bindDn) setClientBind(shareStatus.client.bindDn);
      if (shareStatus.client?.baseDn) setClientBase(shareStatus.client.baseDn);
      if (shareStatus.mode === "server" && shareStatus.ldapServer.listenUrl) {
        setClientUrl(shareStatus.ldapServer.listenUrl);
        setClientBind(shareStatus.ldapServer.bindDn);
        setClientBase(shareStatus.ldapServer.baseDn);
      }
    }
    if (ldap && shareStatus?.mode !== "client") {
      setLdapUrl(ldap.url || ldapUrl);
      setLdapBase(ldap.baseDn || ldapBase);
      setLdapFilter(ldap.filter || ldapFilter);
      setLdapBind(ldap.bindDn || "");
    }
  }

  useEffect(() => {
    refresh().catch((err) => setError((err as AppError).message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function setShareMode(mode: ContactsShareMode) {
    setShare((prev) =>
      prev
        ? { ...prev, mode }
        : {
            mode,
            carddav: {
              running: false,
              listenUrl: "",
              addressbookUrl: "",
              contactCount: 0,
              username: "",
              password: "",
            },
            ldapServer: {
              running: false,
              listenUrl: "",
              baseDn: clientBase,
              bindDn: clientBind,
              username: "",
              password: "",
              contactCount: 0,
            },
            client: null,
          },
    );
    setBusy(true);
    setError(null);
    setStatusInfo(null);
    try {
      const status = await api.contactsSetShareMode({
        mode,
        clientUrl: mode === "client" ? clientUrl : null,
        clientBindDn: mode === "client" ? clientBind : null,
        clientPassword: mode === "client" ? clientPassword || null : null,
        clientBaseDn: mode === "client" ? clientBase : null,
      });
      setShare(status);
      const ldapUp = Boolean(status.ldapServer?.running);
      const carddavUp = Boolean(status.carddav?.running);
      if (mode === "server" && !ldapUp) {
        setError(
          status.ldapServer?.lastError
            ? `${t("ldapDown")}: ${status.ldapServer.lastError}`
            : t("shareModeServicesDown"),
        );
      } else if (mode === "server" && !carddavUp) {
        setError(
          status.carddav?.lastError
            ? `${t("cardDavDown")}: ${status.carddav.lastError}`
            : t("shareModeServicesDown"),
        );
      }
    } catch (err) {
      setError(formatApiError(err, t("shareModeSaveFailed")));
      try {
        const latest = await api.contactsShareStatus();
        setShare(latest);
      } catch {
        setShare((prev) => (prev ? { ...prev, mode: "local" } : prev));
      }
    } finally {
      setBusy(false);
    }
  }

  async function runClientSync() {
    setBusy(true);
    setError(null);
    setStatusInfo(null);
    try {
      const status = await api.contactsSetShareMode({
        mode: "client",
        clientUrl,
        clientBindDn: clientBind,
        clientPassword: clientPassword || null,
        clientBaseDn: clientBase,
      });
      setShare(status);
      const result = await api.contactsClientSync();
      setStatusInfo(
        t("clientSyncResult", {
          imported: result.imported,
          updated: result.updated,
          total: result.total,
        }),
      );
    } catch (err) {
      setError(formatApiError(err, t("shareModeSaveFailed")));
    } finally {
      setBusy(false);
    }
  }

  async function runLdapSync() {
    setBusy(true);
    setError(null);
    setStatusInfo(null);
    try {
      const result = await api.ldapSync({
        url: ldapUrl,
        baseDn: ldapBase,
        filter: ldapFilter,
        bindDn: ldapBind || null,
        password: ldapPassword || null,
        saveSettings: true,
      });
      setStatusInfo(
        t("ldapSyncResult", {
          imported: result.imported,
          updated: result.updated,
          total: result.total,
        }),
      );
      if (share?.mode !== "server") {
        await setShareMode("server");
      }
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  async function saveBookSettings() {
    setBusy(true);
    setError(null);
    try {
      const saved = await api.contactsSetBookSettings(bookSettings);
      setBookSettings(saved);
      onBookSettingsChange?.(saved);
      setStatusInfo(t("bookSettingsSaved"));
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-4 text-sm">
  <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
    <h3 className="font-medium">{t("nameOrder")}</h3>
    <div className="grid gap-2 md:grid-cols-2">
      <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
        {t("nameOrder")}
        <Select
          value={bookSettings.nameOrder}
          onChange={(e) =>
            setBookSettings((prev) => ({
              ...prev,
              nameOrder: e.target.value as ContactNameOrder,
            }))
          }
          className="h-9 rounded-[var(--nova-radius-sm)] px-2 text-sm"
        >
          <option value="givenFamily">{t("nameOrderGivenFamily")}</option>
          <option value="familyGiven">{t("nameOrderFamilyGiven")}</option>
        </Select>
      </label>
      <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
        {t("contactSortBy")}
        <Select
          value={bookSettings.sortBy}
          onChange={(e) =>
            setBookSettings((prev) => ({
              ...prev,
              sortBy: e.target.value as ContactSortBy,
            }))
          }
          className="h-9 rounded-[var(--nova-radius-sm)] px-2 text-sm"
        >
          <option value="familyName">{t("contactSortFamily")}</option>
          <option value="givenName">{t("contactSortGiven")}</option>
          <option value="displayName">{t("contactSortDisplay")}</option>
          <option value="organization">
            {t("contactSortOrganization")}
          </option>
        </Select>
      </label>
      <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)] md:col-span-2">
        {t("sortDirection")}
        <Select
          value={bookSettings.sortAscending ? "asc" : "desc"}
          onChange={(e) =>
            setBookSettings((prev) => ({
              ...prev,
              sortAscending: e.target.value === "asc",
            }))
          }
          className="h-9 rounded-[var(--nova-radius-sm)] px-2 text-sm"
        >
          <option value="asc">{t("contactSortAscending")}</option>
          <option value="desc">{t("contactSortDescending")}</option>
        </Select>
      </label>
    </div>
    <Button
      type="button"
      size="sm"
      disabled={busy}
      onClick={() => {
        void saveBookSettings();
      }}
    >
      {t("saveBookSettings")}
    </Button>
  </section>

  <section className="grid gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
    <h3 className="font-medium">{t("cardDavServer")}</h3>

    <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
      {t("shareModeLabel")}
      <Select
        value={share?.mode ?? "local"}
        disabled={busy}
        onChange={(e) => {
          void setShareMode(e.target.value as ContactsShareMode);
        }}
        className="h-9 rounded-[var(--nova-radius-sm)] px-2 text-sm"
      >
        <option value="local">{t("shareModeLocal")}</option>
        <option value="server">{t("shareModeServer")}</option>
        <option value="client">{t("shareModeClient")}</option>
      </Select>
    </label>
    {error ? (
      <p className="text-xs text-[var(--nova-danger)]" role="alert">
        {error}
      </p>
    ) : null}
    {statusInfo ? (
      <p className="text-xs text-[var(--nova-accent)]" role="status">
        {statusInfo}
      </p>
    ) : null}

    {(share?.mode ?? "local") === "local" ? (
      <p className="text-xs text-[var(--nova-ink-muted)]">
        {t("shareModeLocalHint")}
      </p>
    ) : null}

    {share?.mode === "server" ? (
      <>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeServerHint")}
        </p>
        <DirectoryListenStatus
          share={share}
          busy={busy}
          onRestart={() => {
            void setShareMode("server");
          }}
        />
        <label className="grid gap-1 text-xs">
          <span>{t("ldapServerUrlLabel")}</span>
          <Input
            readOnly
            value={share.ldapServer.listenUrl}
            onFocus={(e) => e.currentTarget.select()}
          />
        </label>
        {(share.ldapServer.listenUrls?.length ?? 0) > 1 ? (
          <label className="grid gap-1 text-xs">
            <span>{t("ldapServerUrlsLabel")}</span>
            <Input
              readOnly
              value={(share.ldapServer.listenUrls ?? []).join(" · ")}
              onFocus={(e) => e.currentTarget.select()}
            />
          </label>
        ) : null}
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("ldapRicohHint")}
        </p>
        <div className="grid gap-2 md:grid-cols-2">
          <label className="grid gap-1 text-xs">
            <span>{t("ldapServerBindDn")}</span>
            <Input
              readOnly
              value={share.ldapServer.bindDn}
              onFocus={(e) => e.currentTarget.select()}
            />
          </label>
          <label className="grid gap-1 text-xs">
            <span>{t("ldapServerBaseDn")}</span>
            <Input
              readOnly
              value={share.ldapServer.baseDn}
              onFocus={(e) => e.currentTarget.select()}
            />
          </label>
        </div>
        <label className="grid gap-1 text-xs">
          <span>{t("cardDavUrlLabel")}</span>
          <Input
            readOnly
            value={share.carddav.addressbookUrl}
            onFocus={(e) => e.currentTarget.select()}
          />
        </label>
        <div className="grid gap-2 md:grid-cols-2">
          <label className="grid gap-1 text-xs">
            <span>{t("cardDavUsername")}</span>
            <Input
              readOnly
              value={share.carddav.username}
              onFocus={(e) => e.currentTarget.select()}
            />
          </label>
          <label className="grid gap-1 text-xs">
            <span>{t("cardDavPassword")}</span>
            <Input
              readOnly
              value={share.carddav.password}
              onFocus={(e) => e.currentTarget.select()}
            />
          </label>
        </div>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeServerSteps")}
        </p>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeTlsHint")}
        </p>
      </>
    ) : null}

    {share?.mode === "client" ? (
      <>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeClientHint")}
        </p>
        <label className="grid gap-1 text-xs">
          <span>{t("clientHubUrl")}</span>
          <Input
            value={clientUrl}
            onChange={(e) => setClientUrl(e.target.value)}
            placeholder={t("clientHubUrlPlaceholder")}
            disabled={busy}
          />
        </label>
        <div className="grid gap-2 md:grid-cols-2">
          <label className="grid gap-1 text-xs">
            <span>{t("ldapServerBindDn")}</span>
            <Input
              value={clientBind}
              onChange={(e) => setClientBind(e.target.value)}
              disabled={busy}
            />
          </label>
          <label className="grid gap-1 text-xs">
            <span>{t("ldapServerBaseDn")}</span>
            <Input
              value={clientBase}
              onChange={(e) => setClientBase(e.target.value)}
              disabled={busy}
            />
          </label>
        </div>
        <label className="grid gap-1 text-xs">
          <span>{t("cardDavPassword")}</span>
          <Input
            type="password"
            value={clientPassword}
            onChange={(e) => setClientPassword(e.target.value)}
            disabled={busy}
          />
        </label>
        <Button
          type="button"
          size="sm"
          disabled={busy}
          onClick={() => {
            void runClientSync();
          }}
        >
          {t("clientSyncNow")}
        </Button>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeClientSteps")}
        </p>
        <p className="text-xs text-[var(--nova-ink-muted)]">
          {t("shareModeTlsHint")}
        </p>
      </>
    ) : null}
  </section>

  {share?.mode === "server" ? (
    <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
      <h3 className="font-medium">{t("ldapSyncTitle")}</h3>
      <p className="text-xs text-[var(--nova-ink-muted)]">
        {t("ldapSyncDescription")}
      </p>
      <div className="grid gap-2 md:grid-cols-2">
        <Input
          value={ldapUrl}
          onChange={(e) => setLdapUrl(e.target.value)}
          placeholder={t("ldapUrl")}
        />
        <Input
          value={ldapBase}
          onChange={(e) => setLdapBase(e.target.value)}
          placeholder={t("ldapBaseDn")}
        />
        <Input
          value={ldapFilter}
          onChange={(e) => setLdapFilter(e.target.value)}
          placeholder={t("ldapFilter")}
        />
        <Input
          value={ldapBind}
          onChange={(e) => setLdapBind(e.target.value)}
          placeholder={t("ldapBindDn")}
        />
        <Input
          type="password"
          value={ldapPassword}
          onChange={(e) => setLdapPassword(e.target.value)}
          placeholder={t("ldapPassword")}
          className="md:col-span-2"
        />
      </div>
      <Button
        type="button"
        size="sm"
        variant="secondary"
        disabled={busy}
        onClick={() => {
          void runLdapSync();
        }}
      >
        {t("ldapSyncNow")}
      </Button>
    </section>
  ) : null}

  {statusInfo ? (
    <p className="text-[var(--nova-accent)]" role="status">
      {statusInfo}
    </p>
  ) : null}
  {error ? (
    <p className="text-[var(--nova-danger)]" role="alert">
      {error}
    </p>
  ) : null}
    </div>
  );
}
