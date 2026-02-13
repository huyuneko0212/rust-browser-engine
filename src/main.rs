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
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::gpu::GPU;

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
            } else {
                // 取れない/非CSSは無視
            }

            // 元の @import 行は消す
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

fn main() {
    let url_str = env::args().nth(1).expect("url required");

    // -------------------------
    // ネットワーク → DOM
    // -------------------------
    let url = url::URL::new(&url_str);
    let response = http::request(&url);

    println!("HTML status: {}", response.status_code);
    let body = response.body;

    println!("HTML取得完了");
    let dom_root = html::parse(body);
    println!("DOM生成完了");

    // -------------------------
    // CSS抽出（<style> + <link rel=stylesheet> + @import展開）
    // -------------------------
    let mut css_text = String::new();

    // 1) <style>
    extract_style_text(&dom_root, &mut css_text);

    // 2) <link rel=stylesheet href=...>
    let mut css_links: Vec<String> = Vec::new();
    extract_link_stylesheets(&dom_root, &mut css_links);

    println!("inline <style>: {} bytes", css_text.len());
    println!("link stylesheets: {}", css_links.len());

    // 3) 外部CSS取得 + @import 展開
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
            println!("skip non-css content-type: {:?} ({})", resp.content_type, href);
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

    // ★ <style> 内にも @import が居る可能性があるので、最後に全体も展開しとく（安全）
    {
        let mut visited = HashSet::new();
        let base_key = format!("{}://{}:{}{}", url.scheme, url.host, url.port, url.path);
        visited.insert(base_key);
        css_text = expand_css_imports(&url, &css_text, &mut visited, 0);
    }

    println!("CSS total: {} bytes", css_text.len());

    // -------------------------
    // CSS parse → style tree
    // -------------------------
    let stylesheet = css::Parser::new(css_text).parse_stylesheet();
    let styled_root = style::style_tree(dom_root, &stylesheet);

    // -------------------------
    // layout tree
    // -------------------------
    let mut layout_root = layout::build_layout_tree(styled_root);

    let mut viewport = layout::Dimensions::default();
    viewport.content.width = 800.0;
    viewport.content.height = 600.0;

    let font_bytes = std::fs::read("C:\\Windows\\Fonts\\meiryo.ttc").unwrap();
    let font = fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()).unwrap();

    layout_root.layout_with_font(viewport, &font);
    println!("layout完了");

    // -------------------------
    // display list
    // -------------------------
    let mut display_list = vec![];
    display::build_display_list(&layout_root, &mut display_list, &font);
    println!("display items: {}", display_list.len());

    // -------------------------
    // window + gpu
    // -------------------------
    let event_loop = EventLoop::new().unwrap();

    let window: &'static winit::window::Window = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title("Rust Browser (winit0.29 + wgpu0.19)")
            .build(&event_loop)
            .unwrap(),
    ));

    let mut gpu = pollster::block_on(GPU::new(window));

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(size) => gpu.resize(size),
                    WindowEvent::RedrawRequested => {
                        render::render(&mut gpu, &display_list);
                    }
                    _ => {}
                },
                Event::AboutToWait => window.request_redraw(),
                _ => {}
            }
        })
        .unwrap();
}
