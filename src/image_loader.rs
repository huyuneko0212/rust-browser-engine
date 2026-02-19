use std::collections::HashSet;
use std::fs;
use std::sync::{Mutex, OnceLock};

use image::GenericImageView;

const MAX_REDIRECTS: usize = 5;

/// 画像ロードに失敗したキー（正規化済み URL 文字列）を覚えておく
static FAILED_IMAGE_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn failed_cache() -> &'static Mutex<HashSet<String>> {
    FAILED_IMAGE_KEYS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn load_image_bytes(src: &str) -> Option<Vec<u8>> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }

    // URL系
    if starts_with_ignore_ascii_case(src, "file://") {
        return load_file_url_bytes(src);
    }
    if starts_with_ignore_ascii_case(src, "http://")
        || starts_with_ignore_ascii_case(src, "https://")
    {
        return load_http_url_bytes_follow_redirects(src, MAX_REDIRECTS);
    }

    // それ以外（生の Windows パス救済）
    // 例: D:\path\to\a.png / D:/path/to/a.png
    if looks_like_windows_path(src) {
        let path = percent_decode_minimal(src);
        return fs::read(path).ok();
    }

    // Linux/macOS の絶対パス救済（/home/... など）
    if src.starts_with('/') {
        let path = percent_decode_minimal(src);
        return fs::read(path).ok();
    }

    None
}

// ------------------------------------------------------------
// HTTP/HTTPS（リダイレクト追従）
// ------------------------------------------------------------

fn load_http_url_bytes_follow_redirects(src: &str, max_redirects: usize) -> Option<Vec<u8>> {
    let mut current = crate::url::URL::new(src);

    for _ in 0..=max_redirects {
        let resp = crate::http::request_allow_error(&current);
        if resp.status_code == 0 || resp.body.is_empty() {
            return None;
        }
        // 2xx
        if (200..300).contains(&resp.status_code) {
            // Content-Type はサイトによって雑なので、ここは「ログだけ」で基本許容
            if let Some(ct) = &resp.content_type {
                let ct_l = ct.to_lowercase();
                if !ct_l.starts_with("image/") && !ct_l.contains("octet-stream") {
                    eprintln!(
                        "[img] warn: non-image content-type={:?} url={}",
                        resp.content_type, current.path
                    );
                }
            }
            return Some(resp.body);
        }

        // redirect
        if matches!(resp.status_code, 301 | 302 | 303 | 307 | 308) {
            // http.rs 側で headers は lower-case に統一しているので "location" だけ見ればOK
            let location = resp.header("location").map(|s| s.trim().to_string());

            let location = match location {
                Some(l) if !l.is_empty() => l,
                _ => {
                    eprintln!(
                        "[img] redirect without Location status={} url={}",
                        resp.status_code, current.path
                    );
                    return None;
                }
            };

            let next = current.resolve_location(&location);
            current = next;
            continue;
        }

        // error
        eprintln!(
            "[img] http failed status={} url={}",
            resp.status_code, current.path
        );
        return None;
    }

    eprintln!("[img] too many redirects url={}", src);
    None
}

// ------------------------------------------------------------
// file://
// ------------------------------------------------------------

fn load_file_url_bytes(src: &str) -> Option<Vec<u8>> {
    // file://... をパスに落として fs::read
    let mut rest = src.trim();

    // scheme落とし（大小無視）
    rest = trim_prefix_ignore_ascii_case(rest, "file://");

    // file://localhost/...
    if starts_with_ignore_ascii_case(rest, "localhost/") {
        rest = &rest["localhost/".len()..];
    } else if starts_with_ignore_ascii_case(rest, "localhost") && rest.len() == "localhost".len() {
        // "file://localhost" だけ
        return None;
    }

    // 先頭に "/" が来るパターン:
    //  - file:///D:/...   → "/D:/..." になる
    //  - file:///%3A...   → decode後に直す
    let mut path = rest.to_string();

    // URLエンコード最小デコード
    path = percent_decode_minimal(&path);

    // Windows: "/D:/..." なら先頭の "/" を落とす
    if path.starts_with('/') && path.get(2..3) == Some(":") {
        path = path.trim_start_matches('/').to_string();
    }

    // "C:\..." や "D:/..." ならOK
    // "/home/..." もOK
    fs::read(&path).ok()
}

// ------------------------------------------------------------
// utils
// ------------------------------------------------------------

fn looks_like_windows_path(s: &str) -> bool {
    // "D:\..." / "D:/..." を雑に判定
    let b = s.as_bytes();
    b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

fn percent_decode_minimal(s: &str) -> String {
    // %XX を最小でデコード（UTF-8前提）
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1];
            let h2 = bytes[i + 2];
            let hex = |c: u8| -> Option<u8> {
                match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    b'A'..=b'F' => Some(c - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(a), Some(b)) = (hex(h1), hex(h2)) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn trim_prefix_ignore_ascii_case<'a>(s: &'a str, prefix: &str) -> &'a str {
    if starts_with_ignore_ascii_case(s, prefix) {
        &s[prefix.len()..]
    } else {
        s
    }
}

// ------------------------------------------------------------
// 画像の natural size (+ 失敗キャッシュ)
// ------------------------------------------------------------

pub fn load_image_natural_size_px(src: &str) -> Option<(u32, u32)> {
    {
        let cache = failed_cache().lock().unwrap();
        if cache.contains(src) {
            // 失敗済み → すぐ None を返す（重い処理を避ける）
            // eprintln!("[img] skip (cached failure) src={}", src);
            return None;
        }
    }

    // バイト取得
    let bytes = match load_image_bytes(src) {
        Some(b) => b,
        None => {
            eprintln!("[img] load_image_bytes failed src={}", src);

            let mut cache = failed_cache().lock().unwrap();
            cache.insert(src.to_string());

            return None;
        }
    };

    // デコード
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[img] decode failed src={} err={}", src, e);

            let mut cache = failed_cache().lock().unwrap();
            cache.insert(src.to_string());

            return None;
        }
    };

    let (w, h) = img.dimensions();
    eprintln!("[img] decoded src={} -> {}x{}", src, w, h);

    if w == 0 || h == 0 {
        eprintln!("[img] zero-size image src={}", src);

        let mut cache = failed_cache().lock().unwrap();
        cache.insert(src.to_string());

        None
    } else {
        Some((w, h))
    }
}

pub fn can_load_image(key: &str) -> bool {
    load_image_natural_size_px(key).is_some()
}
