pub fn show(body: &str) {
    let mut in_tag = false;

    for c in body.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => print!("{}", c),
            _ => {}
        }
    }

    println!();
}
