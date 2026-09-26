import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, CardDavServerStatus, ContactDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

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
  const [displayName, setDisplayName] = useState("");
  const [emails, setEmails] = useState("");
  const [phones, setPhones] = useState("");
  const [notes, setNotes] = useState("");
  const [carddav, setCarddav] = useState<CardDavServerStatus | null>(null);
  const [ldapUrl, setLdapUrl] = useState("ldaps://ldap.example.com");
  const [ldapBase, setLdapBase] = useState("ou=people,dc=example,dc=com");
  const [ldapFilter, setLdapFilter] = useState("(mail=*)");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    const [list, status] = await Promise.all([
      api.contactsList(query || null),
      api.carddavStatus(),
    ]);
    setContacts(list);
    setCarddav(status);
  }

  useEffect(() => {
    if (!open) return;
    refresh().catch((err) => setError((err as AppError).message));
  }, [open, query]);

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.contactsUpsert({
        displayName: displayName || emails.split(",")[0]?.trim() || t("contactFallback"),
        emails: emails
          .split(",")
          .map((v) => v.trim())
          .filter(Boolean),
        phones: phones
          .split(",")
          .map((v) => v.trim())
          .filter(Boolean),
        notes,
      });
      setDisplayName("");
      setEmails("");
      setPhones("");
      setNotes("");
      await refresh();
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

  async function runLdap() {
    setBusy(true);
    setError(null);
    try {
      const found = await api.ldapSearch({
        url: ldapUrl,
        baseDn: ldapBase,
        filter: ldapFilter,
        bindDn: null,
        password: null,
      });
      for (const contact of found) {
        await api.contactsUpsert({
          displayName: contact.displayName,
          emails: contact.emails,
          phones: contact.phones,
          notes: contact.notes,
        });
      }
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("addressBook")}
      description={t("addressBookDescription")}
    >
      <div className="grid max-h-[70vh] gap-5 overflow-y-auto text-sm">
        <section className="grid gap-2">
          <h3 className="font-medium">{t("cardDavServer")}</h3>
          <p className="text-[var(--nova-ink-muted)]">
            {carddav?.running
              ? t("cardDavRunning", {
                  url: carddav.addressbookUrl,
                  count: carddav.contactCount,
                })
              : t("cardDavStopped")}
          </p>
          <Button type="button" size="sm" disabled={busy} onClick={toggleCardDav}>
            {carddav?.running ? t("stopCardDav") : t("startCardDav")}
          </Button>
        </section>

        <form onSubmit={handleSave} className="grid gap-2">
          <h3 className="font-medium">{t("addContact")}</h3>
          <Input
            placeholder={t("displayName")}
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
          <Input
            placeholder={t("emailsComma")}
            value={emails}
            onChange={(e) => setEmails(e.target.value)}
          />
          <Input
            placeholder={t("phonesComma")}
            value={phones}
            onChange={(e) => setPhones(e.target.value)}
          />
          <Input
            placeholder={t("notes")}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
          <Button type="submit" size="sm" disabled={busy}>
            {t("saveContact")}
          </Button>
        </form>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("ldapLookup")}</h3>
          <Input value={ldapUrl} onChange={(e) => setLdapUrl(e.target.value)} />
          <Input value={ldapBase} onChange={(e) => setLdapBase(e.target.value)} />
          <Input value={ldapFilter} onChange={(e) => setLdapFilter(e.target.value)} />
          <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={runLdap}>
            {t("searchImport")}
          </Button>
        </section>

        <section className="grid gap-2">
          <div className="flex items-center justify-between gap-2">
            <h3 className="font-medium">{t("contacts")}</h3>
            <Input
              className="max-w-[180px]"
              placeholder={t("filter")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <ul className="space-y-2">
            {contacts.length === 0 ? (
              <li className="text-[var(--nova-ink-muted)]">{t("noContactsYet")}</li>
            ) : (
              contacts.map((contact) => (
                <li
                  key={contact.id}
                  className="flex items-start justify-between gap-3 border-b border-[var(--nova-border)] pb-2"
                >
                  <div>
                    <p className="font-medium">{contact.displayName}</p>
                    <p className="text-[var(--nova-ink-muted)]">
                      {contact.emails.join(", ")}
                      {contact.phones.length
                        ? ` · ${contact.phones.join(", ")}`
                        : ""}
                    </p>
                  </div>
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      await api.contactsDelete(contact.id);
                      await refresh();
                    }}
                  >
                    {t("delete")}
                  </Button>
                </li>
              ))
            )}
          </ul>
        </section>

        {error ? (
          <p className="text-[var(--nova-danger)]" role="alert">
            {error}
          </p>
        ) : null}
      </div>
      <DialogActions>
        <Button onClick={onClose}>{t("done")}</Button>
      </DialogActions>
    </Dialog>
  );
}
