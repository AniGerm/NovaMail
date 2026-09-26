use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ldap3::{LdapConnAsync, Scope, SearchEntry};
use novamail_ipc::{ContactAddress, ContactDto, LdapSearchRequest};
use uuid::Uuid;

use crate::{ContactsError, ContactsResult};

/// Search an LDAP/LDAPS directory for person entries and map them to contacts.
pub async fn search_ldap(request: &LdapSearchRequest) -> ContactsResult<Vec<ContactDto>> {
    let (conn, mut ldap) = LdapConnAsync::new(&request.url)
        .await
        .map_err(|e| ContactsError::Ldap(e.to_string()))?;
    ldap3::drive!(conn);

    if let Some(bind_dn) = &request.bind_dn {
        let password = request.password.clone().unwrap_or_default();
        ldap.simple_bind(bind_dn, &password)
            .await
            .map_err(|e| ContactsError::Ldap(e.to_string()))?
            .success()
            .map_err(|e| ContactsError::Ldap(e.to_string()))?;
    }

    let filter = if request.filter.trim().is_empty() {
        "(objectClass=inetOrgPerson)".to_string()
    } else {
        request.filter.clone()
    };

    let (rs, _res) = ldap
        .search(
            &request.base_dn,
            Scope::Subtree,
            &filter,
            vec![
                "cn",
                "displayName",
                "givenName",
                "sn",
                "mail",
                "telephoneNumber",
                "mobile",
                "facsimileTelephoneNumber",
                "fax",
                "o",
                "organizationName",
                "title",
                "street",
                "postalAddress",
                "l",
                "st",
                "postalCode",
                "c",
                "description",
                "jpegPhoto",
                "labelledURI",
            ],
        )
        .await
        .map_err(|e| ContactsError::Ldap(e.to_string()))?
        .success()
        .map_err(|e| ContactsError::Ldap(e.to_string()))?;

    let mut contacts = Vec::new();
    for entry in rs {
        let entry = SearchEntry::construct(entry);
        let given_name = first_attr(&entry, "givenName").unwrap_or_default();
        let family_name = first_attr(&entry, "sn").unwrap_or_default();
        let display_name = first_attr(&entry, "displayName")
            .or_else(|| first_attr(&entry, "cn"))
            .unwrap_or_else(|| {
                let joined = format!("{given_name} {family_name}").trim().to_string();
                if joined.is_empty() {
                    entry.dn.clone()
                } else {
                    joined
                }
            });
        let emails = all_attr(&entry, "mail");
        let mut phones = all_attr(&entry, "telephoneNumber");
        phones.extend(all_attr(&entry, "mobile"));
        let mut faxes = all_attr(&entry, "facsimileTelephoneNumber");
        faxes.extend(all_attr(&entry, "fax"));
        let organization = first_attr(&entry, "o")
            .or_else(|| first_attr(&entry, "organizationName"))
            .unwrap_or_default();
        let job_title = first_attr(&entry, "title").unwrap_or_default();
        let street = first_attr(&entry, "street")
            .or_else(|| first_attr(&entry, "postalAddress"))
            .unwrap_or_default();
        let city = first_attr(&entry, "l").unwrap_or_default();
        let region = first_attr(&entry, "st").unwrap_or_default();
        let postal_code = first_attr(&entry, "postalCode").unwrap_or_default();
        let country = first_attr(&entry, "c").unwrap_or_default();
        let mut addresses = Vec::new();
        if !street.is_empty()
            || !city.is_empty()
            || !region.is_empty()
            || !postal_code.is_empty()
            || !country.is_empty()
        {
            addresses.push(ContactAddress {
                label: "WORK".into(),
                street,
                city,
                region,
                postal_code,
                country,
            });
        }
        let notes = first_attr(&entry, "description").unwrap_or_default();
        let photo_base64 = entry
            .bin_attrs
            .get("jpegPhoto")
            .and_then(|vals| vals.first())
            .map(|bytes| B64.encode(bytes));

        if emails.is_empty() && phones.is_empty() && faxes.is_empty() && display_name.is_empty() {
            continue;
        }
        contacts.push(ContactDto {
            id: Uuid::new_v4(),
            display_name,
            given_name,
            family_name,
            emails,
            phones,
            faxes,
            organization,
            job_title,
            addresses,
            custom_fields: Vec::new(),
            photo_base64,
            ldap_dn: Some(entry.dn),
            notes,
            updated_at: chrono::Utc::now().timestamp(),
        });
    }

    let _ = ldap.unbind().await;
    Ok(contacts)
}

fn first_attr(entry: &SearchEntry, name: &str) -> Option<String> {
    entry
        .attrs
        .get(name)
        .and_then(|values| values.first().cloned())
}

fn all_attr(entry: &SearchEntry, name: &str) -> Vec<String> {
    entry.attrs.get(name).cloned().unwrap_or_default()
}
