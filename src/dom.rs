#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NodeType {
    Text(String),
    Element(ElementData),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub children: Vec<Node>,
    pub node_type: NodeType,
}

#[derive(Debug, Clone)]
pub struct ElementData {
    pub tag_name: String,
    pub href: Option<String>,
    pub alt: Option<String>,
    pub attributes: HashMap<String, String>,
}

impl ElementData {
    pub fn id(&self) -> Option<&str> {
        self.attributes.get("id").map(|s| s.as_str())
    }

    pub fn classes(&self) -> Vec<&str> {
        self.attributes
            .get("class")
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_else(Vec::new)
    }
}

pub fn text(data: String) -> Node {
    Node {
        children: vec![],
        node_type: NodeType::Text(data),
    }
}

pub fn elem(name: String, attrs: HashMap<String, String>, children: Vec<Node>) -> Node {
    let href = attrs.get("href").cloned();
    let alt = attrs.get("alt").cloned();

    Node {
        children,
        node_type: NodeType::Element(ElementData {
            tag_name: name,
            href,
            alt,
            attributes: attrs,
        }),
    }
}
