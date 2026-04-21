// src/url_utils.rs

use crate::constants::protocol;
use crate::url;

/// URLを「正規化した絶対文字列」にする（default port を省略）
/// これを “画像キャッシュ / visited key / display の key” で統一する
pub fn url_to_abs_string(u: &url::URL) -> String {
    match (u.scheme.as_str(), u.port) {
        ("file", _) => {
            if u.path.starts_with('/') {
                format!("file://{}", u.path)
            } else {
                format!("file:///{}", u.path)
            }
        }
        ("http", protocol::HTTP_PORT) => format!("http://{}{}", u.host, u.path),
        ("https", protocol::HTTPS_PORT) => format!("https://{}{}", u.host, u.path),
        ("http", p) => format!("http://{}:{}{}", u.host, p, u.path),
        ("https", p) => format!("https://{}:{}{}", u.host, p, u.path),
        _ => format!("{}://{}:{}{}", u.scheme, u.host, u.port, u.path),
    }
}

/// base に対して href/src を resolve する（trimだけここで吸収）
pub fn normalize_against(base: &url::URL, href: &str) -> url::URL {
    base.resolve_location(href.trim())
}

/// base に対して href/src を resolve して、その結果を正規化キーにする
pub fn normalized_key_against(base: &url::URL, href: &str) -> Option<String> {
    let h = href.trim();
    (!h.is_empty()).then(|| url_to_abs_string(&normalize_against(base, h)))
}
