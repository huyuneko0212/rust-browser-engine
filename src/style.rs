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

// =========================================================
// descendant selector のための祖先情報
// =========================================================

#[derive(Debug, Clone)]
struct Ancestor {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
}

fn ancestor_of(e: &ElementData) -> Ancestor {
    Ancestor {
        tag: e.tag_name.clone(),
        id: e.id().map(|s| s.to_string()),
        classes: e.classes().iter().map(|s| s.to_string()).collect(),
    }
}

// =========================================================
// ★ 継承するプロパティ（最小）
// =========================================================

fn is_inheritable_prop(name: &str) -> bool {
    matches!(
        name,
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "line-height"
            | "text-decoration"
    )
}

fn inherit_only(values: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in values {
        if is_inheritable_prop(k.as_str()) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

// =========================================================

pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    style_tree_with_ctx(root, stylesheet, None, &Vec::new(), &HashMap::new())
}

fn style_tree_with_ctx(
    root: Node,
    stylesheet: &Stylesheet,
    inherited_link: Option<String>,
    ancestors: &Vec<Ancestor>,
    inherited_values: &HashMap<String, String>,
) -> StyledNode {
    // まず継承値をベースにする（Textにも効く）
    let mut specified_values = inherited_values.clone();

    // Elementなら、自分に当たるCSSで上書き
    if let NodeType::Element(ref e) = root.node_type {
        let own = specified_values_for(e, stylesheet, ancestors);
        for (k, v) in own {
            specified_values.insert(k, v);
        }
    }

    // link 継承（a の href を子孫テキストへ渡す）
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

    // 子へ渡す ancestor stack
    let mut next_ancestors = ancestors.clone();
    if let NodeType::Element(ref e) = root.node_type {
        next_ancestors.push(ancestor_of(e));
    }

    // ★ 子へ渡す継承値（継承プロパティだけ）
    let next_inherited_values = inherit_only(&specified_values);

    let children = root
        .children
        .iter()
        .cloned()
        .map(|c| {
            style_tree_with_ctx(
                c,
                stylesheet,
                link_here.clone(),
                &next_ancestors,
                &next_inherited_values,
            )
        })
        .collect();

    StyledNode {
        node: root,
        specified_values,
        children,
        link_href: link_here,
    }
}

// ---------------- selector matching ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32); // (id, class, tag)

fn specified_values_for(
    elem: &ElementData,
    stylesheet: &Stylesheet,
    ancestors: &[Ancestor],
) -> HashMap<String, String> {
    let mut matched: Vec<(Specificity, usize, &Rule)> = vec![];

    for (i, rule) in stylesheet.rules.iter().enumerate() {
        if rule_matches(elem, rule, ancestors) {
            let spec = rule_specificity(elem, rule, ancestors);
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

fn rule_matches(elem: &ElementData, rule: &Rule, ancestors: &[Ancestor]) -> bool {
    rule.selectors
        .iter()
        .any(|sel| selector_matches_descendant(elem, ancestors, &sel.simple))
}

fn rule_specificity(elem: &ElementData, rule: &Rule, ancestors: &[Ancestor]) -> Specificity {
    rule.selectors
        .iter()
        .filter_map(|sel| selector_specificity_if_matches(elem, ancestors, &sel.simple))
        .max()
        .unwrap_or(Specificity(0, 0, 0))
}

fn selector_matches_descendant(elem: &ElementData, ancestors: &[Ancestor], selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
        return false;
    }

    // まだ未対応
    if s.contains('>') || s.contains('+') || s.contains('[') || s.contains(':') {
        return false;
    }

    let parts: Vec<&str> = s.split_whitespace().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return false;
    }

    if !selector_matches_simple_elem(elem, parts[parts.len() - 1]) {
        return false;
    }

    let mut upto = ancestors.len();
    for part in parts[..parts.len() - 1].iter().rev() {
        if let Some(pos) = find_ancestor_match(ancestors, upto, part) {
            upto = pos;
        } else {
            return false;
        }
    }

    true
}

fn find_ancestor_match(ancestors: &[Ancestor], upto: usize, part: &str) -> Option<usize> {
    let mut i = upto;
    while i > 0 {
        i -= 1;
        if selector_matches_simple_ancestor(&ancestors[i], part) {
            return Some(i);
        }
    }
    None
}

fn selector_matches_simple_elem(elem: &ElementData, selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
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

fn selector_matches_simple_ancestor(a: &Ancestor, selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
        return false;
    }

    if let Some(id) = s.strip_prefix('#') {
        return a.id.as_deref() == Some(id);
    }

    if let Some(class) = s.strip_prefix('.') {
        return a.classes.iter().any(|c| c == class);
    }

    if let Some((tag, class)) = s.split_once('.') {
        if a.tag != tag {
            return false;
        }
        return a.classes.iter().any(|c| c == class);
    }

    a.tag == s
}

fn selector_specificity_if_matches(
    elem: &ElementData,
    ancestors: &[Ancestor],
    selector: &str,
) -> Option<Specificity> {
    if !selector_matches_descendant(elem, ancestors, selector) {
        return None;
    }

    let parts: Vec<&str> = selector.trim().split_whitespace().collect();

    let mut id = 0u32;
    let mut class = 0u32;
    let mut tag = 0u32;

    for p in parts {
        let sp = specificity_of_simple(p);
        id += sp.0;
        class += sp.1;
        tag += sp.2;
    }

    Some(Specificity(id, class, tag))
}

fn specificity_of_simple(s: &str) -> Specificity {
    let s = s.trim();
    if s.is_empty() {
        return Specificity(0, 0, 0);
    }
    if s.starts_with('#') {
        return Specificity(1, 0, 0);
    }
    if s.starts_with('.') {
        return Specificity(0, 1, 0);
    }
    if s.contains('.') {
        return Specificity(0, 1, 1);
    }
    Specificity(0, 0, 1)
}

// ---------------- color ----------------

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
