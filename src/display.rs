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
    pub base_color: [f32; 4], // hover解除で戻す用
    pub href: Option<String>, // 下線だけリンクに紐付ける（背景はNone）
}

#[derive(Debug, Clone)]
pub struct DrawText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size_px: f32,

    pub color: [f32; 4],
    pub base_color: [f32; 4],

    pub href: Option<String>,
    pub hit: crate::layout::Rect,
}

const UNDERLINE_THICKNESS: f32 = 1.5;
const UNDERLINE_GAP: f32 = 2.0;

pub fn build_display_list(root: &LayoutBox, out: &mut Vec<DisplayItem>, font: &Font) {
    out.clear();
    walk(root, out, font);
}

fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>, font: &Font) {
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
                        base_color: bg,
                        href: None,
                    }));
                }
            }
        }

        // ★li の bullet を描く
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "li" {
                    let c = &node.dimensions.content;

                    // li のフォントサイズ
                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);

                    // bullet位置：本文の少し左、baselineは本文と同じ寄せ
                    // UA_CSSで ul/ol の margin-left を入れてる前提でだいたい合う
                    let bx = c.x - (font_size * 1.1);

                    // baselineっぽく：display側の本文と同じ (c.y + font_size)
                    let by = c.y + font_size;

                    let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                    let base_color = color;

                    // bullet が ul/ol の外に出すぎる場合は係数を 0.9〜1.3 で調整
                    out.push(DisplayItem::Text(DrawText {
                        x: bx,
                        y: by,
                        text: "•".to_string(),
                        size_px: font_size,
                        color,
                        base_color,
                        href: None,
                        hit: crate::layout::Rect {
                            x: bx,
                            y: c.y,
                            width: font_size,
                            height: line_h,
                        },
                    }));
                }
            }
        }
    }

    // Text：InlineNode の Text ノードだけ描く
    if let Some(sn) = node.get_style_node() {
        if matches!(node.box_type, BoxType::InlineNode(_)) {
            if let crate::dom::NodeType::Text(t) = &sn.node.node_type {
                // ★trimやめ：空白を1個に畳む
                let collapsed = collapse_whitespace(t);
                let txt = collapsed.trim();

                // 空 or 空白だけは描かない（レイアウトはlayout側で幅を取ってる）
                if txt.is_empty() || txt == " " {
                    // ただし hit を潰しておく（保険）
                    // ※ node.dimensions は layout が持ってるので display 側では何もしなくてOK
                } else {
                    let c = &node.dimensions.content;
                    if c.width > 0.0 && c.height > 0.0 {
                        let is_link = sn.link_href.is_some();
                        let font_size = font_size_px(sn).unwrap_or(16.0);

                        // 色：CSS color が無ければデフォルト。リンクは青（CSS未指定時のみ）
                        let mut color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                        if is_link && sn.value("color").is_none() {
                            color = [0.0, 0.35, 0.95, 1.0];
                        }
                        let base_color = color;

                        out.push(DisplayItem::Text(DrawText {
                            x: c.x,
                            y: c.y + font_size,
                            text: txt.to_string(),
                            size_px: font_size,
                            color,
                            base_color,
                            href: sn.link_href.clone(),
                            hit: c.clone(),
                        }));

                        // text-decoration: none ならリンクでも下線を出さない
                        let underline_allowed = is_link && !text_decoration_none(sn);
                        if underline_allowed {
                            let underline_y = c.y + font_size + UNDERLINE_GAP;
                            out.push(DisplayItem::Rect(DrawRect {
                                x: c.x,
                                y: underline_y,
                                w: c.width.max(0.0),
                                h: UNDERLINE_THICKNESS,
                                color,
                                base_color,
                                href: sn.link_href.clone(),
                            }));
                        }
                    }
                }
            }
        }
    }

    for child in &node.children {
        walk(child, out, font);
    }
}

// ---------------------------
// CSS helpers（最低限）
// ---------------------------

fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    sn.value("font-size").and_then(|v| parse_px(v))
}

fn line_height_px(sn: &crate::style::StyledNode, font_size: f32) -> f32 {
    if let Some(v) = sn.value("line-height") {
        if let Some(px) = parse_px(v) {
            return px;
        }
        if let Ok(m) = v.trim().parse::<f32>() {
            return font_size * m;
        }
    }
    font_size * 1.2
}

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(num) = t.strip_suffix("px") {
        return num.trim().parse::<f32>().ok();
    }
    None
}

// text-decoration: none 判定（最小）
fn text_decoration_none(sn: &crate::style::StyledNode) -> bool {
    sn.value("text-decoration")
        .map(|v| v.to_lowercase().split_whitespace().any(|x| x == "none"))
        .unwrap_or(false)
}

// ★空白を1個に畳む
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;

    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}
