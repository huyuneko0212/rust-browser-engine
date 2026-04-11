pub mod browser {
    pub const INITIAL_VIEWPORT_WIDTH: f32 = 800.0;
    pub const INITIAL_VIEWPORT_HEIGHT: f32 = 600.0;
    pub const LINE_SCROLL_PX: f32 = 40.0;
    pub const LINK_HOVER_DARKEN_FACTOR: f32 = 0.75;
    pub const MAX_CSS_IMPORT_DEPTH: usize = 10;
    pub const WINDOW_TITLE: &str = "Rust Browser (winit0.29 + wgpu0.19)";
    pub const UA_CSS: &str = r#"
/* --- minimal UA stylesheet (px only) --- */
html, body { display: block; margin: 8px; padding: 0; background: #ffffff; color: #111111; }
body { line-height: 1.35; }

a { color: #0645ad; text-decoration: underline; }
a:visited { color: #0b0080; }

h1 { display: block; font-size: 32px; margin: 16px 0; }
h2 { display: block; font-size: 24px; margin: 14px 0; }
h3 { display: block; font-size: 18px; margin: 12px 0; }

p { display: block; margin: 10px 0; }

ul, ol { display: block; margin: 10px 0 10px 18px; padding: 0; }
li { display: block; margin: 4px 0; }

small { font-size: 12px; }
"#;
}

pub mod color {
    pub const OPAQUE_ALPHA: f32 = 1.0;
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
    pub const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    pub const GRAY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
    pub const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    pub const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    pub const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

    pub const DEFAULT_TEXT: [f32; 4] = [0.1, 0.1, 0.12, 1.0];
    pub const DEFAULT_LINK: [f32; 4] = [0.0, 0.35, 0.95, 1.0];
    pub const CLEAR_BACKGROUND: wgpu::Color = wgpu::Color {
        r: 0.97,
        g: 0.97,
        b: 0.98,
        a: 1.0,
    };
    pub const CHANNEL_MAX: f32 = 255.0;
}

pub mod display {
    pub const UNDERLINE_THICKNESS: f32 = 1.5;
    pub const UNDERLINE_GAP: f32 = 2.0;
    pub const LIST_MARKER_OFFSET_EM: f32 = 1.1;
}

pub mod gpu {
    pub const MIN_SURFACE_SIZE_PX: u32 = 1;
    pub const MAX_FRAME_LATENCY: u32 = 2;
    pub const INITIAL_RECT_VERTEX_CAPACITY: usize = 12_000;
    pub const INITIAL_TEXT_VERTEX_CAPACITY: usize = 48_000;
    pub const INITIAL_IMAGE_VERTEX_CAPACITY: usize = 12_000;
    pub const GLYPH_ATLAS_SIZE_PX: u32 = 1024;
    pub const GLYPH_ATLAS_PADDING_PX: u32 = 1;
    pub const MIN_TEXT_SIZE_PX: f32 = 8.0;
    pub const VERTICES_PER_QUAD: usize = 6;
    pub const RGBA_BYTES_PER_PIXEL: u32 = 4;
    pub const FONT_COLLECTION_INDEX: u32 = 0;
}

pub mod layout {
    pub const DEFAULT_FONT_SIZE_PX: f32 = 16.0;
    pub const DEFAULT_LINE_HEIGHT_MULTIPLIER: f32 = 1.2;
    pub const MIN_LAYOUT_SIZE_PX: f32 = 1.0;
    pub const MIN_LINE_HEIGHT_PX: f32 = 18.0;
    pub const DEFAULT_VIEWPORT_HEIGHT_PX: f32 = 600.0;
    pub const DEFAULT_IMAGE_WIDTH_PX: f32 = 300.0;
    pub const DEFAULT_IMAGE_HEIGHT_PX: f32 = 150.0;
    pub const PERCENT_DENOMINATOR: f32 = 100.0;
}

pub mod network {
    pub const HTTP_MAX_REDIRECTS: usize = 10;
    pub const IMAGE_MAX_REDIRECTS: usize = 5;
    pub const SOCKET_TIMEOUT_SECS: u64 = 20;
    pub const BROTLI_BUFFER_SIZE: usize = 4096;
}

pub mod http_status {
    pub const REQUEST_FAILED: u16 = 0;
    pub const OK: u16 = 200;
    pub const NO_CONTENT: u16 = 204;
    pub const NOT_MODIFIED: u16 = 304;
    pub const INFORMATIONAL_MIN: u16 = 100;
    pub const INFORMATIONAL_MAX_EXCLUSIVE: u16 = 200;
    pub const SUCCESS_MIN: u16 = 200;
    pub const SUCCESS_MAX_EXCLUSIVE: u16 = 300;
    pub const REDIRECTS: [u16; 5] = [301, 302, 303, 307, 308];
}

pub mod protocol {
    pub const HTTP_PORT: u16 = 80;
    pub const HTTPS_PORT: u16 = 443;
    pub const FILE_PORT: u16 = 0;
}
