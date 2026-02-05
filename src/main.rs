mod dom;
mod url;
mod http;
mod show;
mod html;
mod css;
// use url::URL;
use css::Parser as CssParser;
// use crate::html::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("usage: cargo run https://example.org");
        return;
    }

    // let url = URL::new(&args[1]);
    // let response = http::request(&url);
    // let html = response.body;
    // let mut parser = Parser::new(html);
    // let nodes = parser.parse_nodes();
    // println!("{:#?}", nodes);

//     println!("status: {}",response.status_code);
//     println!("--- headers ---");
//     for (k, v) in &response.headers {
//         println!("{}: {}", k, v);
//     }
// println!("--------------");
    // show::show(&response.body);
    let css = "
        h1 { color: red; font-size: 20px; }
        p { color: blue; }
    ";

    let mut parser = CssParser::new(css.to_string());
    println!("1");
    let stylesheet = parser.parse_stylesheet();
    println!("2");

    println!("{:#?}", stylesheet);
}
