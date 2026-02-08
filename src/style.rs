use std::collections::HashMap;

use crate::{css, dom};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    None,
}

#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node: dom::Node,
    pub specified_values: HashMap<String, String>,
    pub children: Vec<StyledNode>,
}

impl StyledNode {
    pub fn value(&self, name: &str) -> Option<&String> {
        self.specified_values.get(name)
    }

    pub fn display(&self) -> Display {
        if let Some(v) = self.value("display") {
            match v.as_str() {
                "none" => Display::None,
                "block" => Display::Block,
                "inline" => Display::Inline,
                _ => Display::Inline,
            }
        } else {
            // デフォルト：ElementはBlock寄り、TextはInline
            match self.node.node_type {
                dom::NodeType::Element(_) => Display::Block,
                dom::NodeType::Text(_) => Display::Inline,
            }
        }
    }

    pub fn lookup_px(&self, name: &str) -> f32 {
        self.value(name)
            .and_then(|s| parse_px(s))
            .unwrap_or(0.0)
    }

    pub fn background_color(&self) -> Option<[f32; 4]> {
        self.value("background")
            .or_else(|| self.value("background-color"))
            .and_then(|s| parse_color(s))
    }

    pub fn color(&self) -> Option<[f32; 4]> {
        self.value("color").and_then(|s| parse_color(s))
    }
}

pub fn style_tree(root: dom::Node, stylesheet: &css::Stylesheet) -> StyledNode {
    fn style_node(node: dom::Node, stylesheet: &css::Stylesheet) -> StyledNode {
        let mut specified = HashMap::new();

        if let dom::NodeType::Element(ed) = &node.node_type {
            // UAデフォルト：script/style は表示しない
            if ed.tag_name == "script" || ed.tag_name == "style" {
                specified.insert("display".to_string(), "none".to_string());
            }

            // CSS rule 適用（タグ名 selector のみ）
            for rule in &stylesheet.rules {
                if matches_rule(ed, rule) {
                    for decl in &rule.declarations {
                        specified.insert(decl.name.clone(), decl.value.clone());
                    }
                }
            }
        }

        let children = node
            .children
            .iter()
            .cloned()
            .map(|c| style_node(c, stylesheet))
            .collect::<Vec<_>>();

        StyledNode {
            node,
            specified_values: specified,
            children,
        }
    }

    style_node(root, stylesheet)
}

fn matches_rule(ed: &dom::ElementData, rule: &css::Rule) -> bool {
    // selector: Vec<Selector> だけど、ここでは "tag名" だけ見る
    // 例：body { ... } / h1 { ... }
    for sel in &rule.selectors {
        if sel.simple == ed.tag_name {
            return true;
        }
    }
    false
}

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.ends_with("px") {
        t.trim_end_matches("px").trim().parse::<f32>().ok()
    } else {
        None
    }
}

fn parse_color(s: &str) -> Option<[f32; 4]> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    // 超最低限（必要なら増やせる）
    match t {
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "gray" | "grey" => Some([0.5, 0.5, 0.5, 1.0]),
        _ => None,
    }
}

fn parse_hex_color(hex: &str) -> Option<[f32; 4]> {
    let h = hex.trim();
    match h.len() {
        3 => {
            let r = u8::from_str_radix(&h[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&h[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&h[2..3].repeat(2), 16).ok()?;
            Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
        }
        6 => {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
        }
        _ => None,
    }
}
