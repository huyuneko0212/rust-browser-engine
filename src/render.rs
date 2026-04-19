use crate::display::DisplayItem;
use crate::gpu::GPU;

pub fn render(gpu: &mut GPU, items: &Vec<DisplayItem>, scroll_y: f32) {
    gpu.render_items(items, scroll_y);
}
