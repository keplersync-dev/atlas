//! The end of a browser sign-in: a one-shot HTTP listener on the loopback address that the
//! browser is redirected to with `?code=…` (or `?error=…`). The Atlas server sends it to
//! `http://127.0.0.1:{port}/callback`, Microsoft to `http://localhost:{port}/`; browsers
//! may try `localhost` over IPv6 first, so it listens on both.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use crate::error::{AppError, AppResult};

pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub struct Listener {
    v4: TcpListener,
    v6: Option<TcpListener>,
    pub port: u16,
}

pub async fn bind() -> AppResult<Listener> {
    let v4 = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
        tracing::error!(error = %e, "couldn't open the sign-in listener");
        AppError::unknown("Couldn't start signing in")
    })?;
    let port = v4.local_addr().map(|a| a.port()).unwrap_or_default();
    let v6 = TcpListener::bind(("::1", port)).await.ok();
    Ok(Listener { v4, v6, port })
}

/// Where the browser comes back, and the OAuth `state` it must bring (when the sign-in
/// sent one).
pub struct Expect<'a> {
    pub path: &'a str,
    pub state: Option<&'a str>,
}

impl Listener {
    /// Wait for the browser to come back. Other requests (a favicon) get a 404.
    pub async fn wait_for_code(self, expect: Expect<'_>) -> AppResult<String> {
        tokio::time::timeout(TIMEOUT, async {
            loop {
                let Ok(stream) = self.accept().await else {
                    continue;
                };
                if let Some(result) = handle(stream, &expect).await {
                    return result;
                }
            }
        })
        .await
        .map_err(|_| AppError::invalid_input("Sign-in timed out"))?
    }

    async fn accept(&self) -> std::io::Result<TcpStream> {
        let (stream, _) = match &self.v6 {
            Some(v6) => tokio::select! {
                r = self.v4.accept() => r,
                r = v6.accept() => r,
            }?,
            None => self.v4.accept().await?,
        };
        Ok(stream)
    }
}

async fn handle(mut stream: TcpStream, expect: &Expect<'_>) -> Option<AppResult<String>> {
    let mut buf = vec![0u8; 8192];
    let mut len = 0;
    while len < buf.len() {
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf[len..]))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        len += n;
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..len]);
    let target = head
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|rest| rest.split(' ').next())?;
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != expect.path {
        respond(&mut stream, "404 Not Found", "").await;
        return None;
    }
    let get = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    let state_ok = expect
        .state
        .is_none_or(|s| get("state").as_deref() == Some(s));
    let (result, title, text) = match (get("code"), get("error").as_deref()) {
        (Some(code), _) if state_ok => (
            Ok(code),
            "You're signed in",
            "You can close this tab and go back to Atlas.",
        ),
        // Atlas's server says "cancelled", Microsoft "access_denied".
        (None, Some("cancelled" | "access_denied")) => (
            Err(AppError::invalid_input("Sign-in was cancelled")),
            "Sign-in cancelled",
            "You can close this tab. Nothing was changed.",
        ),
        _ => (
            Err(AppError::unknown("Signing in didn't work. Try again.")),
            "Signing in didn't work",
            "Close this tab and try again from Atlas.",
        ),
    };
    respond(&mut stream, "200 OK", &page(title, text)).await;
    Some(result)
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn page(title: &str, text: &str) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>{title} · Atlas</title>\
         <body style=\"font:16px system-ui;max-width:28rem;margin:20vh auto;padding:0 1rem;\
         background:#111418;color:#e8eaed\"><h1 style=\"font-size:1.25rem\">{title}</h1>\
         <p style=\"color:#9aa3ad\">{text}</p>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALLBACK: Expect<'static> = Expect {
        path: "/callback",
        state: None,
    };

    async fn browser(port: u16, target: &str) -> String {
        browser_at("127.0.0.1", port, target).await
    }

    async fn browser_at(host: &str, port: u16, target: &str) -> String {
        let mut s = TcpStream::connect((host, port)).await.unwrap();
        s.write_all(format!("GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn takes_the_code_and_ignores_other_requests() {
        let listener = bind().await.unwrap();
        let port = listener.port;
        let waiting = tokio::spawn(listener.wait_for_code(CALLBACK));
        assert!(
            browser(port, "/favicon.ico")
                .await
                .starts_with("HTTP/1.1 404")
        );
        let page = browser(port, "/callback?code=abc%2B1").await;
        assert!(page.contains("You're signed in"));
        assert_eq!(waiting.await.unwrap().unwrap(), "abc+1");
    }

    #[tokio::test]
    async fn reports_a_cancelled_sign_in() {
        let listener = bind().await.unwrap();
        let port = listener.port;
        let waiting = tokio::spawn(listener.wait_for_code(CALLBACK));
        browser(port, "/callback?error=cancelled").await;
        assert_eq!(
            waiting.await.unwrap().unwrap_err().message,
            "Sign-in was cancelled"
        );
    }

    #[tokio::test]
    async fn microsoft_comes_back_to_the_root_over_ipv6_with_its_state() {
        let listener = bind().await.unwrap();
        let port = listener.port;
        let expect = Expect {
            path: "/",
            state: Some("s1"),
        };
        let waiting = tokio::spawn(listener.wait_for_code(expect));
        let page = browser_at("::1", port, "/?code=M.C1&state=s1").await;
        assert!(page.contains("You're signed in"));
        assert_eq!(waiting.await.unwrap().unwrap(), "M.C1");

        let listener = bind().await.unwrap();
        let port = listener.port;
        let expect = Expect {
            path: "/",
            state: Some("s1"),
        };
        let waiting = tokio::spawn(listener.wait_for_code(expect));
        browser(port, "/?code=M.C1&state=forged").await;
        assert!(waiting.await.unwrap().is_err(), "another sign-in's code");
    }
}
