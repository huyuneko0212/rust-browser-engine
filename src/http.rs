#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;

use native_tls::TlsConnector;

use crate::url::URL;

use std::collections::HashMap;

pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub fn request(url: &URL) -> Response {
    return request_inner(url, 0);
}
fn request_inner(url: &URL, depth: usize) -> Response {
    if depth > 5 {
        panic!("Too many redirects");
    };
    let addr = format!("{}:{}", url.host, url.port);
    let mut stream = TcpStream::connect(addr).unwrap();

    let req = format!(
    "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: rust-browser/0.1\r\nConnection: close\r\n\r\n",
    url.path,
    url.host
    );
    let mut raw = String::new();
    if url.scheme == "https"{
        let connector = TlsConnector::new().unwrap();
        let mut tls = connector.connect(&url.host, stream).unwrap();
        tls.write_all(req.as_bytes()).unwrap();
        tls.read_to_string(&mut raw).unwrap();
    }else {
        stream.write_all(req.as_bytes()).unwrap();
        stream.read_to_string(&mut raw).unwrap();
    };
    parse_response(raw, depth)
}
fn parse_response(resp: String,depth: usize) -> Response{
    let mut parts =resp.split("\r\n\r\n");
    let head = parts.next().unwrap();
    let body =parts.next().unwrap_or("").to_string();

    let mut lines = head.split("\r\n");

    let status_line = lines.next().unwrap();
    println!("{}",status_line);
    let mut status_parts = status_line.split(" ");
    let _http =status_parts.next().unwrap();
    let status_code:u16 = status_parts.next().unwrap().parse().unwrap();


    //header
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':'){
            headers.insert(k.to_lowercase(), v.trim().to_string());
        }
    };
    // redirect
    if status_code == 301 || status_code == 302 {
        if let Some(loc) = headers.get("location") {
            println!("redirect -> {}", loc);
            let new_url = URL::new(loc);
            return request_inner(&new_url, depth + 1);
        };
    };
    Response {
        status_code,
        headers,
        body,
    }
}

