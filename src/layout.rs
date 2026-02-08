#![allow(dead_code)]

use crate::style::*;

#[derive(Debug)]
pub struct LayoutBox {
    pub box_type: BoxType,
    pub dimensions: Dimensions,
    pub children: Vec<LayoutBox>,
}

#[derive(Debug)]
pub enum BoxType {
    BlockNode(StyledNode),
    InlineNode(StyledNode),
    Anonymous,
}

#[derive(Debug, Default, Clone)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

#[derive(Debug, Default, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Default, Clone)]
pub struct EdgeSizes {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

pub fn build_layout_tree(style_node: StyledNode) -> LayoutBox {
    let mut root = LayoutBox {
        box_type: match style_node.display() {
            Display::Block => BoxType::BlockNode(style_node.clone()),
            Display::Inline => BoxType::InlineNode(style_node.clone()),
            Display::None => BoxType::Anonymous,
        },
        dimensions: Default::default(),
        children: vec![],
    };

    for child in style_node.children {
        match child.display() {
            Display::Block | Display::Inline => {
                root.children.push(build_layout_tree(child));
            }
            Display::None => {}
        }
    }

    root
}

impl LayoutBox {
    pub fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block(containing_block),
            _ => {}
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());
        self.layout_children();
        self.calculate_block_height();
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let width = 800.0; // 仮：画面幅

        self.dimensions.content.width = width;
    }
    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        self.dimensions.content.x = containing_block.content.x;
        self.dimensions.content.y = containing_block.content.y + containing_block.content.height;
    }
    fn layout_children(&mut self) {
        for child in &mut self.children {
            child.layout(self.dimensions.clone());
            self.dimensions.content.height += child.dimensions.content.height;
        }
    }
    fn calculate_block_height(&mut self) {
        if self.dimensions.content.height == 0.0 {
            self.dimensions.content.height = 18.0;
        }
    }
}
