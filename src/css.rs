#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: Vec<Selector>,
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

pub fn parse_inline_declarations(input: &str) -> Vec<Declaration> {
    let mut parser = Parser::new(format!("inline {{{}}}", input));
    parser
        .parse_rule()
        .map(|rule| rule.declarations)
        .unwrap_or_default()
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

    fn consume_whitespace_and_comments(&mut self) {
        loop {
            self.consume_whitespace();

            if self.starts_with("/*") {
                self.consume_char(); // '/'
                self.consume_char(); // '*'
                while !self.eof() && !self.starts_with("*/") {
                    self.consume_char();
                }
                if self.starts_with("*/") {
                    self.consume_char(); // '*'
                    self.consume_char(); // '/'
                }
                continue;
            }

            break;
        }
    }

    fn skip_at_rule(&mut self) {
        while !self.eof() && self.next_char() != ';' && self.next_char() != '{' {
            if self.starts_with("/*") {
                self.consume_whitespace_and_comments();
                continue;
            }
            self.consume_char();
        }
        if self.eof() {
            return;
        }
        if self.next_char() == ';' {
            self.consume_char();
            return;
        }

        if self.next_char() == '{' {
            self.consume_char();
            let mut depth = 1i32;
            while !self.eof() && depth > 0 {
                if self.starts_with("/*") {
                    self.consume_whitespace_and_comments();
                    continue;
                }
                let c = self.consume_char();
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                }
            }
        }
    }

    pub fn parse_stylesheet(&mut self) -> Stylesheet {
        let mut rules = vec![];

        while !self.eof() {
            self.consume_whitespace_and_comments();
            if self.eof() {
                break;
            }

            if self.next_char() == '@' {
                self.skip_at_rule();
                continue;
            }

            if let Some(rule) = self.parse_rule() {
                rules.push(rule);
            } else {
                // 解析できないゴミは少し進めてリカバリ
                self.recover_to_next_rule();
            }
        }

        Stylesheet { rules }
    }

    fn recover_to_next_rule(&mut self) {
        while !self.eof() {
            if self.starts_with("/*") {
                self.consume_whitespace_and_comments();
                continue;
            }
            let c = self.next_char();
            if c == '{' || c == '}' || c == ';' {
                self.consume_char();
                break;
            }
            self.consume_char();
        }
    }

    fn parse_rule(&mut self) -> Option<Rule> {
        self.consume_whitespace_and_comments();

        let selectors = self.parse_selectors();
        self.consume_whitespace_and_comments();

        if selectors.is_empty() {
            return None;
        }

        let declarations = self.parse_declarations();

        Some(Rule {
            selectors,
            declarations,
        })
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        let raw = self.consume_while(|c| c != '{' && c != '\0');
        let raw = raw.trim();

        if raw.is_empty() {
            return vec![];
        }

        raw.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| Selector {
                simple: s.to_string(),
            })
            .collect()
    }

    fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut decls = vec![];

        self.consume_whitespace_and_comments();

        if self.eof() || self.next_char() != '{' {
            return decls;
        }
        self.consume_char(); // '{'

        loop {
            self.consume_whitespace_and_comments();
            if self.eof() {
                break;
            }

            if self.next_char() == '}' {
                self.consume_char();
                break;
            }

            if self.next_char() == '@' {
                self.skip_at_rule();
                continue;
            }

            if let Some(ds) = self.parse_declaration() {
                decls.extend(ds);
            } else {
                self.skip_bad_declaration();
            }
        }

        decls
    }

    /// 1つの CSS 宣言をパースする。
    /// 通常は 1 つの Declaration を返すが、
    /// `border: 1px solid red` のようなショートハンドは
    /// `border-width` / `border-color` など複数に展開する。
    fn parse_declaration(&mut self) -> Option<Vec<Declaration>> {
        self.consume_whitespace_and_comments();

        let name = self.consume_while(|c| c.is_alphanumeric() || c == '-' || c == '_');
        self.consume_whitespace_and_comments();

        if name.is_empty() {
            return None;
        }

        if self.eof() || self.next_char() != ':' {
            return None;
        }
        self.consume_char(); // ':'
        self.consume_whitespace_and_comments();

        let mut value = String::new();
        let mut paren_depth = 0i32;

        while !self.eof() {
            if self.starts_with("/*") {
                self.consume_whitespace_and_comments();
                continue;
            }

            let c = self.next_char();

            if c == '(' {
                paren_depth += 1;
                value.push(self.consume_char());
                continue;
            }
            if c == ')' {
                paren_depth -= 1;
                value.push(self.consume_char());
                continue;
            }

            if paren_depth == 0 && (c == ';' || c == '}') {
                break;
            }

            value.push(self.consume_char());
        }

        if !self.eof() && self.next_char() == ';' {
            self.consume_char();
        }

        let value = value.trim().to_string();

        Some(self.expand_declaration(name, value))
    }

    fn skip_bad_declaration(&mut self) {
        while !self.eof() {
            if self.starts_with("/*") {
                self.consume_whitespace_and_comments();
                continue;
            }
            let c = self.next_char();
            if c == ';' {
                self.consume_char();
                return;
            }
            if c == '}' {
                return;
            }
            self.consume_char();
        }
    }

    fn expand_declaration(&self, name: String, value: String) -> Vec<Declaration> {
        if name.eq_ignore_ascii_case("border") {
            self.expand_border_shorthand(value)
        } else if name.eq_ignore_ascii_case("list-style") {
            self.expand_list_style_shorthand(value)
        } else {
            vec![Declaration { name, value }]
        }
    }

    fn expand_list_style_shorthand(&self, value: String) -> Vec<Declaration> {
        let mut decls = Vec::new();

        for token in value.split_whitespace() {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }

            if is_list_style_type_token(token) {
                decls.push(Declaration {
                    name: "list-style-type".to_string(),
                    value: token.to_ascii_lowercase(),
                });
            } else if matches!(token.to_ascii_lowercase().as_str(), "inside" | "outside") {
                decls.push(Declaration {
                    name: "list-style-position".to_string(),
                    value: token.to_ascii_lowercase(),
                });
            }
        }

        if decls.is_empty() {
            decls.push(Declaration {
                name: "list-style".to_string(),
                value,
            });
        }

        decls
    }

    fn expand_border_shorthand(&self, value: String) -> Vec<Declaration> {
        let mut decls = Vec::new();

        let mut width: Option<String> = None;
        let mut color: Option<String> = None;

        for token in value.split_whitespace() {
            if token.ends_with("px") && width.is_none() {
                width = Some(token.to_string());
            } else if token.eq_ignore_ascii_case("solid") {
                continue;
            } else if color.is_none() {
                color = Some(token.to_string());
            }
        }

        if let Some(w) = width {
            decls.push(Declaration {
                name: "border-width".to_string(),
                value: w,
            });
        }
        if let Some(c) = color {
            decls.push(Declaration {
                name: "border-color".to_string(),
                value: c,
            });
        }

        if decls.is_empty() {
            decls.push(Declaration {
                name: "border".to_string(),
                value,
            });
        }

        decls
    }
}

fn is_list_style_type_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "none" | "disc" | "circle" | "square" | "decimal"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_style_declarations_expand_border_shorthand() {
        let declarations =
            parse_inline_declarations("background: #cfe; border: 2px solid #c96b00;");

        assert!(declarations.iter().any(|declaration| {
            declaration.name == "background" && declaration.value == "#cfe"
        }));
        assert!(declarations.iter().any(|declaration| {
            declaration.name == "border-width" && declaration.value == "2px"
        }));
        assert!(declarations.iter().any(|declaration| {
            declaration.name == "border-color" && declaration.value == "#c96b00"
        }));
    }

    #[test]
    fn list_style_shorthand_expands_type_and_position() {
        let declarations = parse_inline_declarations("list-style: none inside;");

        assert!(declarations.iter().any(|declaration| {
            declaration.name == "list-style-type" && declaration.value == "none"
        }));
        assert!(declarations.iter().any(|declaration| {
            declaration.name == "list-style-position" && declaration.value == "inside"
        }));
    }
}
