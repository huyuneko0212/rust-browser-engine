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

#[derive(Debug, Default, Clone)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
    pub border_radius: f32,
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

#[derive(Debug)]
pub enum BoxType {
    BlockNode(StyledNode),
    InlineNode(StyledNode),
    Anonymous, // anonymous block box (for inline formatting context)
}

#[derive(Debug)]
pub struct LayoutBox {
    pub box_type: BoxType,
    pub dimensions: Dimensions,
    pub children: Vec<LayoutBox>,
}

impl LayoutBox {
    pub fn new(box_type: BoxType) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            children: vec![],
        }
    }

    pub fn get_style_node(&self) -> Option<&StyledNode> {
        match &self.box_type {
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
            let fs = font_size_px(sn).unwrap_or(16.0);
            let lh = line_height_px(sn, fs);

            match &sn.node.node_type {
                crate::dom::NodeType::Text(t) => {
                    let s = t.trim();
                    (
                        fs,
                        lh,
                        if s.is_empty() {
                            None
                        } else {
                            Some(s.to_string())
                        },
                        None,
                    )
                }
                crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                    let (w, h) = img_intrinsic_size_px(sn, img_cache);
                    (fs, lh, None, Some((w, h)))
                }
                _ => (fs, lh, None, None),
            }
        } else {
            (16.0, 16.0 * 1.2, None, None)
        };

        let d = &mut self.dimensions;
        d.content.x = containing_block.content.x;
        d.content.y = containing_block.content.y;
        d.content.width = containing_block.content.width.max(1.0);

        if let Some(txt) = text_opt {
            let w = measure_width_fontdue(font, &txt, font_size);
            d.content.width = d.content.width.min(w.max(1.0));
            d.content.height = line_h;
        } else {
            d.content.height = line_h;
        }

        if let Some((iw, ih)) = img_opt {
            d.content.width = iw.max(1.0).min(containing_block.content.width.max(1.0));
            d.content.height = ih.max(1.0);
            return;
        }
    }

    /// margin/padding/border/border-radius を style から読む
    fn calculate_block_model(&mut self, containing: Dimensions) {
        let viewport_w = containing.content.width;
        let viewport_h = containing.content.height.max(1.0);
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
            border_radius_s,
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
                // border-radius（単一値だけ対応）
                style.value("border-radius").cloned(),
            )
        } else {
            (None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None)
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

        // ★ shorthand border-width（個別指定が無い場合のみ）
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

        // ★ border-radius（単一値だけに対応）
        let mut border_radius = 0.0;
        if let Some(v) = border_radius_s.as_deref() {
            if let Some(r) = parse_length(v, parent_w, viewport_w, viewport_h) {
                border_radius = r.max(0.0);
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
        let viewport_h = containing_block.content.height.max(1.0);
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
            cb.content.height = self.dimensions.content.height.max(1.0);

            child.layout_with_font(cb, font, img_cache);

            y += child.dimensions.margin_box_height().max(0.0);
        }

        self.dimensions.content.height = (y - self.dimensions.content.y).max(0.0);
    }

    fn calculate_block_height_with_font(&mut self, font: &Font, img_cache: &dyn ImageSizeProvider) {
        let (h_str, viewport_w, viewport_h, parent_w) = {
            let vw = self.dimensions.content.width.max(1.0);
            (
                self.get_style_node()
                    .and_then(|s| s.value("height"))
                    .cloned(),
                vw,
                600.0,
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
                        self.dimensions.content.height = ih.max(1.0);
                        return;
                    }
                }

                let mut buf = String::new();
                collect_text_nodes(sn, &mut buf);

                let txt = buf.trim();
                if !txt.is_empty() {
                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);
                    let max_w = self.dimensions.content.width.max(1.0);

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
        let max_w = self.dimensions.content.width.max(1.0);

        let mut cursor_x = start_x;
        let mut cursor_y = start_y;
        let mut current_line_h = 0.0f32;

        fn walk_inline(
            node: &mut LayoutBox,
            font: &Font,
            img_cache: &dyn ImageSizeProvider,
            start_x: f32,
            max_w: f32,
            cursor_x: &mut f32,
            cursor_y: &mut f32,
            current_line_h: &mut f32,
        ) {
            match &mut node.box_type {
                BoxType::InlineNode(_) => {
                    let (is_text, text, font_size, line_h, img_opt) =
                        if let Some(sn) = node.get_style_node() {
                            let fs = font_size_px(sn).unwrap_or(16.0);
                            let lh = line_height_px(sn, fs);

                            match &sn.node.node_type {
                                crate::dom::NodeType::Text(t) => {
                                    let collapsed = collapse_whitespace(t);
                                    if collapsed.trim().is_empty() {
                                        (true, Some(" ".to_string()), fs, lh, None)
                                    } else {
                                        (true, Some(collapsed.trim().to_string()), fs, lh, None)
                                    }
                                }
                                crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                                    let (w, h) = img_intrinsic_size_px(sn, img_cache);
                                    (false, None, fs, lh, Some((w, h)))
                                }
                                _ => (false, None, fs, lh, None),
                            }
                        } else {
                            (false, None, 16.0, 16.0 * 1.2, None)
                        };

                    if let Some((iw, ih)) = img_opt {
                        let iw = iw.max(1.0);
                        let ih = ih.max(1.0);

                        if *cursor_x > start_x && *cursor_x + iw > start_x + max_w {
                            *cursor_x = start_x;
                            *cursor_y += (*current_line_h).max(ih);
                            *current_line_h = 0.0;
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
                            let is_space_only = txt == " ";

                            if is_space_only && (*cursor_x == start_x) {
                                node.dimensions.content.x = *cursor_x;
                                node.dimensions.content.y = *cursor_y;
                                node.dimensions.content.width = 0.0;
                                node.dimensions.content.height = 0.0;
                                return;
                            }

                            let w = measure_width_fontdue(font, &txt, font_size);
                            let h = line_h;

                            if !is_space_only
                                && *cursor_x > start_x
                                && *cursor_x + w > start_x + max_w
                            {
                                *cursor_x = start_x;
                                *cursor_y += (*current_line_h).max(h);
                                *current_line_h = 0.0;
                            }

                            if is_space_only {
                                node.dimensions.content.x = *cursor_x;
                                node.dimensions.content.y = *cursor_y;
                                node.dimensions.content.width = 0.0;
                                node.dimensions.content.height = 0.0;

                                *cursor_x += w;
                                *current_line_h = (*current_line_h).max(h);
                                return;
                            }

                            node.dimensions.content.x = *cursor_x;
                            node.dimensions.content.y = *cursor_y;
                            node.dimensions.content.width = w.max(0.0);
                            node.dimensions.content.height = h;

                            *cursor_x += w;
                            *current_line_h = (*current_line_h).max(h);
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
                            );
                        }
                        node.dimensions.content.width = 0.0;
                        node.dimensions.content.height = 0.0;
                        node.dimensions.content.x = *cursor_x;
                        node.dimensions.content.y = *cursor_y;
                    }
                }
                BoxType::BlockNode(_) | BoxType::Anonymous => {
                    if *cursor_x > start_x {
                        *cursor_x = start_x;
                        *cursor_y += (*current_line_h).max(18.0);
                        *current_line_h = 0.0;
                    }
                    let mut cb = Dimensions::default();
                    cb.content.x = start_x;
                    cb.content.y = *cursor_y;
                    cb.content.width = max_w;
                    cb.content.height = 1.0;

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
            );
        }

        let total_h = (cursor_y - start_y) + current_line_h.max(0.0);
        self.dimensions.content.height = total_h.max(0.0);
    }
}

pub fn build_layout_tree(style_node: StyledNode) -> LayoutBox {
    // browser.engineering 的に
    // - block の子: block はそのまま
    // - inline の連続: Anonymous block box にまとめて、その中に inline を入れる
    let display = style_node.display();

    let mut root = LayoutBox::new(match display {
        Display::Block => BoxType::BlockNode(style_node.clone()),
        Display::Inline => BoxType::InlineNode(style_node.clone()),
        Display::None => BoxType::Anonymous,
    });

    // Display::None は上で Anonymous に落ちてるので、ここでは children を作らない（最小）
    if display == Display::None {
        return root;
    }

    // 子をグルーピング
    let mut anon: Option<LayoutBox> = None;

    for child in style_node.children {
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
        return Some(viewport_w * (v / 100.0));
    }
    if t.ends_with("vh") {
        let v: f32 = t.trim_end_matches("vh").trim().parse().ok()?;
        return Some(viewport_h * (v / 100.0));
    }
    if t.ends_with('%') {
        let v: f32 = t.trim_end_matches('%').trim().parse().ok()?;
        return Some(containing * (v / 100.0));
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

/// ★img の最小 intrinsic size
/// 優先順位:
/// 1) CSS width/height (px)
/// 2) HTML attributes width/height (数値)
/// 3) fallback 300x150
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
        return (w.max(1.0), h.max(1.0));
    }

    if let Some(w) = css_w.or(attr_w) {
        // 片方だけ指定：もう片方は自然サイズで補完
        if let Some((nw, nh)) = natural {
            if nw > 0 && nh > 0 {
                let ratio = (nh as f32) / (nw as f32);
                return (w.max(1.0), (w * ratio).max(1.0));
            }
        }
        return (w.max(1.0), 150.0);
    }

    if let Some(h) = css_h.or(attr_h) {
        if let Some((nw, nh)) = natural {
            if nw > 0 && nh > 0 {
                let ratio = (nw as f32) / (nh as f32);
                return ((h * ratio).max(1.0), h.max(1.0));
            }
        }
        return (300.0, h.max(1.0));
    }

    // 明示指定が無いなら自然サイズ
    if let Some((nw, nh)) = natural {
        return ((nw as f32).max(1.0), (nh as f32).max(1.0));
    }

    (300.0, 150.0)
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
