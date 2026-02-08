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

use std::env;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::gpu::GPU;

fn extract_style_text(node: &dom::Node, out: &mut String) {
    match &node.node_type {
        dom::NodeType::Element(ed) => {
            if ed.tag_name == "style" {
                // style要素直下のTextを集める
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

fn main() {
    let url_str = env::args().nth(1).expect("url required");

    // -------------------------
    // ネットワーク → DOM
    // -------------------------
    let url = url::URL::new(&url_str);
    let response = http::request(&url);
    let body = response.body;

    println!("HTML取得完了");
    let dom_root = html::parse(body);
    println!("DOM生成完了");

    // -------------------------
    // CSS抽出（<style>）
    // -------------------------
    let mut css_text = String::new();
    extract_style_text(&dom_root, &mut css_text);
    println!("CSS抽出: {} bytes", css_text.len());

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

    layout_root.layout(viewport);
    println!("layout完了");

    // -------------------------
    // display list
    // -------------------------
    let font_bytes = std::fs::read("C:\\Windows\\Fonts\\meiryo.ttc").unwrap();
    let font = fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()).unwrap();
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
