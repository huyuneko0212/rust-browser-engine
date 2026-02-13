mod css;
mod dom;
mod html;
mod http;
mod layout;
mod style;
mod url;

mod display;
mod gpu;
mod render;

use std::collections::HashSet;
use std::env;

use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, WindowBuilder},
};

use crate::{display::DisplayItem, gpu::GPU};

// ★追加：最小UAスタイル（pxのみ）
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

// -------------------------
// <style> 抽出
// -------------------------
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
                for c in &node.children {
                    extract_style_text(c, out);
                }
            }
        }
        _ => {
            for c in &node.children {
                extract_style_text(c, out);
            }
        }
    }
}

// -------------------------
// <link rel="stylesheet" href="..."> 抽出
// -------------------------
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

            for c in &node.children {
                extract_link_stylesheets(c, out);
            }
        }
        _ => {
            for c in &node.children {
                extract_link_stylesheets(c, out);
            }
        }
    }
}

fn is_css_content_type(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_lowercase().contains("text/css"))
        .unwrap_or(false)
}

// -------------------------
// @import 展開（最小）
// -------------------------
fn extract_import_url(line: &str) -> Option<String> {
    let mut s = line.trim();

    if !s.starts_with("@import") {
        return None;
    }

    s = s.trim_start_matches("@import").trim();

    if let Some(x) = s.strip_suffix(';') {
        s = x.trim();
    }

    // url(...)
    if let Some(rest) = s.strip_prefix("url(") {
        let mut inner = rest.trim();
        if let Some(pos) = inner.find(')') {
            inner = inner[..pos].trim();
        } else {
            return None;
        }

        let inner = inner.trim().trim_matches('"').trim_matches('\'').trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        // "..." or '...' (mediaは無視)
        let first = s.split_whitespace().next().unwrap_or("").trim();
        let first = first.trim_matches('"').trim_matches('\'').trim();
        if first.is_empty() {
            None
        } else {
            Some(first.to_string())
        }
    }
}

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

    // 超簡易：行単位でimportを置換
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

            let import_url = base_url.resolve_location(h);

            let key = format!(
                "{}://{}:{}{}",
                import_url.scheme, import_url.host, import_url.port, import_url.path
            );

            if visited.contains(&key) {
                continue;
            }
            visited.insert(key);

            let resp = http::request(&import_url);
            if (200..300).contains(&resp.status_code) && is_css_content_type(&resp.content_type) {
                out.push_str("\n/* ---- @import expanded ---- */\n");
                let expanded = expand_css_imports(&import_url, &resp.body, visited, depth + 1);
                out.push_str(&expanded);
                out.push('\n');
            }

            // 元の @import 行は消す
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

// -------------------------
// リンクの当たり判定
// -------------------------
fn hit_test_link(display_list: &[DisplayItem], x: f32, y: f32) -> Option<String> {
    for item in display_list.iter().rev() {
        if let DisplayItem::Text(t) = item {
            if let Some(href) = &t.href {
                let r = &t.hit;
                if x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height {
                    return Some(href.clone());
                }
            }
        }
    }
    None
}

fn apply_hover(display_list: &mut [DisplayItem], hovered: Option<&str>) {
    for it in display_list.iter_mut() {
        match it {
            DisplayItem::Text(t) => {
                // まず元の色へ戻す
                t.color = t.base_color;

                // hover中ならリンクだけ濃くする
                if let (Some(h), Some(my)) = (hovered, t.href.as_deref()) {
                    if my == h {
                        t.color = darker(t.base_color, 0.75); // 0.75 = 濃く
                    }
                }
            }
            DisplayItem::Rect(r) => {
                // まず元の色へ戻す（背景も含む）
                r.color = r.base_color;

                // 下線だけ（href付き）hoverで濃く
                if let (Some(h), Some(my)) = (hovered, r.href.as_deref()) {
                    if my == h {
                        r.color = darker(r.base_color, 0.75);
                    }
                }
            }
        }
    }
}

fn darker(c: [f32; 4], factor: f32) -> [f32; 4] {
    // RGBだけ暗く、alphaは維持
    [c[0] * factor, c[1] * factor, c[2] * factor, c[3]]
}

// -------------------------
// 「ページを作る」処理を関数化
// -------------------------
fn build_page(url: &url::URL) -> Vec<DisplayItem> {
    // 1) HTML取得 → DOM
    let response = http::request(url);
    println!("HTML status: {}", response.status_code);

    let dom_root = html::parse(response.body);
    println!("DOM生成完了");

    // 2) CSS抽出（UA CSS + <style> + <link rel=stylesheet> + @import展開）
    let mut css_text = String::from(UA_CSS);
    css_text.push('\n');

    extract_style_text(&dom_root, &mut css_text);

    let mut css_links: Vec<String> = Vec::new();
    extract_link_stylesheets(&dom_root, &mut css_links);

    println!("inline <style>: {} bytes", css_text.len());
    println!("link stylesheets: {}", css_links.len());

    for href in css_links {
        let css_url = url.resolve_location(&href);

        println!(
            "fetch css -> {}://{}:{}{}",
            css_url.scheme, css_url.host, css_url.port, css_url.path
        );

        let resp = http::request(&css_url);

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
        let base_key = format!(
            "{}://{}:{}{}",
            css_url.scheme, css_url.host, css_url.port, css_url.path
        );
        visited.insert(base_key);

        let expanded = expand_css_imports(&css_url, &resp.body, &mut visited, 0);
        css_text.push_str(&expanded);
        css_text.push('\n');
    }

    // <style> 内にも @import が居る可能性があるので、最後に全体も展開
    {
        let mut visited = HashSet::new();
        let base_key = format!("{}://{}:{}{}", url.scheme, url.host, url.port, url.path);
        visited.insert(base_key);
        css_text = expand_css_imports(url, &css_text, &mut visited, 0);
    }

    println!("CSS total: {} bytes", css_text.len());

    // 3) CSS parse → style tree
    let stylesheet = css::Parser::new(css_text).parse_stylesheet();
    let styled_root = style::style_tree(dom_root, &stylesheet);

    // 4) layout tree
    let mut layout_root = layout::build_layout_tree(styled_root);

    let mut viewport = layout::Dimensions::default();
    viewport.content.width = 800.0;
    viewport.content.height = 600.0;

    let font_bytes = std::fs::read("C:\\Windows\\Fonts\\meiryo.ttc").unwrap();
    let font = fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()).unwrap();

    layout_root.layout_with_font(viewport, &font);
    println!("layout完了");

    // 5) display list
    let mut display_list = vec![];
    display::build_display_list(&layout_root, &mut display_list, &font);
    println!("display items: {}", display_list.len());

    display_list
}

struct BrowserState {
    url: url::URL,
    display_list: Vec<DisplayItem>,
}

impl BrowserState {
    fn new(initial: url::URL) -> Self {
        let display_list = build_page(&initial);
        Self {
            url: initial,
            display_list,
        }
    }

    fn navigate(&mut self, next: url::URL) {
        println!(
            "\n=== navigate -> {}://{}:{}{} ===",
            next.scheme, next.host, next.port, next.path
        );
        self.display_list = build_page(&next);
        self.url = next;
    }
}

fn main() {
    let url_str = env::args().nth(1).expect("url required");
    let initial_url = url::URL::new(&url_str);

    // window + gpu
    let event_loop = EventLoop::new().unwrap();

    let window: &'static winit::window::Window = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title("Rust Browser (winit0.29 + wgpu0.19)")
            .build(&event_loop)
            .unwrap(),
    ));

    let mut gpu = pollster::block_on(GPU::new(window));

    // ブラウザ状態
    let mut state = BrowserState::new(initial_url);

    // マウス座標
    let mut mouse_x = 0.0f32;
    let mut mouse_y = 0.0f32;
    // hoverしてるhref（変化検出用）
    let mut hovered_href: Option<String> = None;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(size) => gpu.resize(size),

                    WindowEvent::CursorMoved { position, .. } => {
                        mouse_x = position.x as f32;
                        mouse_y = position.y as f32;
                        let now = hit_test_link(&state.display_list, mouse_x, mouse_y);

                        // カーソル変更
                        window.set_cursor_icon(if now.is_some() {
                            CursorIcon::Pointer
                        } else {
                            CursorIcon::Default
                        });

                        // hover対象が変わったら色を更新
                        if now != hovered_href {
                            hovered_href = now;
                            apply_hover(&mut state.display_list, hovered_href.as_deref());
                        }
                    }

                    WindowEvent::MouseInput {
                        state: st, button, ..
                    } => {
                        if button == MouseButton::Left && st == ElementState::Pressed {
                            if let Some(href) = hit_test_link(&state.display_list, mouse_x, mouse_y)
                            {
                                // ★遷移前にhoverをリセット（色残り防止）
                                hovered_href = None;
                                apply_hover(&mut state.display_list, None);
                                window.set_cursor_icon(CursorIcon::Default);

                                let next = state.url.resolve_location(&href);
                                state.navigate(next);
                            }
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        render::render(&mut gpu, &state.display_list);
                    }

                    _ => {}
                },

                Event::AboutToWait => window.request_redraw(),
                _ => {}
            }
        })
        .unwrap();
}
