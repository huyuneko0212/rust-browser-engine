use crate::constants::layout as layout_constants;
use crate::style::{Display, StyledNode};
use fontdue::Font;

pub trait ImageSizeProvider {
    /// layout が持っている src（相対/絶対/ポート付きなど）を
    /// “キャッシュキーと同じ正規化済み絶対URL文字列” に変換する
    fn normalize_src_key(&self, src: &str) -> Option<String>;

    /// key(正規化済み絶対URL文字列) から自然サイズ(px)を返す
    fn natural_size_px(&self, key: &str) -> Option<(u32, u32)>;
}

#[derive(Debug, Default, Clone)]
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

#[derive(Debug)]
pub struct LayoutBox<'a> {
    pub box_type: BoxType<'a>,
    pub dimensions: Dimensions,
    pub children: Vec<LayoutBox<'a>>,
    pub text_fragments: Vec<TextFragment>,
}

impl<'a> LayoutBox<'a> {
    pub fn new(box_type: BoxType<'a>) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            children: vec![],
            text_fragments: vec![],
        }
    }

    pub fn get_style_node(&self) -> Option<&StyledNode> {
        match self.box_type {
            BoxType::BlockNode(node) | BoxType::InlineNode(node) => Some(node),
            BoxType::Anonymous => None,
        }
    }

    pub fn layout_with_font(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.text_fragments.clear();

        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block_with_font(containing_block, font, img_cache),
            BoxType::InlineNode(_) => {
                self.layout_inline_leaf_fallback(containing_block, font, img_cache)
            }
            BoxType::Anonymous => {
                self.layout_anonymous_block_with_font(containing_block, font, img_cache)
            }
        }
    }

    fn layout_block_with_font(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.calculate_block_model(containing_block.clone());
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());

        self.layout_block_children_with_font(font, img_cache);

        self.calculate_block_height_with_font(font, img_cache);
    }

    fn layout_anonymous_block_with_font(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.dimensions.content.x = containing_block.content.x;
        self.dimensions.content.y = containing_block.content.y;
        self.dimensions.content.width = containing_block.content.width;
        self.dimensions.content.height = 0.0;

        self.layout_inline_formatting_context(font, img_cache);
    }

    fn layout_inline_leaf_fallback(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
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

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let d = &mut self.dimensions;

        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;
        d.content.y = containing_block.content.y + d.margin.top + d.border.top + d.padding.top;
    }

    fn layout_block_children_with_font(&mut self, font: &Font, img_cache: &dyn ImageSizeProvider) {
        let mut y = self.dimensions.content.y;

        for child in &mut self.children {
            let mut cb = Dimensions::default();
            cb.content.x = self.dimensions.content.x;
            cb.content.y = y;
            cb.content.width = self.dimensions.content.width;
            cb.content.height = self
                .dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);

            child.layout_with_font(cb, font, img_cache);

            y += child.dimensions.margin_box_height().max(0.0);
        }

        self.dimensions.content.height = (y - self.dimensions.content.y).max(0.0);
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
    fn layout_inline_formatting_context(&mut self, font: &Font, img_cache: &dyn ImageSizeProvider) {
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

        fn walk_inline<'a>(
            node: &mut LayoutBox<'a>,
            font: &Font,
            img_cache: &dyn ImageSizeProvider,
            start_x: f32,
            max_w: f32,
            cursor_x: &mut f32,
            cursor_y: &mut f32,
            current_line_h: &mut f32,
            pending_space_w: &mut f32,
            pending_space_h: &mut f32,
        ) {
            node.text_fragments.clear();

            match &mut node.box_type {
                BoxType::InlineNode(_) => {
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
                                cursor_x,
                                cursor_y,
                                current_line_h,
                                pending_space_w,
                                pending_space_h,
                            );
                        }
                        node.dimensions.content.width = 0.0;
                        node.dimensions.content.height = 0.0;
                        node.dimensions.content.x = *cursor_x;
                        node.dimensions.content.y = *cursor_y;
                    }
                }
                BoxType::BlockNode(_) | BoxType::Anonymous => {
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
                    let mut cb = Dimensions::default();
                    cb.content.x = start_x;
                    cb.content.y = *cursor_y;
                    cb.content.width = max_w;
                    cb.content.height = layout_constants::MIN_LAYOUT_SIZE_PX;

                    node.layout_with_font(cb, font, img_cache);
                    *cursor_y += node.dimensions.margin_box_height().max(0.0);
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
}

pub fn build_layout_tree(style_node: &StyledNode) -> LayoutBox<'_> {
    // browser.engineering 的に
    // - block の子: block はそのまま
    // - inline の連続: Anonymous block box にまとめて、その中に inline を入れる
    // - inline の子: Chrome と同じように同じ IFC の中へ直列に流す
    let display = style_node.display();

    let mut root = LayoutBox::new(match display {
        Display::Block => BoxType::BlockNode(style_node),
        Display::Inline => BoxType::InlineNode(style_node),
        Display::None => BoxType::Anonymous,
    });

    // Display::None は上で Anonymous に落ちてるので、ここでは children を作らない（最小）
    if display == Display::None {
        return root;
    }

    if display == Display::Inline {
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
        match child.display() {
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

fn parse_length(s: &str, containing: f32, viewport_w: f32, viewport_h: f32) -> Option<f32> {
    let t = s.trim();

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
}
