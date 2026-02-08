use crate::layout::{BoxType, LayoutBox};

#[derive(Debug, Clone)]
pub enum DisplayItem {
    Rect(DrawRect),
    Text(DrawText),
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct DrawText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size_px: f32,
    pub color: [f32; 4],
}

pub fn build_display_list(root: &LayoutBox, out: &mut Vec<DisplayItem>) {
    fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>) {
        // style/script は描画しない（配下も止める）
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "style" || ed.tag_name == "script" {
                    return;
                }
            }
        }

        let c = &node.dimensions.content;

        // 背景色：指定があれば描く（なければ描かない）
        if c.width > 0.0 && c.height > 0.0 {
            if let Some(sn) = node.get_style_node() {
                if let Some(bg) = sn.background_color() {
                    out.push(DisplayItem::Rect(DrawRect {
                        x: c.x,
                        y: c.y,
                        w: c.width,
                        h: c.height,
                        color: bg,
                    }));
                }
            }
        }

        // Text
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Text(t) = &sn.node.node_type {
                let txt = t.trim();
                if !txt.is_empty() && c.width > 0.0 {
                    let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                    out.push(DisplayItem::Text(DrawText {
                        x: c.x,
                        y: c.y + 18.0,
                        text: txt.to_string(),
                        size_px: 16.0,
                        color,
                    }));
                }
            }
        }

        for child in &node.children {
            walk(child, out);
        }
    }

    walk(root, out);
}
