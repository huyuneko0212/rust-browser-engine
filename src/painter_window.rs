use minifb::{Key, Window, WindowOptions};

use crate::paint::{DisplayCommand};
use crate::layout::Rect;

pub fn run(display_list: Vec<DisplayCommand>) {
    let width = 800;
    let height = 600;

    let mut window = Window::new(
        "Rust Browser",
        width,
        height,
        WindowOptions::default(),
    )
    .unwrap();

    let mut buffer: Vec<u32> = vec![0xffffff; width * height];

    draw_display_list(&mut buffer, width, &display_list);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&buffer, width, height).unwrap();
    }
}

fn draw_display_list(buffer: &mut [u32], width: usize, list: &Vec<DisplayCommand>) {
    for cmd in list {
        match cmd {
            DisplayCommand::SolidColor(color, rect) => {
                let c = parse_color(color);
                draw_rect(buffer, width, rect, c);
            }
            DisplayCommand::Text(text, x, y) => {
                draw_text(buffer, width, text, *x as usize, *y as usize);
            }
        }
    }
}

fn parse_color(color: &str) -> u32 {
    match color.trim() {
        "red" => 0xff0000,
        "blue" => 0x0000ff,
        "green" => 0x00ff00,
        "black" => 0x000000,
        "gray" => 0x888888,
        _ => 0xcccccc,
    }
}

fn draw_rect(buffer: &mut [u32], width: usize, rect: &Rect, color: u32) {
    let x0 = rect.x as usize;
    let y0 = rect.y as usize;
    let w = rect.width as usize;
    let h = rect.height as usize;

    for y in y0..(y0 + h) {
        for x in x0..(x0 + w) {
            let idx = y * width + x;
            if idx < buffer.len() {
                buffer[idx] = color;
            }
        }
    }
}

// 超簡易テキスト描画（□表示）
fn draw_text(buffer: &mut [u32], width: usize, text: &str, x: usize, y: usize) {
    let mut offset = 0;

    for _ in text.chars() {
        for dy in 0..8 {
            for dx in 0..6 {
                let px = x + offset + dx;
                let py = y + dy;
                let idx = py * width + px;
                if idx < buffer.len() {
                    buffer[idx] = 0x000000;
                }
            }
        }
        offset += 8;
    }
}
