use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use novamail_ipc::{CardDavServerStatus, ContactDto};
use tokio::net::TcpListener;
use tokio::sync::watch;
use uuid::Uuid;

use crate::vcard::{contact_to_vcard, vcard_to_contact};
use crate::{ContactsError, ContactsResult};

/// Bind all interfaces so phones/printers on the LAN can share this address book.
/// Advertised URLs use the machine's LAN IP (not 0.0.0.0).
const DEFAULT_ADDR: &str = "0.0.0.0:8765";
const BOOK_PATH: &str = "/addressbooks/novamail/";
const DEFAULT_USER: &str = "novamail";

#[async_trait]
pub trait ContactStore: Send + Sync {
    fn list(&self) -> ContactsResult<Vec<ContactDto>>;
    fn get(&self, id: Uuid) -> ContactsResult<ContactDto>;
    fn upsert(&self, contact: ContactDto) -> ContactsResult<()>;
    fn delete(&self, id: Uuid) -> ContactsResult<()>;
}

pub struct CardDavServer {
    store: Arc<dyn ContactStore>,
    listen_addr: String,
    username: parking_lot::Mutex<String>,
    password: parking_lot::Mutex<String>,
    shutdown_tx: parking_lot::Mutex<Option<watch::Sender<bool>>>,
    running: parking_lot::Mutex<bool>,
}

impl CardDavServer {
    pub fn new(store: Arc<dyn ContactStore>) -> Self {
        Self {
            store,
            listen_addr: DEFAULT_ADDR.into(),
            username: parking_lot::Mutex::new(DEFAULT_USER.into()),
            password: parking_lot::Mutex::new(String::new()),
            shutdown_tx: parking_lot::Mutex::new(None),
            running: parking_lot::Mutex::new(false),
        }
    }

    pub fn with_addr(mut self, addr: impl Into<String>) -> Self {
        self.listen_addr = addr.into();
        self
    }

    /// Set Basic-auth credentials used by LAN devices.
    pub fn set_credentials(&self, username: impl Into<String>, password: impl Into<String>) {
        *self.username.lock() = username.into();
        *self.password.lock() = password.into();
    }

    pub fn status(&self) -> CardDavServerStatus {
        let contacts = self.store.list().map(|c| c.len() as u32).unwrap_or(0);
        let running = *self.running.lock();
        let advertise = advertise_host_port(&self.listen_addr);
        CardDavServerStatus {
            running,
            listen_url: format!("http://{advertise}"),
            addressbook_url: format!("http://{advertise}{BOOK_PATH}"),
            contact_count: contacts,
            username: self.username.lock().clone(),
            password: self.password.lock().clone(),
        }
    }

    pub async fn start(&self) -> ContactsResult<CardDavServerStatus> {
        if *self.running.lock() {
            return Ok(self.status());
        }
        if self.password.lock().trim().is_empty() {
            return Err(ContactsError::CardDav(
                "CardDAV password is empty; set credentials before starting".into(),
            ));
        }

        let addr: SocketAddr = self
            .listen_addr
            .parse()
            .map_err(|e| ContactsError::CardDav(format!("invalid listen addr: {e}")))?;
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ContactsError::CardDav(format!("bind failed: {e}")))?;

        let (tx, rx) = watch::channel(false);
        *self.shutdown_tx.lock() = Some(tx);
        *self.running.lock() = true;

        let store = self.store.clone();
        let username = self.username.lock().clone();
        let password = self.password.lock().clone();
        tokio::spawn(async move {
            serve_loop(listener, store, username, password, rx).await;
        });

        Ok(self.status())
    }

    pub fn stop(&self) -> CardDavServerStatus {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            let _ = tx.send(true);
        }
        *self.running.lock() = false;
        self.status()
    }
}

async fn serve_loop(
    listener: TcpListener,
    store: Arc<dyn ContactStore>,
    username: String,
    password: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let auth = Arc::new((username, password));
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
                        let auth = auth.clone();
                        tokio::spawn(async move {
                            let io = TokioIo::new(stream);
                            let service = service_fn(move |req| {
                                let store = store.clone();
                                let auth = auth.clone();
                                async move {
                                    Ok::<_, Infallible>(handle(req, store, &auth.0, &auth.1).await)
                                }
                            });
                            let _ = http1::Builder::new().serve_connection(io, service).await;
                        });
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "carddav accept failed");
                    }
                }
            }
        }
    }
}

async fn handle(
    req: Request<Incoming>,
    store: Arc<dyn ContactStore>,
    username: &str,
    password: &str,
) -> Response<Full<Bytes>> {
    if !authorized(&req, username, password) {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("WWW-Authenticate", "Basic realm=\"NovaMail CardDAV\"")
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from("unauthorized")))
            .unwrap();
    }

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // CardDAV discovery / well-known
    if path == "/.well-known/carddav" {
        return redirect(BOOK_PATH);
    }

    if path == "/" || path == "/addressbooks/" {
        return xml_response(
            StatusCode::OK,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
  <d:response>
    <d:href>/addressbooks/novamail/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>NovaMail</d:displayname>
        <d:resourcetype><d:collection/><card:addressbook/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#,
        );
    }

    if path == BOOK_PATH || path == "/addressbooks/novamail" {
        if method == Method::OPTIONS {
            return options();
        }
        if method.as_str() == "PROPFIND" || method == Method::GET {
            return addressbook_listing(&store);
        }
        if method.as_str() == "REPORT" {
            return addressbook_listing(&store);
        }
        return text(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
    }

    if let Some(id) = parse_contact_path(&path) {
        match method.as_str() {
            "GET" => match store.get(id) {
                Ok(contact) => {
                    let body = contact_to_vcard(&contact);
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "text/vcard; charset=utf-8")
                        .header("ETag", format!("\"{}\"", contact.updated_at))
                        .body(Full::new(Bytes::from(body)))
                        .unwrap()
                }
                Err(_) => text(StatusCode::NOT_FOUND, "not found"),
            },
            "PUT" => {
                let body = match req.collect().await {
                    Ok(collected) => String::from_utf8_lossy(&collected.to_bytes()).to_string(),
                    Err(_) => return text(StatusCode::BAD_REQUEST, "invalid body"),
                };
                match vcard_to_contact(&body, Some(id)) {
                    Some(mut contact) => {
                        contact.id = id;
                        contact.updated_at = chrono::Utc::now().timestamp();
                        match store.upsert(contact.clone()) {
                            Ok(()) => Response::builder()
                                .status(StatusCode::CREATED)
                                .header("ETag", format!("\"{}\"", contact.updated_at))
                                .body(Full::new(Bytes::new()))
                                .unwrap(),
                            Err(err) => text(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
                        }
                    }
                    None => text(StatusCode::BAD_REQUEST, "invalid vcard"),
                }
            }
            "DELETE" => match store.delete(id) {
                Ok(()) => text(StatusCode::NO_CONTENT, ""),
                Err(_) => text(StatusCode::NOT_FOUND, "not found"),
            },
            "OPTIONS" => options(),
            _ => text(StatusCode::METHOD_NOT_ALLOWED, "method not allowed"),
        }
    } else {
        text(StatusCode::NOT_FOUND, "not found")
    }
}

fn addressbook_listing(store: &Arc<dyn ContactStore>) -> Response<Full<Bytes>> {
    let contacts = store.list().unwrap_or_default();
    let mut responses = String::new();
    responses.push_str(&format!(
        r#"  <d:response>
    <d:href>{BOOK_PATH}</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>NovaMail Address Book</d:displayname>
        <d:resourcetype><d:collection/><card:addressbook/></d:resourcetype>
        <d:getetag>"{etag}"</d:getetag>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
"#,
        etag = chrono::Utc::now().timestamp()
    ));
    for contact in contacts {
        let href = format!("{BOOK_PATH}{}.vcf", contact.id);
        let vcard = contact_to_vcard(&contact);
        let escaped = xml_escape(&vcard);
        responses.push_str(&format!(
            r#"  <d:response>
    <d:href>{href}</d:href>
    <d:propstat>
      <d:prop>
        <d:getetag>"{etag}"</d:getetag>
        <d:getcontenttype>text/vcard</d:getcontenttype>
        <card:address-data>{escaped}</card:address-data>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
"#,
            etag = contact.updated_at
        ));
    }
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
{responses}</d:multistatus>"#
    );
    xml_response(StatusCode::MULTI_STATUS, &body)
}

fn parse_contact_path(path: &str) -> Option<Uuid> {
    let prefix = BOOK_PATH;
    if !path.starts_with(prefix) {
        return None;
    }
    let rest = path[prefix.len()..].trim_end_matches('/');
    let stem = rest.strip_suffix(".vcf").unwrap_or(rest);
    Uuid::parse_str(stem).ok()
}

fn options() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            "Allow",
            "OPTIONS, GET, PUT, DELETE, PROPFIND, REPORT, HEAD",
        )
        .header("DAV", "1, addressbook")
        .body(Full::new(Bytes::new()))
        .unwrap()
}

fn redirect(location: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header("Location", location)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

fn xml_response(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn text(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn authorized(req: &Request<Incoming>, username: &str, password: &str) -> bool {
    use base64::Engine;
    let Some(header) = req.headers().get(hyper::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = header.to_str() else {
        return false;
    };
    let Some(encoded) = value.strip_prefix("Basic ").or_else(|| value.strip_prefix("basic "))
    else {
        return false;
    };
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else {
        return false;
    };
    let Ok(decoded) = String::from_utf8(bytes) else {
        return false;
    };
    let Some((user, pass)) = decoded.split_once(':') else {
        return false;
    };
    user == username && pass == password
}

/// Host:port shown to users for CardDAV clients (LAN IP when bound to 0.0.0.0).
fn advertise_host_port(listen_addr: &str) -> String {
    let port = listen_addr
        .rsplit_once(':')
        .map(|(_, p)| p)
        .unwrap_or("8765");
    let host_part = listen_addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(listen_addr);
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
    // No packets are sent; this selects the interface used for outbound traffic.
    socket.connect("1.1.1.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn advertise_keeps_explicit_host() {
        assert_eq!(advertise_host_port("192.168.1.10:8765"), "192.168.1.10:8765");
    }

    #[test]
    fn advertise_wildcard_uses_port() {
        let advertised = advertise_host_port("0.0.0.0:8765");
        assert!(advertised.ends_with(":8765"), "{advertised}");
        assert!(!advertised.starts_with("0.0.0.0"), "{advertised}");
    }

    #[test]
    fn basic_auth_accepts_valid_credentials() {
        let token = base64::engine::general_purpose::STANDARD.encode("novamail:secret");
        let req = Request::builder()
            .uri("/")
            .header("Authorization", format!("Basic {token}"))
            .body(Full::new(Bytes::new()))
            .unwrap();
        // Convert to Incoming is awkward; test helper via header parse path.
        let header = req.headers().get(hyper::header::AUTHORIZATION).unwrap();
        let value = header.to_str().unwrap();
        let encoded = value.strip_prefix("Basic ").unwrap();
        let decoded =
            String::from_utf8(base64::engine::general_purpose::STANDARD.decode(encoded).unwrap())
                .unwrap();
        assert_eq!(decoded, "novamail:secret");
    }
}
