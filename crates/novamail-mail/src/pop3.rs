//! Minimal POP3 client for providers that do not offer IMAP.
//!
//! Supports USER/PASS auth over TLS (or cleartext for local bridges), LIST + RETR,
//! and optional DELE after successful local store by the caller.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::tls::connect_tls;
use crate::{MailError, MailResult};

enum PopStream {
    Plain(BufReader<TcpStream>),
    Tls(BufReader<TlsStream<TcpStream>>),
}

impl PopStream {
    async fn write_all(&mut self, data: &[u8]) -> MailResult<()> {
        match self {
            Self::Plain(s) => s.get_mut().write_all(data).await,
            Self::Tls(s) => s.get_mut().write_all(data).await,
        }
        .map_err(|e| MailError::Pop3(e.to_string()))
    }

    async fn read_line(&mut self, buf: &mut String) -> MailResult<usize> {
        buf.clear();
        match self {
            Self::Plain(s) => s.read_line(buf).await,
            Self::Tls(s) => s.read_line(buf).await,
        }
        .map_err(|e| MailError::Pop3(e.to_string()))
    }
}

pub struct Pop3Client {
    stream: PopStream,
}

pub struct Pop3Message {
    pub index: u32,
    pub size: u64,
    pub raw: Vec<u8>,
}

impl Pop3Client {
    pub async fn connect(host: &str, port: u16, use_tls: bool) -> MailResult<Self> {
        let mut stream = if use_tls {
            let tls = connect_tls(host, port).await?;
            PopStream::Tls(BufReader::new(tls))
        } else {
            let addr = format!("{host}:{port}");
            let tcp = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect(&addr))
                .await
                .map_err(|_| MailError::Pop3("connect timeout".into()))?
                .map_err(|e| MailError::Pop3(e.to_string()))?;
            PopStream::Plain(BufReader::new(tcp))
        };

        let mut greeting = String::new();
        stream.read_line(&mut greeting).await?;
        ensure_ok(&greeting)?;

        Ok(Self { stream })
    }

    pub async fn login(&mut self, user: &str, password: &str) -> MailResult<()> {
        self.command(&format!("USER {user}")).await?;
        self.command(&format!("PASS {password}")).await?;
        Ok(())
    }

    pub async fn list(&mut self) -> MailResult<Vec<(u32, u64)>> {
        let response = self.multiline("LIST").await?;
        let mut out = Vec::new();
        for line in response.lines().skip(1) {
            if line == "." {
                break;
            }
            let mut parts = line.split_whitespace();
            let idx = parts
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| MailError::Pop3(format!("bad LIST line: {line}")))?;
            let size = parts
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| MailError::Pop3(format!("bad LIST line: {line}")))?;
            out.push((idx, size));
        }
        Ok(out)
    }

    pub async fn retr(&mut self, index: u32) -> MailResult<Vec<u8>> {
        let response = self.multiline(&format!("RETR {index}")).await?;
        let mut lines = response.lines();
        let _status = lines.next();
        let mut body = String::new();
        for line in lines {
            if line == "." {
                break;
            }
            let content = line.strip_prefix('.').unwrap_or(line);
            body.push_str(content);
            body.push('\n');
        }
        Ok(body.into_bytes())
    }

    pub async fn dele(&mut self, index: u32) -> MailResult<()> {
        self.command(&format!("DELE {index}")).await?;
        Ok(())
    }

    pub async fn quit(mut self) -> MailResult<()> {
        let _ = self.command("QUIT").await;
        Ok(())
    }

    pub async fn fetch_all(&mut self) -> MailResult<Vec<Pop3Message>> {
        let listed = self.list().await?;
        let mut messages = Vec::new();
        for (index, size) in listed {
            let raw = self.retr(index).await?;
            messages.push(Pop3Message { index, size, raw });
        }
        Ok(messages)
    }

    async fn command(&mut self, cmd: &str) -> MailResult<String> {
        self.stream
            .write_all(format!("{cmd}\r\n").as_bytes())
            .await?;
        let mut line = String::new();
        self.stream.read_line(&mut line).await?;
        ensure_ok(&line)?;
        Ok(line)
    }

    async fn multiline(&mut self, cmd: &str) -> MailResult<String> {
        self.stream
            .write_all(format!("{cmd}\r\n").as_bytes())
            .await?;
        let mut out = String::new();
        loop {
            let mut line = String::new();
            let n = self.stream.read_line(&mut line).await?;
            if n == 0 {
                break;
            }
            out.push_str(&line);
            if line.trim_end() == "." {
                break;
            }
            if out.lines().count() == 1 {
                ensure_ok(&line)?;
            }
        }
        Ok(out)
    }
}

fn ensure_ok(line: &str) -> MailResult<()> {
    if line.starts_with("+OK") {
        Ok(())
    } else {
        Err(MailError::Pop3(line.trim().to_string()))
    }
}
