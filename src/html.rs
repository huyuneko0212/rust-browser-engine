#![allow(dead_code)]

use crate::dom::*;

pub fn parse(source: String) -> Node {
    let mut parser = Parser::new(source);
    let nodes = parser.parse_nodes(None);

    if nodes.len() == 1 {
        nodes.into_iter().next().unwrap()
    } else {
        Node {
            children: nodes,
            node_type: NodeType::Element(ElementData {
                tag_name: "html".to_string(),
            }),
        }
    }
}

pub struct Parser {
    pos: usize,
    input: String,
}

impl Parser {
    pub fn new(input: String) -> Self {
        Self { pos: 0, input }
    }

    fn next_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
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
    }

    /// "<tag ...>" の tag 名だけ覗く（posは進めない）
    fn peek_start_tag_name(&self) -> Option<String> {
        let s = &self.input[self.pos..];
        if !s.starts_with('<') {
            return None;
        }
        if s.starts_with("</") || s.starts_with("<!") {
            return None;
        }

        let mut idx = 1usize; // '<' の次
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

        if name.is_empty() { None } else { Some(name) }
    }

    /// "</tag ...>" の tag 名だけ覗く（posは進めない）
    fn peek_end_tag_name(&self) -> Option<String> {
        let s = &self.input[self.pos..];
        if !s.starts_with("</") {
            return None;
        }

        let mut idx = 2usize; // "</" の次
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

        if name.is_empty() { None } else { Some(name) }
    }

    fn parse_text(&mut self) -> Node {
        let text = self.consume_while(|c| c != '<');
        Node {
            children: vec![],
            node_type: NodeType::Text(text),
        }
    }

    fn parse_node(&mut self) -> Node {
        if self.next_char() == '<' {
            // DOCTYPE / comment / <!...> を飛ばす（超簡易）
            if self.starts_with("<!") {
                self.consume_while(|c| c != '>');
                if !self.eof() {
                    self.consume_char(); // '>'
                }
                self.consume_whitespace();
                return self.parse_node();
            }
            return self.parse_element();
        }
        self.parse_text()
    }

    fn parse_element(&mut self) -> Node {
        // "<"
        assert!(self.consume_char() == '<');

        let tag_name = self.parse_tag_name();

        // 属性は今は無視： '>' まで読み飛ばす
        self.consume_while(|c| c != '>');
        if !self.eof() {
            self.consume_char(); // '>'
        }

        // Voidタグ（閉じタグなし）
        let void_tags = [
            "meta", "img", "br", "hr", "input", "link", "area", "base", "col", "embed", "param",
            "source", "track", "wbr",
        ];
        if void_tags.contains(&tag_name.as_str()) {
            return Node {
                children: vec![],
                node_type: NodeType::Element(ElementData { tag_name }),
            };
        }

        // ★子ノードを読む（parent_tag を渡す）
        let children = self.parse_nodes(Some(&tag_name));

        // ★閉じタグがあれば消費（HTMLは壊れてても前に進む）
        if self.starts_with("</") {
            // "</"
            self.consume_char(); // '<'
            self.consume_char(); // '/'

            let end_tag = self.parse_tag_name();

            // ">" まで飛ばす
            self.consume_while(|c| c != '>');
            if !self.eof() {
                self.consume_char(); // '>'
            }

            if tag_name != end_tag {
                // HTMLでは普通に起こるのでログだけ
                println!("tag mismatch {} {}", tag_name, end_tag);
            }
        }

        Node {
            children,
            node_type: NodeType::Element(ElementData { tag_name }),
        }
    }

    /// parent_tag を見て “暗黙閉じ” を入れる
    pub fn parse_nodes(&mut self, parent_tag: Option<&str>) -> Vec<Node> {
        let mut nodes = vec![];

        loop {
            self.consume_whitespace();

            if self.eof() {
                break;
            }

            // 明示的な閉じタグが来たら親に任せる
            if self.starts_with("</") {
                // --- 暗黙閉じの強化 ---
                // body の中で </html> が来たら bodyは閉じた扱い
                if parent_tag == Some("body") {
                    if let Some(end) = self.peek_end_tag_name() {
                        if end == "html" {
                            break;
                        }
                    }
                }
                // html の中で </html> は終端
                if parent_tag == Some("html") {
                    if let Some(end) = self.peek_end_tag_name() {
                        if end == "html" {
                            break;
                        }
                    }
                }

                break;
            }

            // --- HTMLらしさ（最低限） ---
            // <p> の中でブロック要素が来たら </p> 省略とみなす
            if parent_tag == Some("p") && self.next_char() == '<' {
                if let Some(next_tag) = self.peek_start_tag_name() {
                    if is_block_tag(&next_tag) {
                        break;
                    }
                }
            }

            // body の中で <html> / <body> が来たら（壊れHTML対策）body閉じ
            if parent_tag == Some("body") && self.next_char() == '<' {
                if let Some(next_tag) = self.peek_start_tag_name() {
                    if next_tag == "html" || next_tag == "body" {
                        break;
                    }
                }
            }

            // html の中で <html> が来たら html閉じ（壊れHTML対策）
            if parent_tag == Some("html") && self.next_char() == '<' {
                if let Some(next_tag) = self.peek_start_tag_name() {
                    if next_tag == "html" {
                        break;
                    }
                }
            }

            nodes.push(self.parse_node());
        }

        nodes
    }
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "html" | "body" | "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol"
            | "li" | "section" | "header" | "footer" | "main" | "article" | "nav" | "table"
            | "form" | "pre"
    )
}
