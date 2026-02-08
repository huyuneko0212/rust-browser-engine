#![allow(dead_code)]

use crate::dom::*;

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
            return '\0'; // EOFならNULL返す（ブラウザ流）
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

    fn parse_text(&mut self) -> Node {
        let text = self.consume_while(|c| c != '<');
        Node {
            children: vec![],
            node_type: NodeType::Text(text),
        }
    }

    fn parse_element(&mut self) -> Node {
        assert!(self.consume_char() == '<');
        let tag_name = self.parse_tag_name();

        self.consume_while(|c| c != '>');
        assert!(self.consume_char() == '>');

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

        let children = self.parse_nodes();
        if self.starts_with("</") {
            assert!(self.consume_char() == '<');
            assert!(self.consume_char() == '/');
            let end_tag = self.parse_tag_name();
            if tag_name != end_tag {
                println!("tag mismatch {} {}", tag_name, end_tag);
            }
            self.consume_while(|c| c != '>');
            assert!(self.consume_char() == '>');
        }

        Node {
            children,
            node_type: NodeType::Element(ElementData { tag_name }),
        }
    }
    pub fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = vec![];

        loop {
            self.consume_whitespace();

            if self.eof() || self.starts_with("</") {
                break;
            }

            nodes.push(self.parse_node());
        }

        nodes
    }

    fn parse_node(&mut self) -> Node {
        if self.next_char() == '<' {
            // DOCTYPE or comment
            if self.starts_with("<!") {
                self.consume_while(|c| c != '>');
                self.consume_char();
                self.consume_whitespace();
                return self.parse_node();
            }

            return self.parse_element();
        }

        self.parse_text()
    }
    
}
pub fn parse(source: String) -> Node {
        let mut parser = Parser::new(source);
        let nodes = parser.parse_nodes();

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