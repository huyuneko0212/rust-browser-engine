use std::fs;

use fontdue::Font;
use pixels::{Pixels, SurfaceTexture};
use winit::event_loop::ControlFlow;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub fn run_text(text: String) {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Rust Browser")
        .with_inner_size(LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let size = window.inner_size();
    let surface_texture = SurfaceTexture::new(size.width, size.height, &window);
    let mut pixels = Pixels::new(size.width, size.height, surface_texture).unwrap();

    // フォント読み込み
    let font_data = fs::read("assets/DejaVuSans.ttf").expect("font not found");
    let font = Font::from_bytes(font_data, fontdue::FontSettings::default()).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,

            Event::RedrawRequested(_) => {
                let frame = pixels.frame_mut();

                // 背景白
                for pixel in frame.chunks_exact_mut(4) {
                    pixel[0] = 255;
                    pixel[1] = 255;
                    pixel[2] = 255;
                    pixel[3] = 255;
                }

                draw_text(frame, size.width, 20, 60, &text, &font, 24.0);

                pixels.render().unwrap();
            }

            Event::MainEventsCleared => {
                window.request_redraw();
            }

            _ => {}
        }
    });
}

fn draw_text(
    frame: &mut [u8],
    width: u32,
    start_x: i32,
    start_y: i32,
    text: &str,
    font: &Font,
    size: f32,
) {
    let mut pen_x = start_x;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);

        for y in 0..metrics.height {
            for x in 0..metrics.width {
                let alpha = bitmap[y * metrics.width + x];
                if alpha == 0 {
                    continue;
                }

                let px = pen_x + x as i32;
                let py = start_y + y as i32;

                if px < 0 || py < 0 {
                    continue;
                }

                let idx = ((py as u32 * width + px as u32) * 4) as usize;
                if idx + 3 >= frame.len() {
                    continue;
                }

                frame[idx] = 0;
                frame[idx + 1] = 0;
                frame[idx + 2] = 0;
                frame[idx + 3] = alpha;
            }
        }

        pen_x += metrics.advance_width as i32;
    }
}
