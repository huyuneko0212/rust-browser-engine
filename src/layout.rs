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

impl Dimensions {
    pub fn margin_box_height(&self) -> f32 {
        self.margin.top
            + self.border.top
            + self.padding.top
            + self.content.height
            + self.padding.bottom
            + self.border.bottom
            + self.margin.bottom
    }
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
            BoxType::InlineNode(_) => self.layout_inline(containing_block),
            BoxType::Anonymous => {}
        }
    }

    fn layout_inline(&mut self, containing_block: Dimensions) {
        // 仮：inline = 1行テキスト（本物は後で）
        let d = &mut self.dimensions;
        d.content.x = containing_block.content.x;
        d.content.y = containing_block.content.y;
        d.content.width = containing_block.content.width;
        d.content.height = 18.0;
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_model(containing_block.clone());
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());
        self.layout_block_children();
        self.calculate_block_height();
    }

    fn calculate_block_model(&mut self, _containing: Dimensions) {
        // style参照はここで終わらせる（= 値をローカルにコピーする）
        let (mut ml, mut mr, mut mt, mut mb, mut pl, mut pr, mut pt, mut pb, margin_sh, padding_sh) =
            if let Some(style) = self.get_style_node() {
                (
                    style.lookup_px("margin-left"),
                    style.lookup_px("margin-right"),
                    style.lookup_px("margin-top"),
                    style.lookup_px("margin-bottom"),
                    style.lookup_px("padding-left"),
                    style.lookup_px("padding-right"),
                    style.lookup_px("padding-top"),
                    style.lookup_px("padding-bottom"),
                    style.value("margin").cloned(),
                    style.value("padding").cloned(),
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None, None)
            };

        // shorthand margin
        if ml == 0.0 && mr == 0.0 && mt == 0.0 && mb == 0.0 {
            if let Some(m) = margin_sh.as_deref().and_then(parse_4px) {
                mt = m.0;
                mr = m.1;
                mb = m.2;
                ml = m.3;
            }
        }

        // shorthand padding
        if pl == 0.0 && pr == 0.0 && pt == 0.0 && pb == 0.0 {
            if let Some(p) = padding_sh.as_deref().and_then(parse_4px) {
                pt = p.0;
                pr = p.1;
                pb = p.2;
                pl = p.3;
            }
        }

        // ここから self.dimensions を書き換える（styleの借用はもう終わってる）
        self.dimensions.margin.left = ml;
        self.dimensions.margin.right = mr;
        self.dimensions.margin.top = mt;
        self.dimensions.margin.bottom = mb;

        self.dimensions.padding.left = pl;
        self.dimensions.padding.right = pr;
        self.dimensions.padding.top = pt;
        self.dimensions.padding.bottom = pb;
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        // 先にstyleから width を取る（借用終了）
        let specified_width_px = self
            .get_style_node()
            .and_then(|s| s.value("width"))
            .and_then(|v| parse_px(v));

        // ここから mutable 参照OK
        let d = &mut self.dimensions;

        if let Some(w) = specified_width_px {
            d.content.width = w;
            return;
        }

        let available = containing_block.content.width
            - d.margin.left
            - d.margin.right
            - d.padding.left
            - d.padding.right
            - d.border.left
            - d.border.right;

        d.content.width = available.max(0.0);
    }

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let d = &mut self.dimensions;

        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;

        d.content.y = containing_block.content.y + d.margin.top + d.border.top + d.padding.top;
    }

    fn layout_block_children(&mut self) {
        let mut y = self.dimensions.content.y;

        for child in &mut self.children {
            let mut cb = Dimensions::default();
            cb.content.x = self.dimensions.content.x;
            cb.content.y = y;
            cb.content.width = self.dimensions.content.width;

            child.layout(cb);

            y += child.dimensions.margin_box_height().max(0.0);
        }

        self.dimensions.content.height = (y - self.dimensions.content.y).max(0.0);
    }

    fn calculate_block_height(&mut self) {
        if let Some(style) = self.get_style_node() {
            if let Some(h) = style.value("height").and_then(|s| parse_px(s)) {
                self.dimensions.content.height = h;
            }
        }
    }
}

pub fn build_layout_tree(style_node: StyledNode) -> LayoutBox {
    let display = style_node.display();
    let mut root = LayoutBox::new(match display {
        Display::Block => BoxType::BlockNode(style_node.clone()),
        Display::Inline => BoxType::InlineNode(style_node.clone()),
        Display::None => BoxType::Anonymous,
    });

    for child in style_node.children {
        match child.display() {
            Display::None => {}
            Display::Block | Display::Inline => root.children.push(build_layout_tree(child)),
        }
    }
    root
}

// ---------------- helpers ----------------

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.ends_with("px") {
        t.trim_end_matches("px").trim().parse::<f32>().ok()
    } else {
        None
    }
}

// CSS 4-value shorthand: "top right bottom left"
// 1個: all, 2個: vertical/horizontal, 3個: top/horizontal/bottom, 4個: TRBL
fn parse_4px(s: &str) -> Option<(f32, f32, f32, f32)> {
    let parts = s
        .split_whitespace()
        .filter_map(|p| parse_px(p))
        .collect::<Vec<_>>();

    match parts.len() {
        1 => Some((parts[0], parts[0], parts[0], parts[0])),
        2 => Some((parts[0], parts[1], parts[0], parts[1])),
        3 => Some((parts[0], parts[1], parts[2], parts[1])),
        4 => Some((parts[0], parts[1], parts[2], parts[3])),
        _ => None,
    }
}
