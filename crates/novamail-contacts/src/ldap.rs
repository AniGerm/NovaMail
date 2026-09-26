use ldap3::{LdapConnAsync, Scope, SearchEntry};
use novamail_ipc::{ContactDto, LdapSearchRequest};
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
            vec!["cn", "displayName", "mail", "telephoneNumber", "mobile"],
        )
        .await
        .map_err(|e| ContactsError::Ldap(e.to_string()))?
        .success()
        .map_err(|e| ContactsError::Ldap(e.to_string()))?;

    let mut contacts = Vec::new();
    for entry in rs {
        let entry = SearchEntry::construct(entry);
        let display_name = first_attr(&entry, "displayName")
            .or_else(|| first_attr(&entry, "cn"))
            .unwrap_or_else(|| entry.dn.clone());
        let emails = all_attr(&entry, "mail");
        let mut phones = all_attr(&entry, "telephoneNumber");
        phones.extend(all_attr(&entry, "mobile"));
        if emails.is_empty() && phones.is_empty() {
            continue;
        }
        contacts.push(ContactDto {
            id: Uuid::new_v4(),
            display_name,
            emails,
            phones,
            notes: format!("ldap:{}", entry.dn),
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
