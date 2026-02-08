use std::env;

mod css;
mod dom;
mod html;
mod http;
mod layout;
mod paint;
mod painter_window;
mod show;
mod style;
mod url;
mod renderer;

use url::URL;

fn main() {
    // -------------------------------
    // 引数取得
    // -------------------------------
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("usage: cargo run <url>");
        return;
    }

    let url_str = &args[1];
    let url = URL::new(url_str);

    // println!("fetching: {}", url_str);

    // -------------------------------
    // HTML取得
    // -------------------------------
    let response = http::request(&url);
    let body = response.body;

    // -------------------------------
    // Style tree生成
    // -------------------------------
    // let styled_root = style::style_tree(dom, &stylesheet);
    // println!("---- STYLE TREE ----");
    // println!("{:#?}", styled_root);

    let css_string = "
    h1 { width:300px; height:40px; }
    p { width:500px; }
    "
    .to_string();

    let dom = html::parse(body.clone());
    let css = css::Parser::new(css_string).parse_stylesheet();
    let styled_root = style::style_tree(dom, &css);
    let mut layout_root = layout::build_layout_tree(styled_root);
    let mut viewport = layout::Dimensions::default();
    viewport.content.width = 800.0;
    layout_root.layout(viewport);
    let display_list = paint::build_display_list(&layout_root);
    
    let body = "Rust Browser Engine".to_string();
    renderer::run_text(body);
    // painter_window::run(display_list);

    println!("---- 完了 ----");
}
