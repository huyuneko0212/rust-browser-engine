use crate::constants::protocol;

#[derive(Debug, Clone)]
pub struct URL {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
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

        if scheme == "file" {
            let mut host = String::new();
            let mut path = rest.to_string();

            path = path.replace('\\', "/");

            if !path.starts_with('/') {
                let is_windows_drive = path.len() >= 2
                    && path.as_bytes()[1] == b':'
                    && (path.as_bytes()[0] as char).is_ascii_alphabetic();

                if !is_windows_drive {
                    if let Some((h, p)) = path.split_once('/') {
                        host = h.to_string();
                        path = format!("/{}", p);
                    } else {
                        host = path.clone();
                        path = "/".to_string();
                    }
                }
            }

            path = strip_leading_slash_before_drive(path);

            path = normalize_file_path(path);

            if host == "localhost" {
                host.clear();
            }

            return URL {
                scheme,
                host,
                port: protocol::FILE_PORT,
                path,
            };
        }

        let mut rest = rest.to_string();
        if !rest.contains('/') {
            rest.push('/');
        }

        let mut parts = rest.splitn(2, '/');
        let mut host = parts.next().unwrap().to_string();
        let path = format!("/{}", parts.next().unwrap());

        let mut port = match scheme.as_str() {
            "http" => protocol::HTTP_PORT,
            "https" => protocol::HTTPS_PORT,
            _ => unreachable!(),
        };

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

        if loc.starts_with("http://") || loc.starts_with("https://") || loc.starts_with("file://") {
            return URL::new(loc);
        }

        if self.scheme == "file" {
            if loc.starts_with('#') {
                return self.clone();
            }
            if loc.starts_with('?') {
                return self.clone();
            }

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

        if let Some(rest) = loc.strip_prefix("//") {
            return URL::new(&format!("{}://{}", self.scheme, rest));
        }

        if loc.starts_with('?') {
            let base_path = self.path.split('?').next().unwrap_or(&self.path);
            let mut u = self.clone();
            u.path = format!("{}{}", base_path, loc);
            return u;
        }

        if loc.starts_with('#') {
            return self.clone();
        }

        if loc.starts_with('/') {
            let mut u = self.clone();
            u.path = loc.to_string();
            return u;
        }

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

fn strip_leading_slash_before_drive(mut p: String) -> String {
    if p.len() >= 3 {
        let b = p.as_bytes();
        if b[0] == b'/' && b[2] == b':' && ((b[1] as char).is_ascii_alphabetic()) {
            p.remove(0);
        }
    }
    p
}

fn normalize_file_path(p: String) -> String {
    let parts: Vec<&str> = p.split('/').collect();

    let mut out: Vec<&str> = Vec::new();

    for seg in parts.iter().copied() {
        let seg = seg.trim();
        if seg.is_empty() {
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

    if p.starts_with('/') && !joined.starts_with('/') {
        joined = format!("/{}", joined);
    }

    if joined.len() == 2 && joined.as_bytes()[1] == b':' {
        joined.push('/');
    }

    joined
}
