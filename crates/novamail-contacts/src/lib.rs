//! Local contacts: embedded CardDAV server, vCard, and LDAP lookup.
//!
//! The CardDAV endpoint lets MFPs / fax appliances sync the NovaMail address book
//! without a separate directory server. LDAP remains available for corporate lookup.

mod carddav;
mod error;
mod ldap;
mod vcard;

pub use carddav::{CardDavServer, ContactStore};
pub use error::{ContactsError, ContactsResult};
pub use ldap::search_ldap;
pub use vcard::{contact_to_vcard, vcard_to_contact};
