//! LDAP v3 hub server for shared contacts (NovaMail PCs, Ricoh MFPs, generic clients).
//!
//! Supports Simple Bind (optional anonymous read), Search with filter/scope,
//! Add, Modify, and Delete. Default listen `0.0.0.0:1389`; optional second bind
//! on port 389 for appliances that expect the well-known LDAP port.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use bytes::{Buf, BytesMut};
use lber::common::TagClass;
use lber::parse::parse_uint;
use lber::structure::{StructureTag, PL};
use lber::structures::{
    ASNTag, Enumerated, Integer, OctetString, Sequence, Set, Tag,
};
use lber::universal::Types;
use lber::write;
use novamail_ipc::{ContactAddress, ContactDto, LdapServerStatus};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use uuid::Uuid;

use crate::ldap_filter::{filter_matches, parse_filter, LdapFilter};
use crate::{ContactsError, ContactsResult, ContactStore};

const DEFAULT_ADDR: &str = "0.0.0.0:1389";
const PRIVILEGED_ADDR: &str = "0.0.0.0:389";
pub const DEFAULT_BASE_DN: &str = "ou=people,dc=novamail";
pub const DEFAULT_BIND_DN: &str = "cn=novamail,dc=novamail";
const ROOT_DN: &str = "dc=novamail";

pub struct LdapServer {
    store: Arc<dyn ContactStore>,
    listen_addrs: parking_lot::Mutex<Vec<String>>,
    also_listen_389: parking_lot::Mutex<bool>,
    allow_anonymous: parking_lot::Mutex<bool>,
    username: Arc<parking_lot::Mutex<String>>,
    password: Arc<parking_lot::Mutex<String>>,
    bound_addrs: parking_lot::Mutex<Vec<String>>,
    shutdown_tx: parking_lot::Mutex<Option<watch::Sender<bool>>>,
    running: Arc<AtomicBool>,
    /// Prevents two startup paths from binding the same port at once.
    starting: Arc<AtomicBool>,
    last_error: Arc<parking_lot::Mutex<Option<String>>>,
}

impl LdapServer {
    pub fn new(store: Arc<dyn ContactStore>) -> Self {
        Self {
            store,
            listen_addrs: parking_lot::Mutex::new(vec![DEFAULT_ADDR.into()]),
            also_listen_389: parking_lot::Mutex::new(true),
            allow_anonymous: parking_lot::Mutex::new(false),
            username: Arc::new(parking_lot::Mutex::new("novamail".into())),
            password: Arc::new(parking_lot::Mutex::new(String::new())),
            bound_addrs: parking_lot::Mutex::new(Vec::new()),
            shutdown_tx: parking_lot::Mutex::new(None),
            running: Arc::new(AtomicBool::new(false)),
            starting: Arc::new(AtomicBool::new(false)),
            last_error: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    pub fn with_addr(self, addr: impl Into<String>) -> Self {
        *self.listen_addrs.lock() = vec![addr.into()];
        self
    }

    pub fn set_also_listen_389(&self, enabled: bool) {
        *self.also_listen_389.lock() = enabled;
    }

    pub fn set_allow_anonymous(&self, enabled: bool) {
        *self.allow_anonymous.lock() = enabled;
    }

    pub fn set_credentials(&self, username: impl Into<String>, password: impl Into<String>) {
        *self.username.lock() = username.into();
        *self.password.lock() = password.into();
    }

    pub fn status(&self) -> LdapServerStatus {
        let bound = self.bound_addrs.lock().clone();
        let addrs = if bound.is_empty() {
            self.listen_addrs.lock().clone()
        } else {
            bound
        };
        let listen_urls: Vec<String> = addrs
            .iter()
            .map(|a| format!("ldap://{}", advertise_host_port(a)))
            .collect();
        let listen_url = listen_urls
            .first()
            .cloned()
            .unwrap_or_else(|| "ldap://127.0.0.1:1389".into());
        LdapServerStatus {
            running: self.running.load(Ordering::Acquire),
            last_error: self.last_error.lock().clone(),
            listen_url,
            listen_urls,
            allow_anonymous: *self.allow_anonymous.lock(),
            base_dn: DEFAULT_BASE_DN.into(),
            bind_dn: DEFAULT_BIND_DN.into(),
            username: self.username.lock().clone(),
            password: self.password.lock().clone(),
            contact_count: self.store.list().map(|c| c.len() as u32).unwrap_or(0),
        }
    }

    pub async fn start(&self) -> ContactsResult<LdapServerStatus> {
        if self.running.load(Ordering::Acquire) {
            return Ok(self.status());
        }
        if self
            .starting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(self.status());
        }
        let result = self.start_bound().await;
        self.starting.store(false, Ordering::Release);
        result
    }

    async fn start_bound(&self) -> ContactsResult<LdapServerStatus> {
        if self.running.load(Ordering::Acquire) {
            return Ok(self.status());
        }
        let allow_anon = *self.allow_anonymous.lock();
        if self.password.lock().trim().is_empty() && !allow_anon {
            let msg = "LDAP password is empty; set credentials before starting".to_string();
            *self.last_error.lock() = Some(msg.clone());
            return Err(ContactsError::Ldap(msg));
        }

        let mut candidates = self.listen_addrs.lock().clone();
        if *self.also_listen_389.lock() && !candidates.iter().any(|a| a.ends_with(":389")) {
            candidates.push(PRIVILEGED_ADDR.into());
        }

        let mut listeners = Vec::new();
        let mut bound = Vec::new();
        let mut errors = Vec::new();
        for addr_s in candidates {
            let addr: SocketAddr = match addr_s.parse() {
                Ok(a) => a,
                Err(e) => {
                    errors.push(format!("{addr_s}: {e}"));
                    continue;
                }
            };
            match bind_with_retry(addr).await {
                Ok(listener) => {
                    let local = listener
                        .local_addr()
                        .map(|a| a.to_string())
                        .unwrap_or(addr_s);
                    tracing::info!(listen = %local, "LDAP listening");
                    bound.push(local);
                    listeners.push(listener);
                }
                Err(e) => {
                    tracing::warn!(addr = %addr, error = %e, "LDAP bind failed (continuing)");
                    errors.push(format!("{addr_s}: {e}"));
                }
            }
        }
        if listeners.is_empty() {
            let msg = format!(
                "LDAP bind failed on all addresses: {}",
                errors.join("; ")
            );
            *self.last_error.lock() = Some(msg.clone());
            return Err(ContactsError::Ldap(msg));
        }
        if !errors.is_empty() {
            tracing::warn!(
                skipped = %errors.join("; "),
                "LDAP started without every requested port"
            );
        }

        let (tx, rx) = watch::channel(false);
        *self.shutdown_tx.lock() = Some(tx);
        *self.bound_addrs.lock() = bound;
        *self.last_error.lock() = None;
        self.running.store(true, Ordering::Release);

        let store = self.store.clone();
        let username = self.username.clone();
        let password = self.password.clone();
        // Snapshot anonymous flag for this run (toggle requires restart).
        let allow_anonymous = Arc::new(parking_lot::Mutex::new(allow_anon));
        let alive = Arc::new(AtomicUsize::new(listeners.len()));

        for listener in listeners {
            let store = store.clone();
            let username = username.clone();
            let password = password.clone();
            let allow_anonymous = allow_anonymous.clone();
            let rx = rx.clone();
            let running = self.running.clone();
            let last_error = self.last_error.clone();
            let alive = alive.clone();
            tokio::spawn(async move {
                serve_loop(
                    listener,
                    store,
                    username,
                    password,
                    allow_anonymous,
                    rx,
                    running,
                    last_error,
                    alive,
                )
                .await;
            });
        }
        Ok(self.status())
    }

    pub fn stop(&self) -> LdapServerStatus {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            let _ = tx.send(true);
        }
        self.running.store(false, Ordering::Release);
        self.bound_addrs.lock().clear();
        self.status()
    }
}

async fn serve_loop(
    listener: TcpListener,
    store: Arc<dyn ContactStore>,
    username: Arc<parking_lot::Mutex<String>>,
    password: Arc<parking_lot::Mutex<String>>,
    allow_anonymous: Arc<parking_lot::Mutex<bool>>,
    mut shutdown: watch::Receiver<bool>,
    running: Arc<AtomicBool>,
    last_error: Arc<parking_lot::Mutex<Option<String>>>,
    alive: Arc<AtomicUsize>,
) {
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let store = store.clone();
                        let username = username.clone();
                        let password = password.clone();
                        let allow_anonymous = allow_anonymous.clone();
                        tokio::spawn(async move {
                            if let Err(err) = handle_conn(stream, store, username, password, allow_anonymous).await {
                                tracing::debug!(error = %err, "ldap client connection ended");
                            }
                        });
                    }
                    Err(err) => tracing::warn!(error = %err, "ldap accept failed"),
                }
            }
        }
    }
    let left = alive.fetch_sub(1, Ordering::AcqRel);
    if left == 1 && !*shutdown.borrow() {
        running.store(false, Ordering::Release);
        *last_error.lock() = Some("LDAP listener exited unexpectedly".into());
        tracing::error!("LDAP listener exited unexpectedly");
    }
}

async fn handle_conn(
    mut stream: TcpStream,
    store: Arc<dyn ContactStore>,
    username: Arc<parking_lot::Mutex<String>>,
    password: Arc<parking_lot::Mutex<String>>,
    allow_anonymous: Arc<parking_lot::Mutex<bool>>,
) -> ContactsResult<()> {
    let mut buf = BytesMut::with_capacity(4096);
    let mut authed = false;
    let mut can_write = false;
    loop {
        let Some(tag) = read_message(&mut stream, &mut buf).await? else {
            break;
        };
        let (msg_id, op) = split_ldap_message(tag)?;
        match op.id {
            0 => {
                // BindRequest — re-read credentials so set_credentials applies live.
                let user = username.lock().clone();
                let pass = password.lock().clone();
                let anon = *allow_anonymous.lock();
                let (ok, write_ok) = check_bind(&op, &user, &pass, anon);
                authed = ok;
                can_write = write_ok;
                write_tag(
                    &mut stream,
                    ldap_result_message(msg_id, 1, if ok { 0 } else { 49 }, ""),
                )
                .await?;
                if !ok {
                    break;
                }
            }
            2 => {
                // UnbindRequest
                break;
            }
            3 => {
                // SearchRequest
                if !authed {
                    write_tag(&mut stream, ldap_result_message(msg_id, 5, 50, "bind required"))
                        .await?;
                    continue;
                }
                handle_search(&mut stream, msg_id, &op, &store).await?;
            }
            6 => {
                // ModifyRequest
                if !authed || !can_write {
                    write_tag(&mut stream, ldap_result_message(msg_id, 7, 50, "insufficientAccessRights"))
                        .await?;
                    continue;
                }
                let rc = handle_modify(&op, &store);
                write_tag(&mut stream, ldap_result_message(msg_id, 7, rc, "")).await?;
            }
            8 => {
                // AddRequest
                if !authed || !can_write {
                    write_tag(&mut stream, ldap_result_message(msg_id, 9, 50, "insufficientAccessRights"))
                        .await?;
                    continue;
                }
                let rc = handle_add(&op, &store);
                write_tag(&mut stream, ldap_result_message(msg_id, 9, rc, "")).await?;
            }
            10 => {
                // DelRequest
                if !authed || !can_write {
                    write_tag(&mut stream, ldap_result_message(msg_id, 11, 50, "insufficientAccessRights"))
                        .await?;
                    continue;
                }
                let rc = handle_delete(&op, &store);
                write_tag(&mut stream, ldap_result_message(msg_id, 11, rc, "")).await?;
            }
            other => {
                tracing::debug!(op = other, "unsupported ldap op");
                write_tag(
                    &mut stream,
                    ldap_result_message(msg_id, 5, 2, "protocol error"),
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn read_message(
    stream: &mut TcpStream,
    buf: &mut BytesMut,
) -> ContactsResult<Option<StructureTag>> {
    loop {
        let mut parser = lber::Parser::new();
        match parser.parse(buf) {
            Ok((remaining, tag)) => {
                let consumed = buf.len() - remaining.len();
                buf.advance(consumed);
                return Ok(Some(tag));
            }
            Err(err) if err.is_incomplete() => {
                let mut tmp = [0u8; 4096];
                let n = stream.read(&mut tmp).await?;
                if n == 0 {
                    return Ok(None);
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(_) => {
                return Err(ContactsError::Ldap("invalid LDAP BER".into()));
            }
        }
    }
}

async fn write_tag(stream: &mut TcpStream, tag: StructureTag) -> ContactsResult<()> {
    let mut out = BytesMut::new();
    write::encode_into(&mut out, tag).map_err(|e| ContactsError::Ldap(e.to_string()))?;
    stream.write_all(&out).await?;
    Ok(())
}

fn split_ldap_message(tag: StructureTag) -> ContactsResult<(i64, StructureTag)> {
    let mut parts = tag
        .match_id(Types::Sequence as u64)
        .and_then(|t| t.expect_constructed())
        .ok_or_else(|| ContactsError::Ldap("expected LDAPMessage".into()))?;
    if parts.len() < 2 {
        return Err(ContactsError::Ldap("short LDAPMessage".into()));
    }
    let op = parts.remove(1);
    let id_tag = parts.remove(0);
    let id = match parse_uint(
        id_tag
            .match_id(Types::Integer as u64)
            .and_then(|t| t.expect_primitive())
            .ok_or_else(|| ContactsError::Ldap("bad messageID".into()))?
            .as_slice(),
    ) {
        Ok((_, v)) => v as i64,
        Err(_) => return Err(ContactsError::Ldap("bad messageID int".into())),
    };
    Ok((id, op))
}

/// Returns `(authenticated, can_write)`. Anonymous bind is read-only when enabled.
fn check_bind(
    op: &StructureTag,
    username: &str,
    password: &str,
    allow_anonymous: bool,
) -> (bool, bool) {
    let Some(parts) = op.clone().expect_constructed() else {
        return (false, false);
    };
    if parts.len() < 3 {
        return (false, false);
    }
    let name = octet_string(&parts[1]).unwrap_or_default();
    let auth = &parts[2];
    // simple auth is context-specific primitive id 0
    let pass = match &auth.payload {
        PL::P(bytes) if auth.class == TagClass::Context && auth.id == 0 => {
            String::from_utf8_lossy(bytes).to_string()
        }
        _ => return (false, false),
    };

    // Anonymous: empty DN + empty password (common MFP default).
    if name.trim().is_empty() && pass.is_empty() {
        return (allow_anonymous, false);
    }

    if pass != password || password.is_empty() {
        return (false, false);
    }
    let name_l = name.to_ascii_lowercase();
    let user_l = username.to_ascii_lowercase();
    let ok = name_l.is_empty()
        || name_l == DEFAULT_BIND_DN
        || name_l == format!("uid={user_l},dc=novamail")
        || name_l == format!("cn={user_l},dc=novamail")
        || name_l == user_l
        || name_l.starts_with(&format!("cn={user_l},"))
        || name_l.starts_with(&format!("uid={user_l},"));
    (ok, ok)
}

async fn emit_search_entry(
    stream: &mut TcpStream,
    msg_id: i64,
    dn: &str,
    attrs: &HashMap<String, Vec<String>>,
    filter: &LdapFilter,
    requested_attrs: &[String],
) -> ContactsResult<()> {
    if !filter_matches(filter, attrs) {
        return Ok(());
    }
    let out = if requested_attrs.len() == 1 && requested_attrs[0] == "1.1" {
        HashMap::new()
    } else if requested_attrs.is_empty() {
        attrs.clone()
    } else {
        attrs
            .iter()
            .filter(|(k, _)| {
                requested_attrs
                    .iter()
                    .any(|want| want.eq_ignore_ascii_case(k))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    write_tag(stream, search_entry_message(msg_id, dn, &out)).await
}

fn dn_in_scope(dn_l: &str, base_l: &str, scope: u64) -> bool {
    let under_base = base_l.is_empty()
        || base_l == ROOT_DN
        || base_l == DEFAULT_BASE_DN
        || dn_l == base_l
        || dn_l.ends_with(&format!(",{base_l}"));
    if !under_base {
        return false;
    }
    match scope {
        0 => dn_l == base_l,
        1 => {
            // immediate children of base
            let parent = dn_l.split_once(',').map(|(_, p)| p).unwrap_or("");
            parent == base_l && dn_l != base_l
        }
        _ => true, // subtree
    }
}

async fn handle_search(
    stream: &mut TcpStream,
    msg_id: i64,
    op: &StructureTag,
    store: &Arc<dyn ContactStore>,
) -> ContactsResult<()> {
    let parts = op
        .clone()
        .expect_constructed()
        .ok_or_else(|| ContactsError::Ldap("bad search".into()))?;
    let base = octet_string(parts.first().unwrap_or(&StructureTag {
        id: 0,
        class: TagClass::Universal,
        payload: PL::P(vec![]),
    }))
    .unwrap_or_default();
    let base_l = base.to_ascii_lowercase();

    // scope: 0 baseObject, 1 singleLevel, 2 wholeSubtree
    let scope = parts
        .get(1)
        .and_then(|t| match &t.payload {
            PL::P(bytes) => bytes.last().copied().map(u64::from),
            _ => None,
        })
        .unwrap_or(2);

    let filter = parts
        .get(6)
        .map(parse_filter)
        .unwrap_or(LdapFilter::MatchAll);

    let requested_attrs: Vec<String> = parts
        .get(7)
        .and_then(|t| t.clone().expect_constructed())
        .into_iter()
        .flatten()
        .filter_map(|t| octet_string(&t))
        .filter(|a| !a.is_empty() && a != "*")
        .collect();

    let mut root = HashMap::new();
    root.insert(
        "objectClass".into(),
        vec!["top".into(), "organization".into(), "dcObject".into()],
    );
    root.insert("o".into(), vec!["NovaMail".into()]);
    root.insert("dc".into(), vec!["novamail".into()]);
    root.insert(
        "namingContexts".into(),
        vec![ROOT_DN.into(), DEFAULT_BASE_DN.into()],
    );
    root.insert("supportedLDAPVersion".into(), vec!["3".into()]);

    let mut ou = HashMap::new();
    ou.insert(
        "objectClass".into(),
        vec!["top".into(), "organizationalUnit".into()],
    );
    ou.insert("ou".into(), vec!["people".into()]);

    // Root DSE
    if base_l.is_empty() && scope == 0 {
        emit_search_entry(stream, msg_id, "", &root, &filter, &requested_attrs).await?;
        write_tag(stream, ldap_result_message(msg_id, 5, 0, "")).await?;
        return Ok(());
    }

    if base_l.is_empty() || base_l == ROOT_DN {
        if scope != 1 || base_l == ROOT_DN || base_l.is_empty() {
            if scope != 1 || base_l == ROOT_DN {
                // include root itself on subtree / base
                if scope != 1 {
                    emit_search_entry(stream, msg_id, ROOT_DN, &root, &filter, &requested_attrs)
                        .await?;
                }
            }
        }
        if scope == 0 && base_l == ROOT_DN {
            emit_search_entry(stream, msg_id, ROOT_DN, &root, &filter, &requested_attrs).await?;
            write_tag(stream, ldap_result_message(msg_id, 5, 0, "")).await?;
            return Ok(());
        }
        if scope == 1 && (base_l.is_empty() || base_l == ROOT_DN) {
            emit_search_entry(
                stream,
                msg_id,
                DEFAULT_BASE_DN,
                &ou,
                &filter,
                &requested_attrs,
            )
            .await?;
            write_tag(stream, ldap_result_message(msg_id, 5, 0, "")).await?;
            return Ok(());
        }
        if scope == 2 {
            emit_search_entry(
                stream,
                msg_id,
                DEFAULT_BASE_DN,
                &ou,
                &filter,
                &requested_attrs,
            )
            .await?;
        }
    }

    if base_l == DEFAULT_BASE_DN && scope == 0 {
        emit_search_entry(
            stream,
            msg_id,
            DEFAULT_BASE_DN,
            &ou,
            &filter,
            &requested_attrs,
        )
        .await?;
        write_tag(stream, ldap_result_message(msg_id, 5, 0, "")).await?;
        return Ok(());
    }

    let contacts = store.list().unwrap_or_default();
    for contact in contacts {
        let dn = contact_dn(&contact);
        let dn_l = dn.to_ascii_lowercase();
        if !dn_in_scope(&dn_l, &base_l, scope) {
            continue;
        }
        let attrs = contact_attrs(&contact);
        emit_search_entry(stream, msg_id, &dn, &attrs, &filter, &requested_attrs).await?;
    }

    write_tag(stream, ldap_result_message(msg_id, 5, 0, "")).await?;
    Ok(())
}

fn handle_add(op: &StructureTag, store: &Arc<dyn ContactStore>) -> u64 {
    let Some(parts) = op.clone().expect_constructed() else {
        return 1;
    };
    if parts.len() < 2 {
        return 1;
    }
    let Some(dn) = octet_string(&parts[0]) else {
        return 1;
    };
    let attrs = parse_attr_list(&parts[1]);
    let mut contact = attrs_to_contact(&dn, &attrs, None);
    contact.updated_at = chrono::Utc::now().timestamp();
    match store.upsert(contact) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn handle_modify(op: &StructureTag, store: &Arc<dyn ContactStore>) -> u64 {
    let Some(parts) = op.clone().expect_constructed() else {
        return 1;
    };
    if parts.len() < 2 {
        return 1;
    }
    let Some(dn) = octet_string(&parts[0]) else {
        return 1;
    };
    let existing = find_by_dn(store, &dn);
    let Some(existing) = existing else {
        return 32; // noSuchObject
    };
    // Collapse all modifications into a replace-style attr map, then merge.
    let mut attrs = contact_attrs(&existing);
    if let Some(changes) = parts[1].clone().expect_constructed() {
        for change in changes {
            let Some(seq) = change.expect_constructed() else {
                continue;
            };
            if seq.len() < 2 {
                continue;
            }
            // operation ENUM (0 add, 1 delete, 2 replace) — we treat all as replace/merge.
            let partial = &seq[1];
            let Some(partial_parts) = partial.clone().expect_constructed() else {
                continue;
            };
            if partial_parts.is_empty() {
                continue;
            }
            let Some(name) = octet_string(&partial_parts[0]) else {
                continue;
            };
            let values = if partial_parts.len() > 1 {
                set_values(&partial_parts[1])
            } else {
                Vec::new()
            };
            if values.is_empty() {
                attrs.remove(&name.to_ascii_lowercase());
            } else {
                attrs.insert(name.to_ascii_lowercase(), values);
            }
        }
    }
    let mut contact = attrs_to_contact(&dn, &attrs, Some(existing.id));
    contact.updated_at = chrono::Utc::now().timestamp();
    match store.upsert(contact) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn handle_delete(op: &StructureTag, store: &Arc<dyn ContactStore>) -> u64 {
    let Some(dn) = op
        .clone()
        .expect_primitive()
        .and_then(|b| String::from_utf8(b).ok())
        .or_else(|| {
            op.clone()
                .expect_constructed()
                .and_then(|p| p.into_iter().next())
                .and_then(|t| octet_string(&t))
        })
    else {
        return 1;
    };
    let Some(existing) = find_by_dn(store, &dn) else {
        return 32;
    };
    match store.delete(existing.id) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn find_by_dn(store: &Arc<dyn ContactStore>, dn: &str) -> Option<ContactDto> {
    let dn_l = dn.to_ascii_lowercase();
    store.list().ok()?.into_iter().find(|c| contact_dn(c).to_ascii_lowercase() == dn_l)
}

fn contact_dn(contact: &ContactDto) -> String {
    if let Some(dn) = &contact.ldap_dn {
        if !dn.trim().is_empty() {
            return dn.clone();
        }
    }
    format!("uid={},{}", contact.id, DEFAULT_BASE_DN)
}

fn contact_attrs(contact: &ContactDto) -> HashMap<String, Vec<String>> {
    // Canonical LDAP attribute names (caseExact for wire; clients match case-insensitively).
    let mut attrs: HashMap<String, Vec<String>> = HashMap::new();
    attrs.insert(
        "objectClass".into(),
        vec![
            "top".into(),
            "person".into(),
            "organizationalPerson".into(),
            "inetOrgPerson".into(),
        ],
    );
    attrs.insert("cn".into(), vec![contact.display_name.clone()]);
    if !contact.display_name.is_empty() {
        attrs.insert("displayName".into(), vec![contact.display_name.clone()]);
    }
    if !contact.given_name.is_empty() {
        attrs.insert("givenName".into(), vec![contact.given_name.clone()]);
    }
    let sn = if contact.family_name.is_empty() {
        contact.display_name.clone()
    } else {
        contact.family_name.clone()
    };
    attrs.insert("sn".into(), vec![sn]);
    if !contact.emails.is_empty() {
        attrs.insert("mail".into(), contact.emails.clone());
    }
    if !contact.phones.is_empty() {
        attrs.insert("telephoneNumber".into(), contact.phones.clone());
    }
    if !contact.faxes.is_empty() {
        attrs.insert("facsimileTelephoneNumber".into(), contact.faxes.clone());
    }
    if !contact.organization.is_empty() {
        attrs.insert("o".into(), vec![contact.organization.clone()]);
    }
    if !contact.job_title.is_empty() {
        attrs.insert("title".into(), vec![contact.job_title.clone()]);
    }
    if !contact.notes.is_empty() {
        attrs.insert("description".into(), vec![contact.notes.clone()]);
    }
    if let Some(addr) = contact.addresses.first() {
        if !addr.street.is_empty() {
            attrs.insert("street".into(), vec![addr.street.clone()]);
        }
        if !addr.city.is_empty() {
            attrs.insert("l".into(), vec![addr.city.clone()]);
        }
        if !addr.region.is_empty() {
            attrs.insert("st".into(), vec![addr.region.clone()]);
        }
        if !addr.postal_code.is_empty() {
            attrs.insert("postalCode".into(), vec![addr.postal_code.clone()]);
        }
        if !addr.country.is_empty() {
            attrs.insert("c".into(), vec![addr.country.clone()]);
        }
    }
    attrs.insert("uid".into(), vec![contact.id.to_string()]);
    attrs
}

fn parse_attr_list(tag: &StructureTag) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let Some(items) = tag.clone().expect_constructed() else {
        return out;
    };
    for item in items {
        let Some(parts) = item.expect_constructed() else {
            continue;
        };
        if parts.is_empty() {
            continue;
        }
        let Some(name) = octet_string(&parts[0]) else {
            continue;
        };
        let values = if parts.len() > 1 {
            set_values(&parts[1])
        } else {
            Vec::new()
        };
        out.insert(name.to_ascii_lowercase(), values);
    }
    out
}

fn set_values(tag: &StructureTag) -> Vec<String> {
    match tag.clone().expect_constructed() {
        Some(vals) => vals.iter().filter_map(octet_string).collect(),
        None => octet_string(tag).into_iter().collect(),
    }
}

fn attrs_to_contact(
    dn: &str,
    attrs: &HashMap<String, Vec<String>>,
    existing_id: Option<Uuid>,
) -> ContactDto {
    let id = existing_id.unwrap_or_else(|| {
        attrs
            .get("uid")
            .and_then(|v| v.first())
            .and_then(|u| Uuid::parse_str(u).ok())
            .unwrap_or_else(Uuid::new_v4)
    });
    let given_name = first(attrs, "givenname").unwrap_or_default();
    let family_name = first(attrs, "sn").unwrap_or_default();
    let display_name = first(attrs, "displayname")
        .or_else(|| first(attrs, "cn"))
        .unwrap_or_else(|| format!("{given_name} {family_name}").trim().to_string());
    let mut addresses = Vec::new();
    let street = first(attrs, "street").unwrap_or_default();
    let city = first(attrs, "l").unwrap_or_default();
    let region = first(attrs, "st").unwrap_or_default();
    let postal_code = first(attrs, "postalcode").unwrap_or_default();
    let country = first(attrs, "c").unwrap_or_default();
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
    ContactDto {
        id,
        display_name,
        given_name,
        family_name,
        emails: attr_list(attrs, "mail"),
        phones: attr_list(attrs, "telephoneNumber"),
        faxes: attr_list(attrs, "facsimileTelephoneNumber"),
        organization: first(attrs, "o").unwrap_or_default(),
        job_title: first(attrs, "title").unwrap_or_default(),
        addresses,
        custom_fields: Vec::new(),
        photo_base64: None,
        ldap_dn: Some(dn.to_string()),
        notes: first(attrs, "description").unwrap_or_default(),
        updated_at: chrono::Utc::now().timestamp(),
    }
}

fn attr_list(attrs: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn first(attrs: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.first())
        .cloned()
}

fn octet_string(tag: &StructureTag) -> Option<String> {
    tag.clone()
        .expect_primitive()
        .and_then(|b| String::from_utf8(b).ok())
}

fn ldap_result_message(msg_id: i64, app_id: u64, result_code: u64, text: &str) -> StructureTag {
    Tag::Sequence(Sequence {
        inner: vec![
            Tag::Integer(Integer {
                inner: msg_id,
                ..Default::default()
            }),
            Tag::Sequence(Sequence {
                id: app_id,
                class: TagClass::Application,
                inner: vec![
                    Tag::Enumerated(Enumerated {
                        inner: result_code as i64,
                        ..Default::default()
                    }),
                    Tag::OctetString(OctetString {
                        inner: Vec::new(),
                        ..Default::default()
                    }),
                    Tag::OctetString(OctetString {
                        inner: text.as_bytes().to_vec(),
                        ..Default::default()
                    }),
                ],
            }),
        ],
        ..Default::default()
    })
    .into_structure()
}

fn search_entry_message(
    msg_id: i64,
    dn: &str,
    attrs: &HashMap<String, Vec<String>>,
) -> StructureTag {
    let attr_tags = attrs
        .iter()
        .filter(|(_, vals)| !vals.is_empty())
        .map(|(name, vals)| {
            Tag::Sequence(Sequence {
                inner: vec![
                    Tag::OctetString(OctetString {
                        inner: name.as_bytes().to_vec(),
                        ..Default::default()
                    }),
                    Tag::Set(Set {
                        inner: vals
                            .iter()
                            .map(|v| {
                                Tag::OctetString(OctetString {
                                    inner: v.as_bytes().to_vec(),
                                    ..Default::default()
                                })
                            })
                            .collect(),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })
        })
        .collect();

    Tag::Sequence(Sequence {
        inner: vec![
            Tag::Integer(Integer {
                inner: msg_id,
                ..Default::default()
            }),
            Tag::Sequence(Sequence {
                id: 4,
                class: TagClass::Application,
                inner: vec![
                    Tag::OctetString(OctetString {
                        inner: dn.as_bytes().to_vec(),
                        ..Default::default()
                    }),
                    Tag::Sequence(Sequence {
                        inner: attr_tags,
                        ..Default::default()
                    }),
                ],
            }),
        ],
        ..Default::default()
    })
    .into_structure()
}

/// Retry only when a previous listener has not dropped the socket yet.
async fn bind_with_retry(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let mut last = None;
    for attempt in 0..8 {
        match TcpListener::bind(addr).await {
            Ok(listener) => return Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && attempt < 7 => {
                last = Some(err);
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(last.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::AddrInUse, "bind failed")
    }))
}

fn advertise_host_port(listen_addr: &str) -> String {
    let port = listen_addr
        .rsplit_once(':')
        .map(|(_, p)| p)
        .unwrap_or("1389");
    let host_part = listen_addr
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(listen_addr);
    let wildcard = host_part == "0.0.0.0" || host_part == "::" || host_part == "[::]";
    if wildcard {
        let host = detect_lan_ipv4().unwrap_or_else(|| "127.0.0.1".into());
        format!("{host}:{port}")
    } else {
        let host = host_part.trim_matches(|c| c == '[' || c == ']');
        format!("{host}:{port}")
    }
}

fn detect_lan_ipv4() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use novamail_ipc::LdapSearchRequest;
    use parking_lot::Mutex;

    struct MemStore(Mutex<Vec<ContactDto>>);

    #[async_trait]
    impl ContactStore for MemStore {
        fn list(&self) -> ContactsResult<Vec<ContactDto>> {
            Ok(self.0.lock().clone())
        }
        fn get(&self, id: Uuid) -> ContactsResult<ContactDto> {
            self.0
                .lock()
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .ok_or_else(|| ContactsError::NotFound(id.to_string()))
        }
        fn upsert(&self, contact: ContactDto) -> ContactsResult<()> {
            let mut g = self.0.lock();
            if let Some(slot) = g.iter_mut().find(|c| c.id == contact.id) {
                *slot = contact;
            } else {
                g.push(contact);
            }
            Ok(())
        }
        fn delete(&self, id: Uuid) -> ContactsResult<()> {
            self.0.lock().retain(|c| c.id != id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn ldap_client_can_search_hub() {
        let id = Uuid::new_v4();
        let store = Arc::new(MemStore(Mutex::new(vec![ContactDto {
            id,
            display_name: "Ada Lovelace".into(),
            given_name: "Ada".into(),
            family_name: "Lovelace".into(),
            emails: vec!["ada@example.com".into()],
            phones: vec!["030111".into()],
            faxes: vec!["030999".into()],
            organization: "Analytical".into(),
            job_title: "Engineer".into(),
            addresses: vec![],
            custom_fields: vec![],
            photo_base64: None,
            ldap_dn: None,
            notes: String::new(),
            updated_at: 1,
        }])));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let server = LdapServer::new(store)
            .with_addr(addr.to_string());
        server.set_also_listen_389(false);
        server.set_credentials("novamail", "secret");
        server.start().await.unwrap();

        let found = crate::search_ldap(&LdapSearchRequest {
            url: format!("ldap://{addr}"),
            bind_dn: Some(DEFAULT_BIND_DN.into()),
            password: Some("secret".into()),
            base_dn: DEFAULT_BASE_DN.into(),
            filter: "(objectClass=inetOrgPerson)".into(),
        })
        .await
        .unwrap();
        assert!(
            found
                .iter()
                .any(|c| c.emails.iter().any(|e| e == "ada@example.com")),
            "expected Ada in {found:?}"
        );
        let ada = found
            .iter()
            .find(|c| c.display_name.contains("Ada"))
            .expect("ada");
        assert!(
            ada.faxes.iter().any(|f| f == "030999"),
            "canonical facsimileTelephoneNumber must round-trip: {ada:?}"
        );
        assert!(
            ada.phones.iter().any(|p| p == "030111"),
            "canonical telephoneNumber must round-trip: {ada:?}"
        );

        // Filter must exclude non-matches (Ricoh-style search).
        let filtered = crate::search_ldap(&LdapSearchRequest {
            url: format!("ldap://{addr}"),
            bind_dn: Some(DEFAULT_BIND_DN.into()),
            password: Some("secret".into()),
            base_dn: DEFAULT_BASE_DN.into(),
            filter: "(cn=*NoSuchPerson*)".into(),
        })
        .await
        .unwrap();
        assert!(
            filtered.is_empty(),
            "filter should exclude Ada, got {filtered:?}"
        );

        let _ = server.stop();
    }

    #[tokio::test]
    async fn anonymous_read_only_when_enabled() {
        let store = Arc::new(MemStore(Mutex::new(vec![ContactDto {
            id: Uuid::new_v4(),
            display_name: "Fax Desk".into(),
            given_name: String::new(),
            family_name: "Desk".into(),
            emails: vec![],
            phones: vec![],
            faxes: vec!["040123".into()],
            organization: String::new(),
            job_title: String::new(),
            addresses: vec![],
            custom_fields: vec![],
            photo_base64: None,
            ldap_dn: None,
            notes: String::new(),
            updated_at: 1,
        }])));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let server = LdapServer::new(store).with_addr(addr.to_string());
        server.set_also_listen_389(false);
        server.set_allow_anonymous(true);
        server.set_credentials("novamail", "secret");
        server.start().await.unwrap();

        let found = crate::search_ldap(&LdapSearchRequest {
            url: format!("ldap://{addr}"),
            bind_dn: Some(String::new()),
            password: Some(String::new()),
            base_dn: DEFAULT_BASE_DN.into(),
            filter: "(facsimileTelephoneNumber=*)".into(),
        })
        .await
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].faxes, vec!["040123".to_string()]);
        let _ = server.stop();
    }
}
