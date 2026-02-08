#![allow(dead_code)]

use std::collections::HashMap;

use crate::css::{Declaration, Rule, Stylesheet};
use crate::dom::{ElementData, Node, NodeType};

#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node: Node,
    pub specified_values: HashMap<String, String>,
    pub children: Vec<StyledNode>,
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

    pub fn display(&self) -> Display {
        match self.value("display").map(|s| s.as_str()) {
            Some("block") => Display::Block,
            Some("none") => Display::None,
            _ => Display::Block, // 最小実装は全部 block でOK（後でinline判定する）
        }
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
            // "none" や "transparent" を弾く
            if v.trim() == "none" || v.trim() == "transparent" {
                return None;
            }
            return parse_color(v);
        }
        None
    }
}

pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    let specified_values = match root.node_type {
        NodeType::Element(ref e) => specified_values_for(e, stylesheet),
        NodeType::Text(_) => HashMap::new(),
    };

    let children = root
        .children
        .iter()
        .cloned()
        .map(|child| style_tree(child, stylesheet))
        .collect();

    StyledNode {
        node: root,
        specified_values,
        children,
    }
}

// ---------------- selector matching ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32); // (id, class, tag)

fn specified_values_for(elem: &ElementData, stylesheet: &Stylesheet) -> HashMap<String, String> {
    // いちばん強い宣言が最後に勝つように、(specificity, order) で適用する
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
    rule.selectors.iter().any(|sel| selector_matches(elem, &sel.simple))
}

fn rule_specificity(elem: &ElementData, rule: &Rule) -> Specificity {
    // 当たったセレクタの最大specificityを採用
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
/// それ以外は false（必要になったら拡張）
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
