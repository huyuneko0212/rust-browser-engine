#![allow(dead_code)]

use std::collections::HashMap;

use crate::css::{Declaration, Rule, Stylesheet};
use crate::dom::{ElementData, Node, NodeType};

#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node: Node,
    pub specified_values: HashMap<String, String>,
    pub children: Vec<StyledNode>,
    pub link_href: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    None,
}

impl StyledNode {
    pub fn value(&self, name: &str) -> Option<&String> {
        self.specified_values.get(name)
    }

    /// display の最小実装（IFC向け）
    /// - Textノードは必ず Inline
    /// - CSS display 指定があれば最優先
    /// - head/title/meta/script/style/link は None
    /// - よくある block 要素は Block
    /// - それ以外は Inline（迷ったら inline のほうが IFC が動きやすい）
    pub fn display(&self) -> Display {
        // Textノードは必ず inline
        if matches!(self.node.node_type, NodeType::Text(_)) {
            return Display::Inline;
        }

        // CSSで指定があれば最優先
        if let Some(v) = self.value("display").map(|s| s.as_str()) {
            return match v {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "none" => Display::None,
                _ => Display::Inline,
            };
        }

        // Element のデフォルト表示（超最小）
        if let NodeType::Element(ref e) = self.node.node_type {
            let tag = e.tag_name.as_str();

            if is_hidden_element(tag) {
                return Display::None;
            }
            if is_block_element(tag) {
                return Display::Block;
            }
            // それ以外は inline 扱い
            return Display::Inline;
        }

        Display::Inline
    }

    pub fn color(&self) -> Option<[f32; 4]> {
        self.value("color").and_then(|v| parse_color(v))
    }

    pub fn background_color(&self) -> Option<[f32; 4]> {
        // background も background-color も見る（最小実装）
        if let Some(v) = self.value("background-color") {
            return parse_color(v);
        }
        if let Some(v) = self.value("background") {
            let t = v.trim().to_lowercase();
            if t == "none" || t == "transparent" {
                return None;
            }
            return parse_color(v);
        }
        None
    }
}

/// そもそも表示しない（display:none相当）
fn is_hidden_element(tag: &str) -> bool {
    matches!(tag, "head" | "meta" | "title" | "script" | "style" | "link")
}

/// HTMLのデフォルト表示に寄せた “最小” block 判定
fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
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
            | "article"
            | "header"
            | "footer"
            | "nav"
            | "main"
            | "pre"
            | "blockquote"
    )
}

pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    style_tree_with_ctx(root, stylesheet, None)
}

fn style_tree_with_ctx(
    root: Node,
    stylesheet: &Stylesheet,
    inherited_link: Option<String>,
) -> StyledNode {
    let specified_values = match root.node_type {
        NodeType::Element(ref e) => specified_values_for(e, stylesheet),
        NodeType::Text(_) => HashMap::new(),
    };

    let mut link_here = inherited_link.clone();
    if let NodeType::Element(ref e) = root.node_type {
        if e.tag_name == "a" {
            if let Some(href) = e.attributes.get("href") {
                let h = href.trim();
                if !h.is_empty()
                    && !h.starts_with('#')
                    && !h.to_lowercase().starts_with("javascript:")
                    && !h.to_lowercase().starts_with("data:")
                {
                    link_here = Some(h.to_string());
                }
            }
        }
    }

    let children = root.children
        .iter()
        .cloned()
        .map(|c| style_tree_with_ctx(c, stylesheet, link_here.clone()))
        .collect();

    StyledNode { node: root, specified_values, children, link_href: link_here }
}

// ---------------- selector matching ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32); // (id, class, tag)

fn specified_values_for(elem: &ElementData, stylesheet: &Stylesheet) -> HashMap<String, String> {
    // いちばん強い宣言が最後に勝つように、(specificity, order) で適用
    let mut matched: Vec<(Specificity, usize, &Rule)> = vec![];

    for (i, rule) in stylesheet.rules.iter().enumerate() {
        if rule_matches(elem, rule) {
            let spec = rule_specificity(elem, rule);
            matched.push((spec, i, rule));
        }
    }

    matched.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut values = HashMap::new();
    for (_, _, rule) in matched {
        for Declaration { name, value } in &rule.declarations {
            values.insert(name.clone(), value.clone());
        }
    }

    values
}

fn rule_matches(elem: &ElementData, rule: &Rule) -> bool {
    // 複数セレクタ: どれか当たればOK
    rule.selectors
        .iter()
        .any(|sel| selector_matches(elem, &sel.simple))
}

fn rule_specificity(elem: &ElementData, rule: &Rule) -> Specificity {
    // 当たったセレクタの最大specificity
    rule.selectors
        .iter()
        .filter_map(|sel| selector_specificity_if_matches(elem, &sel.simple))
        .max()
        .unwrap_or(Specificity(0, 0, 0))
}

/// selector文字列を “最小で” 解釈して一致判定
/// - "#id"
/// - ".class"
/// - "tag"
/// - "tag.class"（簡易）
/// それ以外は false
fn selector_matches(elem: &ElementData, selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
        return false;
    }

    // 複雑なセレクタ（空白、>、+、[attr]、:pseudo 等）は今は捨てる
    if s.contains(' ') || s.contains('>') || s.contains('+') || s.contains('[') || s.contains(':') {
        return false;
    }

    // #id
    if let Some(id) = s.strip_prefix('#') {
        return elem.id() == Some(id);
    }

    // .class
    if let Some(class) = s.strip_prefix('.') {
        return elem.classes().iter().any(|c| *c == class);
    }

    // tag.class（簡易）
    if let Some((tag, class)) = s.split_once('.') {
        if elem.tag_name != tag {
            return false;
        }
        return elem.classes().iter().any(|c| *c == class);
    }

    // tag
    elem.tag_name == s
}

fn selector_specificity_if_matches(elem: &ElementData, selector: &str) -> Option<Specificity> {
    if !selector_matches(elem, selector) {
        return None;
    }

    let s = selector.trim();
    if s.starts_with('#') {
        return Some(Specificity(1, 0, 0));
    }
    if s.starts_with('.') {
        return Some(Specificity(0, 1, 0));
    }
    if s.contains('.') {
        return Some(Specificity(0, 1, 1)); // tag.class
    }
    Some(Specificity(0, 0, 1)) // tag
}

fn parse_color(s: &str) -> Option<[f32; 4]> {
    let t = s.trim().to_lowercase();

    // transparent
    if t == "transparent" {
        return Some([0.0, 0.0, 0.0, 0.0]);
    }

    // #rgb / #rrggbb
    if let Some(hex) = t.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()? as f32 / 255.0;
            return Some([r, g, b, 1.0]);
        }
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
            return Some([r, g, b, 1.0]);
        }
    }

    // rgb(r,g,b) / rgba(r,g,b,a)
    if t.starts_with("rgb(") || t.starts_with("rgba(") {
        let inside = t
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')')
            .to_string();
        let parts: Vec<&str> = inside.split(',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parts[0].parse::<f32>().ok()? / 255.0;
        let g = parts[1].parse::<f32>().ok()? / 255.0;
        let b = parts[2].parse::<f32>().ok()? / 255.0;
        let a = if parts.len() >= 4 {
            parts[3].parse::<f32>().ok()?
        } else {
            1.0
        };
        return Some([r, g, b, a]);
    }

    // named colors（最小）
    match t.as_str() {
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "gray" | "grey" => Some([0.5, 0.5, 0.5, 1.0]),
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 1.0, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        _ => None,
    }
}
