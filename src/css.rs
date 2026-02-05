#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
pub struct Selector {
    pub simple: String,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub name: String,
    pub value: String,
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

    fn consume_char(&mut self) -> char {
        if self.eof() {
            return '\0';
        }
        let mut iter = self.input[self.pos..].char_indices();
        let (_, cur) = iter.next().unwrap();
        let next = iter.next().map(|(i, _)| i).unwrap_or(cur.len_utf8());
        self.pos += next;
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

    pub fn parse_stylesheet(&mut self) -> Stylesheet {
        let mut rules = vec![];

        while !self.eof() {
            self.consume_whitespace();
            if self.eof() {
                break;
            }
            rules.push(self.parse_rule());
        }
        Stylesheet { rules }
    }
    fn parse_rule(&mut self) -> Rule {
        let selector = self.parse_selector();
        self.consume_whitespace();
        let declarations = self.parse_declarations();
        Rule {
            selector,
            declarations,
        }
    }

    fn parse_selector(&mut self) -> Vec<Selector> {
        let mut selector = vec![];

        loop {
            self.consume_whitespace();
            let name = self.consume_while(|c| c.is_alphanumeric());
            selector.push(Selector { simple: name });

            self.consume_whitespace();

            if self.next_char() == '{' {
                break;
            }
            self.consume_char();
        }
        selector
    }
    fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut decls = vec![];

        // 最初の { を消費
        assert!(self.consume_char() == '{');

        loop {
            self.consume_whitespace();

            if self.eof() {
                break;
            }

            if self.next_char() == '}' {
                self.consume_char(); // } 消費
                break;
            }

            decls.push(self.parse_declaration());
        }

        decls
    }

    fn parse_declaration(&mut self) -> Declaration {
        let name = self.consume_while(|c| c.is_alphanumeric() || c == '-');
        self.consume_whitespace();
        assert!(self.consume_char() == ':');
        self.consume_whitespace();

        let value = self.consume_while(|c| c != ';' && c != '}');

        if self.next_char() == ';' {
            self.consume_char();
        }

        Declaration {
            name,
            value: value.trim().to_string(),
        }
    }
}
