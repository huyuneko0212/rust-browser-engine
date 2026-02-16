#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

use brotli::Decompressor;
use flate2::read::GzDecoder;
use native_tls::TlsConnector;

use crate::url::URL;

pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>, // ★ bytes に変更
    pub content_type: Option<String>,
}

impl Response {
    /// HTML/CSS 用（バイナリは壊れるので画像には使わない）
    pub fn body_text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

pub fn request(url: &URL) -> Response {
    request_inner(url, 0)
}

fn request_inner(url: &URL, depth: usize) -> Response {
    if depth > 10 {
        panic!("Too many redirects");
    }

    // -------------------------
    // file://
    // -------------------------
    if url.scheme == "file" {
        return request_file(url);
    }

    // -------------------------
    // http(s)://
    // -------------------------
    let addr = format!("{}:{}", url.host, url.port);
    let stream = TcpStream::connect(addr).unwrap();

    let req = format!(
        "GET {} HTTP/1.1\r\n\
Host: {}\r\n\
User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) rust-browser/0.1\r\n\
Accept: text/html,application/xhtml+xml,text/css,image/*,*/*;q=0.8\r\n\
Accept-Language: ja,en-US;q=0.9,en;q=0.8\r\n\
Accept-Encoding: gzip, br\r\n\
Connection: close\r\n\
\r\n",
        url.path, url.host
    );

    let raw_bytes = if url.scheme == "https" {
        let connector = TlsConnector::new().unwrap();
        let mut tls = connector.connect(&url.host, stream).unwrap();
        tls.write_all(req.as_bytes()).unwrap();

        let mut buf = Vec::new();
        tls.read_to_end(&mut buf).unwrap();
        buf
    } else {
        let mut s = stream;
        s.write_all(req.as_bytes()).unwrap();

        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        buf
    };

    parse_response(url, raw_bytes, depth)
}

fn request_file(url: &URL) -> Response {
    // url.path は "D:/.../a.html" 等
    let mut fs_path = url.path.clone();

    // 念のためバックスラッシュ許容
    fs_path = fs_path.replace('\\', "/");

    // Windows: "D:/..." を std::fs が読める形に
    // （このままでも読めることが多いが、\ に寄せてもOK）
    let fs_path_os = fs_path.replace('/', "\\");

    let bytes = match std::fs::read(&fs_path_os) {
        Ok(b) => b,
        Err(e) => {
            return Response {
                status_code: 404,
                headers: HashMap::new(),
                body: format!(
                    "<html><body><h1>File not found</h1><p>{}</p><p>{}</p></body></html>",
                    fs_path_os, e
                )
                .into_bytes(),
                content_type: Some("text/html; charset=utf-8".to_string()),
            };
        }
    };

    let content_type = guess_content_type_from_path(&fs_path);

    Response {
        status_code: 200,
        headers: HashMap::new(),
        body: bytes,
        content_type,
    }
}

fn guess_content_type_from_path(path: &str) -> Option<String> {
    let p = path.to_lowercase();
    if p.ends_with(".html") || p.ends_with(".htm") {
        Some("text/html; charset=utf-8".to_string())
    } else if p.ends_with(".css") {
        Some("text/css; charset=utf-8".to_string())
    } else if p.ends_with(".txt") {
        Some("text/plain; charset=utf-8".to_string())
    } else if p.ends_with(".png") {
        Some("image/png".to_string())
    } else if p.ends_with(".jpg") || p.ends_with(".jpeg") {
        Some("image/jpeg".to_string())
    } else if p.ends_with(".gif") {
        Some("image/gif".to_string())
    } else if p.ends_with(".webp") {
        Some("image/webp".to_string())
    } else {
        Some("application/octet-stream".to_string())
    }
}

fn parse_response(url: &URL, resp: Vec<u8>, depth: usize) -> Response {
    // HTTPヘッダ終端 "\r\n\r\n" を探す
    let header_end = match find_bytes(&resp, b"\r\n\r\n") {
        Some(p) => p,
        None => {
            // ヘッダが無い/壊れてる：全部ボディ扱い
            return Response {
                status_code: 0,
                headers: HashMap::new(),
                body: resp,
                content_type: None,
            };
        }
    };

    let head_bytes = &resp[..header_end];
    let body_bytes = &resp[header_end + 4..];

    let head = String::from_utf8_lossy(head_bytes).to_string();
    let mut lines = head.split("\r\n");

    let status_line = lines.next().unwrap_or("");
    let mut status_parts = status_line.split_whitespace();
    let _http = status_parts.next().unwrap_or("");
    let status_code: u16 = status_parts.next().unwrap_or("0").parse().unwrap_or(0);

    // headers
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.to_lowercase(), v.trim().to_string());
        }
    }

    let content_type = headers.get("content-type").cloned();

    // redirect
    if matches!(status_code, 301 | 302 | 303 | 307 | 308) {
        if let Some(loc) = headers.get("location").cloned() {
            let new_url = url.resolve_location(&loc);
            return request_inner(&new_url, depth + 1);
        }
    }

    // 304/204/1xx は基本ボディ無し
    if status_code == 204 || status_code == 304 || (100..200).contains(&status_code) {
        return Response {
            status_code,
            headers,
            body: Vec::new(),
            content_type,
        };
    }

    // 1) Transfer-Encoding: chunked
    let mut decoded_body: Vec<u8> = body_bytes.to_vec();
    if let Some(te) = headers.get("transfer-encoding") {
        if te.to_lowercase().contains("chunked") {
            decoded_body = decode_chunked(&decoded_body);
        }
    }

    // 2) gzip / br
    if let Some(enc) = headers.get("content-encoding") {
        let encs: Vec<String> = enc
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty() && s != "identity")
            .collect();

        for e in encs.into_iter().rev() {
            decoded_body = match e.as_str() {
                "gzip" => {
                    let mut gz = GzDecoder::new(decoded_body.as_slice());
                    let mut out = Vec::new();
                    if gz.read_to_end(&mut out).is_ok() {
                        out
                    } else {
                        return Response {
                            status_code,
                            headers,
                            body: b"gzip decode failed".to_vec(),
                            content_type,
                        };
                    }
                }
                "br" => {
                    let mut br = Decompressor::new(decoded_body.as_slice(), 4096);
                    let mut out = Vec::new();
                    if br.read_to_end(&mut out).is_ok() {
                        out
                    } else {
                        return Response {
                            status_code,
                            headers,
                            body: b"brotli decode failed".to_vec(),
                            content_type,
                        };
                    }
                }
                other => {
                    return Response {
                        status_code,
                        headers,
                        body: format!("content-encoding {} not supported yet", other).into_bytes(),
                        content_type,
                    };
                }
            };
        }
    }

    // UTF-8化しない、bytesのまま返す
    Response {
        status_code,
        headers,
        body: decoded_body,
        content_type,
    }
}

// resp の中から pattern を探して先頭 index を返す
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

// Transfer-Encoding: chunked の最小デコーダ
fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < input.len() {
        // chunk size line
        let line_end = match find_bytes(&input[i..], b"\r\n") {
            Some(p) => i + p,
            None => break,
        };
        let line = &input[i..line_end];
        i = line_end + 2;

        // ";" 以降は拡張なので捨てる
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

        // chunk data
        if i + size > input.len() {
            break;
        }
        out.extend_from_slice(&input[i..i + size]);
        i += size;

        // trailing CRLF
        if i + 2 <= input.len() && &input[i..i + 2] == b"\r\n" {
            i += 2;
        } else {
            break;
        }
    }

    out
}
