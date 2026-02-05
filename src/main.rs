mod url;
mod http;
mod show;

use url::URL;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("usage: cargo run https://example.org");
        return;
    }

    let url = URL::new(&args[1]);
    let body = http::request(&url);
    show::show(&body);
}
