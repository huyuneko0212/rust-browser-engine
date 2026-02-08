#![allow(dead_code)]

use crate::css::*;
use crate::dom::*;
use std::collections::HashMap;

/// display種別
#[derive(Debug, Clone, PartialEq)]
pub enum Display {
    Block,
    Inline,
    None,
}

/// style適用済みDOM
#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node: Node,
    pub specified_values: HashMap<String, String>,
    pub children: Vec<StyledNode>,
}

impl StyledNode {
    /// CSS値取得
    pub fn value(&self, name: &str) -> Option<String> {
        self.specified_values.get(name).cloned()
    }

    pub fn text(&self) -> Option<&str> {
        match &self.node.node_type {
            NodeType::Text(t) => Some(t),
            _ => None,
        }
    }

    /// display取得
    pub fn display(&self) -> Display {
        match self.value("display") {
            Some(v) if v == "block" => Display::Block,
            Some(v) if v == "inline" => Display::Inline,
            Some(v) if v == "none" => Display::None,
            _ => Display::Block, // デフォ block
        }
    }
}

/// DOM + CSS → style tree
pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    StyledNode {
        specified_values: specified_values(&root, stylesheet),
        children: root
            .children
            .clone()
            .into_iter()
            .map(|child| style_tree(child, stylesheet))
            .collect(),
        node: root,
    }
}

/// 指定CSS取得
fn specified_values(node: &Node, stylesheet: &Stylesheet) -> HashMap<String, String> {
    let mut values = HashMap::new();

    if let NodeType::Element(ref elem) = node.node_type {
        let rules = matching_rules(elem, stylesheet);

        for rule in rules {
            for decl in rule.declarations.clone() {
                values.insert(decl.name.clone(), decl.value.clone());
            }
        }
    }

    values
}

/// マッチするCSSルール
fn matching_rules<'a>(elem: &ElementData, stylesheet: &'a Stylesheet) -> Vec<&'a Rule> {
    let mut matched = vec![];

    for rule in &stylesheet.rules {
        for selector in &rule.selector {
            if matches(elem, selector) {
                matched.push(rule);
                break;
            }
        }
    }

    matched
}

/// selector一致判定（超簡易）
fn matches(elem: &ElementData, selector: &Selector) -> bool {
    selector.simple == elem.tag_name
}
