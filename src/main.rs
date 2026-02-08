use std::env;

mod css;
mod dom;
mod html;
mod http;
mod layout;
mod show;
mod style;
mod url;

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

    println!("---- HTML取得完了 ----");

    // -------------------------------
    // CSS文字列（仮）
    // ※まだ <style> 抽出してないから固定
    // -------------------------------
    let css_string = "
        h1 { color: red; }
        p { color: blue; }
        div { color: green; }
    "
    .to_string();

    println!("---- CSSパース開始 ----");

    // -------------------------------
    // DOM構築
    // -------------------------------
    let dom = html::parse(body.clone());

    println!("---- DOM構築完了 ----");

    // -------------------------------
    // CSSパース
    // -------------------------------
    let stylesheet = css::Parser::new(css_string).parse_stylesheet();

    println!("---- CSS構築完了 ----");

    // -------------------------------
    // Style tree生成
    // -------------------------------
    // let styled_root = style::style_tree(dom, &stylesheet);
    // println!("---- STYLE TREE ----");
    // println!("{:#?}", styled_root);

    let dom = html::parse(body.clone());

    let css_string = "
    h1 { width:300px; height:40px; }
    p { width:500px; }
    "
    .to_string();

    let css = css::Parser::new(css_string).parse_stylesheet();

    let styled_root = style::style_tree(dom, &css);

    let mut layout_root = layout::build_layout_tree(styled_root);

    let viewport = layout::Dimensions {
        content: layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 0.0,
        },
        ..Default::default()
    };

    layout_root.layout(viewport);
    println!("{:#?}", layout_root);
    println!("---- 完了 ----");
}
