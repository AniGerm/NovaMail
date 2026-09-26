import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, CardDavServerStatus, ContactDto } from "@/shared/api/types";

export function ContactsDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
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
        displayName: displayName || emails.split(",")[0]?.trim() || "Contact",
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
      title="Address book"
      description="Local contacts with embedded CardDAV for MFP/fax sync and optional LDAP import."
    >
      <div className="grid max-h-[70vh] gap-5 overflow-y-auto text-sm">
        <section className="grid gap-2">
          <h3 className="font-medium">CardDAV server</h3>
          <p className="text-[var(--nova-ink-muted)]">
            {carddav?.running
              ? `Running at ${carddav.addressbookUrl} · ${carddav.contactCount} contacts`
              : "Stopped — start to let printers/fax pull vCards."}
          </p>
          <Button type="button" size="sm" disabled={busy} onClick={toggleCardDav}>
            {carddav?.running ? "Stop CardDAV" : "Start CardDAV"}
          </Button>
        </section>

        <form onSubmit={handleSave} className="grid gap-2">
          <h3 className="font-medium">Add contact</h3>
          <Input
            placeholder="Display name"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
          <Input
            placeholder="Emails (comma-separated)"
            value={emails}
            onChange={(e) => setEmails(e.target.value)}
          />
          <Input
            placeholder="Phones (comma-separated)"
            value={phones}
            onChange={(e) => setPhones(e.target.value)}
          />
          <Input
            placeholder="Notes"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
          <Button type="submit" size="sm" disabled={busy}>
            Save contact
          </Button>
        </form>

        <section className="grid gap-2">
          <h3 className="font-medium">LDAP lookup</h3>
          <Input value={ldapUrl} onChange={(e) => setLdapUrl(e.target.value)} />
          <Input value={ldapBase} onChange={(e) => setLdapBase(e.target.value)} />
          <Input value={ldapFilter} onChange={(e) => setLdapFilter(e.target.value)} />
          <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={runLdap}>
            Search & import
          </Button>
        </section>

        <section className="grid gap-2">
          <div className="flex items-center justify-between gap-2">
            <h3 className="font-medium">Contacts</h3>
            <Input
              className="max-w-[180px]"
              placeholder="Filter"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <ul className="space-y-2">
            {contacts.length === 0 ? (
              <li className="text-[var(--nova-ink-muted)]">No contacts yet</li>
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
                    Delete
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
        <Button onClick={onClose}>Done</Button>
      </DialogActions>
    </Dialog>
  );
}
