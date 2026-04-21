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

            // ★ ここ変更: parse_declaration が Vec<Declaration> を返す
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

        let value = value.trim().to_string();

        // ショートハンド展開
        Some(self.expand_declaration(name, value))
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

    /// ショートハンドを展開する。
    /// - border: 1px solid red;
    ///   → border-width: 1px; / border-color: red;
    /// それ以外はそのまま 1 件の Declaration にする。
    fn expand_declaration(&self, name: String, value: String) -> Vec<Declaration> {
        if name.eq_ignore_ascii_case("border") {
            self.expand_border_shorthand(value)
        } else {
            vec![Declaration { name, value }]
        }
    }

    fn expand_border_shorthand(&self, value: String) -> Vec<Declaration> {
        let mut decls = Vec::new();

        let mut width: Option<String> = None;
        let mut color: Option<String> = None;

        // めちゃくちゃ簡易な実装：
        // - "px" で終わるトークン → 幅
        // - "solid" は無視
        // - それ以外の最初のトークン → 色
        for token in value.split_whitespace() {
            if token.ends_with("px") && width.is_none() {
                width = Some(token.to_string());
            } else if token.eq_ignore_ascii_case("solid") {
                // 今回は solid のみ対応、値としては保持しない
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

        // もし width / color のどちらも取れなかった場合、
        // 元の "border" も残しておくほうが安全かもしれない。
        if decls.is_empty() {
            decls.push(Declaration {
                name: "border".to_string(),
                value,
            });
        }

        decls
    }
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
}
