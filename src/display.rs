use crate::layout::LayoutBox;

#[derive(Debug, Clone)]
pub enum DisplayItem {
    Rect(DrawRect),
    Text(DrawText),
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct DrawText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size_px: f32,
    pub color: [f32; 4],
}

pub fn build_display_list(root: &LayoutBox, out: &mut Vec<DisplayItem>) {
    fn walk(node: &LayoutBox, out: &mut Vec<DisplayItem>) {
        // style/script は描画しない（配下も止める）
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Element(ed) = &sn.node.node_type {
                if ed.tag_name == "style" || ed.tag_name == "script" {
                    return;
                }
            }
        }

        let c = &node.dimensions.content;

        // 背景色：指定があれば描く（なければ描かない）
        if c.width > 0.0 && c.height > 0.0 {
            if let Some(sn) = node.get_style_node() {
                if let Some(bg) = sn.background_color() {
                    out.push(DisplayItem::Rect(DrawRect {
                        x: c.x,
                        y: c.y,
                        w: c.width,
                        h: c.height,
                        color: bg,
                    }));
                }
            }
        }

        // Text：ここで “簡易 inline layout (折り返し)” をやる
        if let Some(sn) = node.get_style_node() {
            if let crate::dom::NodeType::Text(t) = &sn.node.node_type {
                let txt = t.trim();
                if !txt.is_empty() && c.width > 1.0 {
                    let color = sn.color().unwrap_or([0.1, 0.1, 0.12, 1.0]);

                    // font-size / line-height
                    let font_size = font_size_px(sn).unwrap_or(16.0);
                    let line_h = line_height_px(sn, font_size);

                    // テキスト流し込み開始位置（簡易：contentの左上 + ちょい下げ）
                    let start_x = c.x;
                    let start_y = c.y + font_size; // baselineっぽく（暫定）

                    // 折り返し幅（content幅）
                    let max_w = c.width.max(1.0);

                    // tokens：英語は単語、日本語は文字
                    let items = layout_text_naive(txt, start_x, start_y, max_w, font_size, line_h);

                    for (x, y, s) in items {
                        if !s.is_empty() {
                            out.push(DisplayItem::Text(DrawText {
                                x,
                                y,
                                text: s,
                                size_px: font_size,
                                color,
                            }));
                        }
                    }
                }
            }
        }

        for child in &node.children {
            walk(child, out);
        }
    }

    walk(root, out);
}

// ---------------------------
// CSS helpers
// ---------------------------

fn font_size_px(sn: &crate::style::StyledNode) -> Option<f32> {
    sn.value("font-size").and_then(|v| parse_px(v))
}

fn line_height_px(sn: &crate::style::StyledNode, font_size: f32) -> f32 {
    if let Some(v) = sn.value("line-height") {
        if let Some(px) = parse_px(v) {
            return px;
        }
        // "1.2" みたいな倍率
        if let Ok(m) = v.trim().parse::<f32>() {
            return font_size * m;
        }
    }
    font_size * 1.2
}

fn parse_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(num) = t.strip_suffix("px") {
        return num.trim().parse::<f32>().ok();
    }
    None
}

// ---------------------------
// inline layout (naive)
// ---------------------------

fn layout_text_naive(
    text: &str,
    start_x: f32,
    start_y: f32,
    max_w: f32,
    font_size: f32,
    line_h: f32,
) -> Vec<(f32, f32, String)> {
    let mut out = vec![];

    let mut x = start_x;
    let mut y = start_y;

    // 空白が多いなら単語、なければ文字
    let has_spaces = text.contains(' ');
    let tokens: Vec<String> = if has_spaces {
        // split_whitespace すると空白が消えるので、単語間スペースは後で足す
        text.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        text.chars().map(|c| c.to_string()).collect()
    };

    for tok in tokens {
        let w = estimate_width(&tok, font_size);

        // 折り返し
        if x > start_x && x + w > start_x + max_w {
            x = start_x;
            y += line_h;
        }

        out.push((x, y, tok.clone()));

        // x更新
        x += w;

        // 単語ならスペース相当を足す（日本語は足さない）
        if has_spaces {
            x += estimate_width(" ", font_size);
        }
    }

    out
}

/// まずは係数で推定（次で fontdue 実測に置き換える）
fn estimate_width(s: &str, font_size: f32) -> f32 {
    let mut w = 0.0;
    for ch in s.chars() {
        // ASCIIは細め、CJKは太め
        if ch.is_ascii() {
            w += font_size * 0.6;
        } else {
            w += font_size * 1.0;
        }
    }
    w
}
