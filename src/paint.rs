use crate::layout::{LayoutBox, Rect};

#[derive(Debug)]
pub enum DisplayCommand {
    SolidColor(String, Rect),
    Text(String, f32, f32),
}

pub fn build_display_list(layout_root: &LayoutBox) -> Vec<DisplayCommand> {
    let mut list = vec![];
    render_layout_box(layout_root, &mut list);
    list
}

fn render_layout_box(layout_box: &LayoutBox, list: &mut Vec<DisplayCommand>) {
    render_background(layout_box, list);
    render_text(layout_box, list);

    for child in &layout_box.children {
        render_layout_box(child, list);
    }
}
fn render_background(layout_box: &LayoutBox, list: &mut Vec<DisplayCommand>) {
    if let Some(style) = layout_box.get_style_node() {
        if let Some(color) = style.value("background") {
            list.push(DisplayCommand::SolidColor(
                color.clone(),
                layout_box.dimensions.content.clone(),
            ));
        }
    }
}

fn render_text(layout_box: &LayoutBox, list: &mut Vec<DisplayCommand>) {
    if let Some(style) = layout_box.get_style_node() {
        if let Some(text) = style.text() {
            let rect = &layout_box.dimensions.content;
            list.push(DisplayCommand::Text(
                text.to_string(),
                rect.x,
                rect.y + 16.0,
            ));
        }
    }
}
