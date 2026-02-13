#[derive(Debug, Clone)]
pub struct URL {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String, // http(s): "/path?x" / file: "D:/.../a.html" or "/home/.."
}

impl URL {
    pub fn new(url: &str) -> Self {
        let (scheme, rest) = url
            .split_once("://")
            .unwrap_or_else(|| panic!("invalid url (missing ://): {}", url));

        let scheme = scheme.to_string();

        assert!(
            scheme == "http" || scheme == "https" || scheme == "file",
            "unsupported scheme: {}",
            scheme
        );

        // --------------------
        // file://
        // --------------------
        if scheme == "file" {
            // file://<host>/<path> or file:///path or file://C:/path
            // split_once("://") 済みなので rest は:
            //   "D:/a.html"
            //   "/D:/a.html"
            //   "localhost/D:/a.html"
            //   "/home/user/a.html"
            let mut host = String::new();
            let mut path = rest.to_string();

            // まず \ を / へ寄せる
            path = path.replace('\\', "/");

            // rest が "/..." で始まるなら host なし（一般的な file:///...）
            if !path.starts_with('/') {
                // Windows drive "C:/..." なら host なし
                let is_windows_drive = path.len() >= 2
                    && path.as_bytes()[1] == b':'
                    && (path.as_bytes()[0] as char).is_ascii_alphabetic();

                if !is_windows_drive {
                    // host/path 形式として分離してみる（例: localhost/C:/...）
                    if let Some((h, p)) = path.split_once('/') {
                        host = h.to_string();
                        path = format!("/{}", p);
                    } else {
                        // "file://localhost" みたいに / が無いケースは host 扱い
                        host = path.clone();
                        path = "/".to_string();
                    }
                }
            }

            // "/C:/..." → "C:/..." に寄せる（Windows用）
            path = strip_leading_slash_before_drive(path);

            // . / .. を最小で正規化（相対解決が破綻しにくい）
            path = normalize_file_path(path);

            // localhost は実質ローカル扱い（必要なら保持してもOK）
            if host == "localhost" {
                host.clear();
            }

            return URL {
                scheme,
                host,
                port: 0,
                path,
            };
        }

        // --------------------
        // http(s)://
        // --------------------
        let mut rest = rest.to_string();
        if !rest.contains('/') {
            rest.push('/');
        }

        let mut parts = rest.splitn(2, '/');
        let mut host = parts.next().unwrap().to_string();
        let path = format!("/{}", parts.next().unwrap());

        let mut port = match scheme.as_str() {
            "http" => 80,
            "https" => 443,
            _ => unreachable!(),
        };

        // :port 指定対応
        if let Some((h, p)) = host.clone().split_once(':') {
            host = h.to_string();
            port = p.parse().unwrap();
        }

        URL {
            scheme,
            host,
            port,
            path,
        }
    }

    pub fn resolve_location(&self, loc: &str) -> Self {
        let loc = loc.trim();

        // 1) absolute URL
        if loc.starts_with("http://") || loc.starts_with("https://") || loc.starts_with("file://") {
            return URL::new(loc);
        }

        // file: の相対解決
        if self.scheme == "file" {
            if loc.starts_with('#') {
                return self.clone();
            }
            if loc.starts_with('?') {
                // file の ? は今回は無視（必要なら path に付ける）
                return self.clone();
            }

            // 絶対パスっぽい: "D:/..." or "/D:/..." or "/home/..."
            let is_windows_drive = loc.len() >= 2
                && loc.as_bytes()[1] == b':'
                && (loc.as_bytes()[0] as char).is_ascii_alphabetic();

            if loc.starts_with('/') || is_windows_drive {
                let mut p = loc.replace('\\', "/");
                p = strip_leading_slash_before_drive(p);
                p = normalize_file_path(p);

                let mut u = self.clone();
                u.path = p;
                return u;
            }

            // 相対: base のディレクトリに結合
            let mut base = self.path.replace('\\', "/");
            if let Some(pos) = base.rfind('/') {
                base.truncate(pos + 1);
            } else {
                base.push('/');
            }

            let joined = format!("{}{}", base, loc.replace('\\', "/"));
            let mut u = self.clone();
            u.path = normalize_file_path(joined);
            return u;
        }

        // 2) scheme-relative: //example.com/path
        if let Some(rest) = loc.strip_prefix("//") {
            return URL::new(&format!("{}://{}", self.scheme, rest));
        }

        // 3) query-only: ?q=...
        if loc.starts_with('?') {
            let base_path = self.path.split('?').next().unwrap_or(&self.path);
            let mut u = self.clone();
            u.path = format!("{}{}", base_path, loc);
            return u;
        }

        // 4) fragment-only: #id（今回は無視して同じURL扱い）
        if loc.starts_with('#') {
            return self.clone();
        }

        // 5) absolute-path: /xxx
        if loc.starts_with('/') {
            let mut u = self.clone();
            u.path = loc.to_string();
            return u;
        }

        // 6) relative-path: xxx/yyy
        let mut base = self.path.clone();
        if let Some(q) = base.find('?') {
            base.truncate(q);
        }
        if let Some(pos) = base.rfind('/') {
            base.truncate(pos + 1);
        } else {
            base.push('/');
        }

        let mut u = self.clone();
        u.path = format!("{}{}", base, loc);
        u
    }
}

// --------------------
// helpers
// --------------------

fn strip_leading_slash_before_drive(mut p: String) -> String {
    // "/D:/..." → "D:/..."
    if p.len() >= 3 {
        let b = p.as_bytes();
        if b[0] == b'/' && b[2] == b':' && ((b[1] as char).is_ascii_alphabetic()) {
            p.remove(0);
        }
    }
    p
}

fn normalize_file_path(p: String) -> String {
    // 超最小の . / .. 正規化
    // Windows "C:/a/../b" も想定し、先頭 "C:" は特別扱い
    let parts: Vec<&str> = p.split('/').collect();

    // "C:" を root 扱いしたいので head を確保
    let mut out: Vec<&str> = Vec::new();

    for seg in parts.iter().copied() {
        let seg = seg.trim();
        if seg.is_empty() {
            // 先頭 "/" の空セグメントは維持したいならここ調整
            continue;
        }
        if seg == "." {
            continue;
        }
        if seg == ".." {
            if !out.is_empty() {
                if out.len() == 1 && (out[0].is_empty() || out[0].ends_with(':')) {
                    continue;
                }
                out.pop();
            }
            continue;
        }
        out.push(seg);
    }

    let mut joined = out.join("/");

    // "/home" 形式を維持したい場合は、先頭が "" のとき "/" を付ける
    if p.starts_with('/') && !joined.starts_with('/') {
        joined = format!("/{}", joined);
    }

    // Windows の "C:" だけで終わったら "C:/" に寄せる
    if joined.len() == 2 && joined.as_bytes()[1] == b':' {
        joined.push('/');
    }

    joined
}
