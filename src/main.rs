mod config;
mod constants;
mod css;
mod dom;
mod font;
mod html;
mod http;
mod layout;
mod style;
mod url;

mod display;
mod gpu;
mod image_loader;
mod render;
mod utility;

use std::collections::{HashMap, HashSet};
use std::env;

use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, WindowBuilder},
};

use crate::display::DisplayItem;
use crate::gpu::GPU;

use crate::constants::{browser, http_status};
use crate::utility::url_utils::{normalize_against, normalized_key_against, url_to_abs_string};

fn extract_style_text(node: &dom::Node, out: &mut String) {
    match &node.node_type {
        dom::NodeType::Element(ed) => {
            if ed.tag_name == "style" {
                for c in &node.children {
                    if let dom::NodeType::Text(t) = &c.node_type {
                        out.push_str(t);
                        out.push('\n');
                    }
                }
            } else {
                node.children
                    .iter()
                    .for_each(|c| extract_style_text(c, out));
            }
        }
        _ => node
            .children
            .iter()
            .for_each(|c| extract_style_text(c, out)),
    }
}

fn extract_link_stylesheets(node: &dom::Node, out: &mut Vec<String>) {
    match &node.node_type {
        dom::NodeType::Element(ed) => {
            if ed.tag_name == "link" {
                let rel = ed
                    .attributes
                    .get("rel")
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();

                if rel.contains("stylesheet") {
                    if let Some(href) = ed.attributes.get("href") {
                        let h = href.trim();
                        if !h.is_empty()
                            && !h.starts_with('#')
                            && !h.to_lowercase().starts_with("javascript:")
                            && !h.to_lowercase().starts_with("data:")
                        {
                            out.push(h.to_string());
                        }
                    }
                }
            }
            node.children
                .iter()
                .for_each(|c| extract_link_stylesheets(c, out));
        }
        _ => node
            .children
            .iter()
            .for_each(|c| extract_link_stylesheets(c, out)),
    }
}

fn is_css_content_type(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_lowercase().contains("text/css"))
        .unwrap_or(false)
}

fn extract_import_url(line: &str) -> Option<String> {
    let mut s = line.trim();
    if !s.starts_with("@import") {
        return None;
    }
    s = s.trim_start_matches("@import").trim();
    if let Some(x) = s.strip_suffix(';') {
        s = x.trim();
    }

    if let Some(rest) = s.strip_prefix("url(") {
        let inner = rest.trim();
        let pos = inner.find(')')?;
        let inner = inner[..pos]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        (!inner.is_empty()).then(|| inner.to_string())
    } else {
        let first = s.split_whitespace().next().unwrap_or("").trim();
        let first = first.trim_matches('"').trim_matches('\'').trim();
        (!first.is_empty()).then(|| first.to_string())
    }
}

fn expand_css_imports(
    base_url: &url::URL,
    css_text: &str,
    visited: &mut HashSet<String>,
    depth: usize,
) -> String {
    if depth > browser::MAX_CSS_IMPORT_DEPTH {
        return css_text.to_string();
    }

    let mut out = String::new();

    for line in css_text.lines() {
        if let Some(href) = extract_import_url(line) {
            let h = href.trim();
            if h.is_empty()
                || h.starts_with('#')
                || h.to_lowercase().starts_with("javascript:")
                || h.to_lowercase().starts_with("data:")
            {
                continue;
            }

            let import_url = normalize_against(base_url, h);
            let key = url_to_abs_string(&import_url);

            if !visited.insert(key) {
                continue;
            }

            let resp = crate::http::request_allow_error(&import_url);
            if resp.status_code == http_status::REQUEST_FAILED || resp.body.is_empty() {
                continue;
            }
            if (http_status::SUCCESS_MIN..http_status::SUCCESS_MAX_EXCLUSIVE)
                .contains(&resp.status_code)
                && is_css_content_type(&resp.content_type)
            {
                out.push_str("\n/* ---- @import expanded ---- */\n");
                out.push_str(&expand_css_imports(
                    &import_url,
                    &resp.body_text_lossy(),
                    visited,
                    depth + 1,
                ));
                out.push('\n');
            }
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

fn hit_test_link(display_list: &[DisplayItem], x: f32, y: f32, scroll_y: f32) -> Option<String> {
    let doc_y = y + scroll_y;

    display_list.iter().rev().find_map(|item| match item {
        DisplayItem::Text(t) => t.href.as_ref().and_then(|href| {
            let r = &t.hit;
            let test_y = if t.fixed { y } else { doc_y };
            (x >= r.x && x <= r.x + r.width && test_y >= r.y && test_y <= r.y + r.height)
                .then(|| href.clone())
        }),
        DisplayItem::Image(im) => im.href.as_ref().and_then(|href| {
            let r = &im.hit;
            let test_y = if im.fixed { y } else { doc_y };
            (x >= r.x && x <= r.x + r.width && test_y >= r.y && test_y <= r.y + r.height)
                .then(|| href.clone())
        }),
        _ => None,
    })
}

fn hit_test_form_submit(
    display_list: &[DisplayItem],
    x: f32,
    y: f32,
    scroll_y: f32,
) -> Option<style::FormSubmit> {
    let doc_y = y + scroll_y;

    display_list.iter().rev().find_map(|item| match item {
        DisplayItem::Text(t) => t.form_submit.as_ref().and_then(|submit| {
            let r = &t.hit;
            let test_y = if t.fixed { y } else { doc_y };
            (x >= r.x && x <= r.x + r.width && test_y >= r.y && test_y <= r.y + r.height)
                .then(|| submit.clone())
        }),
        DisplayItem::Rect(r) => r.form_submit.as_ref().and_then(|submit| {
            let test_y = if r.fixed { y } else { doc_y };
            (x >= r.x && x <= r.x + r.w && test_y >= r.y && test_y <= r.y + r.h)
                .then(|| submit.clone())
        }),
        _ => None,
    })
}

fn form_submission_url(base_url: &url::URL, submit: &style::FormSubmit) -> Option<url::URL> {
    if !submit.method.eq_ignore_ascii_case("get") {
        println!("unsupported form method: {}", submit.method);
        return None;
    }

    let mut target = submit
        .action
        .as_deref()
        .map(|action| base_url.resolve_location(action))
        .unwrap_or_else(|| base_url.clone());
    let query = encode_form_fields(&submit.fields);

    if !query.is_empty() {
        let separator = if target.path.contains('?') { '&' } else { '?' };
        target.path.push(separator);
        target.path.push_str(&query);
    }

    Some(target)
}

fn encode_form_fields(fields: &[style::FormField]) -> String {
    fields
        .iter()
        .map(|field| {
            format!(
                "{}={}",
                percent_encode_form_component(&field.name),
                percent_encode_form_component(&field.value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_encode_form_component(value: &str) -> String {
    let mut out = String::new();

    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_submission_url_builds_get_query() {
        let base = url::URL::new("https://example.com/index.html");
        let submit = style::FormSubmit {
            action: Some("/search".to_string()),
            method: "get".to_string(),
            fields: vec![
                style::FormField {
                    name: "q".to_string(),
                    value: "rust browser".to_string(),
                },
                style::FormField {
                    name: "lang".to_string(),
                    value: "ja".to_string(),
                },
            ],
        };

        let next = form_submission_url(&base, &submit).expect("GET form should produce url");

        assert_eq!(next.scheme, "https");
        assert_eq!(next.host, "example.com");
        assert_eq!(next.path, "/search?q=rust+browser&lang=ja");
    }

    #[test]
    fn form_submission_url_uses_current_url_without_action() {
        let base = url::URL::new("https://example.com/page.html?old=1");
        let submit = style::FormSubmit {
            action: None,
            method: "get".to_string(),
            fields: vec![style::FormField {
                name: "q".to_string(),
                value: "rust".to_string(),
            }],
        };

        let next = form_submission_url(&base, &submit).expect("GET form should produce url");

        assert_eq!(next.path, "/page.html?old=1&q=rust");
    }
}

fn estimate_doc_height(display_list: &[DisplayItem]) -> f32 {
    display_list.iter().fold(1.0f32, |max_y, it| match it {
        DisplayItem::Rect(r) if !r.fixed => max_y.max(r.y + r.h),
        DisplayItem::Text(t) if !t.fixed => max_y.max(t.hit.y + t.hit.height),
        DisplayItem::Image(im) if !im.fixed => max_y.max(im.y + im.h),
        DisplayItem::Border(b) if !b.fixed => max_y.max(b.y + b.h),
        _ => max_y,
    })
}

fn clamp_scroll(scroll_y: f32, doc_h: f32, view_h: f32) -> f32 {
    let max_scroll = (doc_h - view_h).max(0.0);
    scroll_y.clamp(0.0, max_scroll)
}

fn apply_hover(display_list: &mut [DisplayItem], hovered: Option<&str>) {
    for it in display_list.iter_mut() {
        match it {
            DisplayItem::Text(t) => {
                t.color = t.base_color;
                if hovered.is_some_and(|h| t.href.as_deref() == Some(h)) {
                    t.color = darker(t.base_color, browser::LINK_HOVER_DARKEN_FACTOR);
                }
            }
            DisplayItem::Rect(r) => {
                r.color = r.base_color;
                if hovered.is_some_and(|h| r.href.as_deref() == Some(h)) {
                    r.color = darker(r.base_color, browser::LINK_HOVER_DARKEN_FACTOR);
                }
            }
            DisplayItem::Image(_) => {}
            DisplayItem::Border(_) => {}
        }
    }
}

fn darker(c: [f32; 4], factor: f32) -> [f32; 4] {
    [c[0] * factor, c[1] * factor, c[2] * factor, c[3]]
}

#[derive(Clone)]
struct ImageCache {
    base_url: url::URL,
    sizes: HashMap<String, (u32, u32)>,
}

impl ImageCache {
    fn new(base_url: url::URL) -> Self {
        Self {
            base_url,
            sizes: HashMap::new(),
        }
    }

    fn insert_size(&mut self, key: String, w: u32, h: u32) {
        self.sizes.insert(key, (w, h));
    }
}

impl layout::ImageSizeProvider for ImageCache {
    fn normalize_src_key(&self, src: &str) -> Option<String> {
        let src = src.trim();
        if src.is_empty() {
            None
        } else {
            normalized_key_against(&self.base_url, src)
        }
    }

    fn natural_size_px(&self, key: &str) -> Option<(u32, u32)> {
        self.sizes.get(key).copied()
    }
}

struct PageDocument {
    url: url::URL,
    styled_root: style::StyledNode,
    img_cache: ImageCache,
}

fn extract_img_srcs(node: &dom::Node, out: &mut Vec<String>) {
    match &node.node_type {
        dom::NodeType::Element(ed) => {
            if ed.tag_name == "img" {
                if let Some(src) = ed.attributes.get("src") {
                    let s = src.trim();
                    if !s.is_empty()
                        && !s.to_lowercase().starts_with("data:")
                        && !s.to_lowercase().starts_with("javascript:")
                    {
                        out.push(s.to_string());
                    }
                }
            }
            node.children.iter().for_each(|c| extract_img_srcs(c, out));
        }
        _ => node.children.iter().for_each(|c| extract_img_srcs(c, out)),
    }
}

fn fetch_page_document(url: &url::URL) -> Option<PageDocument> {
    let response = crate::http::request_allow_error(&url);
    if response.status_code == http_status::REQUEST_FAILED || response.body.is_empty() {
        return None;
    }
    println!("HTML status: {}", response.status_code);

    let charset = response
        .charset_from_header()
        .or_else(|| {
            let temp_dom = html::parse(response.body_text_lossy());
            html::extract_charset(&temp_dom)
        })
        .unwrap_or_else(|| "utf-8".to_string());
    println!("Detected charset: {}", charset);

    let html_text = response.body_text_with_charset(&charset);
    let dom_root = html::parse(html_text);
    println!("DOM生成完了");

    let mut img_cache = ImageCache::new(url.clone());
    {
        let mut srcs = Vec::new();
        extract_img_srcs(&dom_root, &mut srcs);

        let mut seen = HashSet::new();
        for src in srcs {
            let Some(key) = normalized_key_against(url, &src) else {
                continue;
            };

            if !seen.insert(key.clone()) {
                continue;
            }

            if let Some((w, h)) = image_loader::load_image_natural_size_px(&key) {
                img_cache.insert_size(key, w, h);
            }
        }
    }

    let mut css_text = String::from(browser::UA_CSS);
    css_text.push('\n');
    extract_style_text(&dom_root, &mut css_text);

    let mut css_links = Vec::new();
    extract_link_stylesheets(&dom_root, &mut css_links);

    println!("inline <style>: {} bytes", css_text.len());
    println!("link stylesheets: {}", css_links.len());

    for href in css_links {
        let css_url = normalize_against(url, &href);
        println!("fetch css -> {}", url_to_abs_string(&css_url));

        let resp = crate::http::request_allow_error(&css_url);
        if resp.status_code == http_status::REQUEST_FAILED || resp.body.is_empty() {
            continue;
        }
        if !(http_status::SUCCESS_MIN..http_status::SUCCESS_MAX_EXCLUSIVE)
            .contains(&resp.status_code)
        {
            println!("css fetch failed (status {}): {}", resp.status_code, href);
            continue;
        }
        if !is_css_content_type(&resp.content_type) {
            println!(
                "skip non-css content-type: {:?} ({})",
                resp.content_type, href
            );
            continue;
        }

        css_text.push_str("\n/* ---- external stylesheet ---- */\n");

        let mut visited = HashSet::new();
        visited.insert(url_to_abs_string(&css_url));

        css_text.push_str(&expand_css_imports(
            &css_url,
            &resp.body_text_lossy(),
            &mut visited,
            0,
        ));
        css_text.push('\n');
    }

    {
        let mut visited = HashSet::new();
        visited.insert(url_to_abs_string(url));
        css_text = expand_css_imports(url, &css_text, &mut visited, 0);
    }

    println!("CSS total: {} bytes", css_text.len());

    let stylesheet = css::Parser::new(css_text).parse_stylesheet();
    let styled_root = style::style_tree(dom_root, &stylesheet);

    Some(PageDocument {
        url: url.clone(),
        styled_root,
        img_cache,
    })
}

fn build_display_list_for_viewport(
    page: &PageDocument,
    viewport_width: f32,
    viewport_height: f32,
    font: &fontdue::Font,
) -> Vec<DisplayItem> {
    let mut layout_root = layout::build_layout_tree(&page.styled_root);
    let mut viewport = layout::Dimensions::default();
    viewport.content.width = viewport_width;
    viewport.content.height = viewport_height;

    layout_root.layout_with_font(viewport, font, &page.img_cache);
    println!("layout完了");

    let mut display_list = vec![];
    display::build_display_list(&layout_root, &mut display_list, font, &page.url);
    println!("display items: {}", display_list.len());

    display_list
}

fn load_layout_font() -> fontdue::Font {
    crate::font::load_default_ui_font()
}

struct BrowserState {
    url: url::URL,
    page: Option<PageDocument>,
    display_list: Vec<DisplayItem>,
    doc_height: f32,
}

impl BrowserState {
    fn new(
        initial: url::URL,
        viewport_width: f32,
        viewport_height: f32,
        font: &fontdue::Font,
    ) -> Self {
        let mut state = Self {
            url: initial,
            page: None,
            display_list: vec![],
            doc_height: 0.0,
        };
        state.load_current_url(viewport_width, viewport_height, font);
        state
    }

    fn navigate(
        &mut self,
        next: url::URL,
        viewport_width: f32,
        viewport_height: f32,
        font: &fontdue::Font,
    ) {
        println!("\n=== navigate -> {} ===", url_to_abs_string(&next));
        self.url = next;
        self.load_current_url(viewport_width, viewport_height, font);
    }

    fn load_current_url(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        font: &fontdue::Font,
    ) {
        self.page = fetch_page_document(&self.url);
        self.relayout(viewport_width, viewport_height, font);
    }

    fn relayout(&mut self, viewport_width: f32, viewport_height: f32, font: &fontdue::Font) {
        self.display_list = self
            .page
            .as_ref()
            .map(|page| {
                build_display_list_for_viewport(page, viewport_width, viewport_height, font)
            })
            .unwrap_or_default();
        self.doc_height = estimate_doc_height(&self.display_list);
    }
}

fn main() {
    let url_str = env::args().nth(1).expect("url required");
    let initial_url = url::URL::new(&url_str);

    let event_loop = EventLoop::new().unwrap();
    let layout_font = load_layout_font();

    let window: &'static winit::window::Window = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title(browser::WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(
                browser::INITIAL_VIEWPORT_WIDTH as f64,
                browser::INITIAL_VIEWPORT_HEIGHT as f64,
            ))
            .build(&event_loop)
            .unwrap(),
    ));

    let mut gpu = pollster::block_on(GPU::new(window));
    let mut state = BrowserState::new(
        initial_url,
        gpu.viewport_width(),
        gpu.viewport_height(),
        &layout_font,
    );

    let mut mouse_x = 0.0f32;
    let mut mouse_y = 0.0f32;
    let mut hovered_href: Option<String> = None;
    let mut scroll_y = 0.0f32;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),

                    WindowEvent::Resized(size) => {
                        gpu.resize(size);
                        if size.width > 0 && size.height > 0 {
                            state.relayout(
                                gpu.viewport_width(),
                                gpu.viewport_height(),
                                &layout_font,
                            );
                            scroll_y =
                                clamp_scroll(scroll_y, state.doc_height, gpu.viewport_height());

                            let now =
                                hit_test_link(&state.display_list, mouse_x, mouse_y, scroll_y);
                            if now != hovered_href {
                                hovered_href = now;
                            }
                            apply_hover(&mut state.display_list, hovered_href.as_deref());
                        }
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        let dy = match delta {
                            MouseScrollDelta::LineDelta(_, y) => -y * browser::LINE_SCROLL_PX,
                            MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                        };
                        scroll_y =
                            clamp_scroll(scroll_y + dy, state.doc_height, gpu.viewport_height());
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        mouse_x = position.x as f32;
                        mouse_y = position.y as f32;

                        let now = hit_test_link(&state.display_list, mouse_x, mouse_y, scroll_y);
                        let form_submit =
                            hit_test_form_submit(&state.display_list, mouse_x, mouse_y, scroll_y);

                        window.set_cursor_icon(if now.is_some() || form_submit.is_some() {
                            CursorIcon::Pointer
                        } else {
                            CursorIcon::Default
                        });

                        if now != hovered_href {
                            hovered_href = now;
                            apply_hover(&mut state.display_list, hovered_href.as_deref());
                        }
                    }

                    WindowEvent::MouseInput {
                        state: st, button, ..
                    } => {
                        if button == MouseButton::Left && st == ElementState::Pressed {
                            if let Some(submit) =
                                hit_test_form_submit(&state.display_list, mouse_x, mouse_y, scroll_y)
                            {
                                if let Some(next) = form_submission_url(&state.url, &submit) {
                                    state.navigate(
                                        next,
                                        gpu.viewport_width(),
                                        gpu.viewport_height(),
                                        &layout_font,
                                    );

                                    scroll_y = 0.0;
                                    hovered_href = None;
                                    apply_hover(&mut state.display_list, None);

                                    scroll_y = clamp_scroll(
                                        scroll_y,
                                        state.doc_height,
                                        gpu.viewport_height(),
                                    );
                                }
                            } else if let Some(href) =
                                hit_test_link(&state.display_list, mouse_x, mouse_y, scroll_y)
                            {
                                let next = state.url.resolve_location(&href);
                                state.navigate(
                                    next,
                                    gpu.viewport_width(),
                                    gpu.viewport_height(),
                                    &layout_font,
                                );

                                scroll_y = 0.0;
                                hovered_href = None;
                                apply_hover(&mut state.display_list, None);

                                scroll_y =
                                    clamp_scroll(scroll_y, state.doc_height, gpu.viewport_height());
                            }
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        render::render(&mut gpu, &state.display_list, scroll_y);
                    }

                    _ => {}
                },
                Event::AboutToWait => window.request_redraw(),
                _ => {}
            }
        })
        .unwrap();
}
