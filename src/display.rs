use crate::layout::{BoxType, LayoutBox};
use fontdue::Font;

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

/// IFC前提：
/// - layout.rs が InlineNode(Text) の x/y/width/height を決める
/// - display.rs は “決まったものを描くだけ”
pub fn build_display_list(root: &LayoutBox, out: &mut Vec<DisplayItem>, _font: &Font) {
    out.clear();
    walk(root, out);
}

fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>) {
    // style/script/head/title/meta/link は描画しない（配下も止める）
    if let Some(sn) = node.get_style_node() {
        if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
            let t = ed.tag_name.as_str();
            if t == "style"
                || t == "script"
                || t == "head"
                || t == "title"
                || t == "meta"
                || t == "link"
            {
                return;
            }
        }
    }

    // 背景：BlockNode だけ（Anonymous/Inlineは基本描かない）
    if matches!(node.box_type, BoxType::BlockNode(_)) {
        let c = &node.dimensions.content;
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
    }

    // Text：InlineNode の Text ノードだけ描く（IFCで配置済み）
    if let Some(sn) = node.get_style_node() {
        if matches!(node.box_type, BoxType::InlineNode(_)) {
            if let crate::dom::NodeType::Text(t) = &sn.node.node_type {
                let txt = t.trim();
                if !txt.is_empty() {
                    let c = &node.dimensions.content;
                    if c.width > 0.0 && c.height > 0.0 {
                        let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                        let font_size = font_size_px(sn).unwrap_or(16.0);

                        // y は layout.rs の IFC が “行の上端” 基準で入れてる想定
                        // あなたの描画側が baseline 前提なら +font_size する等で調整
                        out.push(DisplayItem::Text(DrawText {
                            x: c.x,
                            y: c.y + font_size, // baselineっぽく寄せる（暫定）
                            text: txt.to_string(),
                            size_px: font_size,
                            color,
                        }));
                    }
                }
            }
        }
    }

    for child in &node.children {
        walk(child, out);
    }
}

// ---------------------------
// CSS helpers（最低限）
// ---------------------------
fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    sn.value("font-size").and_then(|v| parse_px(v))
}

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(num) = t.strip_suffix("px") {
        return num.trim().parse::<f32>().ok();
    }
    None
}
