use crate::style::{Display, StyledNode};

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

#[derive(Debug, Default, Clone)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

#[derive(Debug)]
pub enum BoxType {
    BlockNode(StyledNode),
    InlineNode(StyledNode),
    Anonymous,
}

#[derive(Debug)]
pub struct LayoutBox {
    pub box_type: BoxType,
    pub dimensions: Dimensions,
    pub children: Vec<LayoutBox>,
}

impl LayoutBox {
    pub fn new(box_type: BoxType) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            children: vec![],
        }
    }

    pub fn get_style_node(&self) -> Option<&StyledNode> {
        match &self.box_type {
            BoxType::BlockNode(node) | BoxType::InlineNode(node) => Some(node),
            BoxType::Anonymous => None,
        }
    }

    pub fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block(containing_block),
            _ => {}
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());
        self.layout_block_children();
        self.calculate_block_height();
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        if let Some(style) = self.get_style_node() {
            if let Some(w) = style.value("width") {
                if let Ok(px) = w.replace("px", "").parse::<f32>() {
                    self.dimensions.content.width = px;
                    return;
                }
            }
        }

        self.dimensions.content.width = containing_block.content.width;
    }

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let d = &mut self.dimensions;

        d.content.x = containing_block.content.x;
        d.content.y = containing_block.content.y + containing_block.content.height;
    }

    fn layout_block_children(&mut self) {
        let mut y_offset = 0.0;

        for child in &mut self.children {
            let mut cb = self.dimensions.clone();
            cb.content.y += y_offset;

            child.layout(cb.clone());

            y_offset += child.dimensions.content.height;
        }

        self.dimensions.content.height = y_offset;
    }

    fn calculate_block_height(&mut self) {
        if let Some(style) = self.get_style_node() {
            if let Some(h) = style.value("height") {
                if let Ok(px) = h.replace("px", "").parse::<f32>() {
                    self.dimensions.content.height = px;
                }
            }
        }
    }
}

pub fn build_layout_tree(style_node: StyledNode) -> LayoutBox {
    let mut root = LayoutBox::new(match style_node.display() {
        Display::Block => BoxType::BlockNode(style_node.clone()),
        Display::Inline => BoxType::InlineNode(style_node.clone()),
        Display::None => BoxType::Anonymous,
    });

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
