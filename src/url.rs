#[derive(Debug, Clone)]
pub struct URL {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl URL {
    pub fn new(url: &str) -> Self {
        let mut parts = url.split("://");
        let scheme = parts.next().unwrap().to_string();
        assert!(scheme == "http" || scheme == "https");

        let mut rest = parts.next().unwrap().to_string();

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

        // :port指定対応
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
        if loc.starts_with("http://") || loc.starts_with("https://") {
            return URL::new(loc);
        }

        // 2) scheme-relative: //example.com/path
        if let Some(rest) = loc.strip_prefix("//") {
            return URL::new(&format!("{}://{}", self.scheme, rest));
        }

        // 3) query-only: ?q=...
        if loc.starts_with('?') {
            // 今の path の "?" 以降を置き換える
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
        // self.path の末尾をディレクトリにして結合
        let mut base = self.path.clone();
        // クエリは落としておく
        if let Some(q) = base.find('?') {
            base.truncate(q);
        }
        if let Some(pos) = base.rfind('/') {
            base.truncate(pos + 1); // 最後の / まで残す
        } else {
            base.push('/');
        }

        let mut u = self.clone();
        u.path = format!("{}{}", base, loc);
        u
    }
}
