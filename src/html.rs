#![allow(dead_code)]

use crate::dom::*;
use std::collections::HashMap;

pub fn parse(source: String) -> Node {
    let mut parser = Parser::new(source);
    let nodes = parser.parse_nodes(None);

    normalize_document(nodes)
}

pub fn extract_charset(node: &Node) -> Option<String> {
    match &node.node_type {
        NodeType::Element(ed) => {
            if ed.tag_name == "meta" {
                if let Some(charset) = ed.attributes.get("charset") {
                    if !charset.is_empty() {
                        return Some(charset.to_lowercase());
                    }
                }
                if let Some(http_equiv) = ed.attributes.get("http-equiv") {
                    if http_equiv.to_lowercase() == "content-type" {
                        if let Some(content) = ed.attributes.get("content") {
                            if let Some(charset_part) = content.split("charset=").nth(1) {
                                let charset =
                                    charset_part.trim().split(';').next().unwrap_or("").trim();
                                if !charset.is_empty() {
                                    return Some(charset.to_lowercase());
                                }
                            }
                        }
                    }
                }
            }
            for child in &node.children {
                if let Some(charset) = extract_charset(child) {
                    return Some(charset);
                }
            }
        }
        _ => {
            for child in &node.children {
                if let Some(charset) = extract_charset(child) {
                    return Some(charset);
                }
            }
        }
    }

    None
}

fn is_head_like(n: &Node) -> bool {
    match &n.node_type {
        NodeType::Element(ed) => matches!(
            ed.tag_name.as_str(),
            "meta" | "title" | "style" | "link" | "base" | "script"
        ),
        _ => false,
    }
}

fn take_children(node: &mut Node) -> Vec<Node> {
    std::mem::take(&mut node.children)
}

fn normalize_document(mut nodes: Vec<Node>) -> Node {
    let mut html_node: Option<Node> = None;
    let mut rest: Vec<Node> = vec![];

    for n in nodes.drain(..) {
        match &n.node_type {
            NodeType::Element(ed) if ed.tag_name == "html" && html_node.is_none() => {
                html_node = Some(n);
            }
            _ => rest.push(n),
        }
    }

    let mut html = if let Some(h) = html_node {
        h
    } else {
        elem("html".to_string(), HashMap::new(), rest)
    };

    let mut head_node: Option<Node> = None;
    let mut body_node: Option<Node> = None;
    let mut others: Vec<Node> = vec![];

    let html_children = take_children(&mut html);

    for n in html_children {
        match &n.node_type {
            NodeType::Element(ed) if ed.tag_name == "head" && head_node.is_none() => {
                head_node = Some(n);
            }
            NodeType::Element(ed) if ed.tag_name == "body" && body_node.is_none() => {
                body_node = Some(n);
            }
            _ => others.push(n),
        }
    }

    let mut head_children: Vec<Node> = head_node
        .as_mut()
        .map(|h| take_children(h))
        .unwrap_or_default();

    let mut body_children: Vec<Node> = body_node
        .as_mut()
        .map(|b| take_children(b))
        .unwrap_or_default();

    for n in others {
        if is_head_like(&n) {
            head_children.push(n);
        } else {
            body_children.push(n);
        }
    }

    let head = elem("head".to_string(), HashMap::new(), head_children);
    let body = elem("body".to_string(), HashMap::new(), body_children);

    html.children = vec![head, body];
    html
}

pub struct Parser {
    pos: usize,
    input: String,
}

impl Parser {
    pub fn new(input: String) -> Self {
        Self { pos: 0, input }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn next_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn consume_char(&mut self) -> char {
        if self.eof() {
            return '\0';
        }

        let mut iter = self.input[self.pos..].char_indices();
        let (_, cur) = match iter.next() {
            Some(v) => v,
            None => return '\0',
        };

        let next_pos = match iter.next() {
            Some((i, _)) => i,
            None => cur.len_utf8(),
        };

        self.pos += next_pos;
        cur
    }

    fn consume_while<F>(&mut self, test: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let mut result = String::new();
        while !self.eof() && test(self.next_char()) {
            result.push(self.consume_char());
        }
        result
    }

    fn consume_whitespace(&mut self) {
        self.consume_while(|c| c.is_whitespace());
    }

    fn parse_tag_name(&mut self) -> String {
        self.consume_while(|c| c.is_alphanumeric())
            .to_ascii_lowercase()
    }

    fn peek_start_tag_name(&self) -> Option<String> {
        let s = &self.input[self.pos..];
        if !s.starts_with('<') {
            return None;
        }
        if s.starts_with("</") || s.starts_with("<!") {
            return None;
        }

        let mut idx = 1usize;
        let mut name = String::new();
        while idx < s.len() {
            let ch = s[idx..].chars().next().unwrap();
            if ch.is_alphanumeric() {
                name.push(ch);
                idx += ch.len_utf8();
            } else {
                break;
            }
        }

        if name.is_empty() {
            None
        } else {
            Some(name.to_ascii_lowercase())
        }
    }

    fn peek_end_tag_name(&self) -> Option<String> {
        let s = &self.input[self.pos..];
        if !s.starts_with("</") {
            return None;
        }

        let mut idx = 2usize;
        let mut name = String::new();
        while idx < s.len() {
            let ch = s[idx..].chars().next().unwrap();
            if ch.is_alphanumeric() {
                name.push(ch);
                idx += ch.len_utf8();
            } else {
                break;
            }
        }

        if name.is_empty() {
            None
        } else {
            Some(name.to_ascii_lowercase())
        }
    }

    fn parse_text(&mut self) -> Node {
        let t = self.consume_while(|c| c != '<');
        text(decode_entities(&t))
    }

    fn parse_node(&mut self) -> Node {
        if self.next_char() == '<' {
            if self.starts_with("<!") {
                self.consume_while(|c| c != '>');
                if !self.eof() {
                    self.consume_char();
                }
                self.consume_whitespace();
                return self.parse_node();
            }
            return self.parse_element();
        }
        self.parse_text()
    }

    fn consume_raw_text_until_end_tag(&mut self, tag_lc: &str) -> String {
        let end = format!("</{}>", tag_lc);
        let mut out = String::new();

        while !self.eof() {
            if self.input[self.pos..].starts_with(&end) {
                for _ in 0..end.chars().count() {
                    self.consume_char();
                }
                break;
            }
            out.push(self.consume_char());
        }
        out
    }

    fn parse_attributes(&mut self) -> HashMap<String, String> {
        let mut attrs = HashMap::new();

        loop {
            self.consume_whitespace();

            if self.eof() {
                break;
            }

            let c = self.next_char();
            if c == '>' || c == '/' {
                break;
            }

            let key_raw = self.consume_while(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_');
            if key_raw.is_empty() {
                self.consume_char();
                continue;
            }

            let key = key_raw.to_ascii_lowercase();

            self.consume_whitespace();

            if self.next_char() != '=' {
                attrs.insert(key, "".to_string());
                continue;
            }

            self.consume_char(); // '='
            self.consume_whitespace();

            let value = match self.next_char() {
                '"' => {
                    self.consume_char();
                    let v = self.consume_while(|ch| ch != '"');
                    if self.next_char() == '"' {
                        self.consume_char();
                    }
                    v
                }
                '\'' => {
                    self.consume_char();
                    let v = self.consume_while(|ch| ch != '\'');
                    if self.next_char() == '\'' {
                        self.consume_char();
                    }
                    v
                }
                _ => self.consume_while(|ch| !ch.is_whitespace() && ch != '>'),
            };

            attrs.insert(key, value);
        }

        attrs
    }

    fn parse_element(&mut self) -> Node {
        assert!(self.consume_char() == '<');

        let tag_name = self.parse_tag_name();

        let attrs = self.parse_attributes();

        let mut self_closing = false;
        if self.next_char() == '/' {
            self_closing = true;
            self.consume_char();
        }

        self.consume_while(|c| c != '>');
        if !self.eof() {
            self.consume_char();
        }

        let void_tags = [
            "meta", "img", "br", "hr", "input", "link", "area", "base", "col", "embed", "param",
            "source", "track", "wbr",
        ];
        if self_closing || void_tags.contains(&tag_name.as_str()) {
            return elem(tag_name, attrs, vec![]);
        }

        if tag_name == "script" || tag_name == "style" {
            let raw = self.consume_raw_text_until_end_tag(&tag_name);
            return elem(tag_name, attrs, vec![text(raw)]);
        }

        let children = self.parse_nodes(Some(&tag_name));

        if self.starts_with("</") {
            self.consume_char();
            self.consume_char();
            let end_tag = self.parse_tag_name();

            self.consume_while(|c| c != '>');
            if !self.eof() {
                self.consume_char();
            }

            if tag_name != end_tag {
                println!("tag mismatch {} {}", tag_name, end_tag);
            }
        }

        elem(tag_name, attrs, children)
    }

    pub fn parse_nodes(&mut self, parent_tag: Option<&str>) -> Vec<Node> {
        let mut nodes = vec![];

        loop {
            self.consume_whitespace();

            if self.eof() {
                break;
            }

            if self.starts_with("</") {
                if parent_tag == Some("body") {
                    if let Some(end) = self.peek_end_tag_name() {
                        if end == "html" {
                            break;
                        }
                    }
                }
                if parent_tag == Some("html") {
                    if let Some(end) = self.peek_end_tag_name() {
                        if end == "html" {
                            break;
                        }
                    }
                }
                break;
            }

            if parent_tag == Some("p") && self.next_char() == '<' {
                if let Some(next_tag) = self.peek_start_tag_name() {
                    if is_block_tag(&next_tag) {
                        break;
                    }
                }
            }

            nodes.push(self.parse_node());
        }

        nodes
    }
}

fn is_block_tag(tag_lc: &str) -> bool {
    matches!(
        tag_lc,
        "html"
            | "body"
            | "div"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "section"
            | "header"
            | "footer"
            | "main"
            | "article"
            | "nav"
            | "table"
            | "form"
            | "pre"
    )
}

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '&' {
            let mut j = i + 1;
            let mut found = None;
            while j < chars.len() && j - i <= 32 {
                if chars[j] == ';' {
                    found = Some(j);
                    break;
                }
                j += 1;
            }

            if let Some(end) = found {
                let entity: String = chars[i + 1..end].iter().collect();
                if let Some(decoded) = decode_one_entity(&entity) {
                    out.push_str(&decoded);
                    i = end + 1;
                    continue;
                }
                out.push('&');
                i += 1;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn decode_one_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => return Some("&".to_string()),
        "lt" => return Some("<".to_string()),
        "gt" => return Some(">".to_string()),
        "quot" => return Some("\"".to_string()),
        "apos" => return Some("'".to_string()),
        "nbsp" => return Some("\u{00A0}".to_string()),
        _ => {}
    }

    if let Some(num) = entity.strip_prefix('#') {
        if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            let v = u32::from_str_radix(hex, 16).ok()?;
            return char::from_u32(v).map(|c| c.to_string());
        } else {
            let v = num.parse::<u32>().ok()?;
            return char::from_u32(v).map(|c| c.to_string());
        }
    }

    None
}
