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
    // いまは「文字列で保持」するだけ（例: "body", "h1", ".card", "#main", "div.content"）
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

            // コメント
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

    /// '@media ... { ... }' / '@import ...;' 等を丸ごと捨てる
    fn skip_at_rule(&mut self) {
        // '@' を含む行を ';' または '{...}' 終端まで飛ばす
        while !self.eof() && self.next_char() != ';' && self.next_char() != '{' {
            // コメントも飛ばす
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

        // '{' ブロック
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

            // at-rule をスキップ
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
        // 次の '{' または ';' または '}' まで進める
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

        // selectors ... '{'
        let selectors = self.parse_selectors();
        self.consume_whitespace_and_comments();

        if selectors.is_empty() {
            return None;
        }

        // declarations block
        let declarations = self.parse_declarations();

        Some(Rule {
            selectors,
            declarations,
        })
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        // '{' までを selector text として読み、 ',' で分割
        // 例: "body, html" / "div.content > p" も “文字列として保持” する
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

        // '{' が無ければ空
        if self.eof() || self.next_char() != '{' {
            return decls;
        }
        self.consume_char(); // '{'

        loop {
            self.consume_whitespace_and_comments();
            if self.eof() {
                break;
            }

            // ブロック終了
            if self.next_char() == '}' {
                self.consume_char();
                break;
            }

            // at-rule が来てもブロック内でスキップ
            if self.next_char() == '@' {
                self.skip_at_rule();
                continue;
            }

            if let Some(d) = self.parse_declaration() {
                decls.push(d);
            } else {
                self.skip_bad_declaration();
            }
        }

        decls
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        self.consume_whitespace_and_comments();

        // name: 英数字/ハイフン/アンダースコア を許可（CSS変数: --foo も通す）
        let name = self.consume_while(|c| c.is_alphanumeric() || c == '-' || c == '_');
        self.consume_whitespace_and_comments();

        if name.is_empty() {
            return None;
        }

        // ':' が来なければ宣言として成立しない（panicしない）
        if self.eof() || self.next_char() != ':' {
            return None;
        }
        self.consume_char(); // ':'
        self.consume_whitespace_and_comments();

        // value：';' or '}' まで。ただし括弧内は ';' を無視する
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

            // 終端判定
            if paren_depth == 0 && (c == ';' || c == '}') {
                break;
            }

            value.push(self.consume_char());
        }

        // ';' があれば消費
        if !self.eof() && self.next_char() == ';' {
            self.consume_char();
        }

        Some(Declaration {
            name,
            value: value.trim().to_string(),
        })
    }

    fn skip_bad_declaration(&mut self) {
        // ';' または '}' まで飛ばす（'}' は消費しない）
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
}
