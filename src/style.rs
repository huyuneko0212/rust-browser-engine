use std::collections::HashMap;
use crate::dom::*;
use crate::css::*;

#[derive(Debug)]
pub struct StyledNode {
    pub node: Node,
    pub specified_values: HashMap<String, String>,
    pub children: Vec<StyledNode>,
}

pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    StyledNode {
        specified_values: specified_values(&root, stylesheet),
        children: root
        .children
        .iter()
        .map(|child| style_tree(child.clone(),stylesheet))
        .collect(),
        node: root,
    }
}
fn specified_values(node: &Node, stylesheet: &Stylesheet) -> HashMap<String, String> {
    let mut values =HashMap::new();

    match &node.node_type {
        NodeType::Element(elem) => {
            for rule in &stylesheet.rules {
                if matches(elem,&rule.selector) {
                    for decl in &rule.declarations {
                        values.insert(decl.name.clone(),decl.value.clone());
                    }
                }
            }
        }
        _ => {}
    }
    values
}

fn matches(elem: &ElementData, selectors: &Vec<Selector>) -> bool {
    for selector in selectors {
        if selector.simple == elem.tag_name {
            return true;
        }
    }
    false
}
