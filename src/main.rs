mod css;
mod dom;
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
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, WindowBuilder},
};

use crate::display::DisplayItem;
use crate::gpu::GPU;

use crate::utility::url_utils::{normalize_against, normalized_key_against, url_to_abs_string};

// 最小UAスタイル（pxのみ）
const UA_CSS: &str = r#"
/* --- minimal UA stylesheet (px only) --- */
html, body { display: block; margin: 8px; padding: 0; background: #ffffff; color: #111111; }
body { line-height: 1.35; }

a { color: #0645ad; text-decoration: underline; }
a:visited { color: #0b0080; }

h1 { display: block; font-size: 32px; margin: 16px 0; }
h2 { display: block; font-size: 24px; margin: 14px 0; }
h3 { display: block; font-size: 18px; margin: 12px 0; }

p { display: block; margin: 10px 0; }

ul, ol { display: block; margin: 10px 0 10px 18px; padding: 0; }
li { display: block; margin: 4px 0; }

small { font-size: 12px; }
"#;

// ------------------------------------------------------------
// HTML/CSS 抽出
// ------------------------------------------------------------

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

// ------------------------------------------------------------
// URL 正規化（main.rs では “resolve だけ” にして、文字列化は url_utils に任せる）
// ------------------------------------------------------------

fn expand_css_imports(
    base_url: &url::URL,
    css_text: &str,
    visited: &mut HashSet<String>,
    depth: usize,
) -> String {
    if depth > 10 {
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
            if resp.status_code == 0 || resp.body.is_empty() {
                continue;
            }
            if (200..300).contains(&resp.status_code) && is_css_content_type(&resp.content_type) {
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

// ------------------------------------------------------------
// hit test / scroll / hover
// ------------------------------------------------------------

fn hit_test_link(display_list: &[DisplayItem], x: f32, y: f32, scroll_y: f32) -> Option<String> {
    let doc_y = y + scroll_y;

    display_list.iter().rev().find_map(|item| match item {
        DisplayItem::Text(t) => t.href.as_ref().and_then(|href| {
            let r = &t.hit;
            (x >= r.x && x <= r.x + r.width && doc_y >= r.y && doc_y <= r.y + r.height)
                .then(|| href.clone())
        }),
        DisplayItem::Image(im) => im.href.as_ref().and_then(|href| {
            let r = &im.hit;
            (x >= r.x && x <= r.x + r.width && doc_y >= r.y && doc_y <= r.y + r.height)
                .then(|| href.clone())
        }),
        _ => None,
    })
}

fn estimate_doc_height(display_list: &[DisplayItem]) -> f32 {
    display_list.iter().fold(1.0f32, |max_y, it| match it {
        DisplayItem::Rect(r) => max_y.max(r.y + r.h),
        DisplayItem::Text(t) => max_y.max(t.hit.y + t.hit.height),
        DisplayItem::Image(im) => max_y.max(im.y + im.h),
        DisplayItem::Border(b) => max_y.max(b.y + b.h),
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
                    t.color = darker(t.base_color, 0.75);
                }
            }
            DisplayItem::Rect(r) => {
                r.color = r.base_color;
                if hovered.is_some_and(|h| r.href.as_deref() == Some(h)) {
                    r.color = darker(r.base_color, 0.75);
                }
            }
            DisplayItem::Image(_) => {}
            DisplayItem::Border(_) => {
                // 今のところ hover では何もしない
                // （必要になったら DrawBorder に base_color を持たせてここで暗くする）
            }
        }
    }
}

fn darker(c: [f32; 4], factor: f32) -> [f32; 4] {
    [c[0] * factor, c[1] * factor, c[2] * factor, c[3]]
}

// ------------------------------------------------------------
// 画像 natural size cache（base_url を持つ）
// ------------------------------------------------------------

#[derive(Clone)]
struct ImageCache {
    base_url: url::URL,
    sizes: HashMap<String, (u32, u32)>, // key: 正規化済み「絶対URL文字列」
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

// --- DOM から <img src="..."> を集める ---
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

// ------------------------------------------------------------
// ページ構築
// ------------------------------------------------------------

fn build_page(url: &url::URL) -> Vec<DisplayItem> {
    let response = crate::http::request_allow_error(&url);
    if response.status_code == 0 || response.body.is_empty() {
        return vec![];
    }
    println!("HTML status: {}", response.status_code);

    let dom_root = html::parse(response.body_text_lossy());
    println!("DOM生成完了");

    // 1) 画像の自然サイズ cache（正規化 key 統一）
    let mut img_cache = ImageCache::new(url.clone());
    {
        let mut srcs = Vec::new();
        extract_img_srcs(&dom_root, &mut srcs);

        // Rustっぽく：iterator + insert の戻り値で重複排除（setを使うのもOK）
        let mut seen = HashSet::new();
        for src in srcs {
            let Some(key) = normalized_key_against(url, &src) else {
                continue;
            };

            // insert が true のときだけ初登場（重複排除）
            if !seen.insert(key.clone()) {
                continue;
            }

            if let Some((w, h)) = image_loader::load_image_natural_size_px(&key) {
                img_cache.insert_size(key, w, h);
            }
        }
    }

    // 2) CSS（UA + inline + link + @import）
    let mut css_text = String::from(UA_CSS);
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
        if resp.status_code == 0 || resp.body.is_empty() {
            continue;
        }
        if !(200..300).contains(&resp.status_code) {
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
        visited.insert(url_to_abs_string(&css_url)); // visitedも正規化

        css_text.push_str(&expand_css_imports(
            &css_url,
            &resp.body_text_lossy(),
            &mut visited,
            0,
        ));
        css_text.push('\n');
    }

    // 最後に全体も @import 展開
    {
        let mut visited = HashSet::new();
        visited.insert(url_to_abs_string(url));
        css_text = expand_css_imports(url, &css_text, &mut visited, 0);
    }

    println!("CSS total: {} bytes", css_text.len());

    // 3) style/layout/display
    let stylesheet = css::Parser::new(css_text).parse_stylesheet();
    let styled_root = style::style_tree(dom_root, &stylesheet);
    let mut layout_root = layout::build_layout_tree(styled_root);

    let mut viewport = layout::Dimensions::default();
    viewport.content.width = 800.0;
    viewport.content.height = 600.0;

    let font_bytes = std::fs::read(r"C:\Windows\Fonts\meiryo.ttc").unwrap();
    let font = fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()).unwrap();

    layout_root.layout_with_font(viewport, &font, &img_cache);
    println!("layout完了");

    let mut display_list = vec![];
    display::build_display_list(&layout_root, &mut display_list, &font, url);
    println!("display items: {}", display_list.len());

    display_list
}

// ------------------------------------------------------------
// state / main loop
// ------------------------------------------------------------

struct BrowserState {
    url: url::URL,
    display_list: Vec<DisplayItem>,
    doc_height: f32,
}

impl BrowserState {
    fn new(initial: url::URL) -> Self {
        let display_list = build_page(&initial);
        let doc_height = estimate_doc_height(&display_list);
        Self {
            url: initial,
            display_list,
            doc_height,
        }
    }

    fn navigate(&mut self, next: url::URL) {
        println!("\n=== navigate -> {} ===", url_to_abs_string(&next));
        self.display_list = build_page(&next);
        self.doc_height = estimate_doc_height(&self.display_list);
        self.url = next;
    }
}

fn main() {
    let url_str = env::args().nth(1).expect("url required");
    let initial_url = url::URL::new(&url_str);

    let event_loop = EventLoop::new().unwrap();

    let window: &'static winit::window::Window = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title("Rust Browser (winit0.29 + wgpu0.19)")
            .build(&event_loop)
            .unwrap(),
    ));

    let mut gpu = pollster::block_on(GPU::new(window));
    let mut state = BrowserState::new(initial_url);

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
                        scroll_y = clamp_scroll(scroll_y, state.doc_height, gpu.viewport_height());
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        let dy = match delta {
                            MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                            MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                        };
                        scroll_y =
                            clamp_scroll(scroll_y + dy, state.doc_height, gpu.viewport_height());
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        mouse_x = position.x as f32;
                        mouse_y = position.y as f32;

                        let now = hit_test_link(&state.display_list, mouse_x, mouse_y, scroll_y);

                        window.set_cursor_icon(if now.is_some() {
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
                            if let Some(href) =
                                hit_test_link(&state.display_list, mouse_x, mouse_y, scroll_y)
                            {
                                let next = state.url.resolve_location(&href);
                                state.navigate(next);

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
