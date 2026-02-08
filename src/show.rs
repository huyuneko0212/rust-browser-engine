// use winit::{
//     event::*,
//     event_loop::{ControlFlow, EventLoop},
//     window::WindowBuilder,
// };

// use crate::layout::*;

// pub fn render(layout_root: LayoutBox) {

//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new()
//         .with_title("Rust Browser")
//         .build(&event_loop)
//         .unwrap();

//     event_loop.run(move |event, _, control_flow| {
//         *control_flow = ControlFlow::Wait;

//         match event {
//             Event::RedrawRequested(_) => {
//                 println!("描画開始");

//                 draw_layout(&layout_root, 0);
//             }

//             Event::WindowEvent {
//                 event: WindowEvent::CloseRequested,
//                 ..
//             } => *control_flow = ControlFlow::Exit,

//             Event::MainEventsCleared => {
//                 window.request_redraw();
//             }

//             _ => {}
//         }
//     });
// }

// fn draw_layout(layout: &LayoutBox, depth: usize) {
//     let indent = " ".repeat(depth * 2);

//     println!(
//         "{}box x={} y={} w={} h={}",
//         indent,
//         layout.dimensions.content.x,
//         layout.dimensions.content.y,
//         layout.dimensions.content.width,
//         layout.dimensions.content.height
//     );

//     for child in &layout.children {
//         draw_layout(child, depth + 1);
//     }
// }
