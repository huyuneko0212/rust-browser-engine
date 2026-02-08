use crate::display::DisplayItem;
use crate::gpu::GPU;

pub fn render(gpu: &mut GPU, items: &Vec<DisplayItem>) {
    gpu.render_items(items);
}


