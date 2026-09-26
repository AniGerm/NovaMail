//! Localhost OAuth redirect listener for the desktop shell.
//!
//! Listens on `127.0.0.1:17832` for `/oauth/callback?code=...&state=...`.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use crate::{MailError, MailResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCallbackResult {
    pub code: String,
    pub state: Option<String>,
}

pub async fn wait_for_oauth_callback(timeout: Duration) -> MailResult<OAuthCallbackResult> {
    let listener = TcpListener::bind("127.0.0.1:17832")
        .await
        .map_err(|e| MailError::OAuth(format!("failed to bind OAuth callback port: {e}")))?;

    let accept = async {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|e| MailError::OAuth(format!("oauth accept failed: {e}")))?;
        let mut buf = vec![0u8; 4096];
        let n = socket
            .read(&mut buf)
            .await
            .map_err(|e| MailError::OAuth(format!("oauth read failed: {e}")))?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or_default();
        let path = first_line.split_whitespace().nth(1).unwrap_or("/");
        let url = Url::parse(&format!("http://127.0.0.1:17832{path}"))
            .map_err(|e| MailError::OAuth(format!("invalid callback URL: {e}")))?;
        let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let code = params
            .get("code")
            .cloned()
            .ok_or_else(|| MailError::OAuth("callback missing code".into()))?;
        if let Some(error) = params.get("error") {
            return Err(MailError::OAuth(format!(
                "provider returned error: {error}"
            )));
        }
        let body = "<!doctype html><html><body style=\"font-family:sans-serif;padding:2rem\"><h1>NovaMail</h1><p>Sign-in complete. You can close this window.</p></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        Ok(OAuthCallbackResult {
            code,
            state: params.get("state").cloned(),
        })
    };

    tokio::time::timeout(timeout, accept)
        .await
        .map_err(|_| MailError::OAuth("timed out waiting for OAuth callback".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn captures_code_from_redirect() {
        let server = tokio::spawn(async { wait_for_oauth_callback(Duration::from_secs(5)).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = TcpStream::connect("127.0.0.1:17832").await.unwrap();
        client
            .write_all(b"GET /oauth/callback?code=abc123&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let result = server.await.unwrap().unwrap();
        assert_eq!(result.code, "abc123");
        assert_eq!(result.state.as_deref(), Some("xyz"));
    }
}
