use crate::style::{Display, StyledNode};
use fontdue::Font;

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
    Anonymous,
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

    pub fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block(containing_block),
            BoxType::InlineNode(_) => self.layout_inline(containing_block),
            BoxType::Anonymous => {}
        }
    }

    fn layout_inline(&mut self, containing_block: Dimensions) {
        // 仮：inline = 1行テキスト（本物は後で）
        let d = &mut self.dimensions;
        d.content.x = containing_block.content.x;
        d.content.y = containing_block.content.y;
        d.content.width = containing_block.content.width;
        d.content.height = 18.0;
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_model(containing_block.clone());
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());
        self.layout_block_children();
        self.calculate_block_height();
    }

    /// margin/padding を style から読む
    /// - px / vw / vh / % 対応
    /// - shorthand（1〜4値）対応
    /// - auto は left/right のみ扱う（中央寄せ用）
    fn calculate_block_model(&mut self, containing: Dimensions) {
        let viewport_w = containing.content.width;
        let viewport_h = containing.content.height.max(1.0);
        let parent_w = containing.content.width;

        // 借用衝突を避ける：style値は先に全部ローカルへ
        let (
            ml_s, mr_s, mt_s, mb_s,
            pl_s, pr_s, pt_s, pb_s,
            margin_sh, padding_sh
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
            )
        } else {
            (None,None,None,None, None,None,None,None, None,None)
        };

        // default 0
        let mut ml = 0.0;
        let mut mr = 0.0;
        let mut mt = 0.0;
        let mut mb = 0.0;
        let mut pl = 0.0;
        let mut pr = 0.0;
        let mut pt = 0.0;
        let mut pb = 0.0;

        // auto flags（左右だけ）
        let mut ml_auto = false;
        let mut mr_auto = false;

        // 個別指定を優先
        if let Some(v) = ml_s.as_deref() {
            if v.trim() == "auto" { ml_auto = true; } else {
                ml = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mr_s.as_deref() {
            if v.trim() == "auto" { mr_auto = true; } else {
                mr = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mt_s.as_deref() {
            mt = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = mb_s.as_deref() {
            mb = parse_length(v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

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

        // shorthand margin（個別指定が無い場合にのみ入れる）
        if ml_s.is_none() && mr_s.is_none() && mt_s.is_none() && mb_s.is_none() {
            if let Some(sh) = margin_sh.as_deref() {
                let m = parse_4len(sh);
                // top right bottom left
                if let Some(top) = m.0.as_deref() {
                    mt = parse_length(top, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(right) = m.1.as_deref() {
                    if right.trim() == "auto" { mr_auto = true; } else {
                        mr = parse_length(right, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                    }
                }
                if let Some(bottom) = m.2.as_deref() {
                    mb = parse_length(bottom, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                }
                if let Some(left) = m.3.as_deref() {
                    if left.trim() == "auto" { ml_auto = true; } else {
                        ml = parse_length(left, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
                    }
                }
            }
        }

        // shorthand padding
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

        // 書き込み（styleの借用は終わってる）
        self.dimensions.margin.left = ml;
        self.dimensions.margin.right = mr;
        self.dimensions.margin.top = mt;
        self.dimensions.margin.bottom = mb;

        self.dimensions.padding.left = pl;
        self.dimensions.padding.right = pr;
        self.dimensions.padding.top = pt;
        self.dimensions.padding.bottom = pb;

        // autoフラグは width計算後に使うので保存しておく（簡易：borderに埋めるのは嫌なので一旦 content.height に入れない）
        // → ここでは何もしない。auto判定は calculate_block_width 内で再判定する（安全＆簡単）
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let viewport_w = containing_block.content.width;
        let viewport_h = containing_block.content.height.max(1.0);
        let parent_w = containing_block.content.width;

        // width指定を先に取得（借用終了）
        let width_str = self
            .get_style_node()
            .and_then(|s| s.value("width"))
            .cloned();

        // margin:auto 判定（個別 or shorthand）
        let (ml_auto, mr_auto) = self
            .get_style_node()
            .map(|s| {
                let mut la = s.value("margin-left").map(|v| v.trim() == "auto").unwrap_or(false);
                let mut ra = s.value("margin-right").map(|v| v.trim() == "auto").unwrap_or(false);

                // shorthand margin: "15vh auto" みたいなのも拾う
                if (!la || !ra) && s.value("margin-left").is_none() && s.value("margin-right").is_none() {
                    if let Some(m) = s.value("margin") {
                        let m4 = parse_4len(m);
                        if let Some(r) = m4.1.as_deref() {
                            if r.trim() == "auto" { ra = true; }
                        }
                        if let Some(l) = m4.3.as_deref() {
                            if l.trim() == "auto" { la = true; }
                        }
                    }
                }
                (la, ra)
            })
            .unwrap_or((false, false));

        // ここから mutable OK
        let d = &mut self.dimensions;

        // width指定があればそれを使う（vw/vh/%/px）
        if let Some(ws) = width_str.as_deref() {
            if let Some(w) = parse_length(ws, parent_w, viewport_w, viewport_h) {
                d.content.width = w.max(0.0);
            }
        }

        // width指定が無いなら "親幅 - margin/padding" にする
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

        // ★ margin:auto の左右中央寄せ（最小実装）
        if ml_auto || mr_auto {
            let used = d.content.width
                + d.padding.left + d.padding.right
                + d.border.left + d.border.right;

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

        // x: parent + margin + border + padding
        d.content.x = containing_block.content.x
            + d.margin.left
            + d.border.left
            + d.padding.left;

        // y: parent + (前の兄弟の積み上げ) は親が渡してくるので、ここでは margin/padding 分を足す
        d.content.y = containing_block.content.y
            + d.margin.top
            + d.border.top
            + d.padding.top;
    }

    fn layout_block_children(&mut self) {
        let mut y = self.dimensions.content.y;

        for child in &mut self.children {
            let mut cb = Dimensions::default();
            cb.content.x = self.dimensions.content.x;
            cb.content.y = y;
            cb.content.width = self.dimensions.content.width;
            cb.content.height = self.dimensions.content.height.max(1.0); // vh用

            child.layout(cb);

            y += child.dimensions.margin_box_height().max(0.0);
        }

        self.dimensions.content.height = (y - self.dimensions.content.y).max(0.0);
    }

    fn calculate_block_height(&mut self) {
        // height指定があれば反映（px/vh/vw/%）
        let (h_str, viewport_w, viewport_h, parent_w) = {
            let vw = self.dimensions.content.width.max(1.0);
            // viewportは親のviewとみなす（簡易）
            (self.get_style_node().and_then(|s| s.value("height")).cloned(), vw, 600.0, vw)
        };

        if let Some(hs) = h_str.as_deref() {
            if let Some(h) = parse_length(hs, parent_w, viewport_w, viewport_h) {
                self.dimensions.content.height = h.max(0.0);
                return;
            }
        }

        // 何も指定がなければ children で決まる（layout_block_children が計算済み）
    }
    pub fn layout_with_font(&mut self, containing_block: Dimensions, font: &Font) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block_with_font(containing_block, font),
            BoxType::InlineNode(_) => self.layout_inline(containing_block),
            BoxType::Anonymous => {}
        }
    }

    fn layout_block_with_font(&mut self, containing_block: Dimensions, font: &Font) {
        self.calculate_block_model(containing_block.clone());
        self.calculate_block_width(containing_block.clone());
        self.calculate_block_position(containing_block.clone());
        self.layout_block_children_with_font(font);
        self.calculate_block_height_with_font(font);
    }

    fn layout_block_children_with_font(&mut self, font: &Font) {
        let mut y = self.dimensions.content.y;

        for child in &mut self.children {
            let mut cb = Dimensions::default();
            cb.content.x = self.dimensions.content.x;
            cb.content.y = y;
            cb.content.width = self.dimensions.content.width;
            cb.content.height = self.dimensions.content.height.max(1.0); // vh用

            child.layout_with_font(cb, font);

            y += child.dimensions.margin_box_height().max(0.0);
        }

        self.dimensions.content.height = (y - self.dimensions.content.y).max(0.0);
    }

    fn calculate_block_height_with_font(&mut self, font: &Font) {
        // 1) height指定があればそれを優先（既存ロジック）
        let (h_str, viewport_w, viewport_h, parent_w) = {
            let vw = self.dimensions.content.width.max(1.0);
            (
                self.get_style_node().and_then(|s| s.value("height")).cloned(),
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

        // 2) children の積み上げは layout_block_children_with_font が入れてる
        // 3) ★葉ブロックなら「折り返し行数×line-height」で高さを補う
        let is_block = matches!(self.box_type, BoxType::BlockNode(_));
        let has_block_child = self
            .children
            .iter()
            .any(|ch| matches!(ch.box_type, BoxType::BlockNode(_)));

        if is_block && !has_block_child {
            if let Some(sn) = self.get_style_node() {
                let mut buf = String::new();
                collect_text_no_blocks(self, &mut buf);

                let txt = buf.trim();
                if !txt.is_empty() {
                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);
                    let max_w = self.dimensions.content.width.max(1.0);

                    let lines = count_lines_fontdue(font, txt, max_w, font_size).max(1);
                    let text_h = (lines as f32) * line_h;

                    // 子の高さ(0のこと多い) と比べて大きい方を採用
                    self.dimensions.content.height = self.dimensions.content.height.max(text_h);
                }
            }
        }
    }
}

pub fn build_layout_tree(style_node: StyledNode) -> LayoutBox {
    let display = style_node.display();
    let mut root = LayoutBox::new(match display {
        Display::Block => BoxType::BlockNode(style_node.clone()),
        Display::Inline => BoxType::InlineNode(style_node.clone()),
        Display::None => BoxType::Anonymous,
    });

    for child in style_node.children {
        match child.display() {
            Display::None => {}
            Display::Block | Display::Inline => root.children.push(build_layout_tree(child)),
        }
    }
    root
}

// ---------------- helpers ----------------

/// px / vw / vh / % を解釈してピクセルにする
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

/// 4値 shorthand を “文字列のまま” 分解して返す（auto/px/vh/vw/%混在OK）
/// 1個: all, 2個: vertical/horizontal, 3個: top/horizontal/bottom, 4個: TRBL
fn parse_4len(s: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let parts = s
        .split_whitespace()
        .map(|p| p.trim().to_string())
        .collect::<Vec<_>>();

    match parts.len() {
        0 => (None, None, None, None),
        1 => (Some(parts[0].clone()), Some(parts[0].clone()), Some(parts[0].clone()), Some(parts[0].clone())),
        2 => (Some(parts[0].clone()), Some(parts[1].clone()), Some(parts[0].clone()), Some(parts[1].clone())),
        3 => (Some(parts[0].clone()), Some(parts[1].clone()), Some(parts[2].clone()), Some(parts[1].clone())),
        _ => (Some(parts[0].clone()), Some(parts[1].clone()), Some(parts[2].clone()), Some(parts[3].clone())),
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

/// ★display.rs と同じ思想：Blockをまたいで収集しない
fn collect_text_no_blocks(node: &LayoutBox, out: &mut String) {
    if let Some(sn) = node.get_style_node() {
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
                // head/title/meta/link も “画面に出さない” 対策
                if tag == "style" || tag == "script" || tag == "head" || tag == "title" || tag == "meta" || tag == "link" {
                    return;
                }
            }
        }
    }

    for child in &node.children {
        // 子が Block ならここでは集めない（子Blockが描画担当）
        if matches!(child.box_type, BoxType::BlockNode(_)) {
            continue;
        }
        collect_text_no_blocks(child, out);
    }
}

/// ★fontdue実測で「折り返し行数」だけ返す
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
