use std::io::{Read, Write};
use std::net::TcpStream;

use native_tls::TlsConnector;

use crate::url::URL;

pub fn request(url: &URL) -> String {
    let addr = format!("{}:{}", url.host, url.port);
    let mut stream = TcpStream::connect(addr).unwrap();

    if url.scheme == "https" {
        let connector = TlsConnector::new().unwrap();
        let mut tls = connector.connect(&url.host, stream).unwrap();

        let req = format!(
            "GET {} HTTP/1.0\r\nHost: {}\r\n\r\n",
            url.path, url.host
        );

        tls.write_all(req.as_bytes()).unwrap();

        let mut response = String::new();
        tls.read_to_string(&mut response).unwrap();

        extract_body(response)
    } else {
        let req = format!(
            "GET {} HTTP/1.0\r\nHost: {}\r\n\r\n",
            url.path, url.host
        );

        stream.write_all(req.as_bytes()).unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        extract_body(response)
    }
}

fn extract_body(resp: String) -> String {
    if let Some(pos) = resp.find("\r\n\r\n") {
        resp[pos + 4..].to_string()
    } else {
        resp
    }
}
