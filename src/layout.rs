use crate::constants::layout as layout_constants;
use crate::style::{Clear, Display, Float, Position, StyledNode};
use fontdue::Font;
use std::cmp::Ordering;

pub trait ImageSizeProvider {
    /// layout が持っている src (相対/絶対/ポート付きなど)を
    /// "キャッシュキーと同じ正規化済み絶対URL文字列" に変換する
    fn normalize_src_key(&self, src: &str) -> Option<String>;

    /// key (正規化済み絶対URL文字列) から自然サイズ(px)を返す
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlexWrap {
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignItems {
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
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

    fn node_display(&self) -> Option<Display> {
        self.get_style_node().map(|node| node.display())
    }

    fn is_inline_block_box(&self) -> bool {
        self.node_display() == Some(Display::InlineBlock)
    }

    fn is_flex_container(&self) -> bool {
        matches!(self.node_display(), Some(Display::Flex | Display::Grid))
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
            BoxType::BlockNode(_) => {
                if self.is_flex_container() {
                    self.layout_flex_container_with_context(
                        containing_block,
                        positioned_containing_block,
                        viewport,
                        font,
                        img_cache,
                    );
                } else {
                    self.layout_block_with_context(
                        containing_block,
                        positioned_containing_block,
                        viewport,
                        font,
                        img_cache,
                    );
                }
            }
            BoxType::InlineNode(_) => {
                if self.is_inline_block_box() {
                    self.layout_inline_block_with_context(
                        containing_block,
                        positioned_containing_block,
                        viewport,
                        font,
                        img_cache,
                    );
                } else {
                    self.layout_inline_leaf_fallback(containing_block.clone(), font, img_cache);
                    self.apply_relative_position_if_needed(&containing_block);
                }
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

    fn layout_flex_container_with_context(
        &mut self,
        containing_block: Dimensions,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let position = self.node_position();
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
        } else {
            self.calculate_block_width(containing_block.clone());
        }
        self.calculate_block_position(containing_block.clone());

        let child_positioned_containing_block = if position.is_positioned() {
            self.positioned_descendant_containing_block()
        } else {
            positioned_containing_block
        };

        self.layout_flex_children_with_context(
            child_positioned_containing_block,
            viewport.clone(),
            font,
            img_cache,
        );
        self.calculate_block_height_with_font(font, img_cache);
        self.position_flex_children();

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

    fn layout_inline_block_with_context(
        &mut self,
        containing_block: Dimensions,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.calculate_block_model(containing_block.clone());
        self.calculate_inline_block_width(containing_block.clone(), font, img_cache);
        self.calculate_block_position(containing_block.clone());

        let child_positioned_containing_block = if self.node_position().is_positioned() {
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
        sync_paint_fragments_with_content_rect(self);
        self.apply_relative_position_if_needed(&containing_block);
    }

    fn layout_flex_children_with_context(
        &mut self,
        positioned_containing_block: Dimensions,
        viewport: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let direction = flex_direction(self.get_style_node());
        let wrap = flex_wrap(self.get_style_node());
        let align_items = align_items(self.get_style_node());
        let viewport_w = viewport
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let viewport_h = viewport
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let container_x = self.dimensions.content.x;
        let container_y = self.dimensions.content.y;
        let container_w = self
            .dimensions
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let container_h = self
            .dimensions
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let main_gap = flex_main_gap_px(
            self.get_style_node(),
            direction,
            container_w,
            container_h,
            viewport_w,
            viewport_h,
        );
        let grid_columns = grid_template_column_count(self.get_style_node()).unwrap_or(0);

        let mut item_indices = Vec::new();
        let mut total_grow = 0.0f32;
        let mut main_total = 0.0f32;
        let mut cross_max = 0.0f32;

        for (idx, child) in self.children.iter_mut().enumerate() {
            let mut cb = Dimensions::default();
            cb.content.x = container_x;
            cb.content.y = container_y;
            cb.content.height = viewport_h;

            if child.node_position().is_out_of_flow() {
                cb.content.width = container_w;
                child.layout_with_context(
                    cb,
                    positioned_containing_block.clone(),
                    viewport.clone(),
                    font,
                    img_cache,
                );
                continue;
            }

            match direction {
                FlexDirection::Row => {
                    cb.content.width = if grid_columns > 0 {
                        let gaps_total = main_gap * grid_columns.saturating_sub(1) as f32;
                        ((container_w - gaps_total).max(0.0) / grid_columns as f32)
                            .max(layout_constants::MIN_LAYOUT_SIZE_PX)
                    } else {
                        flex_row_item_base_outer_width(
                            child,
                            font,
                            img_cache,
                            container_w,
                            viewport_w,
                            viewport_h,
                        )
                        .max(layout_constants::MIN_LAYOUT_SIZE_PX)
                    };
                }
                FlexDirection::Column => {
                    cb.content.width = flex_column_item_outer_width(
                        child,
                        font,
                        img_cache,
                        container_w,
                        viewport_w,
                        viewport_h,
                        align_items == AlignItems::Stretch && wrap == FlexWrap::NoWrap,
                    )
                    .max(layout_constants::MIN_LAYOUT_SIZE_PX);
                }
            }

            child.layout_with_context(
                cb,
                positioned_containing_block.clone(),
                viewport.clone(),
                font,
                img_cache,
            );

            let grow = if direction == FlexDirection::Row
                && !has_explicit_width(child, container_w, viewport_w, viewport_h)
            {
                flex_grow(child.get_style_node())
            } else {
                0.0
            };

            item_indices.push(idx);
            total_grow += grow;
            main_total += flex_item_main_size(child, direction);
            cross_max = cross_max.max(flex_item_cross_size(child, direction));
        }

        if direction == FlexDirection::Row
            && wrap == FlexWrap::NoWrap
            && total_grow > 0.0
            && !item_indices.is_empty()
        {
            let gaps_total = main_gap * (item_indices.len().saturating_sub(1) as f32);
            let free_space = (container_w - main_total - gaps_total).max(0.0);

            if free_space > 0.5 {
                for idx in &item_indices {
                    let grow = {
                        let child = &self.children[*idx];
                        if has_explicit_width(child, container_w, viewport_w, viewport_h) {
                            0.0
                        } else {
                            flex_grow(child.get_style_node())
                        }
                    };

                    if grow <= 0.0 {
                        continue;
                    }

                    let extra = free_space * (grow / total_grow);
                    let target_outer_w = {
                        let child = &self.children[*idx];
                        child.dimensions.margin_box_width() + extra
                    };

                    let mut cb = Dimensions::default();
                    cb.content.x = container_x;
                    cb.content.y = container_y;
                    cb.content.width = target_outer_w.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                    cb.content.height = viewport_h;

                    self.children[*idx].layout_with_context(
                        cb,
                        positioned_containing_block.clone(),
                        viewport.clone(),
                        font,
                        img_cache,
                    );
                }

                main_total = 0.0;
                cross_max = 0.0;
                for idx in &item_indices {
                    let child = &self.children[*idx];
                    main_total += flex_item_main_size(child, direction);
                    cross_max = cross_max.max(flex_item_cross_size(child, direction));
                }
            }
        }

        self.dimensions.content.height = match direction {
            FlexDirection::Row if wrap == FlexWrap::Wrap => {
                let cross_gap = flex_cross_gap_px(
                    self.get_style_node(),
                    direction,
                    container_w,
                    container_h,
                    viewport_w,
                    viewport_h,
                );
                let lines = flex_lines(&self.children, direction, wrap, container_w, main_gap);
                let gaps_total = cross_gap * (lines.len().saturating_sub(1) as f32);
                (lines.iter().map(|line| line.cross_size).sum::<f32>() + gaps_total).max(0.0)
            }
            FlexDirection::Row => cross_max.max(0.0),
            FlexDirection::Column => {
                let gaps_total = main_gap * (item_indices.len().saturating_sub(1) as f32);
                (main_total + gaps_total).max(0.0)
            }
        };
    }

    fn position_flex_children(&mut self) {
        let direction = flex_direction(self.get_style_node());
        let wrap = flex_wrap(self.get_style_node());
        let justify = justify_content(self.get_style_node());
        let align = align_items(self.get_style_node());
        let container_x = self.dimensions.content.x;
        let container_y = self.dimensions.content.y;
        let container_main = match direction {
            FlexDirection::Row => self.dimensions.content.width,
            FlexDirection::Column => self.dimensions.content.height,
        };
        let container_cross = match direction {
            FlexDirection::Row => self.dimensions.content.height,
            FlexDirection::Column => self.dimensions.content.width,
        };
        let gap = flex_main_gap_px(
            self.get_style_node(),
            direction,
            self.dimensions
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );
        let cross_gap = flex_cross_gap_px(
            self.get_style_node(),
            direction,
            self.dimensions
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            self.dimensions
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );

        let in_flow_count = self
            .children
            .iter()
            .filter(|child| !child.node_position().is_out_of_flow())
            .count();

        if in_flow_count == 0 {
            return;
        }

        if wrap == FlexWrap::Wrap {
            let lines = flex_lines(
                &self.children,
                direction,
                wrap,
                container_main.max(layout_constants::MIN_LAYOUT_SIZE_PX),
                gap,
            );
            let mut cross_cursor = 0.0;

            for line in lines {
                let free_main = (container_main - line.main_size).max(0.0);
                let (leading_space, between_extra) =
                    justify_distribution(justify, free_main, line.indices.len());

                let mut main_cursor = leading_space;
                for index in line.indices {
                    let child = &mut self.children[index];
                    let outer_main = flex_item_main_size(child, direction);
                    let outer_cross = flex_item_cross_size(child, direction);
                    let free_cross = (line.cross_size - outer_cross).max(0.0);
                    let cross_offset = match child_align_items(child.get_style_node(), align) {
                        AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                        AlignItems::FlexEnd => free_cross,
                        AlignItems::Center => free_cross / 2.0,
                    };

                    let (target_x, target_y) = match direction {
                        FlexDirection::Row => (
                            container_x + main_cursor,
                            container_y + cross_cursor + cross_offset,
                        ),
                        FlexDirection::Column => (
                            container_x + cross_cursor + cross_offset,
                            container_y + main_cursor,
                        ),
                    };
                    child.shift_to_margin_box(target_x, target_y);

                    main_cursor += outer_main + gap + between_extra;
                }

                cross_cursor += line.cross_size + cross_gap;
            }

            return;
        }

        let used_main = self
            .children
            .iter()
            .filter(|child| !child.node_position().is_out_of_flow())
            .map(|child| flex_item_main_size(child, direction))
            .sum::<f32>()
            + gap * (in_flow_count.saturating_sub(1) as f32);
        let free_main = (container_main - used_main).max(0.0);
        let (leading_space, between_extra) =
            justify_distribution(justify, free_main, in_flow_count);

        let mut cursor = leading_space;
        for child in &mut self.children {
            if child.node_position().is_out_of_flow() {
                continue;
            }

            let outer_main = flex_item_main_size(child, direction);
            let outer_cross = flex_item_cross_size(child, direction);
            let free_cross = (container_cross - outer_cross).max(0.0);
            let cross_offset = match child_align_items(child.get_style_node(), align) {
                AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                AlignItems::FlexEnd => free_cross,
                AlignItems::Center => free_cross / 2.0,
            };

            let (target_x, target_y) = match direction {
                FlexDirection::Row => (container_x + cursor, container_y + cross_offset),
                FlexDirection::Column => (container_x + cross_offset, container_y + cursor),
            };
            child.shift_to_margin_box(target_x, target_y);

            cursor += outer_main + gap + between_extra;
        }
    }

    fn layout_inline_leaf_fallback(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        self.calculate_block_model(containing_block.clone());
        let style_node = self.get_style_node();

        let (font_size, line_h, text_opt, img_opt) = if let Some(sn) = style_node {
            let fs = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
            let lh = line_height_px(sn, fs);

            match &sn.node.node_type {
                crate::dom::NodeType::Text(t) => (fs, lh, Some(t.clone()), None),
                crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                    let (w, h) = img_intrinsic_size_px(sn, img_cache);
                    (fs, lh, None, Some((w, h)))
                }
                crate::dom::NodeType::Element(ed) if ed.tag_name == "input" => {
                    let (w, h) = input_intrinsic_size_px(sn);
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
            let viewport_w = containing_block
                .content
                .width
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);
            let viewport_h = containing_block
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);
            let containing_h = containing_block
                .content
                .height
                .max(layout_constants::MIN_LAYOUT_SIZE_PX);
            let (min_height, max_height) =
                parse_height_constraints(style_node, containing_h, viewport_w, viewport_h);
            let d = &mut self.dimensions;
            d.content.x = containing_block.content.x;
            d.content.y = containing_block.content.y;
            d.content.width = iw.max(layout_constants::MIN_LAYOUT_SIZE_PX).min(
                containing_block
                    .content
                    .width
                    .max(layout_constants::MIN_LAYOUT_SIZE_PX),
            );
            d.content.height = clamp_height_to_constraints(
                ih.max(layout_constants::MIN_LAYOUT_SIZE_PX),
                min_height,
                max_height,
            );
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

    fn calculate_block_model(&mut self, containing: Dimensions) {
        let viewport_w = containing.content.width;
        let viewport_h = containing
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = containing.content.width;
        let style_node = self.get_style_node();

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
            blw_s,
            brw_s,
            btw_s,
            bbw_s,
            border_width_sh,
            border_radius_sh,
            border_tl_radius_s,
            border_tr_radius_s,
            border_br_radius_s,
            border_bl_radius_s,
        ) = if let Some(style) = style_node {
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
                style.value("border-left-width").cloned(),
                style.value("border-right-width").cloned(),
                style.value("border-top-width").cloned(),
                style.value("border-bottom-width").cloned(),
                style.value("border-width").cloned(),
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

        let mut bl = 0.0;
        let mut br = 0.0;
        let mut bt = 0.0;
        let mut bb = 0.0;

        if let Some(v) = ml_s.as_deref() {
            if v.trim() != "auto" {
                ml = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mr_s.as_deref() {
            if v.trim() != "auto" {
                mr = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
            }
        }
        if let Some(v) = mt_s.as_deref() {
            mt = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = mb_s.as_deref() {
            mb = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        if let Some(v) = pl_s.as_deref() {
            pl = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pr_s.as_deref() {
            pr = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pt_s.as_deref() {
            pt = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = pb_s.as_deref() {
            pb = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        if let Some(v) = blw_s.as_deref() {
            bl = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = brw_s.as_deref() {
            br = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = btw_s.as_deref() {
            bt = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }
        if let Some(v) = bbw_s.as_deref() {
            bb = parse_length(style_node, v, parent_w, viewport_w, viewport_h).unwrap_or(0.0);
        }

        if ml_s.is_none() && mr_s.is_none() && mt_s.is_none() && mb_s.is_none() {
            if let Some(sh) = margin_sh.as_deref() {
                let m = parse_4len(sh);
                if let Some(top) = m.0.as_deref() {
                    mt = parse_length(style_node, top, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(right) = m.1.as_deref() {
                    if right.trim() != "auto" {
                        mr = parse_length(style_node, right, parent_w, viewport_w, viewport_h)
                            .unwrap_or(0.0);
                    }
                }
                if let Some(bottom) = m.2.as_deref() {
                    mb = parse_length(style_node, bottom, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(left) = m.3.as_deref() {
                    if left.trim() != "auto" {
                        ml = parse_length(style_node, left, parent_w, viewport_w, viewport_h)
                            .unwrap_or(0.0);
                    }
                }
            }
        }

        if pl_s.is_none() && pr_s.is_none() && pt_s.is_none() && pb_s.is_none() {
            if let Some(sh) = padding_sh.as_deref() {
                let p = parse_4len(sh);
                if let Some(top) = p.0.as_deref() {
                    pt = parse_length(style_node, top, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(right) = p.1.as_deref() {
                    pr = parse_length(style_node, right, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(bottom) = p.2.as_deref() {
                    pb = parse_length(style_node, bottom, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(left) = p.3.as_deref() {
                    pl = parse_length(style_node, left, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
            }
        }

        if blw_s.is_none() && brw_s.is_none() && btw_s.is_none() && bbw_s.is_none() {
            if let Some(sh) = border_width_sh.as_deref() {
                let b = parse_4len(sh);
                if let Some(top) = b.0.as_deref() {
                    bt = parse_length(style_node, top, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(right) = b.1.as_deref() {
                    br = parse_length(style_node, right, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(bottom) = b.2.as_deref() {
                    bb = parse_length(style_node, bottom, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
                if let Some(left) = b.3.as_deref() {
                    bl = parse_length(style_node, left, parent_w, viewport_w, viewport_h)
                        .unwrap_or(0.0);
                }
            }
        }

        let mut border_radius = CornerRadii::default();
        if let Some(v) = border_radius_sh.as_deref() {
            if let Some(r) =
                parse_border_radius_shorthand(style_node, v, parent_w, viewport_w, viewport_h)
            {
                border_radius = r;
            }
        }
        if let Some(v) = border_tl_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(style_node, v, parent_w, viewport_w, viewport_h) {
                border_radius.top_left = r;
            }
        }
        if let Some(v) = border_tr_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(style_node, v, parent_w, viewport_w, viewport_h) {
                border_radius.top_right = r;
            }
        }
        if let Some(v) = border_br_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(style_node, v, parent_w, viewport_w, viewport_h) {
                border_radius.bottom_right = r;
            }
        }
        if let Some(v) = border_bl_radius_s.as_deref() {
            if let Some(r) = parse_corner_radius(style_node, v, parent_w, viewport_w, viewport_h) {
                border_radius.bottom_left = r;
            }
        }

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
        let style_node = self.get_style_node();
        let (min_width, max_width) =
            parse_width_constraints(style_node, parent_w, viewport_w, viewport_h);

        let width_str = style_node.and_then(|s| s.value("width")).cloned();

        let (ml_auto, mr_auto) = style_node
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
        let resolved_width = width_str
            .as_deref()
            .and_then(|ws| parse_length(style_node, ws, parent_w, viewport_w, viewport_h));
        let has_explicit_width = resolved_width.is_some();

        let d = &mut self.dimensions;

        if let Some(w) = resolved_width {
            d.content.width = w.max(0.0);
        }

        if !has_explicit_width {
            let available = containing_block.content.width
                - d.margin.left
                - d.margin.right
                - d.padding.left
                - d.padding.right
                - d.border.left
                - d.border.right;
            d.content.width = available.max(0.0);
        }

        d.content.width = clamp_width_to_constraints(d.content.width, min_width, max_width);

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
        let style_node = self.get_style_node();
        let (min_width, max_width) =
            parse_width_constraints(style_node, parent_w, viewport_w, viewport_h);

        let width_str = style_node.and_then(|s| s.value("width")).cloned();
        let insets = self.specified_insets(parent_w, viewport_h, viewport_w, viewport_h);
        let resolved_width = width_str
            .as_deref()
            .and_then(|ws| parse_length(style_node, ws, parent_w, viewport_w, viewport_h));
        let d = &mut self.dimensions;

        if let Some(w) = resolved_width {
            d.content.width = clamp_width_to_constraints(w.max(0.0), min_width, max_width);
            return;
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
            d.content.width = clamp_width_to_constraints(available.max(0.0), min_width, max_width);
            return;
        }

        let available = positioning_block.content.width
            - d.margin.left
            - d.margin.right
            - d.padding.left
            - d.padding.right
            - d.border.left
            - d.border.right;
        d.content.width = clamp_width_to_constraints(available.max(0.0), min_width, max_width);
    }

    fn calculate_float_block_width(
        &mut self,
        containing_block: Dimensions,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let viewport_w = containing_block.content.width;
        let viewport_h = containing_block
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = containing_block.content.width;
        let style_node = self.get_style_node();
        let (min_width, max_width) =
            parse_width_constraints(style_node, parent_w, viewport_w, viewport_h);
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
                    d.content.width =
                        clamp_width_to_constraints(d.content.width, min_width, max_width);
                }
            }
        }
    }

    fn calculate_inline_block_width(
        &mut self,
        containing_block: Dimensions,
        font: &Font,
        img_cache: &dyn ImageSizeProvider,
    ) {
        let viewport_w = containing_block.content.width;
        let viewport_h = containing_block
            .content
            .height
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let parent_w = containing_block.content.width;
        let style_node = self.get_style_node();
        let (min_width, max_width) =
            parse_width_constraints(style_node, parent_w, viewport_w, viewport_h);

        let width_str = style_node.and_then(|s| s.value("width")).cloned();

        if let Some(ws) = width_str.as_deref() {
            if let Some(w) = parse_length(style_node, ws, parent_w, viewport_w, viewport_h) {
                self.dimensions.content.width =
                    clamp_width_to_constraints(w.max(0.0), min_width, max_width);
                return;
            }
        }

        let available = (containing_block.content.width
            - self.dimensions.margin.left
            - self.dimensions.margin.right
            - self.dimensions.padding.left
            - self.dimensions.padding.right
            - self.dimensions.border.left
            - self.dimensions.border.right)
            .max(0.0);

        let estimated = estimate_layout_box_content_width(
            self, font, img_cache, available, viewport_w, viewport_h,
        )
        .min(available)
        .max(0.0);

        self.dimensions.content.width = clamp_width_to_constraints(estimated, min_width, max_width);
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
        let style_node = self.get_style_node();
        let viewport_w = self
            .dimensions
            .content
            .width
            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
        let viewport_h = layout_constants::DEFAULT_VIEWPORT_HEIGHT_PX;
        let parent_h = viewport_w;
        let (min_height, max_height) =
            parse_height_constraints(style_node, parent_h, viewport_w, viewport_h);
        let mut resolved_height = style_node
            .and_then(|s| s.value("height"))
            .and_then(|value| parse_length(style_node, value, parent_h, viewport_w, viewport_h))
            .map(|height| height.max(0.0))
            .unwrap_or(self.dimensions.content.height.max(0.0));

        let has_any_child = !self.children.is_empty();
        if !has_any_child {
            if let Some(sn) = style_node {
                if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                    if ed.tag_name == "img" {
                        let (_iw, ih) = img_intrinsic_size_px(sn, img_cache);
                        resolved_height = ih.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                    } else if ed.tag_name == "input" {
                        let (_iw, ih) = input_intrinsic_size_px(sn);
                        resolved_height = ih.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                    } else {
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
                            resolved_height = resolved_height.max(text_h);
                        }
                    }
                }
            }
        }

        self.dimensions.content.height =
            clamp_height_to_constraints(resolved_height, min_height, max_height);
    }

    /// IFC: anonymous block の中の inline subtree を "行に詰める"
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
            let is_inline_block = node.is_inline_block_box();

            match &mut node.box_type {
                BoxType::InlineNode(_) if is_inline_block => {
                    node.calculate_block_model(containing_block.clone());

                    let estimated_outer_w = estimate_inline_atomic_outer_width(
                        node,
                        font,
                        img_cache,
                        max_w,
                        viewport.content.width,
                        viewport
                            .content
                            .height
                            .max(layout_constants::MIN_LAYOUT_SIZE_PX),
                    );

                    consume_pending_space_before_item(
                        estimated_outer_w,
                        start_x,
                        max_w,
                        cursor_x,
                        cursor_y,
                        current_line_h,
                        pending_space_w,
                        pending_space_h,
                    );

                    if *cursor_x > start_x && *cursor_x + estimated_outer_w > start_x + max_w {
                        advance_to_next_line(
                            cursor_x,
                            cursor_y,
                            current_line_h,
                            start_x,
                            layout_constants::MIN_LINE_HEIGHT_PX,
                        );
                    }

                    let mut cb = Dimensions::default();
                    cb.content.x = *cursor_x;
                    cb.content.y = *cursor_y;
                    cb.content.width = max_w;
                    cb.content.height = viewport
                        .content
                        .height
                        .max(layout_constants::MIN_LAYOUT_SIZE_PX);

                    node.layout_with_context(
                        cb,
                        positioned_containing_block.clone(),
                        viewport.clone(),
                        font,
                        img_cache,
                    );

                    *cursor_x += node.dimensions.margin_box_width().max(0.0);
                    *current_line_h =
                        (*current_line_h).max(node.dimensions.margin_box_height().max(0.0));
                }
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
                            crate::dom::NodeType::Element(ed) if ed.tag_name == "input" => {
                                let (w, h) = input_intrinsic_size_px(sn);
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
                        let viewport_w = max_w.max(layout_constants::MIN_LAYOUT_SIZE_PX);
                        let viewport_h = viewport
                            .content
                            .height
                            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
                        let containing_h = containing_block
                            .content
                            .height
                            .max(layout_constants::MIN_LAYOUT_SIZE_PX);
                        let (min_height, max_height) = parse_height_constraints(
                            node.get_style_node(),
                            containing_h,
                            viewport_w,
                            viewport_h,
                        );
                        let ih = clamp_height_to_constraints(
                            ih.max(layout_constants::MIN_LAYOUT_SIZE_PX),
                            min_height,
                            max_height,
                        );

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
                parse_inset_length(Some(sn), &value, vertical_basis, viewport_w, viewport_h)
            }),
            right: specified_inset(sn, InsetEdge::Right).and_then(|value| {
                parse_inset_length(Some(sn), &value, horizontal_basis, viewport_w, viewport_h)
            }),
            bottom: specified_inset(sn, InsetEdge::Bottom).and_then(|value| {
                parse_inset_length(Some(sn), &value, vertical_basis, viewport_w, viewport_h)
            }),
            left: specified_inset(sn, InsetEdge::Left).and_then(|value| {
                parse_inset_length(Some(sn), &value, horizontal_basis, viewport_w, viewport_h)
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
        let style_node = self.get_style_node();
        let (min_height, max_height) =
            parse_height_constraints(style_node, viewport_w, viewport_w, viewport_h);

        let resolved_height = style_node
            .and_then(|s| s.value("height"))
            .and_then(|value| parse_length(style_node, value, viewport_w, viewport_w, viewport_h))
            .map(|height| height.max(0.0))
            .unwrap_or(self.dimensions.content.height.max(0.0));
        let content_height = clamp_height_to_constraints(resolved_height, min_height, max_height);

        containing.content.height =
            content_height + self.dimensions.padding.top + self.dimensions.padding.bottom;

        containing
    }
}

pub fn build_layout_tree(style_node: &StyledNode) -> LayoutBox<'_> {
    build_layout_tree_with_mode(style_node, false)
}

fn build_layout_tree_with_mode(style_node: &StyledNode, force_block_root: bool) -> LayoutBox<'_> {
    let display = style_node.display();
    let layout_display = if force_block_root {
        blockified_display_for(style_node)
    } else {
        layout_display_for(style_node)
    };

    let mut root = LayoutBox::new(match layout_display {
        Display::Block | Display::Flex | Display::Grid => BoxType::BlockNode(style_node),
        Display::Inline | Display::InlineBlock => BoxType::InlineNode(style_node),
        Display::None => BoxType::Anonymous,
    });

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

    if matches!(layout_display, Display::Flex | Display::Grid) {
        for child in &style_node.children {
            append_flex_child(&mut root, child);
        }
        return root;
    }

    let mut anon: Option<LayoutBox<'_>> = None;

    for child in &style_node.children {
        match layout_display_for(child) {
            Display::None => {}
            Display::Block | Display::Flex | Display::Grid => {
                if let Some(a) = anon.take() {
                    root.children.push(a);
                }
                root.children.push(build_layout_tree(child));
            }
            Display::Inline | Display::InlineBlock => {
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

fn append_flex_child<'a>(root: &mut LayoutBox<'a>, child: &'a StyledNode) {
    if child.display() == Display::None {
        return;
    }

    if child.position().is_out_of_flow() {
        root.children.push(build_layout_tree(child));
        return;
    }

    match &child.node.node_type {
        crate::dom::NodeType::Text(_) => {
            let mut anon = LayoutBox::new(BoxType::Anonymous);
            anon.children.push(build_layout_tree(child));
            root.children.push(anon);
        }
        crate::dom::NodeType::Element(_) => match child.display() {
            Display::None => {}
            Display::Block | Display::Flex | Display::Grid => {
                root.children.push(build_layout_tree(child))
            }
            Display::Inline | Display::InlineBlock => {
                root.children.push(build_layout_tree_with_mode(child, true));
            }
        },
    }
}

fn layout_display_for(style_node: &StyledNode) -> Display {
    let display = style_node.display();
    if matches!(display, Display::Flex | Display::Grid) {
        display
    } else if display != Display::None
        && (style_node.position().is_out_of_flow() || style_node.float().is_floating())
    {
        Display::Block
    } else {
        display
    }
}

fn blockified_display_for(style_node: &StyledNode) -> Display {
    match &style_node.node.node_type {
        crate::dom::NodeType::Text(_) => Display::Inline,
        crate::dom::NodeType::Element(_) => match style_node.display() {
            Display::None => Display::None,
            Display::Flex | Display::Grid => style_node.display(),
            Display::Inline | Display::InlineBlock | Display::Block => Display::Block,
        },
    }
}

fn flex_direction(sn: Option<&StyledNode>) -> FlexDirection {
    if let Some(direction) =
        sn.and_then(|node| node.value("flex-direction").map(|value| value.trim()))
    {
        return match direction {
            "column" => FlexDirection::Column,
            _ => FlexDirection::Row,
        };
    }

    if sn
        .and_then(|node| node.value("flex-flow"))
        .is_some_and(|value| value.split_whitespace().any(|token| token == "column"))
    {
        FlexDirection::Column
    } else {
        FlexDirection::Row
    }
}

fn flex_wrap(sn: Option<&StyledNode>) -> FlexWrap {
    if sn.is_some_and(|node| node.display() == Display::Grid) {
        return FlexWrap::Wrap;
    }

    if let Some(wrap) = sn.and_then(|node| node.value("flex-wrap").map(|value| value.trim())) {
        return match wrap {
            "wrap" | "wrap-reverse" => FlexWrap::Wrap,
            _ => FlexWrap::NoWrap,
        };
    }

    if sn
        .and_then(|node| node.value("flex-flow"))
        .is_some_and(|value| {
            value
                .split_whitespace()
                .any(|token| matches!(token, "wrap" | "wrap-reverse"))
        })
    {
        FlexWrap::Wrap
    } else {
        FlexWrap::NoWrap
    }
}

fn grid_template_column_count(sn: Option<&StyledNode>) -> Option<usize> {
    let style_node = sn?;
    if style_node.display() != Display::Grid {
        return None;
    }

    let template = style_node.value("grid-template-columns")?;
    let template = template.trim();
    if template.is_empty() || template == "none" {
        return None;
    }

    if let Some(count) = parse_grid_repeat_count(template) {
        return Some(count.max(1));
    }

    let count = template
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .count();
    (count > 0).then_some(count)
}

fn parse_grid_repeat_count(template: &str) -> Option<usize> {
    let start = template.find("repeat(")? + "repeat(".len();
    let rest = &template[start..];
    let first = rest.split(',').next()?.trim();

    first.parse::<usize>().ok()
}

fn justify_content(sn: Option<&StyledNode>) -> JustifyContent {
    match sn.and_then(|node| node.value("justify-content").map(|value| value.trim())) {
        Some("flex-end") | Some("end") => JustifyContent::FlexEnd,
        Some("center") => JustifyContent::Center,
        Some("space-between") => JustifyContent::SpaceBetween,
        Some("space-around") => JustifyContent::SpaceAround,
        Some("space-evenly") => JustifyContent::SpaceEvenly,
        _ => JustifyContent::FlexStart,
    }
}

fn align_items(sn: Option<&StyledNode>) -> AlignItems {
    match sn.and_then(|node| node.value("align-items").map(|value| value.trim())) {
        Some("flex-start") | Some("start") => AlignItems::FlexStart,
        Some("flex-end") | Some("end") => AlignItems::FlexEnd,
        Some("center") => AlignItems::Center,
        Some("stretch") => AlignItems::Stretch,
        _ => AlignItems::Stretch,
    }
}

fn child_align_items(sn: Option<&StyledNode>, container_align: AlignItems) -> AlignItems {
    match sn.and_then(|node| node.value("align-self").map(|value| value.trim())) {
        Some("auto") | None => container_align,
        Some("flex-start") | Some("start") => AlignItems::FlexStart,
        Some("flex-end") | Some("end") => AlignItems::FlexEnd,
        Some("center") => AlignItems::Center,
        Some("stretch") => AlignItems::Stretch,
        _ => container_align,
    }
}

fn flex_grow(sn: Option<&StyledNode>) -> f32 {
    let Some(style_node) = sn else {
        return 0.0;
    };

    if let Some(value) = style_node
        .value("flex-grow")
        .and_then(|value| value.trim().parse::<f32>().ok())
    {
        return value.max(0.0);
    }

    style_node
        .value("flex")
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value.max(0.0))
        .unwrap_or(0.0)
}

fn flex_basis_px(
    sn: Option<&StyledNode>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    let style_node = sn?;

    if let Some(value) = style_node.value("flex-basis") {
        let value = value.trim();
        if !value.eq_ignore_ascii_case("auto") {
            return parse_length(Some(style_node), value, containing, viewport_w, viewport_h)
                .map(|value| value.max(0.0));
        }
    }

    let flex = style_node.value("flex")?;
    let tokens = flex
        .split_whitespace()
        .map(|token| token.trim())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.is_empty()
        || tokens
            .iter()
            .any(|token| *token == "none" || *token == "auto")
    {
        return None;
    }

    let basis = if tokens.len() >= 3 {
        tokens[2]
    } else if tokens.len() == 2 && is_flex_basis_token(tokens[1]) {
        tokens[1]
    } else if tokens.len() == 1 && is_flex_basis_token(tokens[0]) {
        tokens[0]
    } else {
        return None;
    };

    parse_length(Some(style_node), basis, containing, viewport_w, viewport_h)
        .map(|value| value.max(0.0))
}

fn is_flex_basis_token(token: &str) -> bool {
    let token = token.trim();
    token == "0"
        || token.ends_with("px")
        || token.ends_with('%')
        || token.ends_with("em")
        || token.ends_with("rem")
        || token.ends_with("vw")
        || token.ends_with("vh")
}

fn has_explicit_width(
    node: &LayoutBox<'_>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> bool {
    resolved_content_width(node.get_style_node(), containing, viewport_w, viewport_h).is_some()
}

fn resolved_content_width(
    sn: Option<&StyledNode>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    let style_node = sn?;
    style_node
        .value("width")
        .and_then(|value| parse_length(Some(style_node), value, containing, viewport_w, viewport_h))
        .map(|value| value.max(0.0))
}

fn parse_gap_pair(value: &str) -> (Option<String>, Option<String>) {
    let parts = value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<_>>();

    match parts.len() {
        0 => (None, None),
        1 => (Some(parts[0].clone()), Some(parts[0].clone())),
        _ => (Some(parts[0].clone()), Some(parts[1].clone())),
    }
}

fn flex_main_gap_px(
    sn: Option<&StyledNode>,
    direction: FlexDirection,
    main_basis: f32,
    _cross_basis: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let Some(style_node) = sn else {
        return 0.0;
    };

    let (gap_row_sh, gap_col_sh) = style_node
        .value("gap")
        .map(|value| parse_gap_pair(value))
        .unwrap_or((None, None));
    let row_gap = style_node.value("row-gap").cloned().or(gap_row_sh);
    let col_gap = style_node.value("column-gap").cloned().or(gap_col_sh);
    let value = match direction {
        FlexDirection::Row => col_gap,
        FlexDirection::Column => row_gap,
    };

    value
        .as_deref()
        .and_then(|gap| parse_length(Some(style_node), gap, main_basis, viewport_w, viewport_h))
        .unwrap_or(0.0)
        .max(0.0)
}

fn flex_cross_gap_px(
    sn: Option<&StyledNode>,
    direction: FlexDirection,
    _main_basis: f32,
    cross_basis: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let Some(style_node) = sn else {
        return 0.0;
    };

    let (gap_row_sh, gap_col_sh) = style_node
        .value("gap")
        .map(|value| parse_gap_pair(value))
        .unwrap_or((None, None));
    let row_gap = style_node.value("row-gap").cloned().or(gap_row_sh);
    let col_gap = style_node.value("column-gap").cloned().or(gap_col_sh);
    let value = match direction {
        FlexDirection::Row => row_gap,
        FlexDirection::Column => col_gap,
    };

    value
        .as_deref()
        .and_then(|gap| parse_length(Some(style_node), gap, cross_basis, viewport_w, viewport_h))
        .unwrap_or(0.0)
        .max(0.0)
}

fn flex_lines(
    children: &[LayoutBox<'_>],
    direction: FlexDirection,
    wrap: FlexWrap,
    main_limit: f32,
    gap: f32,
) -> Vec<FlexLine> {
    let mut lines = Vec::new();
    let mut current = FlexLine {
        indices: Vec::new(),
        main_size: 0.0,
        cross_size: 0.0,
    };

    for (index, child) in children.iter().enumerate() {
        if child.node_position().is_out_of_flow() {
            continue;
        }

        let item_main = flex_item_main_size(child, direction);
        let item_cross = flex_item_cross_size(child, direction);
        let next_main = if current.indices.is_empty() {
            item_main
        } else {
            current.main_size + gap + item_main
        };

        if wrap == FlexWrap::Wrap
            && !current.indices.is_empty()
            && next_main > main_limit.max(layout_constants::MIN_LAYOUT_SIZE_PX) + 0.5
        {
            lines.push(current);
            current = FlexLine {
                indices: Vec::new(),
                main_size: 0.0,
                cross_size: 0.0,
            };
        }

        if current.indices.is_empty() {
            current.main_size = item_main;
        } else {
            current.main_size += gap + item_main;
        }
        current.cross_size = current.cross_size.max(item_cross);
        current.indices.push(index);
    }

    if !current.indices.is_empty() {
        lines.push(current);
    }

    lines
}

fn flex_row_item_base_outer_width(
    node: &LayoutBox<'_>,
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let extras = node
        .get_style_node()
        .map(|sn| horizontal_box_model_width(sn, containing_width, viewport_w, viewport_h))
        .unwrap_or(0.0);

    if let Some(basis) = flex_basis_px(
        node.get_style_node(),
        containing_width,
        viewport_w,
        viewport_h,
    ) {
        return (basis + extras).max(layout_constants::MIN_LAYOUT_SIZE_PX);
    }

    if let Some(width) = resolved_content_width(
        node.get_style_node(),
        containing_width,
        viewport_w,
        viewport_h,
    ) {
        return (width + extras).max(layout_constants::MIN_LAYOUT_SIZE_PX);
    }

    estimate_layout_box_outer_width(
        node,
        font,
        img_cache,
        containing_width,
        viewport_w,
        viewport_h,
    )
    .max((extras + layout_constants::MIN_LAYOUT_SIZE_PX).max(layout_constants::MIN_LAYOUT_SIZE_PX))
}

fn flex_column_item_outer_width(
    node: &LayoutBox<'_>,
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
    stretch_cross: bool,
) -> f32 {
    let extras = node
        .get_style_node()
        .map(|sn| horizontal_box_model_width(sn, containing_width, viewport_w, viewport_h))
        .unwrap_or(0.0);

    if let Some(width) = resolved_content_width(
        node.get_style_node(),
        containing_width,
        viewport_w,
        viewport_h,
    ) {
        return (width + extras).max(layout_constants::MIN_LAYOUT_SIZE_PX);
    }

    if stretch_cross {
        return containing_width.max(layout_constants::MIN_LAYOUT_SIZE_PX);
    }

    estimate_layout_box_outer_width(
        node,
        font,
        img_cache,
        containing_width,
        viewport_w,
        viewport_h,
    )
    .max((extras + layout_constants::MIN_LAYOUT_SIZE_PX).max(layout_constants::MIN_LAYOUT_SIZE_PX))
}

fn flex_item_main_size(node: &LayoutBox<'_>, direction: FlexDirection) -> f32 {
    match direction {
        FlexDirection::Row => node.dimensions.margin_box_width(),
        FlexDirection::Column => node.dimensions.margin_box_height(),
    }
}

fn flex_item_cross_size(node: &LayoutBox<'_>, direction: FlexDirection) -> f32 {
    match direction {
        FlexDirection::Row => node.dimensions.margin_box_height(),
        FlexDirection::Column => node.dimensions.margin_box_width(),
    }
}

fn justify_distribution(justify: JustifyContent, free_space: f32, item_count: usize) -> (f32, f32) {
    if item_count == 0 {
        return (0.0, 0.0);
    }

    match justify {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (free_space, 0.0),
        JustifyContent::Center => (free_space / 2.0, 0.0),
        JustifyContent::SpaceBetween if item_count > 1 => {
            (0.0, free_space / (item_count.saturating_sub(1) as f32))
        }
        JustifyContent::SpaceAround => {
            let between = free_space / (item_count as f32);
            (between / 2.0, between)
        }
        JustifyContent::SpaceEvenly => {
            let between = free_space / ((item_count + 1) as f32);
            (between, between)
        }
        _ => (0.0, 0.0),
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
    sn: Option<&StyledNode>,
    value: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    let t = value.trim();
    if t.eq_ignore_ascii_case("auto") {
        return None;
    }

    parse_length(sn, t, containing, viewport_w, viewport_h)
}

fn parse_length(
    sn: Option<&StyledNode>,
    s: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    if let Some(style_node) = sn {
        return style_node.resolve_length_px(s, containing, viewport_w, viewport_h);
    }

    crate::style::resolve_css_length(
        s,
        containing,
        viewport_w,
        viewport_h,
        layout_constants::DEFAULT_FONT_SIZE_PX,
        layout_constants::DEFAULT_FONT_SIZE_PX,
    )
}

fn parse_width_constraints(
    sn: Option<&StyledNode>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> (Option<f32>, Option<f32>) {
    let Some(style_node) = sn else {
        return (None, None);
    };

    let min_width = style_node
        .value("min-width")
        .and_then(|value| parse_length(Some(style_node), value, containing, viewport_w, viewport_h))
        .map(|value| value.max(0.0));
    let max_width = style_node
        .value("max-width")
        .and_then(|value| parse_length(Some(style_node), value, containing, viewport_w, viewport_h))
        .map(|value| value.max(0.0));

    (min_width, max_width)
}

fn clamp_width_to_constraints(width: f32, min_width: Option<f32>, max_width: Option<f32>) -> f32 {
    let min_width = min_width.unwrap_or(0.0).max(0.0);
    let max_width = max_width.map(|value| value.max(min_width));
    let width = width.max(min_width);

    if let Some(max_width) = max_width {
        width.min(max_width)
    } else {
        width
    }
}

fn parse_height_constraints(
    sn: Option<&StyledNode>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> (Option<f32>, Option<f32>) {
    let Some(style_node) = sn else {
        return (None, None);
    };

    let min_height = style_node
        .value("min-height")
        .and_then(|value| parse_length(Some(style_node), value, containing, viewport_w, viewport_h))
        .map(|value| value.max(0.0));
    let max_height = style_node
        .value("max-height")
        .and_then(|value| parse_length(Some(style_node), value, containing, viewport_w, viewport_h))
        .map(|value| value.max(0.0));

    (min_height, max_height)
}

fn clamp_height_to_constraints(
    height: f32,
    min_height: Option<f32>,
    max_height: Option<f32>,
) -> f32 {
    let min_height = min_height.unwrap_or(0.0).max(0.0);
    let max_height = max_height.map(|value| value.max(min_height));
    let height = height.max(min_height);

    if let Some(max_height) = max_height {
        height.min(max_height)
    } else {
        height
    }
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
    sn: Option<&StyledNode>,
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
            let a = parse_length(sn, parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: a,
                bottom_right: a,
                bottom_left: a,
            })
        }
        2 => {
            let a = parse_length(sn, parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(sn, parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: b,
                bottom_right: a,
                bottom_left: b,
            })
        }
        3 => {
            let a = parse_length(sn, parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(sn, parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            let c = parse_length(sn, parts[2], containing, viewport_w, viewport_h)?.max(0.0);
            Some(CornerRadii {
                top_left: a,
                top_right: b,
                bottom_right: c,
                bottom_left: b,
            })
        }
        4 => {
            let a = parse_length(sn, parts[0], containing, viewport_w, viewport_h)?.max(0.0);
            let b = parse_length(sn, parts[1], containing, viewport_w, viewport_h)?.max(0.0);
            let c = parse_length(sn, parts[2], containing, viewport_w, viewport_h)?.max(0.0);
            let d = parse_length(sn, parts[3], containing, viewport_w, viewport_h)?.max(0.0);
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

fn parse_corner_radius(
    sn: Option<&StyledNode>,
    s: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<f32> {
    let first = s.split('/').next()?.split_whitespace().next()?;
    parse_length(sn, first, containing, viewport_w, viewport_h).map(|v| v.max(0.0))
}

fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    Some(sn.font_size_px())
}

fn line_height_px(sn: &crate::style::StyledNode, _font_size: f32) -> f32 {
    sn.line_height_px()
}

fn img_intrinsic_size_px(
    sn: &crate::style::StyledNode,
    img_cache: &dyn ImageSizeProvider,
) -> (f32, f32) {
    let css_w = sn
        .value("width")
        .and_then(|v| sn.resolve_length_px(v, 0.0, 0.0, 0.0));
    let css_h = sn
        .value("height")
        .and_then(|v| sn.resolve_length_px(v, 0.0, 0.0, 0.0));

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

    let natural = src_opt
        .as_deref()
        .and_then(|src| img_cache.normalize_src_key(src))
        .and_then(|key| img_cache.natural_size_px(&key));

    if let (Some(w), Some(h)) = (css_w.or(attr_w), css_h.or(attr_h)) {
        return (
            w.max(layout_constants::MIN_LAYOUT_SIZE_PX),
            h.max(layout_constants::MIN_LAYOUT_SIZE_PX),
        );
    }

    if let Some(w) = css_w.or(attr_w) {
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

#[derive(Debug, Clone)]
struct FlexLine {
    indices: Vec<usize>,
    main_size: f32,
    cross_size: f32,
}

fn input_intrinsic_size_px(sn: &crate::style::StyledNode) -> (f32, f32) {
    let font_size = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
    let line_h = line_height_px(sn, font_size);
    let css_w = sn
        .value("width")
        .and_then(|v| sn.resolve_length_px(v, 0.0, 0.0, 0.0));
    let css_h = sn
        .value("height")
        .and_then(|v| sn.resolve_length_px(v, 0.0, 0.0, 0.0));

    let (kind, attr_size, label_len) = if let crate::dom::NodeType::Element(ed) = &sn.node.node_type
    {
        (
            input_type(ed),
            ed.attributes
                .get("size")
                .and_then(|s| s.trim().parse::<usize>().ok())
                .filter(|size| *size > 0),
            input_label(ed).chars().count(),
        )
    } else {
        ("text".to_string(), None, 0)
    };

    let default_h = line_h.max(layout_constants::MIN_LINE_HEIGHT_PX);
    let default_w = if matches!(kind.as_str(), "checkbox" | "radio") {
        default_h
    } else if matches!(kind.as_str(), "button" | "submit" | "reset") {
        (label_len.max(4) as f32) * font_size * layout_constants::INPUT_CHAR_WIDTH_EM
    } else {
        let char_count = attr_size.unwrap_or(layout_constants::DEFAULT_INPUT_CHARS) as f32;
        char_count * font_size * layout_constants::INPUT_CHAR_WIDTH_EM
    };

    (
        css_w
            .unwrap_or(default_w)
            .max(layout_constants::MIN_LAYOUT_SIZE_PX),
        css_h
            .unwrap_or(default_h)
            .max(layout_constants::MIN_LAYOUT_SIZE_PX),
    )
}

fn input_type(ed: &crate::dom::ElementData) -> String {
    ed.attributes
        .get("type")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "text".to_string())
}

fn input_label(ed: &crate::dom::ElementData) -> String {
    if let Some(value) = ed.attributes.get("value") {
        return value.clone();
    }

    match input_type(ed).as_str() {
        "submit" => "Submit".to_string(),
        "reset" => "Reset".to_string(),
        _ => String::new(),
    }
}

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

fn estimate_inline_atomic_outer_width(
    node: &LayoutBox<'_>,
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let d = &node.dimensions;
    estimate_layout_box_content_width(
        node,
        font,
        img_cache,
        containing_width,
        viewport_w,
        viewport_h,
    ) + d.margin.left
        + d.margin.right
        + d.padding.left
        + d.padding.right
        + d.border.left
        + d.border.right
}

fn estimate_layout_box_content_width(
    node: &LayoutBox<'_>,
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let Some(sn) = node.get_style_node() else {
        return estimate_inline_children_outer_width(
            &node.children,
            font,
            img_cache,
            containing_width,
            viewport_w,
            viewport_h,
        );
    };

    let raw_width = if let Some(width) = sn
        .value("width")
        .and_then(|value| parse_length(Some(sn), value, containing_width, viewport_w, viewport_h))
    {
        width.max(0.0)
    } else {
        match &sn.node.node_type {
            crate::dom::NodeType::Text(text) => {
                let font_size = font_size_px(sn).unwrap_or(layout_constants::DEFAULT_FONT_SIZE_PX);
                measure_collapsed_text_width(font, text, font_size)
            }
            crate::dom::NodeType::Element(ed) if ed.tag_name == "img" => {
                img_intrinsic_size_px(sn, img_cache).0.max(0.0)
            }
            crate::dom::NodeType::Element(ed) if ed.tag_name == "input" => {
                input_intrinsic_size_px(sn).0.max(0.0)
            }
            _ if node.is_inline_block_box() || matches!(node.box_type, BoxType::BlockNode(_)) => {
                estimate_block_children_outer_width(
                    &node.children,
                    font,
                    img_cache,
                    containing_width,
                    viewport_w,
                    viewport_h,
                )
            }
            _ => estimate_inline_children_outer_width(
                &node.children,
                font,
                img_cache,
                containing_width,
                viewport_w,
                viewport_h,
            ),
        }
    };
    let (min_width, max_width) =
        parse_width_constraints(Some(sn), containing_width, viewport_w, viewport_h);

    clamp_width_to_constraints(raw_width, min_width, max_width)
}

fn estimate_block_children_outer_width(
    children: &[LayoutBox<'_>],
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    children.iter().fold(0.0, |max_width, child| {
        max_width.max(estimate_layout_box_outer_width(
            child,
            font,
            img_cache,
            containing_width,
            viewport_w,
            viewport_h,
        ))
    })
}

fn estimate_inline_children_outer_width(
    children: &[LayoutBox<'_>],
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    children
        .iter()
        .map(|child| {
            estimate_layout_box_outer_width(
                child,
                font,
                img_cache,
                containing_width,
                viewport_w,
                viewport_h,
            )
        })
        .sum()
}

fn estimate_layout_box_outer_width(
    node: &LayoutBox<'_>,
    font: &Font,
    img_cache: &dyn ImageSizeProvider,
    containing_width: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let content_width = estimate_layout_box_content_width(
        node,
        font,
        img_cache,
        containing_width,
        viewport_w,
        viewport_h,
    );

    let horizontal_extras = node
        .get_style_node()
        .map(|sn| horizontal_box_model_width(sn, containing_width, viewport_w, viewport_h))
        .unwrap_or(0.0);

    content_width + horizontal_extras
}

fn horizontal_box_model_width(
    sn: &StyledNode,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> f32 {
    let margin = parse_horizontal_edges(
        sn,
        sn.value("margin-left").map(|value| value.as_str()),
        sn.value("margin-right").map(|value| value.as_str()),
        sn.value("margin").map(|value| value.as_str()),
        containing,
        viewport_w,
        viewport_h,
        true,
    );
    let padding = parse_horizontal_edges(
        sn,
        sn.value("padding-left").map(|value| value.as_str()),
        sn.value("padding-right").map(|value| value.as_str()),
        sn.value("padding").map(|value| value.as_str()),
        containing,
        viewport_w,
        viewport_h,
        false,
    );
    let border = parse_horizontal_edges(
        sn,
        sn.value("border-left-width").map(|value| value.as_str()),
        sn.value("border-right-width").map(|value| value.as_str()),
        sn.value("border-width").map(|value| value.as_str()),
        containing,
        viewport_w,
        viewport_h,
        false,
    );

    margin.0 + margin.1 + padding.0 + padding.1 + border.0 + border.1
}

fn parse_horizontal_edges(
    sn: &StyledNode,
    left: Option<&str>,
    right: Option<&str>,
    shorthand: Option<&str>,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
    allow_auto: bool,
) -> (f32, f32) {
    let mut left_px = 0.0;
    let mut right_px = 0.0;

    if let Some(value) = left {
        left_px = parse_horizontal_edge(sn, value, containing, viewport_w, viewport_h, allow_auto);
    }
    if let Some(value) = right {
        right_px = parse_horizontal_edge(sn, value, containing, viewport_w, viewport_h, allow_auto);
    }

    if left.is_none() && right.is_none() {
        if let Some(value) = shorthand {
            let values = parse_4len(value);
            if let Some(right_value) = values.1.as_deref() {
                right_px = parse_horizontal_edge(
                    sn,
                    right_value,
                    containing,
                    viewport_w,
                    viewport_h,
                    allow_auto,
                );
            }
            if let Some(left_value) = values.3.as_deref() {
                left_px = parse_horizontal_edge(
                    sn, left_value, containing, viewport_w, viewport_h, allow_auto,
                );
            }
        }
    }

    (left_px, right_px)
}

fn parse_horizontal_edge(
    sn: &StyledNode,
    value: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
    allow_auto: bool,
) -> f32 {
    if allow_auto && value.trim() == "auto" {
        return 0.0;
    }

    parse_length(Some(sn), value, containing, viewport_w, viewport_h).unwrap_or(0.0)
}

fn measure_collapsed_text_width(font: &Font, text: &str, font_size: f32) -> f32 {
    let collapsed = collapse_whitespace(text);
    let trimmed = collapsed.trim();

    if trimmed.is_empty() {
        0.0
    } else {
        measure_width_fontdue(font, trimmed, font_size)
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
    fn inline_block_shrink_wraps_and_stays_in_inline_flow() {
        let styled = styled_tree_with_css(
            r#"<p>before <span id="tag">badge</span> after</p>"#,
            r#"
            p { display: block; width: 320px; margin: 0; padding: 0; }
            #tag {
                display: inline-block;
                padding: 4px;
                border: 2px solid red;
                margin: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let tag = find_element_by_id(&layout, "tag").unwrap();
        let tag_margin_box = tag.dimensions.margin_box_rect();

        assert!(matches!(tag.box_type, BoxType::InlineNode(_)));
        assert!(tag.dimensions.margin_box_width() < 200.0);

        let mut fragments = Vec::new();
        collect_fragments(&layout, &mut fragments);

        let before = fragments.iter().find(|frag| frag.text == "before").unwrap();
        let after = fragments.iter().find(|frag| frag.text == "after").unwrap();

        assert!((before.rect.y - tag_margin_box.y).abs() <= 0.5);
        assert!((after.rect.y - tag_margin_box.y).abs() <= 0.5);
        assert!(after.rect.x + 0.5 >= tag_margin_box.x + tag_margin_box.width);
    }

    #[test]
    fn inline_block_lays_out_children_as_block_container() {
        let styled = styled_tree_with_css(
            r#"
            <p>
                <span id="tag">
                    <span id="first"></span>
                    <span id="second"></span>
                </span>
            </p>
            "#,
            r#"
            p { display: block; width: 320px; margin: 0; padding: 0; }
            #tag {
                display: inline-block;
                padding: 4px;
                border: 2px solid red;
                margin: 0;
            }
            #first, #second {
                display: block;
                width: 40px;
                height: 12px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let tag = find_element_by_id(&layout, "tag").unwrap();
        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!(matches!(tag.box_type, BoxType::InlineNode(_)));
        assert!((first.dimensions.content.x - second.dimensions.content.x).abs() <= 0.5);
        assert!(second.dimensions.content.y > first.dimensions.content.y + 0.5);
        assert!(tag.dimensions.content.width >= first.dimensions.margin_box_width());
    }

    #[test]
    fn em_and_rem_lengths_resolve_from_computed_font_sizes() {
        let styled = styled_tree_with_css(
            r#"
            <div id="outer">
                <div id="em-box"></div>
                <div id="rem-box"></div>
            </div>
            "#,
            r#"
            html { font-size: 20px; }
            #outer {
                display: block;
                font-size: 1.5em;
                margin: 0;
                padding: 0;
            }
            #em-box {
                display: block;
                width: 2em;
                height: 1em;
                margin: 0;
                padding: 0;
            }
            #rem-box {
                display: block;
                width: 2rem;
                height: 1rem;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 400.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let em_box = find_element_by_id(&layout, "em-box").unwrap();
        let rem_box = find_element_by_id(&layout, "rem-box").unwrap();

        assert!((em_box.dimensions.content.width - 60.0).abs() <= 0.5);
        assert!((em_box.dimensions.content.height - 30.0).abs() <= 0.5);
        assert!((rem_box.dimensions.content.width - 40.0).abs() <= 0.5);
        assert!((rem_box.dimensions.content.height - 20.0).abs() <= 0.5);
    }

    #[test]
    fn block_auto_width_honors_max_width_and_auto_margins() {
        let styled = styled_tree_with_css(
            r#"<div id="page"></div>"#,
            r#"
            #page {
                display: block;
                max-width: 120px;
                height: 10px;
                margin: 0 auto;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 300.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let page = find_element_by_id(&layout, "page").unwrap();

        assert!((page.dimensions.content.width - 120.0).abs() <= 0.5);
        assert!((page.dimensions.margin.left - 90.0).abs() <= 0.5);
        assert!((page.dimensions.margin.right - 90.0).abs() <= 0.5);
    }

    #[test]
    fn explicit_width_is_clamped_by_min_and_max_width() {
        let styled = styled_tree_with_css(
            r#"<div id="min"></div><div id="max"></div>"#,
            r#"
            #min {
                display: block;
                width: 40px;
                min-width: 90px;
                height: 10px;
                margin: 0;
                padding: 0;
            }
            #max {
                display: block;
                width: 180px;
                max-width: 120px;
                height: 10px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 300.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let min_box = find_element_by_id(&layout, "min").unwrap();
        let max_box = find_element_by_id(&layout, "max").unwrap();

        assert!((min_box.dimensions.content.width - 90.0).abs() <= 0.5);
        assert!((max_box.dimensions.content.width - 120.0).abs() <= 0.5);
    }

    #[test]
    fn auto_and_explicit_heights_are_clamped_by_min_and_max_height() {
        let styled = styled_tree_with_css(
            r#"
            <div id="auto-min"></div>
            <div id="auto-max"><div id="inner"></div></div>
            <div id="explicit-min"></div>
            <div id="explicit-max"></div>
            "#,
            r#"
            #auto-min {
                display: block;
                width: 100px;
                min-height: 80px;
                margin: 0;
                padding: 0;
            }
            #auto-max {
                display: block;
                width: 100px;
                max-height: 50px;
                margin: 0;
                padding: 0;
            }
            #inner {
                display: block;
                width: 100px;
                height: 100px;
                margin: 0;
                padding: 0;
            }
            #explicit-min {
                display: block;
                width: 100px;
                height: 20px;
                min-height: 70px;
                margin: 0;
                padding: 0;
            }
            #explicit-max {
                display: block;
                width: 100px;
                height: 120px;
                max-height: 45px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 300.0;
        viewport.content.height = 240.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let auto_min = find_element_by_id(&layout, "auto-min").unwrap();
        let auto_max = find_element_by_id(&layout, "auto-max").unwrap();
        let explicit_min = find_element_by_id(&layout, "explicit-min").unwrap();
        let explicit_max = find_element_by_id(&layout, "explicit-max").unwrap();

        assert!((auto_min.dimensions.content.height - 80.0).abs() <= 0.5);
        assert!((auto_max.dimensions.content.height - 50.0).abs() <= 0.5);
        assert!((explicit_min.dimensions.content.height - 70.0).abs() <= 0.5);
        assert!((explicit_max.dimensions.content.height - 45.0).abs() <= 0.5);
    }

    #[test]
    fn positioned_container_min_height_affects_absolute_bottom_offset() {
        let styled = styled_tree_with_css(
            r#"<div id="container"><div id="abs"></div></div>"#,
            r#"
            #container {
                display: block;
                position: relative;
                width: 200px;
                min-height: 100px;
                padding: 10px;
                margin: 0;
            }
            #abs {
                display: block;
                position: absolute;
                left: 0;
                bottom: 15px;
                width: 30px;
                height: 10px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 320.0;
        viewport.content.height = 240.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let container = find_element_by_id(&layout, "container").unwrap();
        let abs = find_element_by_id(&layout, "abs").unwrap();

        let padding_box_y = container.dimensions.content.y - container.dimensions.padding.top;
        let padding_box_height = container.dimensions.content.height
            + container.dimensions.padding.top
            + container.dimensions.padding.bottom;

        assert!((container.dimensions.content.height - 100.0).abs() <= 0.5);
        assert!(
            (abs.dimensions.content.y - (padding_box_y + padding_box_height - 15.0 - 10.0)).abs()
                <= 0.5
        );
    }

    #[test]
    fn inline_image_height_is_clamped_by_max_height() {
        let styled = styled_tree_with_css(
            r#"<p><img id="pic" src="150x150.png" alt=""></p>"#,
            r#"
            p {
                display: block;
                width: 240px;
                margin: 0;
                padding: 0;
            }
            #pic {
                max-height: 40px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 300.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let pic = find_element_by_id(&layout, "pic").unwrap();

        assert!((pic.dimensions.content.height - 40.0).abs() <= 0.5);
    }

    #[test]
    fn inline_block_estimate_respects_max_width_before_line_wrapping() {
        let styled = styled_tree_with_css(
            r#"<p>a <span id="tag">alpha beta gamma</span></p>"#,
            r#"
            p {
                display: block;
                width: 80px;
                margin: 0;
                padding: 0;
            }
            #tag {
                display: inline-block;
                max-width: 60px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 240.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let tag = find_element_by_id(&layout, "tag").unwrap();
        let mut fragments = Vec::new();
        collect_fragments(&layout, &mut fragments);

        let lead = fragments
            .iter()
            .find(|fragment| fragment.text == "a")
            .unwrap();

        assert!((tag.dimensions.content.width - 60.0).abs() <= 0.5);
        assert!((tag.dimensions.content.y - lead.rect.y).abs() <= 0.5);
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

    #[test]
    fn flex_row_places_items_horizontally_with_gap() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                width: 200px;
                gap: 12px;
                margin: 0;
                padding: 0;
            }
            #first, #second {
                display: block;
                width: 40px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 320.0;
        viewport.content.height = 240.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!((first.dimensions.content.y - second.dimensions.content.y).abs() <= 0.5);
        assert!(
            (second.dimensions.margin_box_rect().x
                - (first.dimensions.margin_box_rect().x
                    + first.dimensions.margin_box_width()
                    + 12.0))
                .abs()
                <= 0.5
        );
    }

    #[test]
    fn flex_row_distributes_remaining_width_with_flex_grow() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                width: 180px;
                margin: 0;
                padding: 0;
            }
            #first {
                flex: 1;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            #second {
                flex: 2;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 320.0;
        viewport.content.height = 240.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!(
            ((first.dimensions.content.width + second.dimensions.content.width) - 180.0).abs()
                <= 1.0
        );
        assert!(second.dimensions.content.width > first.dimensions.content.width);
        assert!(
            (second.dimensions.content.width - (first.dimensions.content.width * 2.0)).abs() <= 2.0
        );
    }

    #[test]
    fn flex_row_wraps_items_when_enabled() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
                <div id="third"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                flex-wrap: wrap;
                width: 100px;
                gap: 10px 5px;
                margin: 0;
                padding: 0;
            }
            #first, #second, #third {
                display: block;
                width: 60px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 200.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let container = find_element_by_id(&layout, "container").unwrap();
        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();
        let third = find_element_by_id(&layout, "third").unwrap();

        assert!((first.dimensions.content.x - second.dimensions.content.x).abs() <= 0.5);
        assert!((second.dimensions.content.x - third.dimensions.content.x).abs() <= 0.5);
        assert!((second.dimensions.content.y - (first.dimensions.content.y + 30.0)).abs() <= 0.5);
        assert!((third.dimensions.content.y - (second.dimensions.content.y + 30.0)).abs() <= 0.5);
        assert!((container.dimensions.content.height - 80.0).abs() <= 0.5);
    }

    #[test]
    fn flex_flow_wrap_shorthand_wraps_items() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                flex-flow: row wrap;
                width: 100px;
                row-gap: 6px;
                margin: 0;
                padding: 0;
            }
            #first, #second {
                display: block;
                width: 70px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 200.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!((second.dimensions.content.y - (first.dimensions.content.y + 26.0)).abs() <= 0.5);
    }

    #[test]
    fn flex_basis_from_flex_shorthand_drives_wrapping() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first">a</div>
                <div id="second">b</div>
                <div id="third">c</div>
                <div id="fourth">d</div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                flex-wrap: wrap;
                width: 120px;
                margin: 0;
                padding: 0;
            }
            #first, #second, #third, #fourth {
                display: block;
                flex: 0 0 50%;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 200.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();
        let third = find_element_by_id(&layout, "third").unwrap();
        let fourth = find_element_by_id(&layout, "fourth").unwrap();

        assert!((first.dimensions.content.width - 60.0).abs() <= 0.5);
        assert!((second.dimensions.content.x - (first.dimensions.content.x + 60.0)).abs() <= 0.5);
        assert!((third.dimensions.content.y - (first.dimensions.content.y + 20.0)).abs() <= 0.5);
        assert!((fourth.dimensions.content.x - (third.dimensions.content.x + 60.0)).abs() <= 0.5);
    }

    #[test]
    fn display_grid_places_items_in_template_columns() {
        let styled = styled_tree_with_css(
            r#"
            <ol id="trend">
                <li id="one">one</li>
                <li id="two">two</li>
                <li id="three">three</li>
                <li id="four">four</li>
            </ol>
            "#,
            r#"
            #trend {
                display: grid;
                grid-template-columns: repeat(3, 1fr);
                column-gap: 6px;
                row-gap: 4px;
                width: 156px;
                margin: 0;
                padding: 0;
            }
            #trend > li {
                display: block;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 240.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let one = find_element_by_id(&layout, "one").unwrap();
        let two = find_element_by_id(&layout, "two").unwrap();
        let three = find_element_by_id(&layout, "three").unwrap();
        let four = find_element_by_id(&layout, "four").unwrap();

        assert!((one.dimensions.content.width - 48.0).abs() <= 0.5);
        assert!((two.dimensions.content.x - (one.dimensions.content.x + 54.0)).abs() <= 0.5);
        assert!((three.dimensions.content.x - (two.dimensions.content.x + 54.0)).abs() <= 0.5);
        assert!((four.dimensions.content.x - one.dimensions.content.x).abs() <= 0.5);
        assert!((four.dimensions.content.y - (one.dimensions.content.y + 24.0)).abs() <= 0.5);
    }

    #[test]
    fn flex_column_wrap_flows_items_into_next_column() {
        let styled = styled_tree_with_css(
            r#"
            <div id="trend">
                <span id="one">呼称</span>
                <span id="two">大谷</span>
                <span id="three">ホームラン</span>
                <span id="four">WBC</span>
                <span id="five">物価高</span>
            </div>
            "#,
            r#"
            #trend {
                display: flex;
                flex-flow: column wrap;
                height: 48px;
                width: 240px;
                row-gap: 0;
                column-gap: 8px;
                margin: 0;
                padding: 0;
            }
            #trend > span {
                display: block;
                height: 16px;
                margin: 0;
                padding: 0;
                font-size: 12px;
                line-height: 16px;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 320.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let one = find_element_by_id(&layout, "one").unwrap();
        let two = find_element_by_id(&layout, "two").unwrap();
        let three = find_element_by_id(&layout, "three").unwrap();
        let four = find_element_by_id(&layout, "four").unwrap();
        let five = find_element_by_id(&layout, "five").unwrap();

        assert!((two.dimensions.content.y - (one.dimensions.content.y + 16.0)).abs() <= 0.5);
        assert!((three.dimensions.content.y - (two.dimensions.content.y + 16.0)).abs() <= 0.5);
        assert!(four.dimensions.content.x > one.dimensions.content.x + 8.0);
        assert!((four.dimensions.content.y - one.dimensions.content.y).abs() <= 0.5);
        assert!((five.dimensions.content.y - (four.dimensions.content.y + 16.0)).abs() <= 0.5);
    }

    #[test]
    fn flex_row_without_wrap_keeps_overflow_on_single_line() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                width: 100px;
                gap: 5px;
                margin: 0;
                padding: 0;
            }
            #first, #second {
                display: block;
                width: 60px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 200.0;
        viewport.content.height = 200.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!((first.dimensions.content.y - second.dimensions.content.y).abs() <= 0.5);
        assert!(
            (second.dimensions.margin_box_rect().x
                - (first.dimensions.margin_box_rect().x
                    + first.dimensions.margin_box_width()
                    + 5.0))
                .abs()
                <= 0.5
        );
    }

    #[test]
    fn flex_column_stacks_items_vertically_with_gap() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <div id="first"></div>
                <div id="second"></div>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                flex-direction: column;
                gap: 10px;
                width: 120px;
                margin: 0;
                padding: 0;
            }
            #first, #second {
                display: block;
                width: 30px;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let mut layout = build_layout_tree(&styled);
        let mut viewport = Dimensions::default();
        viewport.content.width = 320.0;
        viewport.content.height = 240.0;

        layout.layout_with_font(viewport, &test_font(), &EmptyImageCache);

        let first = find_element_by_id(&layout, "first").unwrap();
        let second = find_element_by_id(&layout, "second").unwrap();

        assert!((first.dimensions.content.x - second.dimensions.content.x).abs() <= 0.5);
        assert!(
            (second.dimensions.margin_box_rect().y
                - (first.dimensions.margin_box_rect().y
                    + first.dimensions.margin_box_height()
                    + 10.0))
                .abs()
                <= 0.5
        );
    }

    #[test]
    fn flex_container_blockifies_inline_children() {
        let styled = styled_tree_with_css(
            r#"
            <div id="container">
                <span id="item"><b>hello</b> world</span>
            </div>
            "#,
            r#"
            #container {
                display: flex;
                width: 200px;
                margin: 0;
                padding: 0;
            }
            "#,
        );
        let layout = build_layout_tree(&styled);
        let item = find_element_by_id(&layout, "item").unwrap();

        assert!(matches!(item.box_type, BoxType::BlockNode(_)));
    }
}
