use std::sync::Arc;

use async_trait::async_trait;
use novamail_contacts::{ContactStore, ContactsError, ContactsResult};
use novamail_db::Database;
use novamail_ipc::ContactDto;
use uuid::Uuid;

pub struct DbContactStore {
    db: Database,
}

impl DbContactStore {
    pub fn new(db: Database) -> Arc<Self> {
        Arc::new(Self { db })
    }
}

#[async_trait]
impl ContactStore for DbContactStore {
    fn list(&self) -> ContactsResult<Vec<ContactDto>> {
        self.db
            .list_contacts(None)
            .map_err(|e| ContactsError::Store(e.to_string()))
    }

    fn get(&self, id: Uuid) -> ContactsResult<ContactDto> {
        self.db
            .get_contact(id)
            .map_err(|e| ContactsError::Store(e.to_string()))
    }

    fn upsert(&self, contact: ContactDto) -> ContactsResult<()> {
        let record = novamail_db::models::ContactRecord {
            id: contact.id,
            display_name: contact.display_name,
            emails: contact.emails,
            phones: contact.phones,
            faxes: contact.faxes,
            organization: contact.organization,
            job_title: contact.job_title,
            addresses: contact.addresses,
            custom_fields: contact.custom_fields,
            photo_base64: contact.photo_base64,
            ldap_dn: contact.ldap_dn,
            notes: contact.notes,
            updated_at: contact.updated_at,
        };
        self.db
            .upsert_contact(&record)
            .map_err(|e| ContactsError::Store(e.to_string()))
    }

    fn delete(&self, id: Uuid) -> ContactsResult<()> {
        self.db
            .delete_contact(id)
            .map_err(|e| ContactsError::Store(e.to_string()))
    }
}
