use crate::constants::{color, display as display_constants, layout as layout_constants};
use crate::layout::{BoxType, CornerRadii, LayoutBox};
use crate::style::Position;
use fontdue::Font;
use std::cmp::Ordering;

use crate::utility::url_utils::url_to_abs_string;

#[derive(Debug, Clone)]
pub enum DisplayItem {
    Rect(DrawRect),
    Border(DrawBorder), // ★ 追加: 枠線専用
    Text(DrawText),
    Image(DrawImage),
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,

    pub radius: CornerRadii,

    pub color: [f32; 4],
    pub base_color: [f32; 4], // hover解除で戻す用
    pub href: Option<String>, // 下線だけリンクに紐付ける（背景はNone）
    pub link_id: Option<usize>,
    pub fixed: bool,
}

#[derive(Debug, Clone)]
pub struct DrawBorder {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,

    pub radius: CornerRadii,
    pub border_width: f32, // とりあえず単一値

    pub color: [f32; 4],
    pub href: Option<String>,
    pub fixed: bool,
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
    pub fixed: bool,
}

#[derive(Debug, Clone)]
pub struct DrawImage {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,

    pub src: String,
    pub key: String,

    pub alt: Option<String>,
    pub href: Option<String>,
    pub hit: crate::layout::Rect,
    pub fixed: bool,
}

/// base_url を受け取って、画像 src を正規化できるようにする
pub fn build_display_list(
    root: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
) {
    out.clear();
    walk(root, out, font, base_url, false);
    merge_adjacent_link_underlines(out);
}

fn walk(
    node: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) {
    let fixed = fixed_context
        || node
            .get_style_node()
            .map(|sn| sn.position() == Position::Fixed)
            .unwrap_or(false);

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
    // （inline border は未対応なので背景だけ）
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
                                radius: CornerRadii::default(),
                                color: bg,
                                base_color: bg,
                                href: None,
                                link_id: None,
                                fixed,
                            }));
                        }
                    }
                }
            }
        }
    }

    // --------------------------------------------------------
    // BlockNode の背景 + border（border-radius 対応）
    // --------------------------------------------------------
    if matches!(node.box_type, BoxType::BlockNode(_)) {
        let d = &node.dimensions;

        // ★ border-box の矩形を計算
        let border_box = crate::layout::Rect {
            x: d.content.x - d.padding.left - d.border.left,
            y: d.content.y - d.padding.top - d.border.top,
            width: d.content.width
                + d.padding.left
                + d.padding.right
                + d.border.left
                + d.border.right,
            height: d.content.height
                + d.padding.top
                + d.padding.bottom
                + d.border.top
                + d.border.bottom,
        };

        if border_box.width > 0.0 && border_box.height > 0.0 {
            if let Some(sn) = node.get_style_node() {
                let radius = d
                    .border_radius
                    .normalize(border_box.width, border_box.height);

                // 背景（background-color は border-box まで塗る）
                if let Some(bg) = sn.background_color() {
                    out.push(DisplayItem::Rect(DrawRect {
                        x: border_box.x,
                        y: border_box.y,
                        w: border_box.width,
                        h: border_box.height,
                        radius,
                        color: bg,
                        base_color: bg,
                        href: None,
                        link_id: None,
                        fixed,
                    }));
                }

                // border（とりあえず四辺同じ太さ前提で max を使う）
                let border_width = d
                    .border
                    .left
                    .max(d.border.right)
                    .max(d.border.top)
                    .max(d.border.bottom);

                if border_width > 0.0 {
                    let border_color = sn.border_color().unwrap_or(color::BLACK);

                    out.push(DisplayItem::Border(DrawBorder {
                        x: border_box.x,
                        y: border_box.y,
                        w: border_box.width,
                        h: border_box.height,
                        radius,
                        border_width,
                        color: border_color,
                        href: None,
                        fixed,
                    }));
                }
            }
        }

        // li の bullet
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "li" {
                    let c = &node.dimensions.content;

                    let font_size =
                        font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
                    let line_h = line_height_px(sn, font_size);

                    let bx = c.x - (font_size * display_constants::LIST_MARKER_OFFSET_EM);
                    let by = c.y + font_size;

                    let color = sn.color().unwrap_or(color::DEFAULT_TEXT);
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
                        fixed,
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
                    let is_link = sn.link_href.is_some();
                    let font_size =
                        font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);

                    let mut color = sn.color().unwrap_or(color::DEFAULT_TEXT);
                    if is_link && sn.value("color").is_none() {
                        color = color::DEFAULT_LINK;
                    }
                    let base_color = color;
                    let underline_allowed = is_link && !text_decoration_none(sn);

                    if node.text_fragments.is_empty() {
                        let c = &node.dimensions.content;
                        if c.width > 0.0 && c.height > 0.0 {
                            out.push(DisplayItem::Text(DrawText {
                                x: c.x,
                                y: c.y + font_size,
                                text: txt.to_string(),
                                size_px: font_size,
                                color,
                                base_color,
                                href: sn.link_href.clone(),
                                hit: c.clone(),
                                fixed,
                            }));

                            if underline_allowed {
                                let underline_y =
                                    c.y + font_size + display_constants::UNDERLINE_GAP;
                                out.push(DisplayItem::Rect(DrawRect {
                                    x: c.x,
                                    y: underline_y,
                                    w: c.width.max(0.0),
                                    h: display_constants::UNDERLINE_THICKNESS,
                                    radius: CornerRadii::default(),
                                    color,
                                    base_color,
                                    href: sn.link_href.clone(),
                                    link_id: sn.link_id,
                                    fixed,
                                }));
                            }
                        }
                    } else {
                        for frag in &node.text_fragments {
                            let c = &frag.rect;
                            if c.width <= 0.0 || c.height <= 0.0 {
                                continue;
                            }

                            out.push(DisplayItem::Text(DrawText {
                                x: c.x,
                                y: c.y + font_size,
                                text: frag.text.clone(),
                                size_px: font_size,
                                color,
                                base_color,
                                href: sn.link_href.clone(),
                                hit: c.clone(),
                                fixed,
                            }));

                            if underline_allowed {
                                let underline_y =
                                    c.y + font_size + display_constants::UNDERLINE_GAP;
                                out.push(DisplayItem::Rect(DrawRect {
                                    x: c.x,
                                    y: underline_y,
                                    w: c.width.max(0.0),
                                    h: display_constants::UNDERLINE_THICKNESS,
                                    radius: CornerRadii::default(),
                                    color,
                                    base_color,
                                    href: sn.link_href.clone(),
                                    link_id: sn.link_id,
                                    fixed,
                                }));
                            }
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

                // サイズが無いなら描かない（layout問題）
                if c.width <= 0.0 || c.height <= 0.0 {
                    // 「最低限」なら何もしない
                } else if src_raw.is_empty() {
                    // src が無い/空 → alt だけ出す
                    if let Some(alt_text) = alt {
                        push_alt_text(out, sn, c, alt_text, fixed);
                    }
                } else {
                    // src を base_url で解決して、正規化キーを作る
                    let abs = base_url.resolve_location(src_raw);
                    let key = url_to_abs_string(&abs);

                    // 画像ロード可否を軽くチェック（失敗なら alt）
                    if crate::image_loader::can_load_image(&key) {
                        out.push(DisplayItem::Image(DrawImage {
                            x: c.x,
                            y: c.y,
                            w: c.width,
                            h: c.height,
                            src: key.clone(), // ★src と key を統一
                            key,
                            alt,
                            href: sn.link_href.clone(),
                            hit: c.clone(),
                            fixed,
                        }));
                    } else if let Some(alt_text) = alt {
                        push_alt_text(out, sn, c, alt_text, fixed);
                    } else {
                        // alt すら無いなら最低限 "[image]" を出してもいい
                        // push_alt_text(out, sn, c, "[image]".to_string(), fixed);
                    }
                }
            }
        }
    }

    for child in &node.children {
        walk(child, out, font, base_url, fixed);
    }
}

#[derive(Debug, Clone)]
struct UnderlineRun {
    insert_at: usize,
    rect: DrawRect,
}

fn merge_adjacent_link_underlines(items: &mut Vec<DisplayItem>) {
    let mut underlines = Vec::<(usize, DrawRect)>::new();
    let mut remove = vec![false; items.len()];

    for (idx, item) in items.iter().enumerate() {
        if let DisplayItem::Rect(rect) = item {
            if rect.href.is_some() && rect.link_id.is_some() && rect.w > 0.0 && rect.h > 0.0 {
                underlines.push((idx, rect.clone()));
                remove[idx] = true;
            }
        }
    }

    if underlines.len() < 2 {
        return;
    }

    underlines.sort_by(|a, b| compare_underlines(a, b));

    let mut runs = Vec::<UnderlineRun>::new();
    for (idx, rect) in underlines {
        if let Some(last) = runs.last_mut() {
            if can_join_underlines(&last.rect, &rect) {
                let new_end = (last.rect.x + last.rect.w).max(rect.x + rect.w);
                last.rect.x = last.rect.x.min(rect.x);
                last.rect.w = new_end - last.rect.x;
                last.insert_at = last.insert_at.min(idx);
                continue;
            }
        }

        runs.push(UnderlineRun {
            insert_at: idx,
            rect,
        });
    }

    runs.sort_by_key(|run| run.insert_at);

    let mut merged = Vec::with_capacity(items.len());
    let mut runs = runs.into_iter().peekable();

    for (idx, item) in items.drain(..).enumerate() {
        while runs.peek().is_some_and(|run| run.insert_at == idx) {
            let run = runs.next().unwrap();
            merged.push(DisplayItem::Rect(run.rect));
        }

        if remove[idx] {
            continue;
        }

        merged.push(item);
    }

    for run in runs {
        merged.push(DisplayItem::Rect(run.rect));
    }

    *items = merged;
}

fn compare_underlines(a: &(usize, DrawRect), b: &(usize, DrawRect)) -> Ordering {
    let (_, ar) = a;
    let (_, br) = b;

    ar.link_id
        .cmp(&br.link_id)
        .then_with(|| cmp_f32(ar.y, br.y))
        .then_with(|| cmp_f32(ar.h, br.h))
        .then_with(|| cmp_color(ar.color, br.color))
        .then_with(|| cmp_color(ar.base_color, br.base_color))
        .then_with(|| ar.fixed.cmp(&br.fixed))
        .then_with(|| cmp_f32(ar.x, br.x))
        .then_with(|| a.0.cmp(&b.0))
}

fn can_join_underlines(a: &DrawRect, b: &DrawRect) -> bool {
    if a.link_id != b.link_id
        || a.href != b.href
        || a.color != b.color
        || a.base_color != b.base_color
        || a.fixed != b.fixed
    {
        return false;
    }

    if !nearly_equal(a.y, b.y) || !nearly_equal(a.h, b.h) {
        return false;
    }

    let gap = b.x - (a.x + a.w);
    gap <= display_constants::UNDERLINE_JOIN_MAX_GAP_PX
}

fn cmp_f32(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn cmp_color(a: [f32; 4], b: [f32; 4]) -> Ordering {
    for (left, right) in a.into_iter().zip(b) {
        let ord = cmp_f32(left, right);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

fn nearly_equal(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn underline(x: f32, y: f32, w: f32, link_id: usize) -> DisplayItem {
        let color = color::DEFAULT_LINK;
        DisplayItem::Rect(DrawRect {
            x,
            y,
            w,
            h: display_constants::UNDERLINE_THICKNESS,
            radius: CornerRadii::default(),
            color,
            base_color: color,
            href: Some("https://example.com".to_string()),
            link_id: Some(link_id),
            fixed: false,
        })
    }

    #[test]
    fn joins_same_link_underlines_on_same_line() {
        let mut items = vec![
            underline(10.0, 20.0, 30.0, 1),
            underline(45.0, 20.0, 20.0, 1),
        ];

        merge_adjacent_link_underlines(&mut items);

        assert_eq!(items.len(), 1);
        match &items[0] {
            DisplayItem::Rect(rect) => {
                assert_eq!(rect.x, 10.0);
                assert_eq!(rect.w, 55.0);
            }
            _ => panic!("expected merged underline rect"),
        }
    }

    #[test]
    fn keeps_different_lines_or_links_separate() {
        let mut items = vec![
            underline(10.0, 20.0, 30.0, 1),
            underline(45.0, 40.0, 20.0, 1),
            underline(70.0, 20.0, 20.0, 2),
        ];

        merge_adjacent_link_underlines(&mut items);

        assert_eq!(items.len(), 3);
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
    font_size * layout_constants::DEFAULT_LINE_HEIGHT_MULTIPLIER
}

/// &String / &str どっちでも受けられるように
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

fn collect_descendant_text_bounds(node: &LayoutBox<'_>) -> Option<crate::layout::Rect> {
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
    node: &LayoutBox<'_>,
    min_x: &mut f32,
    min_y: &mut f32,
    max_x: &mut f32,
    max_y: &mut f32,
    any: &mut bool,
) {
    for ch in &node.children {
        if let Some(sn) = ch.get_style_node() {
            if matches!(sn.node.node_type, crate::dom::NodeType::Text(_)) {
                if ch.text_fragments.is_empty() {
                    let c = &ch.dimensions.content;
                    if c.width > 0.0 && c.height > 0.0 {
                        *min_x = (*min_x).min(c.x);
                        *min_y = (*min_y).min(c.y);
                        *max_x = (*max_x).max(c.x + c.width);
                        *max_y = (*max_y).max(c.y + c.height);
                        *any = true;
                    }
                } else {
                    for frag in &ch.text_fragments {
                        let c = &frag.rect;
                        if c.width > 0.0 && c.height > 0.0 {
                            *min_x = (*min_x).min(c.x);
                            *min_y = (*min_y).min(c.y);
                            *max_x = (*max_x).max(c.x + c.width);
                            *max_y = (*max_y).max(c.y + c.height);
                            *any = true;
                        }
                    }
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

fn push_alt_text(
    out: &mut Vec<DisplayItem>,
    sn: &crate::style::StyledNode,
    c: &crate::layout::Rect,
    text: String,
    fixed: bool,
) {
    let font_size = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
    let color = sn.color().unwrap_or(color::DEFAULT_TEXT);
    let base_color = color;

    out.push(DisplayItem::Text(DrawText {
        x: c.x,
        y: c.y + font_size,
        text,
        size_px: font_size,
        color,
        base_color,
        href: sn.link_href.clone(),
        hit: c.clone(),
        fixed,
    }));
}
