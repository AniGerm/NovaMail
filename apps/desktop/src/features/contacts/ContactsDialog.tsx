import { useEffect, useMemo, useState } from "react";
import { Camera, Plus, Trash2, UserRound } from "lucide-react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AppError,
  CardDavServerStatus,
  ContactAddress,
  ContactCustomField,
  ContactDto,
  UpsertContactRequest,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

type Draft = {
  id?: string | null;
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

function contactToDraft(contact: ContactDto): Draft {
  return {
    id: contact.id,
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

function draftToRequest(draft: Draft, fallbackName: string): UpsertContactRequest {
  return {
    id: draft.id,
    displayName: draft.displayName || splitCsv(draft.emails)[0] || fallbackName,
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

export function ContactsDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const t = useT();
  const [contacts, setContacts] = useState<ContactDto[]>([]);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [carddav, setCarddav] = useState<CardDavServerStatus | null>(null);
  const [ldapUrl, setLdapUrl] = useState("ldaps://ldap.example.com");
  const [ldapBase, setLdapBase] = useState("ou=people,dc=example,dc=com");
  const [ldapFilter, setLdapFilter] = useState("(objectClass=inetOrgPerson)");
  const [ldapBind, setLdapBind] = useState("");
  const [ldapPassword, setLdapPassword] = useState("");
  const [syncInfo, setSyncInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh(nextQuery = query) {
    const [list, status, ldap] = await Promise.all([
      api.contactsList(nextQuery || null),
      api.carddavStatus(),
      api.ldapGetSettings().catch(() => null),
    ]);
    setContacts(list);
    setCarddav(status);
    if (ldap) {
      setLdapUrl(ldap.url || ldapUrl);
      setLdapBase(ldap.baseDn || ldapBase);
      setLdapFilter(ldap.filter || ldapFilter);
      setLdapBind(ldap.bindDn || "");
    }
  }

  useEffect(() => {
    if (!open) return;
    refresh().catch((err) => setError((err as AppError).message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, query]);

  const selected = useMemo(
    () => contacts.find((c) => c.id === selectedId) ?? null,
    [contacts, selectedId],
  );

  function startNew() {
    setSelectedId(null);
    setDraft(emptyDraft());
    setError(null);
    setSyncInfo(null);
  }

  function selectContact(contact: ContactDto) {
    setSelectedId(contact.id);
    setDraft(contactToDraft(contact));
    setError(null);
    setSyncInfo(null);
  }

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const saved = await api.contactsUpsert(
        draftToRequest(draft, t("contactFallback")),
      );
      setSelectedId(saved.id);
      setDraft(contactToDraft(saved));
      await refresh();
      setSyncInfo(t("contactSaved"));
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
    setSyncInfo(null);
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
      setSyncInfo(
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

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("addressBook")}
      description={t("addressBookDescription")}
      className="max-w-5xl"
    >
      <div className="grid max-h-[78vh] gap-4 overflow-hidden text-sm lg:grid-cols-[240px_minmax(0,1fr)]">
        <aside className="flex min-h-0 flex-col gap-3 border-b border-[var(--nova-border)] pb-3 lg:border-b-0 lg:border-r lg:pr-3 lg:pb-0">
          <div className="flex items-center gap-2">
            <Input
              className="min-w-0 flex-1"
              placeholder={t("fuzzySearchContacts")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <Button type="button" size="sm" variant="secondary" onClick={startNew}>
              <Plus size={14} />
              {t("add")}
            </Button>
          </div>
          <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
            {contacts.length === 0 ? (
              <li className="px-1 text-[var(--nova-ink-muted)]">
                {t("noContactsYet")}
              </li>
            ) : (
              contacts.map((contact) => {
                const active = contact.id === selectedId;
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
                        name={contact.displayName}
                        photoBase64={contact.photoBase64}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-medium">
                          {contact.displayName}
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

        <div className="min-h-0 space-y-4 overflow-y-auto pr-1">
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
                    <UserRound className="text-[var(--nova-ink-muted)]" size={28} />
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
              <Input
                placeholder={t("displayName")}
                value={draft.displayName}
                onChange={(e) =>
                  setDraft((prev) => ({ ...prev, displayName: e.target.value }))
                }
              />
              <Input
                placeholder={t("organization")}
                value={draft.organization}
                onChange={(e) =>
                  setDraft((prev) => ({ ...prev, organization: e.target.value }))
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
                  onClick={async () => {
                    setBusy(true);
                    try {
                      await api.contactsDelete(draft.id!);
                      startNew();
                      await refresh();
                    } catch (err) {
                      setError((err as AppError).message);
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  {t("delete")}
                </Button>
              ) : null}
            </div>
          </form>

          {syncInfo ? (
            <p className="text-[var(--nova-accent)]" role="status">
              {syncInfo}
            </p>
          ) : null}
          {error ? (
            <p className="text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>
      </div>
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
