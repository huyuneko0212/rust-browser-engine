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
}
