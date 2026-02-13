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

    // --------------------------------------------------------
    // ★ inline element の background を padding込みで塗る
    // --------------------------------------------------------
    // ここで Rect を先に push することで、後で描く Text の「背面」になる
    if matches!(node.box_type, BoxType::InlineNode(_)) {
        if let Some(sn) = node.get_style_node() {
            // InlineNode の Element（span/a等）だけ対象
            if matches!(sn.node.node_type, crate::dom::NodeType::Element(_)) {
                if let Some(bg) = sn.background_color() {
                    // 子孫の Text ボックスを union して背景領域を作る
                    if let Some(mut bounds) = collect_descendant_text_bounds(node) {
                        // padding（px & shorthand対応）
                        let (pt, pr, pb, pl) = padding_trbl(sn);

                        bounds.x -= pl;
                        bounds.y -= pt;
                        bounds.width += pl + pr;
                        bounds.height += pt + pb;

                        if bounds.width > 0.0 && bounds.height > 0.0 {
                            out.push(DisplayItem::Rect(DrawRect {
                                x: bounds.x,
                                y: bounds.y,
                                w: bounds.width,
                                h: bounds.height,
                                color: bg,
                                base_color: bg,
                                href: None,
                            }));
                        }
                    }
                }
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
                    // skip draw
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

// --------------------------------------------------------
// ★ inline背景のための helper
// --------------------------------------------------------

/// node 配下の “Text(LayoutBox)” の content rect を union して返す
fn collect_descendant_text_bounds(node: &LayoutBox) -> Option<crate::layout::Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    let mut any = false;
    collect_descendant_text_bounds_rec(node, &mut min_x, &mut min_y, &mut max_x, &mut max_y, &mut any);

    if !any {
        return None;
    }

    Some(crate::layout::Rect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.0),
        height: (max_y - min_y).max(0.0),
    })
}

fn collect_descendant_text_bounds_rec(
    node: &LayoutBox,
    min_x: &mut f32,
    min_y: &mut f32,
    max_x: &mut f32,
    max_y: &mut f32,
    any: &mut bool,
) {
    for ch in &node.children {
        if let Some(sn) = ch.get_style_node() {
            if matches!(sn.node.node_type, crate::dom::NodeType::Text(_)) {
                let c = &ch.dimensions.content;
                // レイアウト側が幅を確保してるので、空白ノードもここでは含めてOK
                if c.width > 0.0 && c.height > 0.0 {
                    *min_x = (*min_x).min(c.x);
                    *min_y = (*min_y).min(c.y);
                    *max_x = (*max_x).max(c.x + c.width);
                    *max_y = (*max_y).max(c.y + c.height);
                    *any = true;
                }
            }
        }

        collect_descendant_text_bounds_rec(ch, min_x, min_y, max_x, max_y, any);
    }
}

/// padding を (top,right,bottom,left) で返す
/// - padding-top/right/bottom/left があればそれ優先
/// - padding shorthand は 1〜4値に対応（pxのみ）
fn padding_trbl(sn: &crate::style::StyledNode) -> (f32, f32, f32, f32) {
    let mut pt = 0.0;
    let mut pr = 0.0;
    let mut pb = 0.0;
    let mut pl = 0.0;

    // shorthand
    if let Some(v) = sn.value("padding") {
        if let Some((a, b, c, d)) = parse_trbl_px(v) {
            pt = a; pr = b; pb = c; pl = d;
        }
    }

    // longhands override
    if let Some(v) = sn.value("padding-top").and_then(|v| parse_px(v)) { pt = v; }
    if let Some(v) = sn.value("padding-right").and_then(|v| parse_px(v)) { pr = v; }
    if let Some(v) = sn.value("padding-bottom").and_then(|v| parse_px(v)) { pb = v; }
    if let Some(v) = sn.value("padding-left").and_then(|v| parse_px(v)) { pl = v; }

    (pt, pr, pb, pl)
}

/// TRBL の shorthand（pxのみ）
/// 1値: a a a a
/// 2値: a b a b
/// 3値: a b c b
/// 4値: a b c d
fn parse_trbl_px(s: &str) -> Option<(f32, f32, f32, f32)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let px = |x: &str| parse_px(x);

    match parts.len() {
        1 => {
            let a = px(parts[0])?;
            Some((a, a, a, a))
        }
        2 => {
            let a = px(parts[0])?;
            let b = px(parts[1])?;
            Some((a, b, a, b))
        }
        3 => {
            let a = px(parts[0])?;
            let b = px(parts[1])?;
            let c = px(parts[2])?;
            Some((a, b, c, b))
        }
        4 => {
            let a = px(parts[0])?;
            let b = px(parts[1])?;
            let c = px(parts[2])?;
            let d = px(parts[3])?;
            Some((a, b, c, d))
        }
        _ => None,
    }
}
