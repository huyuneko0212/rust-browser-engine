#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use brotli::Decompressor;
use flate2::read::GzDecoder;
use native_tls::{HandshakeError, TlsConnector};

use crate::constants::{http_status, network};
use crate::url::URL;

#[derive(Debug)]
pub enum HttpError {
    Io(std::io::Error),
    Tls(native_tls::Error),
    TlsHandshakeWouldBlock,
    InvalidResponse(&'static str),
    UnsupportedEncoding(String),
    DecodeFailed(&'static str),
    TooManyRedirects,
    File(std::io::Error),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::Io(e) => write!(f, "io error: {}", e),
            HttpError::Tls(e) => write!(f, "tls error: {}", e),
            HttpError::TlsHandshakeWouldBlock => write!(f, "tls handshake would-block"),
            HttpError::InvalidResponse(s) => write!(f, "invalid response: {}", s),
            HttpError::UnsupportedEncoding(enc) => {
                write!(f, "unsupported content-encoding: {}", enc)
            }
            HttpError::DecodeFailed(s) => write!(f, "decode failed: {}", s),
            HttpError::TooManyRedirects => write!(f, "too many redirects"),
            HttpError::File(e) => write!(f, "file error: {}", e),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<std::io::Error> for HttpError {
    fn from(e: std::io::Error) -> Self {
        HttpError::Io(e)
    }
}

impl From<native_tls::Error> for HttpError {
    fn from(e: native_tls::Error) -> Self {
        HttpError::Tls(e)
    }
}

/// ★ここが今回の要点：`tls.connect(...)?` を通すための変換
impl From<HandshakeError<TcpStream>> for HttpError {
    fn from(e: HandshakeError<TcpStream>) -> Self {
        match e {
            HandshakeError::Failure(err) => HttpError::Tls(err),
            HandshakeError::WouldBlock(_) => HttpError::TlsHandshakeWouldBlock,
        }
    }
}

pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>, // lower-case key
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

impl Response {
    pub fn body_text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// HTTP Content-Type ヘッダから charset を抽出する
    pub fn charset_from_header(&self) -> Option<String> {
        self.content_type.as_ref().and_then(|ct| {
            ct.split(';').skip(1).find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                if !name.trim().eq_ignore_ascii_case("charset") {
                    return None;
                }

                let charset = value.trim().trim_matches('"').trim_matches('\'');
                if charset.is_empty() {
                    None
                } else {
                    Some(charset.to_lowercase())
                }
            })
        })
    }

    /// 指定された文字コードでボディをデコード
    /// charsetは例："utf-8", "shift_jis", "euc-jp" など
    pub fn body_text_with_charset(&self, charset: &str) -> String {
        let (cow, _, had_errors) = encoding_rs::Encoding::for_label(charset.as_bytes())
            .unwrap_or(encoding_rs::UTF_8)
            .decode(&self.body);

        if had_errors {
            eprintln!(
                "[http] Warning: charset '{}' decoding had errors, using lossy conversion",
                charset
            );
        }

        cow.to_string()
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }
}

/// ✅ “本命” API：失敗を Result で返す（Rustっぽい / ブラウザっぽい）
fn request(url: &URL) -> Result<Response, HttpError> {
    let tls = TlsConnector::new()?;
    request_with_tls(url, &tls)
}

/// ✅ 「落ちないレスポンス」が欲しい用途向け（画像ロード等）
pub fn request_allow_error(url: &URL) -> Response {
    match request(url) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[http] request failed: {} url={}", e, debug_url(url));
            Response {
                status_code: http_status::REQUEST_FAILED,
                headers: HashMap::new(),
                body: Vec::new(),
                content_type: None,
            }
        }
    }
}

fn request_with_tls(url: &URL, tls: &TlsConnector) -> Result<Response, HttpError> {
    let mut current = url.clone();

    for _ in 0..=network::HTTP_MAX_REDIRECTS {
        let resp = match current.scheme.as_str() {
            "file" => request_file(&current)?,
            "http" | "https" => request_http_like(&current, tls)?,
            _ => return Err(HttpError::InvalidResponse("unsupported scheme")),
        };

        if http_status::REDIRECTS.contains(&resp.status_code) {
            if let Some(loc) = resp.header("location").map(str::to_string) {
                current = current.resolve_location(&loc);
                continue;
            }
        }

        return Ok(resp);
    }

    Err(HttpError::TooManyRedirects)
}

fn request_file(url: &URL) -> Result<Response, HttpError> {
    let mut fs_path = url.path.clone();
    fs_path = fs_path.replace('\\', "/");
    let fs_path_os = fs_path.replace('/', "\\");

    let bytes = std::fs::read(&fs_path_os).map_err(HttpError::File)?;
    let content_type = guess_content_type_from_path(&fs_path);

    Ok(Response {
        status_code: http_status::OK,
        headers: HashMap::new(),
        body: bytes,
        content_type,
    })
}

fn request_http_like(url: &URL, tls: &TlsConnector) -> Result<Response, HttpError> {
    let addr = format!("{}:{}", url.host, url.port);
    let stream = TcpStream::connect(addr)?;
    // ブラウザっぽく：固まり防止（必要なら調整）
    let _ = stream.set_read_timeout(Some(Duration::from_secs(network::SOCKET_TIMEOUT_SECS)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(network::SOCKET_TIMEOUT_SECS)));

    let req = build_request(url);

    let raw = match url.scheme.as_str() {
        "https" => {
            // ★ `HandshakeError<TcpStream>` → `HttpError` 変換があるので `?` が通る
            let mut tls_stream = tls.connect(&url.host, stream)?;
            tls_stream.write_all(req.as_bytes())?;

            let mut buf = Vec::new();
            tls_stream.read_to_end(&mut buf)?;
            buf
        }
        "http" => {
            let mut s = stream;
            s.write_all(req.as_bytes())?;

            let mut buf = Vec::new();
            s.read_to_end(&mut buf)?;
            buf
        }
        _ => return Err(HttpError::InvalidResponse("unsupported scheme")),
    };

    parse_response_bytes(raw)
}

/// “ブラウザっぽい” 最低限のリクエスト
fn build_request(url: &URL) -> String {
    let path = if url.path.is_empty() { "/" } else { &url.path };

    format!(
        "GET {} HTTP/1.1\r\n\
Host: {}\r\n\
User-Agent: rust-browser/0.1\r\n\
Accept: text/html,application/xhtml+xml,text/css,image/*,*/*;q=0.8\r\n\
Accept-Language: ja,en-US;q=0.9,en;q=0.8\r\n\
Accept-Encoding: gzip, br\r\n\
Connection: close\r\n\
\r\n",
        path, url.host
    )
}

fn parse_response_bytes(resp: Vec<u8>) -> Result<Response, HttpError> {
    let header_end = find_bytes(&resp, b"\r\n\r\n")
        .ok_or(HttpError::InvalidResponse("missing header terminator"))?;
    let head_bytes = &resp[..header_end];
    let body_bytes = &resp[header_end + 4..];

    let head = String::from_utf8_lossy(head_bytes);
    let mut lines = head.split("\r\n");

    let status_line = lines
        .next()
        .ok_or(HttpError::InvalidResponse("missing status line"))?;

    let mut status_parts = status_line.split_whitespace();
    let _http = status_parts.next().unwrap_or("");
    let status_code: u16 = status_parts
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(http_status::REQUEST_FAILED);

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.to_lowercase(), v.trim().to_string());
        }
    }
    let content_type = headers.get("content-type").cloned();

    if status_code == http_status::NO_CONTENT
        || status_code == http_status::NOT_MODIFIED
        || (http_status::INFORMATIONAL_MIN..http_status::INFORMATIONAL_MAX_EXCLUSIVE)
            .contains(&status_code)
    {
        return Ok(Response {
            status_code,
            headers,
            body: Vec::new(),
            content_type,
        });
    }

    // 1) chunked
    let mut decoded = body_bytes.to_vec();
    if headers
        .get("transfer-encoding")
        .is_some_and(|v| v.to_lowercase().contains("chunked"))
    {
        decoded = decode_chunked(&decoded);
    }

    // 2) content-encoding
    if let Some(enc) = headers.get("content-encoding") {
        let encs: Vec<String> = enc
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty() && s != "identity")
            .collect();

        for e in encs.into_iter().rev() {
            decoded = match e.as_str() {
                "gzip" => decode_gzip(&decoded)?,
                "br" => decode_brotli(&decoded)?,
                other => return Err(HttpError::UnsupportedEncoding(other.to_string())),
            };
        }
    }

    Ok(Response {
        status_code,
        headers,
        body: decoded,
        content_type,
    })
}

fn decode_gzip(input: &[u8]) -> Result<Vec<u8>, HttpError> {
    let mut gz = GzDecoder::new(input);
    let mut out = Vec::new();
    gz.read_to_end(&mut out)
        .map_err(|_| HttpError::DecodeFailed("gzip"))?;
    Ok(out)
}

fn decode_brotli(input: &[u8]) -> Result<Vec<u8>, HttpError> {
    let mut br = Decompressor::new(input, network::BROTLI_BUFFER_SIZE);
    let mut out = Vec::new();
    br.read_to_end(&mut out)
        .map_err(|_| HttpError::DecodeFailed("brotli"))?;
    Ok(out)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < input.len() {
        let line_end = match find_bytes(&input[i..], b"\r\n") {
            Some(p) => i + p,
            None => break,
        };
        let line = &input[i..line_end];
        i = line_end + 2;

        let size_str = match line.iter().position(|&b| b == b';') {
            Some(semi) => &line[..semi],
            None => line,
        };

        let size_hex = String::from_utf8_lossy(size_str).trim().to_string();
        let size = match usize::from_str_radix(&size_hex, 16) {
            Ok(v) => v,
            Err(_) => break,
        };

        if size == 0 {
            break;
        }
        if i + size > input.len() {
            break;
        }
        out.extend_from_slice(&input[i..i + size]);
        i += size;

        if i + 2 <= input.len() && &input[i..i + 2] == b"\r\n" {
            i += 2;
        } else {
            break;
        }
    }

    out
}

fn guess_content_type_from_path(path: &str) -> Option<String> {
    let p = path.to_lowercase();
    Some(
        if p.ends_with(".html") || p.ends_with(".htm") {
            "text/html; charset=utf-8"
        } else if p.ends_with(".css") {
            "text/css; charset=utf-8"
        } else if p.ends_with(".txt") {
            "text/plain; charset=utf-8"
        } else if p.ends_with(".png") {
            "image/png"
        } else if p.ends_with(".jpg") || p.ends_with(".jpeg") {
            "image/jpeg"
        } else if p.ends_with(".gif") {
            "image/gif"
        } else if p.ends_with(".webp") {
            "image/webp"
        } else {
            "application/octet-stream"
        }
        .to_string(),
    )
}

fn debug_url(u: &URL) -> String {
    format!("{}://{}:{}{}", u.scheme, u.host, u.port, u.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_with(content_type: Option<&str>, body: Vec<u8>) -> Response {
        Response {
            status_code: http_status::OK,
            headers: HashMap::new(),
            body,
            content_type: content_type.map(str::to_string),
        }
    }

    #[test]
    fn extracts_charset_from_content_type_header() {
        let response = response_with(Some("text/html; charset=Shift_JIS"), Vec::new());

        assert_eq!(response.charset_from_header().as_deref(), Some("shift_jis"));
    }

    #[test]
    fn decodes_shift_jis_body() {
        let response = response_with(
            Some("text/html; charset=Shift_JIS"),
            vec![0x90, 0xa2, 0x8a, 0x45],
        );

        assert_eq!(response.body_text_with_charset("shift_jis"), "世界");
    }
}
