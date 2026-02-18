use crate::layout::{BoxType, LayoutBox};
use fontdue::Font;

use crate::utility::url_utils::url_to_abs_string;

#[derive(Debug, Clone)]
pub enum DisplayItem {
    Rect(DrawRect),
    Text(DrawText),
    Image(DrawImage),
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

#[derive(Debug, Clone)]
pub struct DrawImage {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,

    /// ★ src と key を “同じ正規化キー” に統一
    /// 例: https://example.com/a.png （デフォルトポート省略など）
    pub src: String,
    pub key: String,

    /// ★読み込み失敗時フォールバック用
    pub alt: Option<String>,

    pub href: Option<String>,
    pub hit: crate::layout::Rect,
}

const UNDERLINE_THICKNESS: f32 = 1.5;
const UNDERLINE_GAP: f32 = 2.0;

/// ★ base_url を受け取って、画像 src を正規化できるようにする
pub fn build_display_list(
    root: &LayoutBox,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
) {
    out.clear();
    walk(root, out, font, base_url);
}

fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>, font: &Font, base_url: &crate::url::URL) {
    // style/script/head/title/meta/link は描画しない（配下も止める）
    if let Some(sn) = node.get_style_node() {
        if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
            match ed.tag_name.as_str() {
                "style" | "script" | "head" | "title" | "meta" | "link" => return,
                _ => {}
            }
        }
    }

    // --------------------------------------------------------
    // inline element の background を padding込みで塗る
    // --------------------------------------------------------
    if matches!(node.box_type, BoxType::InlineNode(_)) {
        if let Some(sn) = node.get_style_node() {
            if matches!(sn.node.node_type, crate::dom::NodeType::Element(_)) {
                if let Some(bg) = sn.background_color() {
                    if let Some(mut bounds) = collect_descendant_text_bounds(node) {
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

        // li の bullet
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "li" {
                    let c = &node.dimensions.content;

                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);

                    let bx = c.x - (font_size * 1.1);
                    let by = c.y + font_size;

                    let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                    let base_color = color;

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
                let collapsed = collapse_whitespace(t);
                let txt = collapsed.trim();

                if !txt.is_empty() && txt != " " {
                    let c = &node.dimensions.content;
                    if c.width > 0.0 && c.height > 0.0 {
                        let is_link = sn.link_href.is_some();
                        let font_size = font_size_px(sn).unwrap_or(16.0);

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

    // img を描画アイテムにする
    if let Some(sn) = node.get_style_node() {
        if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
            if ed.tag_name == "img" {
                let c = &node.dimensions.content;

                let src_raw = ed.attributes.get("src").map(|s| s.trim()).unwrap_or("");
                let alt = ed
                    .attributes
                    .get("alt")
                    .map(|s| collapse_whitespace(s))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                eprintln!(
                    "[IMG] x={:.1} y={:.1} w={:.1} h={:.1} href={:?} src={:?} alt={:?}",
                    c.x,
                    c.y,
                    c.width,
                    c.height,
                    sn.link_href.as_deref(),
                    ed.attributes.get("src").map(|s| s.as_str()),
                    alt.as_deref()
                );

                // サイズが無いなら描かない（layout問題）
                if c.width <= 0.0 || c.height <= 0.0 {
                    eprintln!("[IMG] skipped: zero-size (likely layout issue)");
                    // それでも alt を出したいならここで Text を出す手もあるが、
                    // まずは layout を直す方が健全なのでスキップにしておく。
                } else if src_raw.is_empty() {
                    // src 自体が無い/空 → ここは render を待てないので最低限 alt を出す
                    if let Some(alt_text) = alt {
                        let font_size = font_size_px(sn).unwrap_or(16.0);
                        let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);
                        out.push(DisplayItem::Text(DrawText {
                            x: c.x,
                            y: c.y + font_size,
                            text: alt_text,
                            size_px: font_size,
                            color,
                            base_color: color,
                            href: sn.link_href.clone(),
                            hit: c.clone(),
                        }));
                    }
                } else {
                    // ★ここが本題：src を base_url で解決して、正規化キーを作る
                    let abs = base_url.resolve_location(src_raw);
                    let key = url_to_abs_string(&abs);

                    // ★ src/key を同じ文字列にする（安定化）
                    let src = key.clone();

                    out.push(DisplayItem::Image(DrawImage {
                        x: c.x,
                        y: c.y,
                        w: c.width,
                        h: c.height,
                        src,
                        key,
                        alt,
                        href: sn.link_href.clone(),
                        hit: c.clone(),
                    }));
                }
            }
        }
    }

    for child in &node.children {
        walk(child, out, font, base_url);
    }
}

// ---------------------------
// CSS helpers（最低限）
// ---------------------------

fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    sn.value("font-size").and_then(parse_px)
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

/// ★ &String / &str どっちでも受けられるようにして E0631 を潰す（Rustっぽく）
fn parse_px(s: impl AsRef<str>) -> Option<f32> {
    let t = s.as_ref().trim();
    t.strip_suffix("px")?.trim().parse::<f32>().ok()
}

fn text_decoration_none(sn: &crate::style::StyledNode) -> bool {
    sn.value("text-decoration")
        .map(|v| v.to_lowercase().split_whitespace().any(|x| x == "none"))
        .unwrap_or(false)
}

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
// inline背景のための helper
// --------------------------------------------------------

fn collect_descendant_text_bounds(node: &LayoutBox) -> Option<crate::layout::Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    let mut any = false;
    collect_descendant_text_bounds_rec(
        node, &mut min_x, &mut min_y, &mut max_x, &mut max_y, &mut any,
    );

    any.then(|| crate::layout::Rect {
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

fn padding_trbl(sn: &crate::style::StyledNode) -> (f32, f32, f32, f32) {
    let mut pt = 0.0;
    let mut pr = 0.0;
    let mut pb = 0.0;
    let mut pl = 0.0;

    if let Some(v) = sn.value("padding") {
        if let Some((a, b, c, d)) = parse_trbl_px(v) {
            pt = a;
            pr = b;
            pb = c;
            pl = d;
        }
    }

    if let Some(v) = sn.value("padding-top").and_then(parse_px) {
        pt = v;
    }
    if let Some(v) = sn.value("padding-right").and_then(parse_px) {
        pr = v;
    }
    if let Some(v) = sn.value("padding-bottom").and_then(parse_px) {
        pb = v;
    }
    if let Some(v) = sn.value("padding-left").and_then(parse_px) {
        pl = v;
    }

    (pt, pr, pb, pl)
}

fn parse_trbl_px(s: &str) -> Option<(f32, f32, f32, f32)> {
    let parts: Vec<&str> = s.split_whitespace().collect();

    match parts.len() {
        1 => {
            let a = parse_px(parts[0])?;
            Some((a, a, a, a))
        }
        2 => {
            let a = parse_px(parts[0])?;
            let b = parse_px(parts[1])?;
            Some((a, b, a, b))
        }
        3 => {
            let a = parse_px(parts[0])?;
            let b = parse_px(parts[1])?;
            let c = parse_px(parts[2])?;
            Some((a, b, c, b))
        }
        4 => {
            let a = parse_px(parts[0])?;
            let b = parse_px(parts[1])?;
            let c = parse_px(parts[2])?;
            let d = parse_px(parts[3])?;
            Some((a, b, c, d))
        }
        _ => None,
    }
}
