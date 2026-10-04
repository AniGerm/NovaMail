# LDAP / CardDAV hub for MFPs (Ricoh) and NovaMail clients

NovaMail’s embedded directory is intended for LAN fax appliances and sibling
NovaMail PCs.

## When the listeners start

The hub is for **trusted LANs only** (office / home). LDAP and CardDAV are
cleartext (`ldap://` / `http://`) with HTTP Basic — **not TLS**. Do not expose
ports 1389/389/8765 to the public internet; use a VPN for remote access.

Open **Settings → Address book** (or the gear in the address book, which jumps
there), then set **Operating mode → Server (main PC)**. That persists
`contacts.share_mode=server` and binds:

- LDAP on `0.0.0.0:1389` (and `0.0.0.0:389` when the process may use that port)
- CardDAV on `0.0.0.0:8765`

The mode is stored in the database. Listeners exist only while this process is
running, so NovaMail binds them again on every launch and whenever share status
is read. The address book shows a small green/red status dot; the Settings tab
shows **LDAP is running** plus the listen URL (`ldap://<lan-ip>:1389`) while the
socket is open, or the bind error and **Restart services** if it is not.

Authenticated LDAP/CardDAV clients (the password shown in Settings) can add,
change, and delete contacts. Anonymous LDAP, when enabled, stays read-only.

## LDAP (default)

| Setting | Value |
| --- | --- |
| Listen | `0.0.0.0:1389` and, when permitted, also `0.0.0.0:389` |
| Base DN | `ou=people,dc=novamail` |
| Bind DN | `cn=novamail,dc=novamail` |
| Password | Same as CardDAV password in the UI |
| Person class | `inetOrgPerson` |
| Fax attribute | `facsimileTelephoneNumber` |
| Voice | `telephoneNumber` |

### Search behaviour (v0.1.21+)

- Filter evaluation: `and` / `or` / `not`, equality, substrings, present
- Scope: baseObject / singleLevel / wholeSubtree
- Attribute names on the wire use canonical casing (`facsimileTelephoneNumber`, …)
- Optional anonymous read-only bind (`contacts.ldap_allow_anonymous=true` in settings DB)
- Optional disable of privileged port (`contacts.ldap_also_389=false`)

### Ricoh MFP panel

1. LDAP server = NovaMail LAN IP  
2. Port **389** if the UI lists it, else **1389**  
3. Base DN / Bind DN / password as above  
4. Map fax → `facsimileTelephoneNumber`, name → `cn`

## CardDAV

| Setting | Value |
| --- | --- |
| URL | `http://<lan-ip>:8765/addressbooks/novamail/` |
| Auth | HTTP Basic (`novamail` + password) |
| Discovery | `/.well-known/carddav` works **without** credentials |

`Depth: 0` PROPFIND returns the collection only; `Depth: 1` lists member `.vcf`
hrefs without embedding address-data (clients GET each card).
