#![allow(dead_code)]

use std::collections::HashMap;

use crate::constants::{color, layout as layout_constants};
use crate::css::{Declaration, Rule, Stylesheet, parse_inline_declarations};
use crate::dom::{ElementData, Node, NodeType};

#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node: Node,
    pub specified_values: HashMap<String, String>,
    pub computed_font_size_px: f32,
    pub computed_root_font_size_px: f32,
    pub children: Vec<StyledNode>,
    pub link_href: Option<String>,
    pub link_id: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    InlineBlock,
    Block,
    Flex,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Float {
    None,
    Left,
    Right,
}

impl Float {
    pub fn is_floating(self) -> bool {
        !matches!(self, Float::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clear {
    None,
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

impl Position {
    pub fn is_positioned(self) -> bool {
        !matches!(self, Position::Static)
    }

    pub fn is_out_of_flow(self) -> bool {
        matches!(self, Position::Absolute | Position::Fixed)
    }

    pub fn behaves_like_relative(self) -> bool {
        matches!(self, Position::Relative | Position::Sticky)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZIndex {
    Auto,
    Integer(i32),
}

impl ZIndex {
    pub fn stack_level(self) -> Option<i32> {
        match self {
            ZIndex::Auto => None,
            ZIndex::Integer(value) => Some(value),
        }
    }
}

impl StyledNode {
    pub fn value(&self, name: &str) -> Option<&String> {
        self.specified_values.get(name)
    }

    pub fn font_size_px(&self) -> f32 {
        self.computed_font_size_px
    }

    pub fn root_font_size_px(&self) -> f32 {
        self.computed_root_font_size_px
    }

    pub fn resolve_length_px(
        &self,
        value: &str,
        containing: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<f32> {
        resolve_css_length(
            value,
            containing,
            viewport_w,
            viewport_h,
            self.computed_font_size_px,
            self.computed_root_font_size_px,
        )
    }

    pub fn line_height_px(&self) -> f32 {
        self.value("line-height")
            .and_then(|value| {
                resolve_line_height_value(
                    value,
                    self.computed_font_size_px,
                    self.computed_root_font_size_px,
                )
            })
            .unwrap_or(
                self.computed_font_size_px * layout_constants::DEFAULT_LINE_HEIGHT_MULTIPLIER,
            )
    }

    /// display の最小実装 (IFC向け)
    /// - Textノードは必ず Inline
    /// - CSS display 指定があれば最優先
    /// - head/title/meta/script/style/link は None
    /// - よくある block 要素は Block
    /// - それ以外は Inline (迷ったら inline のほうが IFC が動きやすい)
    pub fn display(&self) -> Display {
        if matches!(self.node.node_type, NodeType::Text(_)) {
            return Display::Inline;
        }

        if let Some(v) = self.value("display").map(|s| s.trim()) {
            return match v {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "none" => Display::None,
                _ => Display::Inline,
            };
        }

        if let NodeType::Element(ref e) = self.node.node_type {
            let tag = e.tag_name.as_str();

            if is_hidden_element(tag) {
                return Display::None;
            }
            if is_block_element(tag) {
                return Display::Block;
            }
            return Display::Inline;
        }

        Display::Inline
    }

    /// CSS position の最小実装。
    /// sticky はスクロール連動前の状態として relative 相当に扱う。
    pub fn position(&self) -> Position {
        match self.value("position").map(|s| s.trim()) {
            Some("relative") => Position::Relative,
            Some("absolute") => Position::Absolute,
            Some("fixed") => Position::Fixed,
            Some("sticky") => Position::Sticky,
            _ => Position::Static,
        }
    }

    /// CSS float の最小実装。
    /// absolute/fixed 側の無効化は layout 側で行う。
    pub fn float(&self) -> Float {
        match self.value("float").map(|s| s.trim()) {
            Some("left") => Float::Left,
            Some("right") => Float::Right,
            _ => Float::None,
        }
    }

    pub fn clear(&self) -> Clear {
        match self.value("clear").map(|s| s.trim()) {
            Some("left") => Clear::Left,
            Some("right") => Clear::Right,
            Some("both") => Clear::Both,
            _ => Clear::None,
        }
    }

    /// CSS z-index の最小実装。
    /// positioned 要素に効かせる判定は display list 側で行う。
    pub fn z_index(&self) -> ZIndex {
        match self.value("z-index").map(|s| s.trim()) {
            Some("auto") | None => ZIndex::Auto,
            Some(value) => value
                .parse::<i32>()
                .map(ZIndex::Integer)
                .unwrap_or(ZIndex::Auto),
        }
    }

    pub fn color(&self) -> Option<[f32; 4]> {
        self.value("color").and_then(|v| parse_color(v))
    }
    pub fn border_color(&self) -> Option<[f32; 4]> {
        if let Some(v) = self.value("border-color").or_else(|| self.value("color")) {
            parse_color(v)
        } else {
            None
        }
    }

    pub fn background_color(&self) -> Option<[f32; 4]> {
        if let Some(v) = self.value("background-color") {
            return parse_color(v);
        }
        if let Some(v) = self.value("background") {
            let t = v.trim().to_lowercase();
            if t == "none" || t == "transparent" {
                return None;
            }
            return parse_color(v);
        }
        None
    }
}

/// そもそも表示しない (display: none相当)
fn is_hidden_element(tag: &str) -> bool {
    matches!(tag, "head" | "meta" | "title" | "script" | "style" | "link")
}

/// HTMLのデフォルト表示に寄せた "最小" block 判定
fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "html"
            | "body"
            | "div"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "nav"
            | "main"
            | "pre"
            | "blockquote"
    )
}

#[derive(Debug, Clone)]
struct Ancestor {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
}

fn ancestor_of(e: &ElementData) -> Ancestor {
    Ancestor {
        tag: e.tag_name.clone(),
        id: e.id().map(|s| s.to_string()),
        classes: e.classes().iter().map(|s| s.to_string()).collect(),
    }
}

const INHERITABLE_PROPS: &[&str] = &[
    "color",
    "font-size",
    "font-family",
    "font-weight",
    "line-height",
    "text-decoration",
];

fn is_inheritable_prop(name: &str) -> bool {
    INHERITABLE_PROPS.contains(&name)
}

fn inherit_only(values: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in values {
        if is_inheritable_prop(k.as_str()) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn inherited_values_for_children(
    values: &HashMap<String, String>,
    font_size_px: f32,
    root_font_size_px: f32,
) -> HashMap<String, String> {
    let mut out = inherit_only(values);

    out.insert("font-size".to_string(), format_css_px(font_size_px));

    if let Some(line_height) = values.get("line-height") {
        if is_unitless_number(line_height) {
            out.insert("line-height".to_string(), line_height.clone());
        } else if let Some(px) =
            resolve_line_height_value(line_height, font_size_px, root_font_size_px)
        {
            out.insert("line-height".to_string(), format_css_px(px));
        }
    }

    out
}

fn format_css_px(value: f32) -> String {
    format!("{value}px")
}

fn is_unitless_number(value: &str) -> bool {
    value.trim().parse::<f32>().is_ok()
}

pub fn style_tree(root: Node, stylesheet: &Stylesheet) -> StyledNode {
    let mut next_link_id = 1usize;
    let initial_font_size_px = layout_constants::DEFAULT_FONT_SIZE_PX;
    style_tree_with_ctx(
        root,
        stylesheet,
        None,
        None,
        &Vec::new(),
        &HashMap::new(),
        initial_font_size_px,
        initial_font_size_px,
        &mut next_link_id,
    )
}

fn style_tree_with_ctx(
    root: Node,
    stylesheet: &Stylesheet,
    inherited_link: Option<String>,
    inherited_link_id: Option<usize>,
    ancestors: &Vec<Ancestor>,
    inherited_values: &HashMap<String, String>,
    inherited_font_size_px: f32,
    root_font_size_px: f32,
    next_link_id: &mut usize,
) -> StyledNode {
    let mut specified_values = inherited_values.clone();
    let mut own_values = HashMap::new();

    if let NodeType::Element(ref e) = root.node_type {
        own_values = specified_values_for(e, stylesheet, ancestors);
        for declaration in parse_inline_style_attr(e) {
            own_values.insert(declaration.name, declaration.value);
        }
    }

    let root_font_size_reference_px = if ancestors.is_empty() {
        layout_constants::DEFAULT_FONT_SIZE_PX
    } else {
        root_font_size_px
    };
    let computed_font_size_px = own_values
        .get("font-size")
        .and_then(|value| {
            resolve_font_size_value(value, inherited_font_size_px, root_font_size_reference_px)
        })
        .unwrap_or(inherited_font_size_px);
    let computed_root_font_size_px = if ancestors.is_empty() {
        computed_font_size_px
    } else {
        root_font_size_px
    };

    for (k, v) in own_values {
        specified_values.insert(k, v);
    }

    let mut link_here = inherited_link.clone();
    let mut link_id_here = inherited_link_id;
    if let NodeType::Element(ref e) = root.node_type {
        if e.tag_name == "a" {
            if let Some(href) = e.href.as_deref() {
                let h = href.trim();
                if !h.is_empty()
                    && !h.starts_with('#')
                    && !h.to_lowercase().starts_with("javascript:")
                    && !h.to_lowercase().starts_with("data:")
                {
                    link_here = Some(h.to_string());
                    link_id_here = Some(*next_link_id);
                    *next_link_id += 1;
                }
            }
        }
    }

    let mut next_ancestors = ancestors.clone();
    if let NodeType::Element(ref e) = root.node_type {
        next_ancestors.push(ancestor_of(e));
    }

    let next_inherited_values = inherited_values_for_children(
        &specified_values,
        computed_font_size_px,
        computed_root_font_size_px,
    );

    let children = root
        .children
        .iter()
        .cloned()
        .map(|c| {
            style_tree_with_ctx(
                c,
                stylesheet,
                link_here.clone(),
                link_id_here,
                &next_ancestors,
                &next_inherited_values,
                computed_font_size_px,
                computed_root_font_size_px,
                next_link_id,
            )
        })
        .collect();

    StyledNode {
        node: root,
        specified_values,
        computed_font_size_px,
        computed_root_font_size_px,
        children,
        link_href: link_here,
        link_id: link_id_here,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32); // (id, class, tag)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorCombinator {
    Descendant,
    Child,
}

#[derive(Debug, Clone, Copy)]
struct SelectorPart<'a> {
    simple: &'a str,
    combinator_to_left: Option<SelectorCombinator>,
}

fn specified_values_for(
    elem: &ElementData,
    stylesheet: &Stylesheet,
    ancestors: &[Ancestor],
) -> HashMap<String, String> {
    let mut matched: Vec<(Specificity, usize, &Rule)> = vec![];

    for (i, rule) in stylesheet.rules.iter().enumerate() {
        if let Some(spec) = matching_rule_specificity(elem, rule, ancestors) {
            matched.push((spec, i, rule));
        }
    }

    matched.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut values = HashMap::new();
    for (_, _, rule) in matched {
        for Declaration { name, value } in &rule.declarations {
            values.insert(name.clone(), value.clone());
        }
    }

    values
}

fn parse_inline_style_attr(elem: &ElementData) -> Vec<Declaration> {
    elem.attributes
        .get("style")
        .map(|style| parse_inline_declarations(style))
        .unwrap_or_default()
}

fn matching_rule_specificity(
    elem: &ElementData,
    rule: &Rule,
    ancestors: &[Ancestor],
) -> Option<Specificity> {
    rule.selectors
        .iter()
        .filter_map(|sel| selector_specificity_if_matches(elem, ancestors, &sel.simple))
        .max()
}

fn parse_selector_parts(selector: &str) -> Option<Vec<SelectorPart<'_>>> {
    let s = selector.trim();
    if s.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    let mut pos = 0usize;
    let mut pending_combinator: Option<SelectorCombinator> = None;

    while pos < s.len() {
        while pos < s.len() {
            let ch = s[pos..].chars().next().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            pos += ch.len_utf8();
        }

        if pos >= s.len() {
            break;
        }

        if s[pos..].starts_with('>') {
            if parts.is_empty() || pending_combinator.is_some() {
                return None;
            }
            pending_combinator = Some(SelectorCombinator::Child);
            pos += '>'.len_utf8();
            continue;
        }

        let start = pos;
        while pos < s.len() {
            let ch = s[pos..].chars().next().unwrap();
            if ch.is_whitespace() || ch == '>' {
                break;
            }
            if matches!(ch, '+' | '[' | ':' | '~') {
                return None;
            }
            pos += ch.len_utf8();
        }

        let simple = s[start..pos].trim();
        if simple.is_empty() {
            return None;
        }

        let combinator_to_left = if parts.is_empty() {
            None
        } else {
            Some(
                pending_combinator
                    .take()
                    .unwrap_or(SelectorCombinator::Descendant),
            )
        };
        parts.push(SelectorPart {
            simple,
            combinator_to_left,
        });
    }

    if pending_combinator.is_some() {
        return None;
    }

    Some(parts)
}

fn selector_matches_parts(
    elem: &ElementData,
    ancestors: &[Ancestor],
    parts: &[SelectorPart<'_>],
) -> bool {
    let Some(last) = parts.last() else {
        return false;
    };

    if !selector_matches_simple_elem(elem, last.simple) {
        return false;
    }

    let mut current_ancestor_index = ancestors.len();
    for i in (1..parts.len()).rev() {
        let Some(combinator) = parts[i].combinator_to_left else {
            return false;
        };

        let left = parts[i - 1].simple;
        match combinator {
            SelectorCombinator::Descendant => {
                if let Some(pos) = find_ancestor_match(ancestors, current_ancestor_index, left) {
                    current_ancestor_index = pos;
                } else {
                    return false;
                }
            }
            SelectorCombinator::Child => {
                if current_ancestor_index == 0 {
                    return false;
                }

                let parent_index = current_ancestor_index - 1;
                if !selector_matches_simple_ancestor(&ancestors[parent_index], left) {
                    return false;
                }
                current_ancestor_index = parent_index;
            }
        }
    }

    true
}

fn find_ancestor_match(ancestors: &[Ancestor], upto: usize, part: &str) -> Option<usize> {
    let mut i = upto;
    while i > 0 {
        i -= 1;
        if selector_matches_simple_ancestor(&ancestors[i], part) {
            return Some(i);
        }
    }
    None
}

fn selector_matches_simple_elem(elem: &ElementData, selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
        return false;
    }

    if let Some(id) = s.strip_prefix('#') {
        return elem.id() == Some(id);
    }

    if let Some(class) = s.strip_prefix('.') {
        return elem.classes().iter().any(|c| *c == class);
    }

    if let Some((tag, class)) = s.split_once('.') {
        if elem.tag_name != tag {
            return false;
        }
        return elem.classes().iter().any(|c| *c == class);
    }

    elem.tag_name == s
}

fn selector_matches_simple_ancestor(a: &Ancestor, selector: &str) -> bool {
    let s = selector.trim();
    if s.is_empty() {
        return false;
    }

    if let Some(id) = s.strip_prefix('#') {
        return a.id.as_deref() == Some(id);
    }

    if let Some(class) = s.strip_prefix('.') {
        return a.classes.iter().any(|c| c == class);
    }

    if let Some((tag, class)) = s.split_once('.') {
        if a.tag != tag {
            return false;
        }
        return a.classes.iter().any(|c| c == class);
    }

    a.tag == s
}

fn selector_specificity_if_matches(
    elem: &ElementData,
    ancestors: &[Ancestor],
    selector: &str,
) -> Option<Specificity> {
    let parts = parse_selector_parts(selector)?;
    if !selector_matches_parts(elem, ancestors, &parts) {
        return None;
    }

    let mut id = 0u32;
    let mut class = 0u32;
    let mut tag = 0u32;

    for part in parts {
        let sp = specificity_of_simple(part.simple);
        id += sp.0;
        class += sp.1;
        tag += sp.2;
    }

    Some(Specificity(id, class, tag))
}

fn specificity_of_simple(s: &str) -> Specificity {
    let s = s.trim();
    if s.is_empty() {
        return Specificity(0, 0, 0);
    }
    if s.starts_with('#') {
        return Specificity(1, 0, 0);
    }
    if s.starts_with('.') {
        return Specificity(0, 1, 0);
    }
    if s.contains('.') {
        return Specificity(0, 1, 1);
    }
    Specificity(0, 0, 1)
}

pub fn resolve_css_length(
    value: &str,
    containing: f32,
    viewport_w: f32,
    viewport_h: f32,
    font_size_px: f32,
    root_font_size_px: f32,
) -> Option<f32> {
    let t = value.trim();

    if t == "0" || t == "+0" || t == "-0" {
        return Some(0.0);
    }
    if let Some(num) = t.strip_suffix("px") {
        return num.trim().parse::<f32>().ok();
    }
    if let Some(num) = t.strip_suffix("rem") {
        let v = num.trim().parse::<f32>().ok()?;
        return Some(root_font_size_px * v);
    }
    if let Some(num) = t.strip_suffix("em") {
        let v = num.trim().parse::<f32>().ok()?;
        return Some(font_size_px * v);
    }
    if let Some(num) = t.strip_suffix("vw") {
        let v = num.trim().parse::<f32>().ok()?;
        return Some(viewport_w * (v / layout_constants::PERCENT_DENOMINATOR));
    }
    if let Some(num) = t.strip_suffix("vh") {
        let v = num.trim().parse::<f32>().ok()?;
        return Some(viewport_h * (v / layout_constants::PERCENT_DENOMINATOR));
    }
    if let Some(num) = t.strip_suffix('%') {
        let v = num.trim().parse::<f32>().ok()?;
        return Some(containing * (v / layout_constants::PERCENT_DENOMINATOR));
    }

    None
}

fn resolve_font_size_value(
    value: &str,
    parent_font_size_px: f32,
    root_font_size_px: f32,
) -> Option<f32> {
    resolve_css_length(
        value,
        parent_font_size_px,
        0.0,
        0.0,
        parent_font_size_px,
        root_font_size_px,
    )
}

fn resolve_line_height_value(
    value: &str,
    font_size_px: f32,
    root_font_size_px: f32,
) -> Option<f32> {
    let t = value.trim();
    if let Ok(multiplier) = t.parse::<f32>() {
        return Some(font_size_px * multiplier);
    }

    resolve_css_length(t, font_size_px, 0.0, 0.0, font_size_px, root_font_size_px)
}

fn parse_color(s: &str) -> Option<[f32; 4]> {
    let t = s.trim().to_lowercase();

    if t == "transparent" {
        return Some(color::TRANSPARENT);
    }

    if let Some(hex) = t.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()? as f32 / color::CHANNEL_MAX;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()? as f32 / color::CHANNEL_MAX;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()? as f32 / color::CHANNEL_MAX;
            return Some([r, g, b, color::OPAQUE_ALPHA]);
        }
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / color::CHANNEL_MAX;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / color::CHANNEL_MAX;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / color::CHANNEL_MAX;
            return Some([r, g, b, color::OPAQUE_ALPHA]);
        }
    }

    if t.starts_with("rgb(") || t.starts_with("rgba(") {
        let inside = t
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')')
            .to_string();
        let parts: Vec<&str> = inside.split(',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parse_rgb_component(parts[0])?;
        let g = parse_rgb_component(parts[1])?;
        let b = parse_rgb_component(parts[2])?;
        let a = if parts.len() >= 4 {
            parse_alpha(parts[3])?
        } else {
            color::OPAQUE_ALPHA
        };
        return Some([r, g, b, a]);
    }

    match t.as_str() {
        "black" => Some(color::BLACK),
        "white" => Some(color::WHITE),
        "gray" | "grey" => Some(color::GRAY),
        "red" => Some(color::RED),
        "green" => Some(color::GREEN),
        "blue" => Some(color::BLUE),
        "yellow" => Some([1.0, 1.0, 0.0, 1.0]),
        _ => None,
    }
}

fn parse_rgb_component(s: &str) -> Option<f32> {
    let t = s.trim();
    let value = if let Some(percent) = t.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else {
        t.parse::<f32>().ok()? / color::CHANNEL_MAX
    };
    Some(value.clamp(0.0, 1.0))
}

fn parse_alpha(s: &str) -> Option<f32> {
    let t = s.trim();
    let value = if let Some(percent) = t.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else {
        t.parse::<f32>().ok()?
    };
    Some(value.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_element_by_id<'a>(node: &'a StyledNode, id: &str) -> Option<&'a StyledNode> {
        if matches!(
            &node.node.node_type,
            NodeType::Element(element) if element.id() == Some(id)
        ) {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_element_by_id(child, id))
    }

    #[test]
    fn rgba_alpha_uses_css_alpha_range() {
        assert_eq!(
            parse_color("rgba(0, 0, 255, 0.5)").unwrap(),
            [0.0, 0.0, 1.0, 0.5]
        );
        assert_eq!(
            parse_color("rgba(0, 0, 255, 128)").unwrap(),
            [0.0, 0.0, 1.0, 1.0]
        );
    }

    #[test]
    fn rgba_components_are_clamped_and_percentages_parse() {
        assert_eq!(
            parse_color("rgba(300, -10, 0, 150%)").unwrap(),
            [1.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            parse_color("rgba(100%, 50%, 0%, -1)").unwrap(),
            [1.0, 0.5, 0.0, 0.0]
        );
    }

    #[test]
    fn inheritable_properties_are_centralized() {
        for prop in INHERITABLE_PROPS {
            assert!(is_inheritable_prop(prop));
        }
        assert!(!is_inheritable_prop("background"));
    }

    #[test]
    fn inline_style_attribute_overrides_stylesheet_values() {
        let dom = crate::html::parse(
            r#"<p><span id="target" style="background: #cfe; border: 2px solid red;">hello</span></p>"#
                .to_string(),
        );
        let stylesheet = crate::css::Parser::new(
            r#"#target { background: blue; border: 1px solid green; }"#.to_string(),
        )
        .parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let span = find_element_by_id(&styled, "target").expect("target span should exist");

        assert_eq!(
            span.value("background").map(|value| value.as_str()),
            Some("#cfe")
        );
        assert_eq!(
            span.value("border-width").map(|value| value.as_str()),
            Some("2px")
        );
        assert_eq!(
            span.value("border-color").map(|value| value.as_str()),
            Some("red")
        );
    }

    #[test]
    fn named_yellow_color_is_supported() {
        assert_eq!(parse_color("yellow"), Some([1.0, 1.0, 0.0, 1.0]));
    }

    #[test]
    fn child_selector_matches_only_direct_children() {
        let dom = crate::html::parse(
            r#"
            <main>
                <p id="direct">direct child</p>
                <section>
                    <p id="nested">nested child</p>
                </section>
            </main>
            "#
            .to_string(),
        );
        let stylesheet =
            crate::css::Parser::new(r#"main>p { color: red; }"#.to_string()).parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let direct = find_element_by_id(&styled, "direct").expect("direct paragraph should exist");
        let nested = find_element_by_id(&styled, "nested").expect("nested paragraph should exist");

        assert_eq!(
            direct.value("color").map(|value| value.as_str()),
            Some("red")
        );
        assert_eq!(nested.value("color").map(|value| value.as_str()), None);
    }

    #[test]
    fn child_selector_can_mix_with_descendant_selector() {
        let dom = crate::html::parse(
            r#"
            <section>
                <div>
                    <main>
                        <article>
                            <p id="target" class="note">hit</p>
                        </article>
                        <article>
                            <div>
                                <p id="nested" class="note">miss</p>
                            </div>
                        </article>
                    </main>
                </div>
            </section>
            "#
            .to_string(),
        );
        let stylesheet = crate::css::Parser::new(
            r#"section main > article > p.note { color: blue; }"#.to_string(),
        )
        .parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let target = find_element_by_id(&styled, "target").expect("target paragraph should exist");
        let nested = find_element_by_id(&styled, "nested").expect("nested paragraph should exist");

        assert_eq!(
            target.value("color").map(|value| value.as_str()),
            Some("blue")
        );
        assert_eq!(nested.value("color").map(|value| value.as_str()), None);
    }

    #[test]
    fn display_inline_block_is_supported() {
        let dom = crate::html::parse(r#"<p><span id="target">badge</span></p>"#.to_string());
        let stylesheet =
            crate::css::Parser::new(r#"#target { display: inline-block; }"#.to_string())
                .parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let target = find_element_by_id(&styled, "target").expect("target span should exist");

        assert_eq!(target.display(), Display::InlineBlock);
    }

    #[test]
    fn display_flex_is_supported() {
        let dom = crate::html::parse(r#"<div id="target"></div>"#.to_string());
        let stylesheet =
            crate::css::Parser::new(r#"#target { display: flex; }"#.to_string()).parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let target = find_element_by_id(&styled, "target").expect("target div should exist");

        assert_eq!(target.display(), Display::Flex);
    }

    #[test]
    fn computed_font_sizes_resolve_em_and_rem() {
        let dom = crate::html::parse(
            r#"
            <div id="outer">
                <span id="inner">inner</span>
                <span id="root-sized">root</span>
            </div>
            "#
            .to_string(),
        );
        let stylesheet = crate::css::Parser::new(
            r#"
            html { font-size: 20px; }
            #outer { font-size: 1.5em; }
            #inner { font-size: 2em; }
            #root-sized { font-size: 0.5rem; }
            "#
            .to_string(),
        )
        .parse_stylesheet();

        let styled = style_tree(dom, &stylesheet);
        let outer = find_element_by_id(&styled, "outer").expect("outer should exist");
        let inner = find_element_by_id(&styled, "inner").expect("inner should exist");
        let root_sized =
            find_element_by_id(&styled, "root-sized").expect("root-sized should exist");

        assert!((outer.font_size_px() - 30.0).abs() <= 0.01);
        assert!((inner.font_size_px() - 60.0).abs() <= 0.01);
        assert!((root_sized.font_size_px() - 10.0).abs() <= 0.01);
        assert!((inner.root_font_size_px() - 20.0).abs() <= 0.01);
        assert!((root_sized.root_font_size_px() - 20.0).abs() <= 0.01);
    }
}
