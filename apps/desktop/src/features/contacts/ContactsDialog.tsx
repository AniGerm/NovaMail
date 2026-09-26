import { useEffect, useMemo, useState } from "react";
import {
  ArrowLeft,
  Camera,
  Plus,
  Settings2,
  Trash2,
  UserRound,
} from "lucide-react";
import { Button, Dialog, DialogActions, IconButton, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AppError,
  CardDavServerStatus,
  ContactAddress,
  ContactCustomField,
  ContactDto,
  ContactNameOrder,
  ContactSortBy,
  ContactsBookSettings,
  UpsertContactRequest,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

type Panel = "main" | "settings";

type Draft = {
  id?: string | null;
  givenName: string;
  familyName: string;
  displayName: string;
  emails: string;
  phones: string;
  faxes: string;
  organization: string;
  jobTitle: string;
  notes: string;
  addresses: ContactAddress[];
  customFields: ContactCustomField[];
  photoBase64: string | null;
  ldapDn: string | null;
};

const emptyAddress = (): ContactAddress => ({
  label: "WORK",
  street: "",
  city: "",
  region: "",
  postalCode: "",
  country: "",
});

const emptyDraft = (): Draft => ({
  id: null,
  givenName: "",
  familyName: "",
  displayName: "",
  emails: "",
  phones: "",
  faxes: "",
  organization: "",
  jobTitle: "",
  notes: "",
  addresses: [emptyAddress()],
  customFields: [],
  photoBase64: null,
  ldapDn: null,
});

const defaultBookSettings = (): ContactsBookSettings => ({
  nameOrder: "givenFamily",
  sortBy: "familyName",
  sortAscending: true,
});

function formatContactName(
  contact: Pick<ContactDto, "displayName" | "givenName" | "familyName">,
  order: ContactNameOrder,
): string {
  const given = (contact.givenName ?? "").trim();
  const family = (contact.familyName ?? "").trim();
  if (given || family) {
    if (order === "familyGiven") {
      if (family && given) return `${family}, ${given}`;
      return family || given;
    }
    return [given, family].filter(Boolean).join(" ");
  }
  return contact.displayName || "";
}

function buildDisplayName(
  givenName: string,
  familyName: string,
  fallback: string,
  order: ContactNameOrder,
): string {
  return (
    formatContactName(
      { displayName: fallback, givenName, familyName },
      order,
    ) || fallback
  );
}

function contactToDraft(contact: ContactDto): Draft {
  return {
    id: contact.id,
    givenName: contact.givenName ?? "",
    familyName: contact.familyName ?? "",
    displayName: contact.displayName,
    emails: contact.emails.join(", "),
    phones: contact.phones.join(", "),
    faxes: (contact.faxes ?? []).join(", "),
    organization: contact.organization ?? "",
    jobTitle: contact.jobTitle ?? "",
    notes: contact.notes ?? "",
    addresses:
      contact.addresses?.length > 0 ? contact.addresses : [emptyAddress()],
    customFields: contact.customFields ?? [],
    photoBase64: contact.photoBase64 ?? null,
    ldapDn: contact.ldapDn ?? null,
  };
}

function splitCsv(value: string): string[] {
  return value
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean);
}

function draftToRequest(
  draft: Draft,
  fallbackName: string,
  order: ContactNameOrder,
): UpsertContactRequest {
  const displayName =
    draft.displayName.trim() ||
    buildDisplayName(draft.givenName, draft.familyName, fallbackName, order);
  return {
    id: draft.id,
    displayName,
    givenName: draft.givenName,
    familyName: draft.familyName,
    emails: splitCsv(draft.emails),
    phones: splitCsv(draft.phones),
    faxes: splitCsv(draft.faxes),
    organization: draft.organization,
    jobTitle: draft.jobTitle,
    addresses: draft.addresses.filter(
      (a) =>
        a.street || a.city || a.region || a.postalCode || a.country || a.label,
    ),
    customFields: draft.customFields.filter((f) => f.label || f.value),
    photoBase64: draft.photoBase64,
    ldapDn: draft.ldapDn,
    notes: draft.notes,
  };
}

function sortKey(contact: ContactDto, sortBy: ContactSortBy): string {
  switch (sortBy) {
    case "givenName":
      return (contact.givenName || contact.displayName).toLocaleLowerCase();
    case "displayName":
      return contact.displayName.toLocaleLowerCase();
    case "organization":
      return (contact.organization || contact.displayName).toLocaleLowerCase();
    case "familyName":
    default:
      return (contact.familyName || contact.displayName).toLocaleLowerCase();
  }
}

export function ContactsDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const t = useT();
  const [panel, setPanel] = useState<Panel>("main");
  const [contacts, setContacts] = useState<ContactDto[]>([]);
  const [bookSettings, setBookSettings] =
    useState<ContactsBookSettings>(defaultBookSettings);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [carddav, setCarddav] = useState<CardDavServerStatus | null>(null);
  const [ldapUrl, setLdapUrl] = useState("ldaps://ldap.example.com");
  const [ldapBase, setLdapBase] = useState("ou=people,dc=example,dc=com");
  const [ldapFilter, setLdapFilter] = useState("(objectClass=inetOrgPerson)");
  const [ldapBind, setLdapBind] = useState("");
  const [ldapPassword, setLdapPassword] = useState("");
  const [statusInfo, setStatusInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<{
    id: string;
    name: string;
  } | null>(null);

  async function refresh(nextQuery = query) {
    const [list, status, ldap, book] = await Promise.all([
      api.contactsList(nextQuery || null),
      api.carddavStatus(),
      api.ldapGetSettings().catch(() => null),
      api.contactsBookSettings().catch(() => defaultBookSettings()),
    ]);
    setContacts(list);
    setCarddav(status);
    setBookSettings(book);
    if (ldap) {
      setLdapUrl(ldap.url || ldapUrl);
      setLdapBase(ldap.baseDn || ldapBase);
      setLdapFilter(ldap.filter || ldapFilter);
      setLdapBind(ldap.bindDn || "");
    }
  }

  useEffect(() => {
    if (!open) return;
    setPanel("main");
    refresh().catch((err) => setError((err as AppError).message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, query]);

  const selected = useMemo(
    () => contacts.find((c) => c.id === selectedId) ?? null,
    [contacts, selectedId],
  );

  const sortedContacts = useMemo(() => {
    const items = [...contacts];
    items.sort((a, b) => {
      const cmp = sortKey(a, bookSettings.sortBy).localeCompare(
        sortKey(b, bookSettings.sortBy),
        undefined,
        { sensitivity: "base" },
      );
      return bookSettings.sortAscending ? cmp : -cmp;
    });
    return items;
  }, [bookSettings.sortAscending, bookSettings.sortBy, contacts]);

  function startNew() {
    setSelectedId(null);
    setDraft(emptyDraft());
    setError(null);
    setStatusInfo(null);
    setPanel("main");
  }

  function selectContact(contact: ContactDto) {
    setSelectedId(contact.id);
    setDraft(contactToDraft(contact));
    setError(null);
    setStatusInfo(null);
    setPanel("main");
  }

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const saved = await api.contactsUpsert(
        draftToRequest(draft, t("contactFallback"), bookSettings.nameOrder),
      );
      setSelectedId(saved.id);
      setDraft(contactToDraft(saved));
      await refresh();
      setStatusInfo(t("contactSaved"));
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  async function toggleCardDav() {
    setBusy(true);
    setError(null);
    try {
      const status = carddav?.running
        ? await api.carddavStop()
        : await api.carddavStart();
      setCarddav(status);
    } catch (err) {
      setError((err as AppError).message);
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
      await refresh();
      setStatusInfo(
        t("ldapSyncResult", {
          imported: result.imported,
          updated: result.updated,
          total: result.total,
        }),
      );
      if (!carddav?.running) {
        const status = await api.carddavStart();
        setCarddav(status);
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
      setStatusInfo(t("bookSettingsSaved"));
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  async function onPhotoSelected(file: File | null) {
    if (!file) return;
    if (file.size > 220_000) {
      setError(t("photoTooLarge"));
      return;
    }
    const buffer = await file.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    let binary = "";
    bytes.forEach((b) => {
      binary += String.fromCharCode(b);
    });
    setDraft((prev) => ({ ...prev, photoBase64: btoa(binary) }));
  }

  const description =
    panel === "settings"
      ? t("contactsSettingsHint")
      : t("addressBookDescription");

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={panel === "settings" ? t("contactsSettings") : t("addressBook")}
      description={description}
      className="max-w-5xl"
      headerActions={
        panel === "main" ? (
          <IconButton
            label={t("contactsSettings")}
            onClick={() => {
              setPanel("settings");
              setStatusInfo(null);
              setError(null);
            }}
          >
            <Settings2 size={18} />
          </IconButton>
        ) : (
          <IconButton
            label={t("backToContacts")}
            onClick={() => setPanel("main")}
          >
            <ArrowLeft size={18} />
          </IconButton>
        )
      }
    >
      {panel === "settings" ? (
        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pr-1 text-sm">
          <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
            <h3 className="font-medium">{t("nameOrder")}</h3>
            <div className="grid gap-2 md:grid-cols-2">
              <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                {t("nameOrder")}
                <select
                  value={bookSettings.nameOrder}
                  onChange={(e) =>
                    setBookSettings((prev) => ({
                      ...prev,
                      nameOrder: e.target.value as ContactNameOrder,
                    }))
                  }
                  className="h-9 rounded-[var(--nova-radius-sm)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-2 text-sm text-[var(--nova-ink)]"
                >
                  <option value="givenFamily">{t("nameOrderGivenFamily")}</option>
                  <option value="familyGiven">{t("nameOrderFamilyGiven")}</option>
                </select>
              </label>
              <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                {t("contactSortBy")}
                <select
                  value={bookSettings.sortBy}
                  onChange={(e) =>
                    setBookSettings((prev) => ({
                      ...prev,
                      sortBy: e.target.value as ContactSortBy,
                    }))
                  }
                  className="h-9 rounded-[var(--nova-radius-sm)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-2 text-sm text-[var(--nova-ink)]"
                >
                  <option value="familyName">{t("contactSortFamily")}</option>
                  <option value="givenName">{t("contactSortGiven")}</option>
                  <option value="displayName">{t("contactSortDisplay")}</option>
                  <option value="organization">
                    {t("contactSortOrganization")}
                  </option>
                </select>
              </label>
              <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)] md:col-span-2">
                {t("sortDirection")}
                <select
                  value={bookSettings.sortAscending ? "asc" : "desc"}
                  onChange={(e) =>
                    setBookSettings((prev) => ({
                      ...prev,
                      sortAscending: e.target.value === "asc",
                    }))
                  }
                  className="h-9 rounded-[var(--nova-radius-sm)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-2 text-sm text-[var(--nova-ink)]"
                >
                  <option value="asc">{t("contactSortAscending")}</option>
                  <option value="desc">{t("contactSortDescending")}</option>
                </select>
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

          <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
            <h3 className="font-medium">{t("cardDavServer")}</h3>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {carddav?.running
                ? t("cardDavRunning", {
                    url: carddav.addressbookUrl,
                    count: carddav.contactCount,
                  })
                : t("cardDavStopped")}
            </p>
            {carddav?.addressbookUrl ? (
              <Input
                readOnly
                value={carddav.addressbookUrl}
                onFocus={(e) => e.currentTarget.select()}
                aria-label="CardDAV URL"
              />
            ) : null}
            {carddav?.username ? (
              <label className="grid gap-1 text-xs">
                <span>{t("cardDavUsername")}</span>
                <Input
                  readOnly
                  value={carddav.username}
                  onFocus={(e) => e.currentTarget.select()}
                />
              </label>
            ) : null}
            {carddav?.password ? (
              <label className="grid gap-1 text-xs">
                <span>{t("cardDavPassword")}</span>
                <Input
                  readOnly
                  value={carddav.password}
                  onFocus={(e) => e.currentTarget.select()}
                />
              </label>
            ) : null}
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("cardDavAuthHint")}
            </p>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("cardDavDeviceHint")}
            </p>
            <Button type="button" size="sm" disabled={busy} onClick={toggleCardDav}>
              {carddav?.running ? t("stopCardDav") : t("startCardDav")}
            </Button>
          </section>

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
              onClick={runLdapSync}
            >
              {t("ldapSyncNow")}
            </Button>
          </section>

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
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 overflow-hidden text-sm lg:grid-cols-[240px_minmax(0,1fr)]">
          <aside className="flex min-h-0 flex-col gap-3 border-b border-[var(--nova-border)] pb-3 lg:border-b-0 lg:border-r lg:pr-3 lg:pb-0">
            <div className="flex items-center gap-2">
              <Input
                className="min-w-0 flex-1"
                placeholder={t("search")}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={startNew}
              >
                <Plus size={14} />
                {t("add")}
              </Button>
            </div>
            <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
              {sortedContacts.length === 0 ? (
                <li className="px-1 text-[var(--nova-ink-muted)]">
                  {t("noContactsYet")}
                </li>
              ) : (
                sortedContacts.map((contact) => {
                  const active = contact.id === selectedId;
                  const label = formatContactName(
                    contact,
                    bookSettings.nameOrder,
                  );
                  return (
                    <li key={contact.id}>
                      <button
                        type="button"
                        onClick={() => selectContact(contact)}
                        className={
                          active
                            ? "flex w-full items-center gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-2 py-2 text-left"
                            : "flex w-full items-center gap-2 rounded-[var(--nova-radius-sm)] px-2 py-2 text-left hover:bg-[var(--nova-surface-2)]"
                        }
                      >
                        <ContactAvatar
                          name={label}
                          photoBase64={contact.photoBase64}
                        />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate font-medium">
                            {label}
                          </span>
                          <span className="block truncate text-xs text-[var(--nova-ink-muted)]">
                            {contact.organization ||
                              contact.emails[0] ||
                              contact.phones[0] ||
                              contact.faxes?.[0] ||
                              "—"}
                          </span>
                        </span>
                      </button>
                    </li>
                  );
                })
              )}
            </ul>
          </aside>

          <div className="min-h-0 overflow-y-auto pr-1">
            <form
              onSubmit={handleSave}
              className="grid gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h3 className="font-medium">
                    {draft.id ? t("editContact") : t("addContact")}
                  </h3>
                  {selected?.ldapDn ? (
                    <p className="mt-1 text-xs text-[var(--nova-ink-muted)]">
                      LDAP: {selected.ldapDn}
                    </p>
                  ) : null}
                </div>
                <label className="flex cursor-pointer flex-col items-center gap-1">
                  <span className="relative flex h-16 w-16 items-center justify-center overflow-hidden rounded-full bg-[var(--nova-surface-2)]">
                    {draft.photoBase64 ? (
                      <img
                        src={`data:image/jpeg;base64,${draft.photoBase64}`}
                        alt=""
                        className="h-full w-full object-cover"
                      />
                    ) : (
                      <UserRound
                        className="text-[var(--nova-ink-muted)]"
                        size={28}
                      />
                    )}
                    <span className="absolute inset-x-0 bottom-0 flex justify-center bg-[rgba(0,0,0,0.45)] py-0.5 text-[10px] text-white">
                      <Camera size={12} />
                    </span>
                  </span>
                  <input
                    type="file"
                    accept="image/jpeg,image/png,image/webp"
                    className="sr-only"
                    onChange={(e) => {
                      void onPhotoSelected(e.target.files?.[0] ?? null);
                    }}
                  />
                  <span className="text-[10px] text-[var(--nova-ink-muted)]">
                    {t("contactPhoto")}
                  </span>
                </label>
              </div>

              <div className="grid gap-2 md:grid-cols-2">
                {bookSettings.nameOrder === "familyGiven" ? (
                  <>
                    <Input
                      placeholder={t("familyName")}
                      value={draft.familyName}
                      onChange={(e) =>
                        setDraft((prev) => ({
                          ...prev,
                          familyName: e.target.value,
                        }))
                      }
                    />
                    <Input
                      placeholder={t("givenName")}
                      value={draft.givenName}
                      onChange={(e) =>
                        setDraft((prev) => ({
                          ...prev,
                          givenName: e.target.value,
                        }))
                      }
                    />
                  </>
                ) : (
                  <>
                    <Input
                      placeholder={t("givenName")}
                      value={draft.givenName}
                      onChange={(e) =>
                        setDraft((prev) => ({
                          ...prev,
                          givenName: e.target.value,
                        }))
                      }
                    />
                    <Input
                      placeholder={t("familyName")}
                      value={draft.familyName}
                      onChange={(e) =>
                        setDraft((prev) => ({
                          ...prev,
                          familyName: e.target.value,
                        }))
                      }
                    />
                  </>
                )}
                <Input
                  placeholder={t("organization")}
                  value={draft.organization}
                  onChange={(e) =>
                    setDraft((prev) => ({
                      ...prev,
                      organization: e.target.value,
                    }))
                  }
                />
                <Input
                  placeholder={t("jobTitle")}
                  value={draft.jobTitle}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, jobTitle: e.target.value }))
                  }
                />
                <Input
                  placeholder={t("emailsComma")}
                  value={draft.emails}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, emails: e.target.value }))
                  }
                />
                <Input
                  placeholder={t("phonesComma")}
                  value={draft.phones}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, phones: e.target.value }))
                  }
                />
                <Input
                  placeholder={t("faxesComma")}
                  value={draft.faxes}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, faxes: e.target.value }))
                  }
                  className="md:col-span-2"
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <h4 className="text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
                    {t("address")}
                  </h4>
                  <button
                    type="button"
                    className="text-xs text-[var(--nova-accent)]"
                    onClick={() =>
                      setDraft((prev) => ({
                        ...prev,
                        addresses: [...prev.addresses, emptyAddress()],
                      }))
                    }
                  >
                    {t("addAddress")}
                  </button>
                </div>
                {draft.addresses.map((address, index) => (
                  <div
                    key={`addr-${index}`}
                    className="grid gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] p-2 md:grid-cols-2"
                  >
                    <Input
                      placeholder={t("addressLabel")}
                      value={address.label}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            label: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                    <Input
                      placeholder={t("street")}
                      value={address.street}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            street: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                    <Input
                      placeholder={t("postalCode")}
                      value={address.postalCode}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            postalCode: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                    <Input
                      placeholder={t("city")}
                      value={address.city}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            city: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                    <Input
                      placeholder={t("region")}
                      value={address.region}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            region: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                    <Input
                      placeholder={t("country")}
                      value={address.country}
                      onChange={(e) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            country: e.target.value,
                          };
                          return { ...prev, addresses };
                        })
                      }
                    />
                  </div>
                ))}
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <h4 className="text-xs font-semibold uppercase tracking-[0.08em] text-[var(--nova-ink-muted)]">
                    {t("customFields")}
                  </h4>
                  <button
                    type="button"
                    className="text-xs text-[var(--nova-accent)]"
                    onClick={() =>
                      setDraft((prev) => ({
                        ...prev,
                        customFields: [
                          ...prev.customFields,
                          { label: "", value: "" },
                        ],
                      }))
                    }
                  >
                    {t("addCustomField")}
                  </button>
                </div>
                {draft.customFields.length === 0 ? (
                  <p className="text-xs text-[var(--nova-ink-muted)]">
                    {t("customFieldsHint")}
                  </p>
                ) : (
                  draft.customFields.map((field, index) => (
                    <div key={`cf-${index}`} className="flex gap-2">
                      <Input
                        placeholder={t("customFieldLabel")}
                        value={field.label}
                        onChange={(e) =>
                          setDraft((prev) => {
                            const customFields = [...prev.customFields];
                            customFields[index] = {
                              ...customFields[index],
                              label: e.target.value,
                            };
                            return { ...prev, customFields };
                          })
                        }
                      />
                      <Input
                        placeholder={t("customFieldValue")}
                        value={field.value}
                        onChange={(e) =>
                          setDraft((prev) => {
                            const customFields = [...prev.customFields];
                            customFields[index] = {
                              ...customFields[index],
                              value: e.target.value,
                            };
                            return { ...prev, customFields };
                          })
                        }
                      />
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        onClick={() =>
                          setDraft((prev) => ({
                            ...prev,
                            customFields: prev.customFields.filter(
                              (_, i) => i !== index,
                            ),
                          }))
                        }
                      >
                        <Trash2 size={14} />
                      </Button>
                    </div>
                  ))
                )}
              </div>

              <Input
                placeholder={t("notes")}
                value={draft.notes}
                onChange={(e) =>
                  setDraft((prev) => ({ ...prev, notes: e.target.value }))
                }
              />

              <div className="flex flex-wrap gap-2">
                <Button type="submit" size="sm" disabled={busy}>
                  {t("saveContact")}
                </Button>
                {draft.id ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      const name =
                        formatContactName(
                          {
                            displayName: draft.displayName,
                            givenName: draft.givenName,
                            familyName: draft.familyName,
                          },
                          bookSettings.nameOrder,
                        ) || t("contactFallback");
                      setPendingDelete({ id: draft.id!, name });
                      setError(null);
                      setStatusInfo(null);
                    }}
                  >
                    {t("delete")}
                  </Button>
                ) : null}
              </div>
            </form>

            {statusInfo ? (
              <p className="mt-3 text-[var(--nova-accent)]" role="status">
                {statusInfo}
              </p>
            ) : null}
            {error ? (
              <p className="mt-3 text-[var(--nova-danger)]" role="alert">
                {error}
              </p>
            ) : null}
          </div>
        </div>
      )}

      {pendingDelete ? (
        <div
          className="absolute inset-0 z-10 flex items-center justify-center bg-[rgba(14,17,20,0.45)] p-4 backdrop-blur-sm"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="contact-delete-title"
        >
          <div className="w-full max-w-sm rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-5 shadow-[var(--nova-shadow)]">
            <h3
              id="contact-delete-title"
              className="font-[family-name:var(--nova-font-display)] text-lg"
            >
              {t("confirmDeleteContactTitle")}
            </h3>
            <p className="mt-2 text-sm text-[var(--nova-ink-muted)]">
              {t("confirmDeleteContactBody", { name: pendingDelete.name })}
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => setPendingDelete(null)}
              >
                {t("cancel")}
              </Button>
              <Button
                type="button"
                size="sm"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  setError(null);
                  try {
                    await api.contactsDelete(pendingDelete.id);
                    setPendingDelete(null);
                    startNew();
                    await refresh();
                    setStatusInfo(t("contactDeleted"));
                  } catch (err) {
                    setError((err as AppError).message);
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                {t("delete")}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      <DialogActions>
        <Button onClick={onClose}>{t("done")}</Button>
      </DialogActions>
    </Dialog>
  );
}

function ContactAvatar({
  name,
  photoBase64,
}: {
  name: string;
  photoBase64?: string | null;
}) {
  if (photoBase64) {
    return (
      <img
        src={`data:image/jpeg;base64,${photoBase64}`}
        alt=""
        className="h-8 w-8 shrink-0 rounded-full object-cover"
      />
    );
  }
  const initial = name.trim().charAt(0).toUpperCase() || "?";
  return (
    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[var(--nova-accent-soft)] text-xs font-semibold text-[var(--nova-accent)]">
      {initial}
    </span>
  );
}
