import { useEffect, useMemo, useState } from "react";
import {
  Camera,
  Plus,
  Settings2,
  Trash2,
  UserRound,
} from "lucide-react";
import {
  Button,
  Dialog,
  DialogActions,
  IconButton,
  Input,
  Select,
} from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type {
  AppError,
  ContactAddress,
  ContactCustomField,
  ContactDto,
  ContactNameOrder,
  ContactPrefill,
  ContactSortBy,
  ContactsBookSettings,
  ContactsShareStatus,
  RecipientSuggestion,
  UpsertContactRequest,
} from "@/shared/api/types";
import {
  historyFieldPatch,
  mergeEmailList,
  suggestQueryToken,
} from "@/features/contacts/contactSuggest";
import { useT } from "@/shared/i18n/useT";

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

const emptyAddress = (label = ""): ContactAddress => ({
  label,
  street: "",
  city: "",
  region: "",
  postalCode: "",
  country: "",
});


function DirectoryStatusDot({
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
    <span className="group relative inline-flex">
      <button
        type="button"
        className="inline-flex h-8 w-8 items-center justify-center rounded-full"
        aria-label={ldap.running ? t("ldapRunning") : t("ldapDown")}
      >
        <span
          className={
            ldap.running
              ? "h-2.5 w-2.5 rounded-full bg-[var(--nova-success)] ring-2 ring-transparent transition group-hover:ring-[var(--nova-success)]"
              : "h-2.5 w-2.5 rounded-full bg-[var(--nova-danger)] ring-2 ring-transparent transition group-hover:ring-[var(--nova-danger)]"
          }
        />
      </button>
      <div className="invisible absolute right-0 top-full z-30 w-72 pt-1 opacity-0 transition group-hover:visible group-hover:opacity-100 group-focus-within:visible group-focus-within:opacity-100">
        <div
          className="grid gap-1 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-2 text-left text-xs shadow-[var(--nova-shadow)]"
          role="status"
        >
          <p
            className={
              ldap.running
                ? "font-medium text-[var(--nova-success)]"
                : "font-medium text-[var(--nova-danger)]"
            }
          >
            {ldap.running ? t("ldapRunning") : t("ldapDown")}
          </p>
          {urls.map((url) => (
            <p key={url} className="break-all font-mono">
              {url}
            </p>
          ))}
          {!ldap.running && ldap.lastError ? (
            <p className="text-[var(--nova-danger)]">{ldap.lastError}</p>
          ) : null}
          <p
            className={
              card.running
                ? "text-[var(--nova-success)]"
                : "text-[var(--nova-danger)]"
            }
          >
            {card.running ? t("cardDavUp") : t("cardDavDown")}
            {card.running && card.addressbookUrl ? ` · ${card.addressbookUrl}` : ""}
          </p>
          {!card.running && card.lastError ? (
            <p className="text-[var(--nova-danger)]">{card.lastError}</p>
          ) : null}
          {!ldap.running || !card.running ? (
            <Button type="button" size="sm" disabled={busy} onClick={onRestart}>
              {t("directoryRestart")}
            </Button>
          ) : null}
        </div>
      </div>
    </span>
  );
}

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

function prefillToDraft(prefill: ContactPrefill, workLabel: string): Draft {
  return {
    ...emptyDraft(),
    displayName: prefill.displayName ?? "",
    givenName: prefill.givenName ?? "",
    familyName: prefill.familyName ?? "",
    emails: (prefill.emails ?? []).join(", "),
    notes: prefill.notes ?? "",
    addresses: [emptyAddress(workLabel)],
  };
}

export function ContactsDialog({
  open,
  onClose,
  prefill = null,
  onPrefillConsumed,
  onContactSaved,
  onOpenSettings,
}: {
  open: boolean;
  onClose: () => void;
  prefill?: ContactPrefill | null;
  onPrefillConsumed?: () => void;
  /** Fired after a successful save (e.g. advance multi-recipient queue). */
  onContactSaved?: () => void;
  /** Opens the address-book tab in the main settings dialog. */
  onOpenSettings?: () => void;
}) {
  const t = useT();
  const [contacts, setContacts] = useState<ContactDto[]>([]);
  const [bookSettings, setBookSettings] =
    useState<ContactsBookSettings>(defaultBookSettings);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [share, setShare] = useState<ContactsShareStatus | null>(null);
  const [suggestField, setSuggestField] = useState<
    "given" | "family" | "email" | null
  >(null);
  const [suggestQuery, setSuggestQuery] = useState("");
  const [suggestions, setSuggestions] = useState<RecipientSuggestion[]>([]);
  const [suggestOpen, setSuggestOpen] = useState(false);
  const [suggestIndex, setSuggestIndex] = useState(0);
  const [statusInfo, setStatusInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<{
    id: string;
    name: string;
  } | null>(null);

  async function refresh(nextQuery = query) {
    const [list, shareStatus, book] = await Promise.all([
      api.contactsList(nextQuery || null),
      api.contactsShareStatus().catch(() => null),
      api.contactsBookSettings().catch(() => defaultBookSettings()),
    ]);
    setContacts(list);
    if (shareStatus) setShare(shareStatus);
    setBookSettings(book);
  }

  useEffect(() => {
    if (!open) return;
    refresh().catch((err) => setError((err as AppError).message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, query]);

  // Poll while sharing so remote LDAP/CardDAV edits show up here too.
  useEffect(() => {
    if (!open || share?.mode === "local") return;
    const timer = window.setInterval(() => {
      refresh().catch(() => {
        /* keep quiet during background poll */
      });
    }, 4000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, share?.mode, query]);

  useEffect(() => {
    if (!open || !prefill) return;
    setSelectedId(null);
    setDraft(prefillToDraft(prefill, t("addressLabelWork")));
    setError(null);
    setStatusInfo(null);
    onPrefillConsumed?.();
  }, [open, prefill, onPrefillConsumed, t]);

  useEffect(() => {
    if (!open || !isDesktopShell()) return;
    const q = suggestQueryToken(suggestQuery);
    if (!suggestField || q.length < 2) {
      setSuggestions([]);
      setSuggestOpen(false);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(() => {
      void api
        .recipientsSuggest(q, 8)
        .then((items) => {
          if (cancelled) return;
          setSuggestions(items);
          setSuggestIndex(0);
          setSuggestOpen(items.length > 0);
        })
        .catch(() => {
          if (!cancelled) {
            setSuggestions([]);
            setSuggestOpen(false);
          }
        });
    }, 160);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [open, suggestField, suggestQuery]);

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
    setDraft({
      ...emptyDraft(),
      addresses: [emptyAddress(t("addressLabelWork"))],
    });
    setError(null);
    setStatusInfo(null);
    setSuggestOpen(false);
  }

  function selectContact(contact: ContactDto) {
    setSelectedId(contact.id);
    const next = contactToDraft(contact);
    if (!contact.addresses?.length) {
      next.addresses = [emptyAddress(t("addressLabelWork"))];
    }
    setDraft(next);
    setError(null);
    setStatusInfo(null);
    setSuggestOpen(false);
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
      onContactSaved?.();
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

  function trackSuggest(
    field: "given" | "family" | "email",
    value: string,
  ) {
    setSuggestField(field);
    setSuggestQuery(value);
  }

  function suggestKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (!suggestOpen || suggestions.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSuggestIndex((index) => (index + 1) % suggestions.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSuggestIndex(
        (index) => (index - 1 + suggestions.length) % suggestions.length,
      );
    } else if (event.key === "Enter" && suggestions[suggestIndex]) {
      event.preventDefault();
      void applySuggestion(suggestions[suggestIndex]!);
    } else if (event.key === "Escape") {
      setSuggestOpen(false);
    }
  }

  async function applySuggestion(item: RecipientSuggestion) {
    setSuggestOpen(false);
    setSuggestions([]);
    if (item.contactId || item.inContacts) {
      try {
        const list = await api.contactsList(item.email || item.name || null);
        const found =
          list.find((contact) => contact.id === item.contactId) ??
          list.find((contact) =>
            contact.emails.some(
              (email) => email.toLowerCase() === item.email.toLowerCase(),
            ),
          );
        if (found) {
          setSelectedId(found.id);
          const next = contactToDraft(found);
          if (!found.addresses?.length) {
            next.addresses = [emptyAddress(t("addressLabelWork"))];
          }
          setDraft(next);
          return;
        }
      } catch {
        /* fall through to the history fields we already have */
      }
    }
    const patch = historyFieldPatch(item);
    setDraft((prev) => ({
      ...prev,
      givenName: patch.givenName || prev.givenName,
      familyName: patch.familyName || prev.familyName,
      displayName: patch.displayName || prev.displayName,
      emails: mergeEmailList(prev.emails, patch.email),
    }));
  }

  function suggestionList(field: "given" | "family" | "email") {
    if (!suggestOpen || suggestField !== field || suggestions.length === 0) {
      return null;
    }
    return (
      <ul
        role="listbox"
        aria-label={t("contactSuggestions")}
        className="absolute left-0 right-0 top-[calc(100%+4px)] z-20 max-h-56 overflow-y-auto rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
        onMouseDown={(event) => event.preventDefault()}
      >
        {suggestions.map((item, index) => (
          <li key={`${item.email}-${item.source}-${index}`}>
            <button
              type="button"
              role="option"
              aria-selected={index === suggestIndex}
              className={
                index === suggestIndex
                  ? "flex w-full flex-col items-start gap-0.5 bg-[var(--nova-accent-soft)] px-3 py-2 text-left"
                  : "flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-[var(--nova-surface-2)]"
              }
              onMouseEnter={() => setSuggestIndex(index)}
              onClick={() => {
                void applySuggestion(item);
              }}
            >
              <span className="text-sm font-medium">
                {item.name || item.email}
              </span>
              <span className="text-xs text-[var(--nova-ink-muted)]">
                {item.name ? `${item.email} · ` : ""}
                {item.inContacts
                  ? t("recipientFromContacts")
                  : t("recipientFromHistory")}
              </span>
            </button>
          </li>
        ))}
      </ul>
    );
  }

  async function restartDirectory() {
    setBusy(true);
    setError(null);
    try {
      const status = await api.contactsSetShareMode({ mode: "server" });
      setShare(status);
      if (!status.ldapServer.running) {
        setError(
          status.ldapServer.lastError
            ? `${t("ldapDown")}: ${status.ldapServer.lastError}`
            : t("shareModeServicesDown"),
        );
      }
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
      className="max-w-5xl"
      headerActions={
        <>
          {share?.mode === "server" ? (
            <DirectoryStatusDot
              share={share}
              busy={busy}
              onRestart={() => {
                void restartDirectory();
              }}
            />
          ) : null}
          <IconButton
            label={t("contactsSettings")}
            onClick={() => onOpenSettings?.()}
          >
            <Settings2 size={18} />
          </IconButton>
        </>
      }
    >
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
                    <div className="relative">
                      <Input
                        placeholder={t("familyName")}
                        value={draft.familyName}
                        role="combobox"
                        aria-expanded={suggestOpen && suggestField === "family"}
                        aria-autocomplete="list"
                        onFocus={() => trackSuggest("family", draft.familyName)}
                        onBlur={() => {
                          window.setTimeout(() => setSuggestOpen(false), 120);
                        }}
                        onKeyDown={suggestKeyDown}
                        onChange={(e) => {
                          const value = e.target.value;
                          setDraft((prev) => ({ ...prev, familyName: value }));
                          trackSuggest("family", value);
                        }}
                      />
                      {suggestionList("family")}
                    </div>
                    <div className="relative">
                      <Input
                        placeholder={t("givenName")}
                        value={draft.givenName}
                        role="combobox"
                        aria-expanded={suggestOpen && suggestField === "given"}
                        aria-autocomplete="list"
                        onFocus={() => trackSuggest("given", draft.givenName)}
                        onBlur={() => {
                          window.setTimeout(() => setSuggestOpen(false), 120);
                        }}
                        onKeyDown={suggestKeyDown}
                        onChange={(e) => {
                          const value = e.target.value;
                          setDraft((prev) => ({ ...prev, givenName: value }));
                          trackSuggest("given", value);
                        }}
                      />
                      {suggestionList("given")}
                    </div>
                  </>
                ) : (
                  <>
                    <div className="relative">
                      <Input
                        placeholder={t("givenName")}
                        value={draft.givenName}
                        role="combobox"
                        aria-expanded={suggestOpen && suggestField === "given"}
                        aria-autocomplete="list"
                        onFocus={() => trackSuggest("given", draft.givenName)}
                        onBlur={() => {
                          window.setTimeout(() => setSuggestOpen(false), 120);
                        }}
                        onKeyDown={suggestKeyDown}
                        onChange={(e) => {
                          const value = e.target.value;
                          setDraft((prev) => ({ ...prev, givenName: value }));
                          trackSuggest("given", value);
                        }}
                      />
                      {suggestionList("given")}
                    </div>
                    <div className="relative">
                      <Input
                        placeholder={t("familyName")}
                        value={draft.familyName}
                        role="combobox"
                        aria-expanded={suggestOpen && suggestField === "family"}
                        aria-autocomplete="list"
                        onFocus={() => trackSuggest("family", draft.familyName)}
                        onBlur={() => {
                          window.setTimeout(() => setSuggestOpen(false), 120);
                        }}
                        onKeyDown={suggestKeyDown}
                        onChange={(e) => {
                          const value = e.target.value;
                          setDraft((prev) => ({ ...prev, familyName: value }));
                          trackSuggest("family", value);
                        }}
                      />
                      {suggestionList("family")}
                    </div>
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
                <div className="relative">
                  <Input
                    placeholder={t("emailsComma")}
                    value={draft.emails}
                    role="combobox"
                    aria-expanded={suggestOpen && suggestField === "email"}
                    aria-autocomplete="list"
                    onFocus={() => trackSuggest("email", draft.emails)}
                    onBlur={() => {
                      window.setTimeout(() => setSuggestOpen(false), 120);
                    }}
                    onKeyDown={suggestKeyDown}
                    onChange={(e) => {
                      const value = e.target.value;
                      setDraft((prev) => ({ ...prev, emails: value }));
                      trackSuggest("email", value);
                    }}
                  />
                  {suggestionList("email")}
                </div>
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
                        addresses: [
                          ...prev.addresses,
                          emptyAddress(t("addressLabelWork")),
                        ],
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
                    <AddressLabelField
                      value={address.label}
                      onChange={(label) =>
                        setDraft((prev) => {
                          const addresses = [...prev.addresses];
                          addresses[index] = {
                            ...addresses[index],
                            label,
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

      {pendingDelete ? (
        <div
          className="absolute inset-0 z-10 flex items-center justify-center overflow-hidden bg-[rgba(14,17,20,0.62)] p-4"
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

function AddressLabelField({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const t = useT();
  const presets = [
    t("addressLabelWork"),
    t("addressLabelHome"),
    t("addressLabelMobile"),
    t("addressLabelOffice"),
    t("addressLabelOther"),
  ];
  const normalized = (() => {
    const upper = value.trim().toUpperCase();
    if (upper === "WORK" || upper === "OFFICE") return t("addressLabelWork");
    if (upper === "HOME") return t("addressLabelHome");
    if (upper === "CELL" || upper === "MOBILE") return t("addressLabelMobile");
    return value;
  })();
  const isPreset = presets.includes(normalized);
  const [customMode, setCustomMode] = useState(
    Boolean(normalized) && !isPreset,
  );

  useEffect(() => {
    setCustomMode(Boolean(normalized) && !presets.includes(normalized));
    // presets are locale-stable for a render locale
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [normalized]);

  if (customMode) {
    return (
      <div className="grid gap-1">
        <Input
          list="novamail-address-labels"
          placeholder={t("addressLabelPlaceholder")}
          value={normalized}
          onChange={(e) => onChange(e.target.value)}
          autoFocus
        />
        <datalist id="novamail-address-labels">
          {presets.map((label) => (
            <option key={label} value={label} />
          ))}
        </datalist>
        <button
          type="button"
          className="justify-self-start text-xs text-[var(--nova-accent)]"
          onClick={() => {
            setCustomMode(false);
            onChange(t("addressLabelWork"));
          }}
        >
          ← {t("addressLabelWork")}
        </button>
      </div>
    );
  }

  return (
    <Select
      className="h-11 text-sm"
      value={normalized || ""}
      onChange={(e) => {
        const next = e.target.value;
        if (next === "__custom__") {
          setCustomMode(true);
          onChange("");
          return;
        }
        onChange(next);
      }}
      aria-label={t("addressLabel")}
    >
      <option value="">{t("addressLabelPlaceholder")}</option>
      {presets.map((label) => (
        <option key={label} value={label}>
          {label}
        </option>
      ))}
      <option value="__custom__">{t("addressLabelCustom")}</option>
    </Select>
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
