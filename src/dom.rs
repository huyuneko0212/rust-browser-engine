#![allow(dead_code)]


#[derive(Debug,Clone)]
pub enum NodeType {
    Text(String),
    Element(ElementData)
}

#[derive(Debug, Clone)]
pub struct Node {
    pub children: Vec<Node>,
    pub node_type: NodeType
}

#[derive(Debug, Clone)]
pub struct ElementData {
    pub tag_name: String,
}