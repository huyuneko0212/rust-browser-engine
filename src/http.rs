#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

use native_tls::TlsConnector;

use crate::url::URL;

pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub fn request(url: &URL) -> Response {
    request_inner(url, 0)
}

fn request_inner(url: &URL, depth: usize) -> Response {
    if depth > 5 {
        panic!("Too many redirects");
    }

    let addr = format!("{}:{}", url.host, url.port);
    let stream = TcpStream::connect(addr).unwrap();

    // ※ gzip/br を返されると今は解凍できず崩れるので identity を要求
    // ※ Accept-Encoding を送らない/identityにするだけで Google の事故率が下がる
    let req = format!(
        "GET {} HTTP/1.1\r\n\
Host: {}\r\n\
User-Agent: rust-browser/0.1\r\n\
Accept: text/html,application/xhtml+xml\r\n\
Accept-Encoding: identity\r\n\
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

    parse_response(raw_bytes, depth)
}

fn parse_response(resp: Vec<u8>, depth: usize) -> Response {
    // HTTPヘッダ終端 "\r\n\r\n" を探して2分割（bytesで）
    let header_end = find_bytes(&resp, b"\r\n\r\n").unwrap_or(resp.len());
    let head_bytes = &resp[..header_end];
    let body_bytes = if header_end + 4 <= resp.len() {
        &resp[header_end + 4..]
    } else {
        &[][..]
    };

    let head = String::from_utf8_lossy(head_bytes).to_string();

    let mut lines = head.split("\r\n");

    let status_line = lines.next().unwrap_or("");
    // コンソールに出したくないならこの行を消す
    println!("{}", status_line);

    let mut status_parts = status_line.split_whitespace();
    let _http = status_parts.next().unwrap_or("");
    let status_code: u16 = status_parts
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    // headers
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.to_lowercase(), v.trim().to_string());
        }
    }

    // redirect
    if status_code == 301 || status_code == 302 || status_code == 303 || status_code == 307 || status_code == 308 {
        if let Some(loc) = headers.get("location") {
            println!("redirect -> {}", loc);
            let new_url = URL::new(loc);
            return request_inner(&new_url, depth + 1);
        }
    }

    // content-encoding 対応（今回は identity のみ許可）
    if let Some(enc) = headers.clone().get("content-encoding") {
        let e = enc.to_lowercase();
        if e != "identity" && !e.is_empty() {
            // gzip/br 等を読めないままHTML扱いすると崩れるので明示的に止める
            return Response {
                status_code,
                headers,
                body: format!("<html><body><p>content-encoding {} not supported yet</p></body></html>", enc),
            };
        }
    }

    // Transfer-Encoding: chunked をデコード
    let mut decoded_body: Vec<u8> = body_bytes.to_vec();
    if let Some(te) = headers.get("transfer-encoding") {
        if te.to_lowercase().contains("chunked") {
            decoded_body = decode_chunked(&decoded_body);
        }
    }

    // bodyをUTF-8として読む（まずはこれでOK）
    let body = String::from_utf8_lossy(&decoded_body).to_string();

    Response {
        status_code,
        headers,
        body,
    }
}

// resp の中から pattern を探して先頭 index を返す
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

// Transfer-Encoding: chunked の最小デコーダ
fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < input.len() {
        // 1) chunk size line を読む（hex + optional extensions）
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
            // 末尾の "\r\n" を消費（あれば）
            let _ = find_bytes(&input[i..], b"\r\n").map(|p| i + p);
            break;
        }

        // 2) chunk data
        if i + size > input.len() {
            break;
        }
        out.extend_from_slice(&input[i..i + size]);
        i += size;

        // 3) trailing CRLF
        if i + 2 <= input.len() && &input[i..i + 2] == b"\r\n" {
            i += 2;
        } else {
            break;
        }
    }

    out
}
