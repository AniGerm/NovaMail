use std::sync::Arc;

use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use crate::{MailError, MailResult};

pub fn connector() -> MailResult<TlsConnector> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

pub async fn connect_tls(host: &str, port: u16) -> MailResult<TlsStream<TcpStream>> {
    let addr = format!("{host}:{port}");
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| MailError::Tls(format!("tcp connect {addr}: {e}")))?;
    let connector = connector()?;
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| MailError::Tls(format!("invalid server name {host}: {e}")))?;
    connector
        .connect(server_name, stream)
        .await
        .map_err(|e| MailError::Tls(format!("tls handshake {host}: {e}")))
}
