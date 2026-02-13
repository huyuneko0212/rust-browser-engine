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
    pub href: Option<String>,
    pub hit: crate::layout::Rect,
}

const UNDERLINE_THICKNESS: f32 = 1.5;
const UNDERLINE_GAP: f32 = 2.0;

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

    // 背景：BlockNode だけ
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

                        let is_link = sn.link_href.is_some();

                        // デフォルト色
                        let mut color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);

                        // CSSでcolor未指定ならリンクは青に
                        if is_link && sn.value("color").is_none() {
                            color = [0.0, 0.35, 0.95, 1.0];
                        }

                        let font_size = font_size_px(sn).unwrap_or(16.0);

                        // テキスト描画
                        out.push(DisplayItem::Text(DrawText {
                            x: c.x,
                            y: c.y + font_size, // baseline寄せ
                            text: txt.to_string(),
                            size_px: font_size,
                            color,
                            href: sn.link_href.clone(),
                            hit: c.clone(),
                        }));

                        // ★リンクなら下線を引く
                        if is_link {
                            let underline_y = c.y + font_size + UNDERLINE_GAP;

                            out.push(DisplayItem::Rect(DrawRect {
                                x: c.x,
                                y: underline_y,
                                w: c.width.max(0.0),
                                h: UNDERLINE_THICKNESS,
                                color,
                            }));
                        }
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
