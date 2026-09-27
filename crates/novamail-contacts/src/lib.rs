//! Local contacts: CardDAV + LDAP server/client for shared address books.
//!
//! - **Server (hub):** embedded CardDAV (phones/printers) and LDAP (other NovaMail PCs)
//! - **Client:** LDAP sync against the hub (or any corporate directory)

mod carddav;
mod error;
mod ldap;
mod ldap_server;
mod vcard;

pub use carddav::{CardDavServer, ContactStore};
pub use error::{ContactsError, ContactsResult};
pub use ldap::search_ldap;
pub use ldap_server::{LdapServer, DEFAULT_BASE_DN, DEFAULT_BIND_DN};
pub use vcard::{contact_to_vcard, vcard_to_contact};
