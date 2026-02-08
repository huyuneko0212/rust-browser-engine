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

pub fn build_display_list(root: &LayoutBox, out: &mut Vec<DisplayItem>, font: &Font) {
    out.clear();
    walk(root, out, font, false);
}

fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>, font: &Font, skip_text: bool) {
    // style/script/head/title/meta/link は描画しない（配下も止める）
    if let Some(sn) = node.get_style_node() {
        if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
            let t = ed.tag_name.as_str();
            if t == "style" || t == "script" || t == "head" || t == "title" || t == "meta" || t == "link" {
                return;
            }
        }
    }

    let c = &node.dimensions.content;

    // 背景
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

    let is_block = matches!(node.box_type, BoxType::BlockNode(_));
    let has_block_child = node
        .children
        .iter()
        .any(|ch| matches!(ch.box_type, BoxType::BlockNode(_)));

    // ★重要：子にBlockがいるBlockはテキストをまとめ描画しない（重複の根源）
    if is_block && !has_block_child {
        if let Some(sn) = node.get_style_node() {
            if c.width > 1.0 {
                let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);

                let font_size = font_size_px(sn).unwrap_or(16.0);
                let line_h = line_height_px(sn, font_size);

                let start_x = c.x;
                let max_w = c.width.max(1.0);

                // ★ここが「inline flow」：同一ブロック内でカーソル共有して流す
                let mut cursor_x = start_x;
                let mut cursor_y = c.y + font_size; // baselineっぽく（暫定）

                flow_inline_no_blocks(
                    node,
                    out,
                    font,
                    &mut cursor_x,
                    &mut cursor_y,
                    start_x,
                    max_w,
                    font_size,
                    line_h,
                    color,
                );
            }
        }

        // 葉ブロックはここで完結。子へは行ってもいいけどTextはスキップ（念のため）
        for child in &node.children {
            walk(child, out, font, true);
        }
        return;
    }

    // 非ブロック or ブロック(子にブロックあり) は、通常通り子を歩かせる
    // ただし親がまとめ描画済みなら Text は描かない
    if !skip_text {
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Text(t) = &sn.node.node_type {
                let txt = t.trim();
                if !txt.is_empty() && c.width > 1.0 {
                    let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);

                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);

                    let start_x = c.x;
                    let start_y = c.y + font_size;
                    let max_w = c.width.max(1.0);

                    // 単発Textノード描画は従来通り（ただしここは “カーソル共有” ではない）
                    let items = layout_text_fontdue(
                        txt,
                        start_x,
                        start_y,
                        max_w,
                        font_size,
                        line_h,
                        font,
                    );

                    for (x, y, s) in items {
                        let s = s.trim();
                        if !s.is_empty() {
                            out.push(DisplayItem::Text(DrawText {
                                x,
                                y,
                                text: s.to_string(),
                                size_px: font_size,
                                color,
                            }));
                        }
                    }
                }
            }
        }
    }

    for child in &node.children {
        walk(child, out, font, skip_text);
    }
}

/// ★ inline flow（Blockをまたがず）
/// - 子が BlockNode ならそこで止める（その子が描画担当）
/// - a/span 等は “同じカーソル” で中身を流す
/// - p/div/h1 等の “改行っぽいタグ” は簡易改行を入れる（最小実装）
fn flow_inline_no_blocks(
    node: &LayoutBox,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    start_x: f32,
    max_w: f32,
    font_size: f32,
    line_h: f32,
    color: [f32; 4],
) {
    // 自分がTextなら吐く
    if let Some(sn) = node.get_style_node() {
        match &sn.node.node_type {
            crate::dom::NodeType::Text(t) => {
                emit_text_runs_fontdue(
                    t,
                    out,
                    font,
                    cursor_x,
                    cursor_y,
                    start_x,
                    max_w,
                    font_size,
                    line_h,
                    color,
                );
                return;
            }
            crate::dom::NodeType::Element(ed) => {
                let tag = ed.tag_name.as_str();
                if tag == "style" || tag == "script" || tag == "head" || tag == "title" || tag == "meta" || tag == "link" {
                    return;
                }
                // br は改行
                if tag == "br" {
                    *cursor_x = start_x;
                    *cursor_y += line_h;
                    return;
                }
            }
        }
    }

    for child in &node.children {
        // 子がブロックならここでは処理しない（重複防止）
        if matches!(child.box_type, BoxType::BlockNode(_)) {
            continue;
        }

        // 改行っぽいタグ（最小実装）
        if let Some(sn) = child.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                let tag = ed.tag_name.as_str();
                let is_blockish = tag == "p" || tag == "div" || tag == "h1" || tag == "h2" || tag == "h3" || tag == "li" || tag == "ul" || tag == "ol";
                if is_blockish {
                    // ブロック開始：行を落とす
                    if *cursor_x != start_x {
                        *cursor_x = start_x;
                        *cursor_y += line_h;
                    }
                }
            }
        }

        flow_inline_no_blocks(
            child,
            out,
            font,
            cursor_x,
            cursor_y,
            start_x,
            max_w,
            font_size,
            line_h,
            color,
        );

        if let Some(sn) = child.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                let tag = ed.tag_name.as_str();
                let is_blockish = tag == "p" || tag == "div" || tag == "h1" || tag == "h2" || tag == "h3" || tag == "li" || tag == "ul" || tag == "ol";
                if is_blockish {
                    // ブロック終わり：次も改行
                    *cursor_x = start_x;
                    *cursor_y += line_h;
                }
            }
        }
    }
}

fn emit_text_runs_fontdue(
    raw: &str,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    start_x: f32,
    max_w: f32,
    font_size: f32,
    line_h: f32,
    color: [f32; 4],
) {
    let txt = raw.trim();
    if txt.is_empty() {
        return;
    }

    let has_spaces = txt.contains(' ');
    let tokens: Vec<String> = if has_spaces {
        txt.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        txt.chars().map(|c| c.to_string()).collect()
    };

    let space_w = if has_spaces {
        measure_width_fontdue(font, " ", font_size)
    } else {
        0.0
    };

    for tok in tokens {
        let w = measure_width_fontdue(font, &tok, font_size);

        // 折り返し
        if *cursor_x > start_x && *cursor_x + w > start_x + max_w {
            *cursor_x = start_x;
            *cursor_y += line_h;
        }

        out.push(DisplayItem::Text(DrawText {
            x: *cursor_x,
            y: *cursor_y,
            text: tok.clone(),
            size_px: font_size,
            color,
        }));

        *cursor_x += w;

        if has_spaces {
            *cursor_x += space_w;
        }
    }
}

// ---------------------------
// CSS helpers
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

// ---------------------------
// inline layout（単発Text用：あなたのfontdue版そのまま）
// ---------------------------
fn layout_text_fontdue(
    text: &str,
    start_x: f32,
    start_y: f32,
    max_w: f32,
    font_size: f32,
    line_h: f32,
    font: &Font,
) -> Vec<(f32, f32, String)> {
    let mut out = vec![];
    let mut x = start_x;
    let mut y = start_y;

    let has_spaces = text.contains(' ');

    let tokens: Vec<String> = if has_spaces {
        text.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        text.chars().map(|c| c.to_string()).collect()
    };

    let space_w = if has_spaces {
        measure_width_fontdue(font, " ", font_size)
    } else {
        0.0
    };

    for tok in tokens {
        let w = measure_width_fontdue(font, &tok, font_size);

        if x > start_x && x + w > start_x + max_w {
            x = start_x;
            y += line_h;
        }

        out.push((x, y, tok.clone()));
        x += w;

        if has_spaces {
            x += space_w;
        }
    }

    out
}

fn measure_width_fontdue(font: &Font, s: &str, px: f32) -> f32 {
    let mut w = 0.0;
    for ch in s.chars() {
        let m = font.metrics(ch, px);
        w += m.advance_width;
    }
    w
}
