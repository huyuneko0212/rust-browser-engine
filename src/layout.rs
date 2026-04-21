use crate::constants::layout as layout_constants;
use crate::style::{Clear, Display, Float, Position, StyledNode};
use fontdue::Font;
use std::cmp::Ordering;

pub trait ImageSizeProvider {
    /// layout が持っている src（相対/絶対/ポート付きなど）を
    /// “キャッシュキーと同じ正規化済み絶対URL文字列” に変換する
    fn normalize_src_key(&self, src: &str) -> Option<String>;

    /// key(正規化済み絶対URL文字列) から自然サイズ(px)を返す
    fn natural_size_px(&self, key: &str) -> Option<(u32, u32)>;
}

#[derive(Debug, Default, Clone, Copy)]
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

#[derive(Debug, Default, Clone, Copy)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub fn normalize(self, width: f32, height: f32) -> Self {
        let mut out = Self {
            top_left: self.top_left.max(0.0),
            top_right: self.top_right.max(0.0),
            bottom_right: self.bottom_right.max(0.0),
            bottom_left: self.bottom_left.max(0.0),
        };

        if width <= 0.0 || height <= 0.0 {
            return Self::default();
        }

        let scale = 1.0_f32
            .min(scale_for_side(width, out.top_left + out.top_right))
            .min(scale_for_side(width, out.bottom_left + out.bottom_right))
            .min(scale_for_side(height, out.top_left + out.bottom_left))
            .min(scale_for_side(height, out.top_right + out.bottom_right));

        if scale < 1.0 {
            out.top_left *= scale;
            out.top_right *= scale;
            out.bottom_right *= scale;
            out.bottom_left *= scale;
        }

        out
    }

    pub fn inset_uniform(self, inset: f32) -> Self {
        Self {
            top_left: (self.top_left - inset).max(0.0),
            top_right: (self.top_right - inset).max(0.0),
            bottom_right: (self.bottom_right - inset).max(0.0),
            bottom_left: (self.bottom_left - inset).max(0.0),
        }
    }

    pub fn as_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

#[derive(Debug, Default, Clone)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
    pub border_radius: CornerRadii,
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

    pub fn margin_box_width(&self) -> f32 {
        self.margin.left
            + self.border.left
            + self.padding.left
            + self.content.width
            + self.padding.right
            + self.border.right
            + self.margin.right
    }

    fn margin_box_rect(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left - self.border.left - self.margin.left,
            y: self.content.y - self.padding.top - self.border.top - self.margin.top,
            width: self.margin_box_width(),
            height: self.margin_box_height(),
        }
    }

    fn padding_box_as_containing_block(&self) -> Dimensions {
        let mut containing = Dimensions::default();
        containing.content.x = self.content.x - self.padding.left;
        containing.content.y = self.content.y - self.padding.top;
        containing.content.width = self.content.width + self.padding.left + self.padding.right;
        containing.content.height = self.content.height + self.padding.top + self.padding.bottom;
        containing
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BoxType<'a> {
    BlockNode(&'a StyledNode),
    InlineNode(&'a StyledNode),
    Anonymous, // anonymous block box (for inline formatting context)
}

#[derive(Debug, Clone)]
pub struct TextFragment {
    pub rect: Rect,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PaintFragment {
    pub rect: Rect,
}

#[derive(Debug, Default, Clone, Copy)]
struct Insets {
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
struct PlacedFloat {
    side: Float,
    rect: Rect,
}

#[derive(Debug, Default, Clone)]
struct FloatContext {
    floats: Vec<PlacedFloat>,
}

impl FloatContext {
    fn add(&mut self, side: Float, rect: Rect) {
        self.floats.push(PlacedFloat { side, rect });
    }

    fn available_at(&self, container_x: f32, container_w: f32, y: f32) -> (f32, f32) {
        let mut left = container_x;
        let mut right = container_x + container_w;

        for placed in &self.floats {
            if !float_overlaps_y(placed.rect, y) {
                continue;
            }

            match placed.side {
                Float::Left => left = left.max(placed.rect.x + placed.rect.width),
                Float::Right => right = right.min(placed.rect.x),
                Float::None => {}
            }
        }

        (left, (right - left).max(0.0))
    }

    fn find_available(
        &self,
        container_x: f32,
        container_w: f32,
        start_y: f32,
        needed_w: f32,
    ) -> (f32, f32, f32) {
        let mut y = start_y;

        loop {
            let (x, width) = self.available_at(container_x, container_w, y);
            if width + 0.5 >= needed_w || !self.has_active_float_at(y) {
                return (y, x, width);
            }

            if let Some(next_y) = self.next_active_float_bottom(y) {
                if next_y <= y + 0.5 {
                    return (y, x, width);
                }
                y = next_y;
            } else {
                return (y, x, width);
            }
        }
    }

    fn clear_y(&self, y: f32, clear: Clear) -> f32 {
        if matches!(clear, Clear::None) {
            return y;
        }

        self.floats.iter().fold(y, |next_y, placed| {
            if !clear_applies_to_float(clear, placed.side) {
                return next_y;
            }

            let bottom = placed.rect.y + placed.rect.height;
            if bottom > next_y { bottom } else { next_y }
        })
    }

    fn max_bottom(&self) -> Option<f32> {
        self.floats
            .iter()
            .map(|placed| placed.rect.y + placed.rect.height)
            .reduce(f32::max)
    }

    fn has_active_float_at(&self, y: f32) -> bool {
        self.floats
            .iter()
            .any(|placed| float_overlaps_y(placed.rect, y))
    }

    fn next_active_float_bottom(&self, y: f32) -> Option<f32> {
        self.floats
            .iter()
            .filter(|placed| float_overlaps_y(placed.rect, y))
            .map(|placed| placed.rect.y + placed.rect.height)
            .filter(|bottom| *bottom > y + 0.5)
            .reduce(f32::min)
    }
}

fn float_overlaps_y(rect: Rect, y: f32) -> bool {
    y >= rect.y && y < rect.y + rect.height
}

fn clear_applies_to_float(clear: Clear, side: Float) -> bool {
    matches!(
        (clear, side),
        (Clear::Both, Float::Left)
            | (Clear::Both, Float::Right)
            | (Clear::Left, Float::Left)
            | (Clear::Right, Float::Right)
    )
}

#[derive(Debug)]
pub struct LayoutBox<'a> {
    pub box_type: BoxType<'a>,
    pub dimensions: Dimensions,
    pub children: Vec<LayoutBox<'a>>,
    pub text_fragments: Vec<TextFragment>,
    pub paint_fragments: Vec<PaintFragment>,
}

impl<'a> LayoutBox<'a> {
    pub fn new(box_type: BoxType<'a>) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            children: vec![],
            text_fragments: vec![],
            paint_fragments: vec![],
        }
    }

    pub fn get_style_node(&self) -> Option<&StyledNode> {
        match self.box_type {
            BoxType::BlockNode(node) | BoxType::InlineNode(node) => Some(node),
            BoxType::Anonymous => None,
        }
    }

    fn node_position(&self) -> Position {
        self.get_style_node()
            .map(|node| node.position())
            .unwrap_or(Position::Static)
    }

    fn node_float(&self) -> Float {
        if self.node_position().is_out_of_flow() {
            return Float::None;
        }

        self.get_style_node()
            .map(|node| node.float())
            .unwrap_or(Float::None)
    }

    fn node_clear(&self) -> Clear {
        self.get_style_node()
            .map(|node| node.clear())
            .unwrap_or(Clear::None)
    }

    pub fn layout_with_font(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let viewport = containing_block.clone();
        self.layout_with_context(
            containing_block.clone(),
            containing_block,
            viewport,
            font,
            img_cache,
        );
    }

    fn layout_with_context(
        &mut self,
        containing_block: Dimensions,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.text_fragments.clear();
        self.paint_fragments.clear();

        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block_with_context(
                containing_block,
                positioned_containing_block,
                viewport,
                font,
                img_cache,
            ),
            BoxType::InlineNode(_) => {
                self.layout_inline_leaf_fallback(containing_block.clone(), font, img_cache);
                self.apply_relative_position_if_needed(&containing_block);
            }
            BoxType::Anonymous => self.layout_anonymous_block_with_context(
                containing_block,
                positioned_containing_block,
                viewport,
                font,
                img_cache,
            ),
        }
    }

    fn layout_block_with_context(
        &mut self,
        containing_block: Dimensions,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let position = self.node_position();
        let float = self.node_float();
        let out_of_flow = position.is_out_of_flow();
        let positioning_block = if position == Position::Fixed {
            viewport.clone()
        } else {
            positioned_containing_block.clone()
        };
        let model_block = if out_of_flow {
            positioning_block.clone()
        } else {
            containing_block.clone()
        };

        self.calculate_block_model(model_block.clone());
        if out_of_flow {
            self.calculate_positioned_block_width(positioning_block.clone());
        } else if float.is_floating() {
            self.calculate_float_block_width(containing_block.clone(), img_cache);
        } else {
            self.calculate_block_width(containing_block.clone());
        }
        self.calculate_block_position(containing_block.clone());

        let child_positioned_containing_block = if position.is_positioned() {
            self.positioned_descendant_containing_block()
        } else {
            positioned_containing_block
        };

        self.layout_block_children_with_context(
            child_positioned_containing_block,
            viewport,
            font,
            img_cache,
        );

        self.calculate_block_height_with_font(font, img_cache);

        if out_of_flow {
            self.apply_out_of_flow_position(&positioning_block);
        } else {
            self.apply_relative_position_if_needed(&containing_block);
        }
    }

    fn layout_anonymous_block_with_context(
        &mut self,
        containing_block: Dimensions,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.dimensions.content.x = containing_block.content.x;
        self.dimensions.content.y = containing_block.content.y;
        self.dimensions.content.width = containing_block.content.width;
        self.dimensions.content.height = 0.0;

        self.layout_inline_formatting_context(
            positioned_containing_block,
            viewport,
            font,
            img_cache,
        );
    }

    fn layout_inline_leaf_fallback(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.calculate_block_model(containing_block.clone());

        let (font_size, line_h, text_opt, img_opt) = if let Some(sn) = self.get_style_node() {
            let fs = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
            let lh = line_height_px(sn, fs);

            match &sn.node.node_type {
                crate::dom::NodeType::Text(t) => (fs, lh, Some(t.clone()), None),
                crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                    let (w, h) = img_intrinsic_size_px(sn, img_cache);
                    (fs, lh, None, Some((w, h)))
                }
                _ => (fs, lh, None, None),
            }
        } else {
            (
                layout_constants::DEFAULT_FONT_SIZE_PX,
                layout_constants::DEFAULT_FONT_SIZE_PX
                    * layout_constants::DEFAULT_LINE_HEIGHT_MULTIPLIER,
                None,
                None,
            )
        };

        if let Some((iw, ih)) = img_opt {
            let d = &mut self.dimensions;
            d.content.x = containing_block.content.x;
            d.content.y = containing_block.content.y;
            d.content.width = iw.max(layout_constants::MIN_LAYOUT_SIZE_PX).min(
                containing_block
                    .content
                    .width
                    .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            );
            d.content.height = ih.max(layout_constants::MIN_LAYOUT_SIZE_PX);
            sync_paint_fragments_with_content_rect(self);
            return;
        }

        self.dimensions.content.x = containing_block.content.x;
        self.dimensions.content.y = containing_block.content.y;
        self.dimensions.content.width = containing_block
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        self.dimensions.content.height = line_h;

        if let Some(txt) = text_opt {
            let start_x = containing_block.content.x;
            let mut cursor_x = start_x;
            let mut cursor_y = containing_block.content.y;
            let mut current_line_h = 0.0;
            let mut pending_space_w = 0.0;
            let mut pending_space_h = 0.0;

            layout_text_fragments(
                self,
                font,
                &txt,
                font_size,
                line_h,
                start_x,
                containing_block
                    .content
                    .width
                    .max(layout_constants::MIN_LAYOUT_SIZE_PX),
                &mut cursor_x,
                &mut cursor_y,
                &mut current_line_h,
                &mut pending_space_w,
                &mut pending_space_h,
            );
        }
    }

    /// margin/padding/border/border-radius を style から読む
    fn calculate_block_model(&mut self, containing: Dimensions) {
        let viewport_w = containing.content.width;
        let viewport_h = containing
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = containing.content.width;

        let (
            ml_s,
            mr_s,
            mt_s,
            mb_s,
            pl_s,
            pr_s,
            pt_s,
            pb_s,
            margin_sh,
            padding_sh,
            // border-width 系
            blw_s,
            brw_s,
            btw_s,
            bbw_s,
            border_width_sh,
            // border-radius
            border_radius_sh,
            border_tl_radius_s,
            border_tr_radius_s,
            border_br_radius_s,
            border_bl_radius_s,
        ) = if let Some(style) = self.get_style_node() {
            (
                style.value("margin-left").cloned(),
                style.value("margin-right").cloned(),
                style.value("margin-top").cloned(),
                style.value("margin-bottom").cloned(),
                style.value("padding-left").cloned(),
                style.value("padding-right").cloned(),
                style.value("padding-top").cloned(),
                style.value("padding-bottom").cloned(),
                style.value("margin").cloned(),
                style.value("padding").cloned(),
                // border-width
                style.value("border-left-width").cloned(),
                style.value("border-right-width").cloned(),
                style.value("border-top-width").cloned(),
                style.value("border-bottom-width").cloned(),
                style.value("border-width").cloned(),
                // border-radius
                style.value("border-radius").cloned(),
                style.value("border-top-left-radius").cloned(),
                style.value("border-top-right-radius").cloned(),
                style.value("border-bottom-right-radius").cloned(),
                style.value("border-bottom-left-radius").cloned(),
            )
        } else {
            (
                None, None, None, None, // margin *
                None, None, None, None, // padding *
                None, None, // margin, padding shorthand
                None, None, None, None, // border-*-width
                None, // border-width
                None, // border-radius
                None, None, None, None, // border-*-radius
            )
        };

        let mut ml = 0.0;
        let mut mr = 0.0;
        let mut mt = 0.0;
        let mut mb = 0.0;
        let mut pl = 0.0;
        let mut pr = 0.0;
        let mut pt = 0.0;
        let mut pb = 0.0;

        // border-width 初期値
        let mut bl = 0.0;
        let mut br = 0.0;
        let mut bt = 0.0;
        let mut bb = 0.0;

        // --- margin 個別 ---
        if let Some(v) = ml_s.as_deref() {
            if v.trim() != "auto" {
                ml = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mr_s.as_deref() {
            if v.trim() != "auto" {
                mr = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mt_s.as_deref() {
            mt = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = mb_s.as_deref() {
            mb = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        // --- padding 個別 ---
        if let Some(v) = pl_s.as_deref() {
            pl = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pr_s.as_deref() {
            pr = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pt_s.as_deref() {
            pt = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pb_s.as_deref() {
            pb = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        // --- border-width 個別 ---
        if let Some(v) = blw_s.as_deref() {
            bl = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = brw_s.as_deref() {
            br = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = btw_s.as_deref() {
            bt = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = bbw_s.as_deref() {
            bb = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        // shorthand margin（個別指定が無い場合のみ）
        if ml_s.is_none() && mr_s.is_none() && mt_s.is_none() && mb_s.is_none() {
            if let Some(sh) = margin_sh.as_deref() {
                let m = parse_4len(sh);
                if let Some(top) = m.0.as_deref() {
                    mt = parse_length(top, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(right) = m.1.as_deref() {
                    if right.trim() != "auto" {
                        mr = parse_length(right, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                    }
                }
                if let Some(bottom) = m.2.as_deref() {
                    mb = parse_length(bottom, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(left) = m.3.as_deref() {
                    if left.trim() != "auto" {
                        ml = parse_length(left, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                    }
                }
            }
        }

        // shorthand padding（個別指定が無い場合のみ）
        if pl_s.is_none() && pr_s.is_none() && pt_s.is_none() && pb_s.is_none() {
            if let Some(sh) = padding_sh.as_deref() {
                let p = parse_4len(sh);
                if let Some(top) = p.0.as_deref() {
                    pt = parse_length(top, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(right) = p.1.as_deref() {
                    pr = parse_length(right, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(bottom) = p.2.as_deref() {
                    pb = parse_length(bottom, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(left) = p.3.as_deref() {
                    pl = parse_length(left, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
            }
        }

        // shorthand border-width（個別指定が無い場合のみ）
        if blw_s.is_none() && brw_s.is_none() && btw_s.is_none() && bbw_s.is_none() {
            if let Some(sh) = border_width_sh.as_deref() {
                let b = parse_4len(sh);
                if let Some(top) = b.0.as_deref() {
                    bt = parse_length(top, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(right) = b.1.as_deref() {
                    br = parse_length(right, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(bottom) = b.2.as_deref() {
                    bb = parse_length(bottom, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(left) = b.3.as_deref() {
                    bl = parse_length(left, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
            }
        }

        // border-radius
        let mut border_radius = CornerRadii::default();
        if let Some(v) = border_radius_sh.as_deref() {
            if let Some(r) = parse_border_radius_shorthand(v, parent_w, viewport_w, viewport_h) {
                border_radius = r;
            }
        }
        if let Some(v) = border_tl_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(v, parent_w, viewport_w, viewport_h) {
                border_radius.top_left = r;
            }
        }
        if let Some(v) = border_tr_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(v, parent_w, viewport_w, viewport_h) {
                border_radius.top_right = r;
            }
        }
        if let Some(v) = border_br_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(v, parent_w, viewport_w, viewport_h) {
                border_radius.bottom_right = r;
            }
        }
        if let Some(v) = border_bl_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(v, parent_w, viewport_w, viewport_h) {
                border_radius.bottom_left = r;
            }
        }

        // 値を Dimensions に反映
        self.dimensions.margin.left = ml;
        self.dimensions.margin.right = mr;
        self.dimensions.margin.top = mt;
        self.dimensions.margin.bottom = mb;

        self.dimensions.padding.left = pl;
        self.dimensions.padding.right = pr;
        self.dimensions.padding.top = pt;
        self.dimensions.padding.bottom = pb;

        self.dimensions.border.left = bl;
        self.dimensions.border.right = br;
        self.dimensions.border.top = bt;
        self.dimensions.border.bottom = bb;

        self.dimensions.border_radius = border_radius;
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let viewport_w = containing_block.content.width;
        let viewport_h = containing_block
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = containing_block.content.width;

        let width_str = self
            .get_style_node()
            .and_then(|s| s.value("width"))
            .cloned();

        // margin:auto 判定
        let (ml_auto, mr_auto) = self
            .get_style_node()
            .map(|s| {
                let mut la = s
                    .value("margin-left")
                    .map(|v| v.trim() == "auto")
                    .unwrap_or(false);
                let mut ra = s
                    .value("margin-right")
                    .map(|v| v.trim() == "auto")
                    .unwrap_or(false);

                if (!la || !ra)
                    && s.value("margin-left").is_none()
                    && s.value("margin-right").is_none()
                {
                    if let Some(m) = s.value("margin") {
                        let m4 = parse_4len(m);
                        if let Some(r) = m4.1.as_deref() {
                            if r.trim() == "auto" {
                                ra = true;
                            }
                        }
                        if let Some(l) = m4.3.as_deref() {
                            if l.trim() == "auto" {
                                la = true;
                            }
                        }
                    }
                }
                (la, ra)
            })
            .unwrap_or((false, false));

        let d = &mut self.dimensions;

        if let Some(ws) = width_str.as_deref() {
            if let Some(w) = parse_length(ws, parent_w, viewport_w, viewport_h) {
                d.content.width = w.max(0.0);
            }
        }

        if d.content.width == 0.0 {
            let available = containing_block.content.width
                - d.margin.left
                - d.margin.right
                - d.padding.left
                - d.padding.right
                - d.border.left
                - d.border.right;
            d.content.width = available.max(0.0);
        }

        if ml_auto || mr_auto {
            let used =
                d.content.width + d.padding.left + d.padding.right + d.border.left + d.border.right;

            let remaining = (containing_block.content.width - used).max(0.0);

            if ml_auto && mr_auto {
                d.margin.left = remaining / 2.0;
                d.margin.right = remaining / 2.0;
            } else if ml_auto {
                d.margin.left = remaining;
            } else if mr_auto {
                d.margin.right = remaining;
            }
        }
    }

    fn calculate_positioned_block_width(&mut self, positioning_block: Dimensions) {
        let viewport_w = positioning_block.content.width;
        let viewport_h = positioning_block
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = positioning_block.content.width;

        let width_str = self
            .get_style_node()
            .and_then(|s| s.value("width"))
            .cloned();
        let insets = self.specified_insets(parent_w, viewport_h, viewport_w, viewport_h);
        let d = &mut self.dimensions;

        if let Some(ws) = width_str.as_deref() {
            if let Some(w) = parse_length(ws, parent_w, viewport_w, viewport_h) {
                d.content.width = w.max(0.0);
                return;
            }
        }

        if let (Some(left), Some(right)) = (insets.left, insets.right) {
            let available = positioning_block.content.width
                - left
                - right
                - d.margin.left
                - d.margin.right
                - d.padding.left
                - d.padding.right
                - d.border.left
                - d.border.right;
            d.content.width = available.max(0.0);
            return;
        }

        let available = positioning_block.content.width
            - d.margin.left
            - d.margin.right
            - d.padding.left
            - d.padding.right
            - d.border.left
            - d.border.right;
        d.content.width = available.max(0.0);
    }

    fn calculate_float_block_width(
        &mut self,
        containing_block: Dimensions,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let width_str = self
            .get_style_node()
            .and_then(|s| s.value("width"))
            .cloned();

        self.calculate_block_width(containing_block.clone());

        if width_str.is_some() {
            return;
        }

        if let Some(sn) = self.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "img" {
                    let (iw, _) = img_intrinsic_size_px(sn, img_cache);
                    let d = &mut self.dimensions;
                    let available = containing_block.content.width
                        - d.margin.left
                        - d.margin.right
                        - d.padding.left
                        - d.padding.right
                        - d.border.left
                        - d.border.right;
                    d.content.width = iw
                        .max(layout_constants::MIN_LAYOUT_SIZE_PX)
                        .min(available.max(layout_constants::MIN_LAYOUT_SIZE_PX));
                }
            }
        }
    }

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let d = &mut self.dimensions;

        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;
        d.content.y = containing_block.content.y + d.margin.top + d.border.top + d.padding.top;
    }

    fn layout_block_children_with_context(
        &mut self,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let mut y = self.dimensions.content.y;
        let container_x = self.dimensions.content.x;
        let container_w = self.dimensions.content.width;
        let mut floats = FloatContext::default();

        for child in &mut self.children {
            let child_out_of_flow = child.node_position().is_out_of_flow();
            let child_float = child.node_float();
            let clear = child.node_clear();

            if !child_out_of_flow {
                y = floats.clear_y(y, clear);
            }

            let (flow_y, flow_x, flow_w) = floats.find_available(
                container_x,
                container_w,
                y,
                layout_constants::MIN_LAYOUT_SIZE_PX,
            );

            let mut cb = Dimensions::default();
            cb.content.x = if child_out_of_flow {
                container_x
            } else {
                flow_x
            };
            cb.content.y = if child_out_of_flow { y } else { flow_y };
            cb.content.width = if child_out_of_flow {
                container_w
            } else {
                flow_w.max(layout_constants::MIN_LAYOUT_SIZE_PX)
            };
            cb.content.height = self
                .dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);

            child.layout_with_context(
                cb.clone(),
                positioned_containing_block.clone(),
                viewport.clone(),
                font,
                img_cache,
            );

            if child_out_of_flow {
                continue;
            }

            if child_float.is_floating() {
                let margin_w = child
                    .dimensions
                    .margin_box_width()
                    .max(layout_constants::MIN_LAYOUT_SIZE_PX);
                let (float_y, available_x, available_w) =
                    floats.find_available(container_x, container_w, y, margin_w);
                let margin_x = match child_float {
                    Float::Left => available_x,
                    Float::Right => available_x + available_w - margin_w,
                    Float::None => available_x,
                };

                child.shift_to_margin_box(margin_x, float_y);
                floats.add(child_float, child.dimensions.margin_box_rect());
                continue;
            }

            let mut placed_y = flow_y;
            if child.dimensions.margin_box_width() > flow_w + 0.5 {
                let (retry_y, retry_x, retry_w) = floats.find_available(
                    container_x,
                    container_w,
                    flow_y,
                    child.dimensions.margin_box_width(),
                );

                if retry_y > flow_y + 0.5 || (retry_w - flow_w).abs() > 0.5 {
                    cb.content.x = retry_x;
                    cb.content.y = retry_y;
                    cb.content.width = retry_w.max(layout_constants::MIN_LAYOUT_SIZE_PX);

                    child.layout_with_context(
                        cb,
                        positioned_containing_block.clone(),
                        viewport.clone(),
                        font,
                        img_cache,
                    );

                    placed_y = retry_y;
                }
            }

            y = placed_y + child.dimensions.margin_box_height().max(0.0);
        }

        let flow_bottom = y;
        let float_bottom = floats.max_bottom().unwrap_or(self.dimensions.content.y);
        self.dimensions.content.height =
            (flow_bottom.max(float_bottom) - self.dimensions.content.y).max(0.0);
    }

    fn calculate_block_height_with_font(&mut self, font: &Font, img_cache: &dyn ImageSizeProvider) {
        let (h_str, viewport_w, viewport_h, parent_w) = {
            let vw = self
                .dimensions
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);
            (
                self.get_style_node()
                    .and_then(|s| s.value("height"))
                    .cloned(),
                vw,
                layout_constants::DEFAULT_VIEWPORT_HEIGHT_PX,
                vw,
            )
        };

        if let Some(hs) = h_str.as_deref() {
            if let Some(h) = parse_length(hs, parent_w, viewport_w, viewport_h) {
                self.dimensions.content.height = h.max(0.0);
                return;
            }
        }

        let has_any_child = !self.children.is_empty();
        if !has_any_child {
            if let Some(sn) = self.get_style_node() {
                if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                    if ed.tag_name == "img" {
                        let (_iw, ih) = img_intrinsic_size_px(sn, img_cache);
                        self.dimensions.content.height =
                            ih.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                        return;
                    }
                }

                let mut buf = String::new();
                collect_text_nodes(sn, &mut buf);

                let txt = buf.trim();
                if !txt.is_empty() {
                    let font_size =
                        font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
                    let line_h = line_height_px(sn, font_size);
                    let max_w = self
                        .dimensions
                        .content
                        .width
                        .max(layout_constants::MIN_LAYOUT_SIZE_PX);

                    let lines = count_lines_fontdue(font, txt, max_w, font_size).max(1);
                    let text_h = (lines as f32) * line_h;
                    self.dimensions.content.height = self.dimensions.content.height.max(text_h);
                }
            }
        }
    }

    /// ★IFC: anonymous block の中の inline subtree を “行に詰める”
    fn layout_inline_formatting_context(
        &mut self,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let start_x = self.dimensions.content.x;
        let start_y = self.dimensions.content.y;
        let max_w = self
            .dimensions
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);

        let mut cursor_x = start_x;
        let mut cursor_y = start_y;
        let mut current_line_h = 0.0f32;
        let mut pending_space_w = 0.0f32;
        let mut pending_space_h = 0.0f32;
        let mut inline_containing_block = Dimensions::default();
        inline_containing_block.content.x = start_x;
        inline_containing_block.content.y = start_y;
        inline_containing_block.content.width = max_w;
        inline_containing_block.content.height = viewport
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);

        fn walk_inline<'a>(
            node: &mut LayoutBox<'a>,
            font: &Font,
            img_cache: &dyn ImageSizeProvider,
            start_x: f32,
            max_w: f32,
            containing_block: &Dimensions,
            positioned_containing_block: &Dimensions,
            viewport: &Dimensions,
            cursor_x: &mut f32,
            cursor_y: &mut f32,
            current_line_h: &mut f32,
            pending_space_w: &mut f32,
            pending_space_h: &mut f32,
        ) {
            node.text_fragments.clear();
            node.paint_fragments.clear();

            match &mut node.box_type {
                BoxType::InlineNode(_) => {
                    node.calculate_block_model(containing_block.clone());

                    let (is_text, text, font_size, line_h, img_opt) = if let Some(sn) =
                        node.get_style_node()
                    {
                        let fs = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
                        let lh = line_height_px(sn, fs);

                        match &sn.node.node_type {
                            crate::dom::NodeType::Text(t) => {
                                let collapsed = collapse_whitespace(t);
                                (true, Some(collapsed), fs, lh, None)
                            }
                            crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                                let (w, h) = img_intrinsic_size_px(sn, img_cache);
                                (false, None, fs, lh, Some((w, h)))
                            }
                            _ => (false, None, fs, lh, None),
                        }
                    } else {
                        (
                            false,
                            None,
                            layout_constants::DEFAULT_FONT_SIZE_PX,
                            layout_constants::DEFAULT_FONT_SIZE_PX
                                * layout_constants::DEFAULT_LINE_HEIGHT_MULTIPLIER,
                            None,
                        )
                    };

                    if let Some((iw, ih)) = img_opt {
                        let iw = iw.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                        let ih = ih.max(layout_constants::MIN_LAYOUT_SIZE_PX);

                        consume_pending_space_before_item(
                            iw,
                            start_x,
                            max_w,
                            cursor_x,
                            cursor_y,
                            current_line_h,
                            pending_space_w,
                            pending_space_h,
                        );

                        if *cursor_x > start_x && *cursor_x + iw > start_x + max_w {
                            advance_to_next_line(cursor_x, cursor_y, current_line_h, start_x, ih);
                        }

                        node.dimensions.content.x = *cursor_x;
                        node.dimensions.content.y = *cursor_y;
                        node.dimensions.content.width = iw;
                        node.dimensions.content.height = ih;

                        *cursor_x += iw;
                        *current_line_h = (*current_line_h).max(ih);
                        sync_paint_fragments_with_content_rect(node);
                        node.apply_relative_position_if_needed(containing_block);
                        return;
                    }

                    if is_text {
                        if let Some(txt) = text {
                            layout_text_fragments(
                                node,
                                font,
                                &txt,
                                font_size,
                                line_h,
                                start_x,
                                max_w,
                                cursor_x,
                                cursor_y,
                                current_line_h,
                                pending_space_w,
                                pending_space_h,
                            );
                        } else {
                            node.dimensions.content.width = 0.0;
                            node.dimensions.content.height = 0.0;
                            node.dimensions.content.x = *cursor_x;
                            node.dimensions.content.y = *cursor_y;
                        }
                    } else {
                        for ch in &mut node.children {
                            walk_inline(
                                ch,
                                font,
                                img_cache,
                                start_x,
                                max_w,
                                containing_block,
                                positioned_containing_block,
                                viewport,
                                cursor_x,
                                cursor_y,
                                current_line_h,
                                pending_space_w,
                                pending_space_h,
                            );
                        }
                        sync_inline_paint_fragments_from_children(node);
                        set_paint_content_rect(node, *cursor_x, *cursor_y);
                    }

                    node.apply_relative_position_if_needed(containing_block);
                }
                BoxType::BlockNode(_) | BoxType::Anonymous => {
                    let out_of_flow = node.node_position().is_out_of_flow();

                    if !out_of_flow {
                        *pending_space_w = 0.0;
                        *pending_space_h = 0.0;

                        if *cursor_x > start_x {
                            advance_to_next_line(
                                cursor_x,
                                cursor_y,
                                current_line_h,
                                start_x,
                                layout_constants::MIN_LINE_HEIGHT_PX,
                            );
                        }
                    }
                    let mut cb = Dimensions::default();
                    cb.content.x = start_x;
                    cb.content.y = *cursor_y;
                    cb.content.width = max_w;
                    cb.content.height = layout_constants::MIN_LAYOUT_SIZE_PX;

                    node.layout_with_context(
                        cb,
                        positioned_containing_block.clone(),
                        viewport.clone(),
                        font,
                        img_cache,
                    );
                    if !out_of_flow {
                        *cursor_y += node.dimensions.margin_box_height().max(0.0);
                    }
                }
            }
        }

        for child in &mut self.children {
            walk_inline(
                child,
                font,
                img_cache,
                start_x,
                max_w,
                &inline_containing_block,
                &positioned_containing_block,
                &viewport,
                &mut cursor_x,
                &mut cursor_y,
                &mut current_line_h,
                &mut pending_space_w,
                &mut pending_space_h,
            );
        }

        let total_h = (cursor_y - start_y) + current_line_h.max(0.0);
        self.dimensions.content.height = total_h.max(0.0);
    }

    fn apply_relative_position_if_needed(&mut self, containing_block: &Dimensions) {
        if !self.node_position().behaves_like_relative() {
            return;
        }

        let (dx, dy) = self.relative_position_offset(containing_block);
        self.shift_tree(dx, dy);
    }

    fn apply_out_of_flow_position(&mut self, positioning_block: &Dimensions) {
        let insets = self.specified_insets(
            positioning_block.content.width,
            positioning_block.content.height,
            positioning_block.content.width,
            positioning_block.content.height,
        );

        let mut target_x = self.dimensions.content.x;
        let mut target_y = self.dimensions.content.y;
        let d = &self.dimensions;

        if let Some(left) = insets.left {
            target_x =
                positioning_block.content.x + left + d.margin.left + d.border.left + d.padding.left;
        } else if let Some(right) = insets.right {
            target_x = positioning_block.content.x + positioning_block.content.width
                - right
                - d.margin.right
                - d.border.right
                - d.padding.right
                - d.content.width;
        }

        if let Some(top) = insets.top {
            target_y =
                positioning_block.content.y + top + d.margin.top + d.border.top + d.padding.top;
        } else if let Some(bottom) = insets.bottom {
            target_y = positioning_block.content.y + positioning_block.content.height
                - bottom
                - d.margin.bottom
                - d.border.bottom
                - d.padding.bottom
                - d.content.height;
        }

        self.shift_tree(
            target_x - self.dimensions.content.x,
            target_y - self.dimensions.content.y,
        );
    }

    fn relative_position_offset(&self, containing_block: &Dimensions) -> (f32, f32) {
        let insets = self.specified_insets(
            containing_block.content.width,
            containing_block.content.height,
            containing_block.content.width,
            containing_block.content.height,
        );

        let dx = insets
            .left
            .or_else(|| insets.right.map(|v| -v))
            .unwrap_or(0.0);
        let dy = insets
            .top
            .or_else(|| insets.bottom.map(|v| -v))
            .unwrap_or(0.0);

        (dx, dy)
    }

    fn specified_insets(
        &self,
        horizontal_basis: f32,
        vertical_basis: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Insets {
        let Some(sn) = self.get_style_node() else {
            return Insets::default();
        };

        Insets {
            top: specified_inset(sn, InsetEdge::Top).and_then(|value| {
                parse_inset_length(&value, vertical_basis, viewport_w, viewport_h)
            }),
            right: specified_inset(sn, InsetEdge::Right).and_then(|value| {
                parse_inset_length(&value, horizontal_basis, viewport_w, viewport_h)
            }),
            bottom: specified_inset(sn, InsetEdge::Bottom).and_then(|value| {
                parse_inset_length(&value, vertical_basis, viewport_w, viewport_h)
            }),
            left: specified_inset(sn, InsetEdge::Left).and_then(|value| {
                parse_inset_length(&value, horizontal_basis, viewport_w, viewport_h)
            }),
        }
    }

    fn shift_tree(&mut self, dx: f32, dy: f32) {
        if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
            return;
        }

        self.dimensions.content.x += dx;
        self.dimensions.content.y += dy;

        for frag in &mut self.text_fragments {
            frag.rect.x += dx;
            frag.rect.y += dy;
        }

        for frag in &mut self.paint_fragments {
            frag.rect.x += dx;
            frag.rect.y += dy;
        }

        for child in &mut self.children {
            child.shift_tree(dx, dy);
        }
    }

    fn shift_to_margin_box(&mut self, x: f32, y: f32) {
        let current = self.dimensions.margin_box_rect();
        self.shift_tree(x - current.x, y - current.y);
    }

    fn positioned_descendant_containing_block(&self) -> Dimensions {
        let mut containing = self.dimensions.padding_box_as_containing_block();

        let viewport_w = self
            .dimensions
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let viewport_h = layout_constants::DEFAULT_VIEWPORT_HEIGHT_PX;

        if let Some(height) = self
            .get_style_node()
            .and_then(|s| s.value("height"))
            .and_then(|value| parse_length(value, viewport_w, viewport_w, viewport_h))
        {
            containing.content.height =
                height.max(0.0) + self.dimensions.padding.top + self.dimensions.padding.bottom;
        }

        containing
    }
}

pub fn build_layout_tree(style_node: &StyledNode) -> LayoutBox<'_> {
    // browser.engineering 的に
    // - block の子: block はそのまま
    // - inline の連続: Anonymous block box にまとめて、その中に inline を入れる
    // - inline の子: Chrome と同じように同じ IFC の中へ直列に流す
    let display = style_node.display();
    let layout_display = layout_display_for(style_node);

    let mut root = LayoutBox::new(match layout_display {
        Display::Block => BoxType::BlockNode(style_node),
        Display::Inline => BoxType::InlineNode(style_node),
        Display::None => BoxType::Anonymous,
    });

    // Display::None は上で Anonymous に落ちてるので、ここでは children を作らない（最小）
    if display == Display::None {
        return root;
    }

    if layout_display == Display::Inline {
        for child in &style_node.children {
            if child.display() != Display::None {
                root.children.push(build_layout_tree(child));
            }
        }
        return root;
    }

    // 子をグルーピング
    let mut anon: Option<LayoutBox<'_>> = None;

    for child in &style_node.children {
        match layout_display_for(child) {
            Display::None => {}
            Display::Block => {
                // 先に溜まってる inline を flush
                if let Some(a) = anon.take() {
                    root.children.push(a);
                }
                root.children.push(build_layout_tree(child));
            }
            Display::Inline => {
                // inline は anonymous block にまとめる
                if anon.is_none() {
                    anon = Some(LayoutBox::new(BoxType::Anonymous));
                }
                if let Some(a) = anon.as_mut() {
                    a.children.push(build_layout_tree(child));
                }
            }
        }
    }

    if let Some(a) = anon.take() {
        root.children.push(a);
    }

    root
}

// ---------------- helpers ----------------

fn layout_display_for(style_node: &StyledNode) -> Display {
    let display = style_node.display();
    if display != Display::None
        && (style_node.position().is_out_of_flow() || style_node.float().is_floating())
    {
        Display::Block
    } else {
        display
    }
}

#[derive(Debug, Clone, Copy)]
enum InsetEdge {
    Top,
    Right,
    Bottom,
    Left,
}

fn specified_inset(sn: &StyledNode, edge: InsetEdge) -> Option<String> {
    let longhand = match edge {
        InsetEdge::Top => "top",
        InsetEdge::Right => "right",
        InsetEdge::Bottom => "bottom",
        InsetEdge::Left => "left",
    };

    if let Some(value) = sn.value(longhand) {
        return Some(value.clone());
    }

    let shorthand = sn.value("inset")?;
    let values = parse_4len(shorthand);
    match edge {
        InsetEdge::Top => values.0,
        InsetEdge::Right => values.1,
        InsetEdge::Bottom => values.2,
        InsetEdge::Left => values.3,
    }
}

fn parse_inset_length(
    value: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    let t = value.trim();
    if t.eq_ignore_ascii_case("auto") {
        return None;
    }

    parse_length(t, containing, viewport_w, viewport_h)
}

fn parse_length(s: &str, containing: f32, viewport_w: f32, viewport_h: f32) -> Option<f32> {
    let t = s.trim();

    if t == "0" || t == "+0" || t == "-0" {
        return Some(0.0);
    }
    if t.ends_with("px") {
        return t.trim_end_matches("px").trim().parse::<f32>().ok();
    }
    if t.ends_with("vw") {
        let v: f32 = t.trim_end_matches("vw").trim().parse().ok()?;
        return Some(viewport_w * (v / layout_constants::PERCENT_DENOMINATOR));
    }
    if t.ends_with("vh") {
        let v: f32 = t.trim_end_matches("vh").trim().parse().ok()?;
        return Some(viewport_h * (v / layout_constants::PERCENT_DENOMINATOR));
    }
    if t.ends_with('%') {
        let v: f32 = t.trim_end_matches('%').trim().parse().ok()?;
        return Some(containing * (v / layout_constants::PERCENT_DENOMINATOR));
    }
    None
}

fn parse_4len(
    s: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let parts = s
        .split_whitespace()
        .map(|p| p.trim().to_string())
        .collect::<Vec<_>>();

    match parts.len() {
        0 => (None, None, None, None),
        1 => (
            Some(parts[0].clone()),
            Some(parts[0].clone()),
            Some(parts[0].clone()),
            Some(parts[0].clone()),
        ),
        2 => (
            Some(parts[0].clone()),
            Some(parts[1].clone()),
            Some(parts[0].clone()),
            Some(parts[1].clone()),
        ),
        3 => (
            Some(parts[0].clone()),
            Some(parts[1].clone()),
            Some(parts[2].clone()),
            Some(parts[1].clone()),
        ),
        _ => (
            Some(parts[0].clone()),
            Some(parts[1].clone()),
            Some(parts[2].clone()),
            Some(parts[3].clone()),
        ),
    }
}

fn scale_for_side(limit: f32, sum: f32) -> f32 {
    if sum > 0.0 { limit / sum } else { 1.0 }
}

fn parse_border_radius_shorthand(
    s: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<CornerRadii> {
    let horizontal = s.split('/').next()?.trim();
    if horizontal.is_empty() {
        return None;
    }

    let parts: Vec<&str> = horizontal
        .split_whitespace()
        .filter(|p| !p.is_empty())
        .collect();

    match parts.len() {
        1 => {
            let a = parse_length(parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: a,
                bottom_right: a,
                bottom_left: a,
            })
        }
        2 => {
            let a = parse_length(parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: b,
                bottom_right: a,
                bottom_left: b,
            })
        }
        3 => {
            let a = parse_length(parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            let c = parse_length(parts[2], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: b,
                bottom_right: c,
                bottom_left: b,
            })
        }
        4 => {
            let a = parse_length(parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            let c = parse_length(parts[2], containing, viewport_w, viewport_h)?.max(0.0);
            let d = parse_length(parts[3], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: b,
                bottom_right: c,
                bottom_left: d,
            })
        }
        _ => None,
    }
}

fn parse_corner_radius(s: &str, containing: f32, viewport_w: f32, viewport_h: f32) -> Option<f32> {
    let first = s.split('/').next()?.split_whitespace().next()?;
    parse_length(first, containing, viewport_w, viewport_h).map(|v| v.max(0.0))
}

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
    font_size * layout_constants::DEFAULT_LINE_HEIGHT_MULTIPLIER
}

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(num) = t.strip_suffix("px") {
        return num.trim().parse::<f32>().ok();
    }
    None
}

/// ★img の最小 intrinsic size
/// 優先順位:
/// 1) CSS width/height (px)
/// 2) HTML attributes width/height (数値)
/// 3) fallback default image size
fn img_intrinsic_size_px(
    sn: &crate::style::StyledNode,
    img_cache: &dyn ImageSizeProvider,
) -> (f32, f32) {
    let css_w = sn.value("width").and_then(|v| parse_px(v));
    let css_h = sn.value("height").and_then(|v| parse_px(v));

    let (attr_w, attr_h, src_opt) = if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
        let w = ed
            .attributes
            .get("width")
            .and_then(|s| s.trim().parse::<f32>().ok());
        let h = ed
            .attributes
            .get("height")
            .and_then(|s| s.trim().parse::<f32>().ok());
        let src = ed.attributes.get("src").cloned();
        (w, h, src)
    } else {
        (None, None, None)
    };

    // -------------------------
    // ★ここがキモ：src を正規化してから cache を引く
    // -------------------------
    let natural = src_opt
        .as_deref()
        .and_then(|src| img_cache.normalize_src_key(src))
        .and_then(|key| img_cache.natural_size_px(&key));

    // まずは明示指定（CSS/attr）
    if let (Some(w), Some(h)) = (css_w.or(attr_w), css_h.or(attr_h)) {
        return (
            w.max(layout_constants::MIN_LAYOUT_SIZE_PX),
            h.max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );
    }

    if let Some(w) = css_w.or(attr_w) {
        // 片方だけ指定：もう片方は自然サイズで補完
        if let Some((nw, nh)) = natural {
            if nw > 0 && nh > 0 {
                let ratio = (nh as f32) / (nw as f32);
                return (
                    w.max(layout_constants::MIN_LAYOUT_SIZE_PX),
                    (w * ratio).max(layout_constants::MIN_LAYOUT_SIZE_PX),
                );
            }
        }
        return (
            w.max(layout_constants::MIN_LAYOUT_SIZE_PX),
            layout_constants::DEFAULT_IMAGE_HEIGHT_PX,
        );
    }

    if let Some(h) = css_h.or(attr_h) {
        if let Some((nw, nh)) = natural {
            if nw > 0 && nh > 0 {
                let ratio = (nw as f32) / (nh as f32);
                return (
                    (h * ratio).max(layout_constants::MIN_LAYOUT_SIZE_PX),
                    h.max(layout_constants::MIN_LAYOUT_SIZE_PX),
                );
            }
        }
        return (
            layout_constants::DEFAULT_IMAGE_WIDTH_PX,
            h.max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );
    }

    // 明示指定が無いなら自然サイズ
    if let Some((nw, nh)) = natural {
        return (
            (nw as f32).max(layout_constants::MIN_LAYOUT_SIZE_PX),
            (nh as f32).max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );
    }

    (
        layout_constants::DEFAULT_IMAGE_WIDTH_PX,
        layout_constants::DEFAULT_IMAGE_HEIGHT_PX,
    )
}

/// styleノードから Text を集める（leaf block の救済用）
fn collect_text_nodes(sn: &StyledNode, out: &mut String) {
    match &sn.node.node_type {
        crate::dom::NodeType::Text(t) => {
            let s = t.trim();
            if !s.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(s);
            }
        }
        crate::dom::NodeType::Element(ed) => {
            let tag = ed.tag_name.as_str();
            if tag == "style"
                || tag == "script"
                || tag == "head"
                || tag == "title"
                || tag == "meta"
                || tag == "link"
            {
                return;
            }
        }
    }

    for c in &sn.children {
        collect_text_nodes(c, out);
    }
}

fn layout_text_fragments(
    node: &mut LayoutBox<'_>,
    font: &Font,
    text: &str,
    font_size: f32,
    line_h: f32,
    start_x: f32,
    max_w: f32,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    current_line_h: &mut f32,
    pending_space_w: &mut f32,
    pending_space_h: &mut f32,
) {
    node.text_fragments.clear();

    let collapsed = collapse_whitespace(text);
    let space_w = measure_width_fontdue(font, " ", font_size);
    let mut word = String::new();

    for ch in collapsed.chars() {
        if ch == ' ' {
            push_inline_word(
                node,
                font,
                &word,
                font_size,
                line_h,
                start_x,
                max_w,
                cursor_x,
                cursor_y,
                current_line_h,
                pending_space_w,
                pending_space_h,
            );
            word.clear();

            *pending_space_w = space_w;
            *pending_space_h = line_h;
        } else {
            word.push(ch);
        }
    }

    push_inline_word(
        node,
        font,
        &word,
        font_size,
        line_h,
        start_x,
        max_w,
        cursor_x,
        cursor_y,
        current_line_h,
        pending_space_w,
        pending_space_h,
    );

    set_text_content_rect(node, *cursor_x, *cursor_y);
    sync_text_paint_fragments(node);
}

fn push_inline_word(
    node: &mut LayoutBox<'_>,
    font: &Font,
    word: &str,
    font_size: f32,
    line_h: f32,
    start_x: f32,
    max_w: f32,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    current_line_h: &mut f32,
    pending_space_w: &mut f32,
    pending_space_h: &mut f32,
) {
    if word.is_empty() {
        return;
    }

    let line_end = start_x + max_w;
    let word_w = measure_width_fontdue(font, word, font_size);

    consume_pending_space_before_item(
        word_w,
        start_x,
        max_w,
        cursor_x,
        cursor_y,
        current_line_h,
        pending_space_w,
        pending_space_h,
    );

    if word_w <= max_w {
        if *cursor_x > start_x && *cursor_x + word_w > line_end {
            advance_to_next_line(cursor_x, cursor_y, current_line_h, start_x, line_h);
        }

        push_text_fragment(
            node,
            word,
            word_w,
            line_h,
            cursor_x,
            cursor_y,
            current_line_h,
        );
        return;
    }

    let mut part = String::new();
    let mut part_w = 0.0f32;

    for ch in word.chars() {
        let ch_s = ch.to_string();
        let ch_w = measure_width_fontdue(font, &ch_s, font_size);

        if !part.is_empty() && *cursor_x + part_w + ch_w > line_end {
            push_text_fragment(
                node,
                &part,
                part_w,
                line_h,
                cursor_x,
                cursor_y,
                current_line_h,
            );
            advance_to_next_line(cursor_x, cursor_y, current_line_h, start_x, line_h);
            part.clear();
            part_w = 0.0;
        }

        if part.is_empty() && *cursor_x > start_x && *cursor_x + ch_w > line_end {
            advance_to_next_line(cursor_x, cursor_y, current_line_h, start_x, line_h);
        }

        if part.is_empty() && ch_w > max_w {
            push_text_fragment(
                node,
                &ch_s,
                ch_w,
                line_h,
                cursor_x,
                cursor_y,
                current_line_h,
            );
        } else {
            part.push(ch);
            part_w += ch_w;
        }
    }

    if !part.is_empty() {
        push_text_fragment(
            node,
            &part,
            part_w,
            line_h,
            cursor_x,
            cursor_y,
            current_line_h,
        );
    }
}

fn consume_pending_space_before_item(
    item_w: f32,
    start_x: f32,
    max_w: f32,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    current_line_h: &mut f32,
    pending_space_w: &mut f32,
    pending_space_h: &mut f32,
) {
    if *pending_space_w <= 0.0 {
        return;
    }

    if *cursor_x <= start_x {
        *pending_space_w = 0.0;
        *pending_space_h = 0.0;
        return;
    }

    if *cursor_x + *pending_space_w + item_w <= start_x + max_w {
        *cursor_x += *pending_space_w;
        *current_line_h = (*current_line_h).max(*pending_space_h);
    } else {
        advance_to_next_line(
            cursor_x,
            cursor_y,
            current_line_h,
            start_x,
            *pending_space_h,
        );
    }

    *pending_space_w = 0.0;
    *pending_space_h = 0.0;
}

fn advance_to_next_line(
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    current_line_h: &mut f32,
    start_x: f32,
    min_line_h: f32,
) {
    *cursor_x = start_x;
    *cursor_y += (*current_line_h).max(min_line_h);
    *current_line_h = 0.0;
}

fn push_text_fragment(
    node: &mut LayoutBox<'_>,
    text: &str,
    width: f32,
    line_h: f32,
    cursor_x: &mut f32,
    cursor_y: &mut f32,
    current_line_h: &mut f32,
) {
    if !text.is_empty() && width > 0.0 {
        node.text_fragments.push(TextFragment {
            rect: Rect {
                x: *cursor_x,
                y: *cursor_y,
                width,
                height: line_h,
            },
            text: text.to_string(),
        });
    }

    *cursor_x += width;
    *current_line_h = (*current_line_h).max(line_h);
}

fn set_text_content_rect(node: &mut LayoutBox<'_>, fallback_x: f32, fallback_y: f32) {
    if node.text_fragments.is_empty() {
        node.dimensions.content.x = fallback_x;
        node.dimensions.content.y = fallback_y;
        node.dimensions.content.width = 0.0;
        node.dimensions.content.height = 0.0;
        return;
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for frag in &node.text_fragments {
        let r = &frag.rect;
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.width);
        max_y = max_y.max(r.y + r.height);
    }

    node.dimensions.content.x = min_x;
    node.dimensions.content.y = min_y;
    node.dimensions.content.width = (max_x - min_x).max(0.0);
    node.dimensions.content.height = (max_y - min_y).max(0.0);
}

fn sync_text_paint_fragments(node: &mut LayoutBox<'_>) {
    node.paint_fragments = node
        .text_fragments
        .iter()
        .map(|fragment| PaintFragment {
            rect: fragment.rect,
        })
        .collect();
}

fn sync_paint_fragments_with_content_rect(node: &mut LayoutBox<'_>) {
    node.paint_fragments.clear();

    let rect = node.dimensions.content;
    if rect.width > 0.0 && rect.height > 0.0 {
        node.paint_fragments.push(PaintFragment { rect });
    }
}

fn sync_inline_paint_fragments_from_children(node: &mut LayoutBox<'_>) {
    let mut rects = Vec::new();
    for child in &node.children {
        collect_inline_paint_source_rects(child, &mut rects);
    }

    node.paint_fragments = merge_inline_paint_source_rects(rects)
        .into_iter()
        .map(|rect| PaintFragment { rect })
        .collect();
}

fn collect_inline_paint_source_rects(node: &LayoutBox<'_>, out: &mut Vec<Rect>) {
    if node.node_position().is_out_of_flow() || node.node_float().is_floating() {
        return;
    }

    if !node.paint_fragments.is_empty() {
        out.extend(node.paint_fragments.iter().map(|fragment| fragment.rect));
        return;
    }

    if node.children.is_empty() {
        let rect = node.dimensions.content;
        if rect.width > 0.0 && rect.height > 0.0 {
            out.push(rect);
        }
        return;
    }

    for child in &node.children {
        collect_inline_paint_source_rects(child, out);
    }
}

fn merge_inline_paint_source_rects(mut rects: Vec<Rect>) -> Vec<Rect> {
    rects.retain(|rect| rect.width > 0.0 && rect.height > 0.0);
    rects.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal))
    });

    let mut merged = Vec::new();
    for rect in rects {
        let mut line_index = None;
        for (index, line) in merged.iter().enumerate() {
            if rects_share_line(*line, rect) {
                line_index = Some(index);
                break;
            }
        }

        if let Some(index) = line_index {
            merged[index] = union_rects(merged[index], rect);
        } else {
            merged.push(rect);
        }
    }

    merged
}

fn rects_share_line(a: Rect, b: Rect) -> bool {
    (a.y - b.y).abs() <= 0.5
}

fn union_rects(a: Rect, b: Rect) -> Rect {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = (a.x + a.width).max(b.x + b.width);
    let max_y = (a.y + a.height).max(b.y + b.height);

    Rect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.0),
        height: (max_y - min_y).max(0.0),
    }
}

fn set_paint_content_rect(node: &mut LayoutBox<'_>, fallback_x: f32, fallback_y: f32) {
    if node.paint_fragments.is_empty() {
        node.dimensions.content.x = fallback_x;
        node.dimensions.content.y = fallback_y;
        node.dimensions.content.width = 0.0;
        node.dimensions.content.height = 0.0;
        return;
    }

    let mut rect = node.paint_fragments[0].rect;
    for fragment in &node.paint_fragments[1..] {
        rect = union_rects(rect, fragment.rect);
    }

    node.dimensions.content = rect;
}

/// fontdue実測で「折り返し行数」だけ返す
fn count_lines_fontdue(font: &Font, text: &str, max_w: f32, font_size: f32) -> usize {
    let t = text.trim();
    if t.is_empty() {
        return 0;
    }

    let has_spaces = t.contains(' ');
    let tokens: Vec<String> = if has_spaces {
        t.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        t.chars().map(|c| c.to_string()).collect()
    };

    let space_w = if has_spaces {
        measure_width_fontdue(font, " ", font_size)
    } else {
        0.0
    };

    let mut lines = 1usize;
    let mut x = 0.0f32;

    for tok in tokens {
        let w = measure_width_fontdue(font, &tok, font_size);

        if x > 0.0 && x + w > max_w {
            lines += 1;
            x = 0.0;
        }

        x += w;

        if has_spaces {
            x += space_w;
        }
    }

    lines
}

fn measure_width_fontdue(font: &Font, s: &str, px: f32) -> f32 {
    let mut w = 0.0;
    for ch in s.chars() {
        w += font.metrics(ch, px).advance_width;
    }
    w
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

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyImageCache;

    impl ImageSizeProvider for EmptyImageCache {
        fn normalize_src_key(&self, _src: &str) -> Option<String> {
            None
        }

        fn natural_size_px(&self, _key: &str) -> Option<(u32, u32)> {
            None
        }
    }

    fn styled_tree(input: &str) -> crate::style::StyledNode {
        let dom = crate::html::parse(input.to_string());
        crate::style::style_tree(dom, &crate::css::Stylesheet::default())
    }

    fn styled_tree_with_css(input: &str, css: &str) -> crate::style::StyledNode {
        let dom = crate::html::parse(input.to_string());
        let stylesheet = crate::css::Parser::new(css.to_string()).parse_stylesheet();
        crate::style::style_tree(dom, &stylesheet)
    }

    fn test_font() -> Font {
        fontdue::Font::from_bytes(
            include_bytes!("../assets/DejaVuSans.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap()
    }

    fn is_element(node: &LayoutBox<'_>, tag: &str) -> bool {
        node.get_style_node()
            .and_then(|sn| match &sn.node.node_type {
                crate::dom::NodeType::Element(ed) => Some(ed.tag_name.as_str()),
                _ => None,
            })
            == Some(tag)
    }

    fn find_element<'tree, 'style>(
        node: &'tree LayoutBox<'style>,
        tag: &str,
    ) -> Option<&'tree LayoutBox<'style>> {
        if is_element(node, tag) {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_element(child, tag))
    }

    fn is_element_id(node: &LayoutBox<'_>, id: &str) -> bool {
        node.get_style_node()
            .and_then(|sn| match &sn.node.node_type {
                crate::dom::NodeType::Element(ed) => ed.attributes.get("id").map(|s| s.as_str()),
                _ => None,
            })
            == Some(id)
    }

    fn find_element_by_id<'tree, 'style>(
        node: &'tree LayoutBox<'style>,
        id: &str,
    ) -> Option<&'tree LayoutBox<'style>> {
        if is_element_id(node, id) {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_element_by_id(child, id))
    }

    fn collect_fragments(node: &LayoutBox<'_>, out: &mut Vec<TextFragment>) {
        out.extend(node.text_fragments.iter().cloned());
        for child in &node.children {
            collect_fragments(child, out);
        }
    }

    #[test]
    fn inline_children_inside_inline_elements_are_not_anonymous_blocks() {
        let styled = styled_tree(
            r#"<p><a href="https://example.com">Learn <span>more</span> today</a></p>"#,
        );
        let layout = build_layout_tree(&styled);
        let anchor = find_element(&layout, "a").unwrap();

        assert!(matches!(anchor.box_type, BoxType::InlineNode(_)));
        assert_eq!(anchor.children.len(), 3);
        assert!(
            anchor
                .children
                .iter()
                .all(|child| matches!(child.box_type, BoxType::InlineNode(_)))
        );
        assert!(is_element(&anchor.children[1], "span"));
    }

    #[test]
    fn nested_inline_fragments_share_the_same_line_when_there_is_room() {
        let styled = styled_tree(
            r#"<p><a href="https://example.com">Learn <span>more</span> today</a></p>"#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 500.0;
        viewport.content.height = 600.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let mut fragments = Vec::new();
        collect_fragments(&layout, &mut fragments);

        let y_of = |text: &str| {
            fragments
                .iter()
                .find(|frag| frag.text == text)
                .map(|frag| frag.rect.y)
                .unwrap()
        };

        let learn_y = y_of("Learn");
        let more_y = y_of("more");
        let today_y = y_of("today");

        assert!((learn_y - more_y).abs() <= 0.5);
        assert!((more_y - today_y).abs() <= 0.5);
    }

    #[test]
    fn wrapped_inline_element_builds_multiple_paint_fragments() {
        let styled = styled_tree_with_css(
            r#"<p><span id="target">alpha beta gamma delta epsilon</span></p>"#,
            r#"
            p { display: block; width: 90px; margin: 0; padding: 0; }
            #target { padding: 4px; border: 2px solid red; }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 240.0;
        viewport.content.height = 400.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let span = find_element_by_id(&layout, "target").unwrap();

        assert!(span.paint_fragments.len() >= 2);
        assert!(
            span.paint_fragments
                .windows(2)
                .all(|pair| pair[0].rect.y < pair[1].rect.y)
        );
        assert!(
            span.paint_fragments
                .iter()
                .all(|fragment| fragment.rect.width > 0.0 && fragment.rect.height > 0.0)
        );
    }

    #[test]
    fn relative_position_offsets_visual_box_without_moving_following_flow() {
        let styled = styled_tree_with_css(
            r#"<div id="one"></div><div id="rel"></div><div id="two"></div>"#,
            r#"
            #one { display: block; width: 100px; height: 20px; margin: 0; padding: 0; }
            #rel {
                display: block;
                position: relative;
                left: 15px;
                top: 10px;
                width: 100px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            #two { display: block; width: 100px; height: 20px; margin: 0; padding: 0; }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 300.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let one = find_element_by_id(&layout, "one").unwrap();
        let rel = find_element_by_id(&layout, "rel").unwrap();
        let two = find_element_by_id(&layout, "two").unwrap();

        assert!((rel.dimensions.content.x - (one.dimensions.content.x + 15.0)).abs() <= 0.5);
        assert!((rel.dimensions.content.y - (one.dimensions.content.y + 30.0)).abs() <= 0.5);
        assert!((two.dimensions.content.y - (one.dimensions.content.y + 40.0)).abs() <= 0.5);
    }

    #[test]
    fn absolute_position_uses_nearest_positioned_padding_box_and_leaves_flow() {
        let styled = styled_tree_with_css(
            r#"<div id="container"><div id="abs"></div><div id="normal"></div></div>"#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                height: 100px;
                padding: 10px;
                margin: 0;
            }
            #abs {
                display: block;
                position: absolute;
                left: 20px;
                top: 15px;
                width: 30px;
                height: 10px;
                margin: 0;
                padding: 0;
            }
            #normal {
                display: block;
                width: 50px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 300.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let container = find_element_by_id(&layout, "container").unwrap();
        let abs = find_element_by_id(&layout, "abs").unwrap();
        let normal = find_element_by_id(&layout, "normal").unwrap();

        let padding_box_x = container.dimensions.content.x - container.dimensions.padding.left;
        let padding_box_y = container.dimensions.content.y - container.dimensions.padding.top;

        assert!((abs.dimensions.content.x - (padding_box_x + 20.0)).abs() <= 0.5);
        assert!((abs.dimensions.content.y - (padding_box_y + 15.0)).abs() <= 0.5);
        assert!((normal.dimensions.content.y - container.dimensions.content.y).abs() <= 0.5);
    }

    #[test]
    fn left_float_reduces_following_flow_space_and_clear_moves_below_it() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="float"></div>
                <div id="normal"></div>
                <div id="clear"></div>
            </div>
            "#,
            r#"
            #container { display: block; width: 300px; margin: 0; padding: 0; }
            #float {
                display: block;
                float: left;
                width: 80px;
                height: 60px;
                margin: 0;
                padding: 0;
            }
            #normal {
                display: block;
                width: 100px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            #clear {
                display: block;
                clear: both;
                width: 100px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 300.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let container = find_element_by_id(&layout, "container").unwrap();
        let float = find_element_by_id(&layout, "float").unwrap();
        let normal = find_element_by_id(&layout, "normal").unwrap();
        let clear = find_element_by_id(&layout, "clear").unwrap();

        assert!((float.dimensions.content.x - container.dimensions.content.x).abs() <= 0.5);
        assert!((float.dimensions.content.y - container.dimensions.content.y).abs() <= 0.5);
        assert!(
            (normal.dimensions.content.x - (container.dimensions.content.x + 80.0)).abs() <= 0.5
        );
        assert!((normal.dimensions.content.y - container.dimensions.content.y).abs() <= 0.5);
        assert!(
            (clear.dimensions.content.y - (container.dimensions.content.y + 60.0)).abs() <= 0.5
        );
    }
}
