use crate::constants::{color, display as display_constants, layout as layout_constants};
use crate::layout::{BoxType, CornerRadii, LayoutBox};
use crate::style::{Display, Position};
use fontdue::Font;
use std::cmp::Ordering;

use crate::utility::url_utils::url_to_abs_string;

#[derive(Debug, Clone)]
pub enum DisplayItem {
    Rect(DrawRect),
    Border(DrawBorder),
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
    pub base_color: [f32; 4],
    pub href: Option<String>,
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
    pub border_width: f32,

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

pub fn build_display_list(
    root: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
) {
    out.clear();
    paint_stacking_context(root, out, font, base_url, false);
    merge_adjacent_link_underlines(out);
}

fn paint_stacking_context(
    node: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) {
    if skips_display_subtree(node) {
        return;
    }

    let fixed = node_fixed_context(node, fixed_context);
    paint_node_contents(node, out, base_url, fixed);
    paint_stacking_context_children(node, out, font, base_url, fixed);
}

fn paint_node_contents(
    node: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    base_url: &crate::url::URL,
    fixed: bool,
) {
    if matches!(node.box_type, BoxType::InlineNode(_)) && !is_inline_block_box(node) {
        paint_inline_element_fragments(node, out, fixed);
    }

    if matches!(node.box_type, BoxType::BlockNode(_)) || is_inline_block_box(node) {
        let d = &node.dimensions;

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

                if c.width <= 0.0 || c.height <= 0.0 {
                } else if src_raw.is_empty() {
                    if let Some(alt_text) = alt {
                        push_alt_text(out, sn, c, alt_text, fixed);
                    }
                } else {
                    let abs = base_url.resolve_location(src_raw);
                    let key = url_to_abs_string(&abs);

                    if crate::image_loader::can_load_image(&key) {
                        out.push(DisplayItem::Image(DrawImage {
                            x: c.x,
                            y: c.y,
                            w: c.width,
                            h: c.height,
                            src: key.clone(),
                            key,
                            alt,
                            href: sn.link_href.clone(),
                            hit: c.clone(),
                            fixed,
                        }));
                    } else if let Some(alt_text) = alt {
                        push_alt_text(out, sn, c, alt_text, fixed);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct StackingContextPaint {
    level: i32,
    order: usize,
    items: Vec<DisplayItem>,
}

#[derive(Debug, Default)]
struct StackingContextCollector {
    normal_items: Vec<DisplayItem>,
    child_contexts: Vec<StackingContextPaint>,
    next_order: usize,
}

impl StackingContextCollector {
    fn take_order(&mut self) -> usize {
        let order = self.next_order;
        self.next_order += 1;
        order
    }
}

fn paint_stacking_context_children(
    node: &LayoutBox<'_>,
    out: &mut Vec<DisplayItem>,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) {
    let mut collector = StackingContextCollector::default();

    for child in &node.children {
        collect_into_stacking_context(child, &mut collector, font, base_url, fixed_context);
    }

    append_collected_layers(out, collector);
}

fn append_collected_layers(out: &mut Vec<DisplayItem>, mut collector: StackingContextCollector) {
    collector
        .child_contexts
        .sort_by(|a, b| a.level.cmp(&b.level).then_with(|| a.order.cmp(&b.order)));

    extend_matching_contexts(out, &collector.child_contexts, |level| level < 0);
    out.extend(collector.normal_items);
    extend_matching_contexts(out, &collector.child_contexts, |level| level == 0);
    extend_matching_contexts(out, &collector.child_contexts, |level| level > 0);
}

fn collect_into_stacking_context(
    node: &LayoutBox<'_>,
    collector: &mut StackingContextCollector,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) {
    if skips_display_subtree(node) {
        return;
    }

    let fixed = node_fixed_context(node, fixed_context);

    if creates_stacking_context(node) {
        let order = collector.take_order();
        let mut items = Vec::new();
        paint_stacking_context(node, &mut items, font, base_url, fixed_context);

        collector.child_contexts.push(StackingContextPaint {
            level: stacking_context_level(node),
            order,
            items,
        });
        return;
    }

    if paints_in_positioned_layer(node) {
        let order = collector.take_order();
        let items = collect_positioned_layer_items(node, collector, font, base_url, fixed_context);

        collector.child_contexts.push(StackingContextPaint {
            level: 0,
            order,
            items,
        });
        return;
    }

    paint_node_contents(node, &mut collector.normal_items, base_url, fixed);

    for child in &node.children {
        collect_into_stacking_context(child, collector, font, base_url, fixed);
    }
}

fn collect_positioned_layer_items(
    node: &LayoutBox<'_>,
    outer_collector: &mut StackingContextCollector,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) -> Vec<DisplayItem> {
    let fixed = node_fixed_context(node, fixed_context);
    let mut items = Vec::new();
    paint_node_contents(node, &mut items, base_url, fixed);

    let mut local_collector = StackingContextCollector::default();
    for child in &node.children {
        collect_into_positioned_layer(
            child,
            &mut local_collector,
            outer_collector,
            font,
            base_url,
            fixed,
        );
    }

    append_collected_layers(&mut items, local_collector);
    items
}

fn collect_into_positioned_layer(
    node: &LayoutBox<'_>,
    local_collector: &mut StackingContextCollector,
    outer_collector: &mut StackingContextCollector,
    font: &Font,
    base_url: &crate::url::URL,
    fixed_context: bool,
) {
    if skips_display_subtree(node) {
        return;
    }

    let fixed = node_fixed_context(node, fixed_context);

    if creates_stacking_context(node) {
        let order = outer_collector.take_order();
        let mut items = Vec::new();
        paint_stacking_context(node, &mut items, font, base_url, fixed_context);

        outer_collector.child_contexts.push(StackingContextPaint {
            level: stacking_context_level(node),
            order,
            items,
        });
        return;
    }

    if paints_in_positioned_layer(node) {
        let order = local_collector.take_order();
        let items =
            collect_positioned_layer_items(node, outer_collector, font, base_url, fixed_context);

        local_collector.child_contexts.push(StackingContextPaint {
            level: 0,
            order,
            items,
        });
        return;
    }

    paint_node_contents(node, &mut local_collector.normal_items, base_url, fixed);

    for child in &node.children {
        collect_into_positioned_layer(
            child,
            local_collector,
            outer_collector,
            font,
            base_url,
            fixed,
        );
    }
}

fn extend_matching_contexts(
    out: &mut Vec<DisplayItem>,
    contexts: &[StackingContextPaint],
    matches_level: impl Fn(i32) -> bool,
) {
    for context in contexts {
        if matches_level(context.level) {
            out.extend(context.items.iter().cloned());
        }
    }
}

fn skips_display_subtree(node: &LayoutBox<'_>) -> bool {
    node.get_style_node()
        .and_then(|sn| match &sn.node.node_type {
            crate::dom::NodeType::Element(ed) => Some(ed.tag_name.as_str()),
            _ => None,
        })
        .is_some_and(|tag| matches!(tag, "style" | "script" | "head" | "title" | "meta" | "link"))
}

fn node_fixed_context(node: &LayoutBox<'_>, fixed_context: bool) -> bool {
    fixed_context
        || node
            .get_style_node()
            .map(|sn| sn.position() == Position::Fixed)
            .unwrap_or(false)
}

fn is_inline_block_box(node: &LayoutBox<'_>) -> bool {
    node.get_style_node()
        .map(|sn| sn.display() == Display::InlineBlock)
        .unwrap_or(false)
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

fn creates_stacking_context(node: &LayoutBox<'_>) -> bool {
    node.get_style_node()
        .map(|sn| {
            let position = sn.position();
            matches!(position, Position::Fixed | Position::Sticky)
                || (position.is_positioned() && sn.z_index().stack_level().is_some())
        })
        .unwrap_or(false)
}

fn paints_in_positioned_layer(node: &LayoutBox<'_>) -> bool {
    node.get_style_node()
        .map(|sn| sn.position().is_positioned())
        .unwrap_or(false)
}

fn stacking_context_level(node: &LayoutBox<'_>) -> i32 {
    node.get_style_node()
        .and_then(|sn| sn.z_index().stack_level())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyImageCache;

    impl crate::layout::ImageSizeProvider for EmptyImageCache {
        fn normalize_src_key(&self, _src: &str) -> Option<String> {
            None
        }

        fn natural_size_px(&self, _key: &str) -> Option<(u32, u32)> {
            None
        }
    }

    fn test_font() -> Font {
        fontdue::Font::from_bytes(
            include_bytes!("../assets/DejaVuSans.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap()
    }

    fn display_list_for(input: &str, css: &str) -> Vec<DisplayItem> {
        let dom = crate::html::parse(input.to_string());
        let stylesheet = crate::css::Parser::new(css.to_string()).parse_stylesheet();
        let styled = crate::style::style_tree(dom, &stylesheet);
        let mut layout = crate::layout::build_layout_tree(&styled);
        let mut viewport = crate::layout::Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 300.0;
        let font = test_font();

        layout.layout_with_font(viewport, &font, &EmptyImageCache);

        let mut items = Vec::new();
        build_display_list(
            &layout,
            &mut items,
            &font,
            &crate::url::URL::new("http://example.com/"),
        );
        items
    }

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

    fn rect_index(items: &[DisplayItem], color: [f32; 4]) -> usize {
        items
            .iter()
            .position(|item| matches!(item, DisplayItem::Rect(rect) if rect.color == color))
            .expect("colored rect should be painted")
    }

    fn rect_count(items: &[DisplayItem], color: [f32; 4]) -> usize {
        items
            .iter()
            .filter(|item| matches!(item, DisplayItem::Rect(rect) if rect.color == color))
            .count()
    }

    fn border_count(items: &[DisplayItem], color: [f32; 4]) -> usize {
        items
            .iter()
            .filter(|item| matches!(item, DisplayItem::Border(border) if border.color == color))
            .count()
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

    #[test]
    fn wrapped_inline_element_paints_background_and_border_per_fragment() {
        let items = display_list_for(
            r#"<p><span id="target">alpha beta gamma delta epsilon</span></p>"#,
            r#"
            p { display: block; width: 90px; margin: 0; padding: 0; }
            #target {
                background: #ffff00;
                padding: 4px;
                border: 2px solid red;
            }
            "#,
        );

        let yellow = [1.0, 1.0, 0.0, 1.0];
        let background_count = rect_count(&items, yellow);
        let border_count = border_count(&items, color::RED);

        assert!(background_count >= 2);
        assert_eq!(background_count, border_count);
    }

    #[test]
    fn inline_block_paints_single_block_box() {
        let items = display_list_for(
            r#"<p>before <span id="target">badge</span> after</p>"#,
            r#"
            p { display: block; width: 320px; margin: 0; padding: 0; }
            #target {
                display: inline-block;
                padding: 4px;
                border: 2px solid red;
                background: #ffff00;
            }
            "#,
        );

        let yellow = [1.0, 1.0, 0.0, 1.0];

        assert_eq!(rect_count(&items, yellow), 1);
        assert_eq!(border_count(&items, color::RED), 1);
    }

    #[test]
    fn positioned_z_index_reorders_painting_for_overlapping_siblings() {
        let items = display_list_for(
            r#"
            <div id="container">
                <div id="front"></div>
                <div id="back"></div>
            </div>
            "#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #front {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: red;
                z-index: 10;
            }
            #back {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: blue;
                z-index: 1;
            }
            "#,
        );

        let red_index = rect_index(&items, color::RED);
        let blue_index = rect_index(&items, color::BLUE);

        assert!(blue_index < red_index);
    }

    #[test]
    fn auto_z_index_keeps_tree_order_for_positioned_siblings() {
        let items = display_list_for(
            r#"
            <div id="container">
                <div id="front"></div>
                <div id="back"></div>
            </div>
            "#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #front {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: red;
            }
            #back {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: blue;
            }
            "#,
        );

        let red_index = rect_index(&items, color::RED);
        let blue_index = rect_index(&items, color::BLUE);

        assert!(red_index < blue_index);
    }

    #[test]
    fn positioned_auto_paints_after_in_flow_sibling() {
        let items = display_list_for(
            r#"
            <div id="container">
                <div id="abs"></div>
                <div id="normal"></div>
            </div>
            "#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #abs {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: red;
            }
            #normal {
                display: block;
                width: 100px;
                height: 100px;
                margin: 0;
                padding: 0;
                background: blue;
            }
            "#,
        );

        let red_index = rect_index(&items, color::RED);
        let blue_index = rect_index(&items, color::BLUE);

        assert!(blue_index < red_index);
    }

    #[test]
    fn child_context_inside_non_context_parent_participates_in_ancestor_context() {
        let items = display_list_for(
            r#"
            <div id="container">
                <div id="parent">
                    <div id="inner"></div>
                </div>
                <div id="sibling"></div>
            </div>
            "#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #parent {
                display: block;
                position: relative;
                width: 100px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #inner {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: red;
                z-index: 10;
            }
            #sibling {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: blue;
                z-index: 1;
            }
            "#,
        );

        let red_index = rect_index(&items, color::RED);
        let blue_index = rect_index(&items, color::BLUE);

        assert!(blue_index < red_index);
    }

    #[test]
    fn stacking_context_paints_atomically_against_sibling_context() {
        let items = display_list_for(
            r#"
            <div id="container">
                <div id="parent">
                    <div id="inner"></div>
                </div>
                <div id="sibling"></div>
            </div>
            "#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #parent {
                display: block;
                position: relative;
                z-index: 1;
                width: 100px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #inner {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: red;
                z-index: 10;
            }
            #sibling {
                display: block;
                position: absolute;
                left: 0;
                top: 0;
                width: 100px;
                height: 100px;
                background: blue;
                z-index: 2;
            }
            "#,
        );

        let red_index = rect_index(&items, color::RED);
        let blue_index = rect_index(&items, color::BLUE);

        assert!(red_index < blue_index);
    }

    #[test]
    fn text_uses_computed_font_sizes_for_em_and_rem() {
        let items = display_list_for(
            r#"
            <p id="em-text">hello</p>
            <p id="rem-text">world</p>
            "#,
            r#"
            html { font-size: 20px; }
            #em-text { font-size: 1.5em; }
            #rem-text { font-size: 0.5rem; }
            "#,
        );

        let hello_size = items.iter().find_map(|item| match item {
            DisplayItem::Text(text) if text.text == "hello" => Some(text.size_px),
            _ => None,
        });
        let world_size = items.iter().find_map(|item| match item {
            DisplayItem::Text(text) if text.text == "world" => Some(text.size_px),
            _ => None,
        });

        assert_eq!(hello_size, Some(30.0));
        assert_eq!(world_size, Some(10.0));
    }
}

fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    Some(sn.font_size_px())
}

fn line_height_px(sn: &crate::style::StyledNode, _font_size: f32) -> f32 {
    sn.line_height_px()
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

fn paint_inline_element_fragments(node: &LayoutBox<'_>, out: &mut Vec<DisplayItem>, fixed: bool) {
    let Some(sn) = node.get_style_node() else {
        return;
    };

    if !matches!(sn.node.node_type, crate::dom::NodeType::Element(_)) {
        return;
    }

    let bg = sn.background_color();
    let border_width = node
        .dimensions
        .border
        .left
        .max(node.dimensions.border.right)
        .max(node.dimensions.border.top)
        .max(node.dimensions.border.bottom);

    if bg.is_none() && border_width <= 0.0 {
        return;
    }

    let border_color = if border_width > 0.0 {
        Some(sn.border_color().unwrap_or(color::BLACK))
    } else {
        None
    };

    for fragment in &node.paint_fragments {
        let border_box = inline_fragment_border_box(&fragment.rect, &node.dimensions);
        if border_box.width <= 0.0 || border_box.height <= 0.0 {
            continue;
        }

        let radius = node
            .dimensions
            .border_radius
            .normalize(border_box.width, border_box.height);

        if let Some(bg) = bg {
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

        if let Some(border_color) = border_color {
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

fn inline_fragment_border_box(
    fragment: &crate::layout::Rect,
    dimensions: &crate::layout::Dimensions,
) -> crate::layout::Rect {
    crate::layout::Rect {
        x: fragment.x - dimensions.padding.left - dimensions.border.left,
        y: fragment.y - dimensions.padding.top - dimensions.border.top,
        width: fragment.width
            + dimensions.padding.left
            + dimensions.padding.right
            + dimensions.border.left
            + dimensions.border.right,
        height: fragment.height
            + dimensions.padding.top
            + dimensions.padding.bottom
            + dimensions.border.top
            + dimensions.border.bottom,
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
