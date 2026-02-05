mod dom;
mod url;
mod http;
mod show;
mod html;
use url::URL;

use crate::html::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("usage: cargo run https://example.org");
        return;
    }

    let url = URL::new(&args[1]);
    let response = http::request(&url);
    let html = response.body;
    let mut parser = Parser::new(html);
    let nodes = parser.parse_nodes();
    println!("{:#?}", nodes);

//     println!("status: {}",response.status_code);
//     println!("--- headers ---");
//     for (k, v) in &response.headers {
//         println!("{}: {}", k, v);
//     }
// println!("--------------");
    // show::show(&response.body);
}
