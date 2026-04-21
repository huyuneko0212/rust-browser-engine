use std::fs;
use std::path::Path;

use fontdue::{Font, FontSettings};

pub fn load_default_ui_font() -> Font {
    if let Some(font_path) = &crate::config::app_config().font_path {
        return load_configured_font(font_path);
    }

    load_os_default_ui_font()
}

fn load_configured_font(font_path: &Path) -> Font {
    let font = load_font_from_path(font_path, 0).unwrap_or_else(|err| {
        panic!(
            "failed to load configured font `{}`: {err}",
            font_path.display()
        )
    });

    println!("UI font loaded from browser.conf: {}", font_path.display());
    font
}

fn load_os_default_ui_font() -> Font {
    let mut attempted = Vec::new();

    for candidate in system_font_candidates() {
        attempted.push(candidate.path.to_string());

        if let Ok(font) = load_font_from_path(Path::new(candidate.path), candidate.collection_index) {
            println!(
                "UI font loaded: {} ({})",
                candidate.label,
                candidate.path
            );
            return font;
        }
    }

    if let Some(fallback_path) = crate::config::find_existing_file("assets/DejaVuSans.ttf") {
        if let Ok(font) = load_font_from_path(&fallback_path, 0) {
            println!("UI font fallback loaded: {}", fallback_path.display());
            return font;
        }
        attempted.push(fallback_path.display().to_string());
    }

    panic!(
        "failed to load an OS default font. Set `font.path` in browser.conf or install a system font. attempted: {}",
        attempted.join(", ")
    );
}

fn load_font_from_path(font_path: &Path, collection_index: u32) -> Result<Font, String> {
    let font_bytes = fs::read(font_path)
        .map_err(|err| format!("failed to read {}: {err}", font_path.display()))?;
    let settings = FontSettings {
        collection_index,
        ..FontSettings::default()
    };

    Font::from_bytes(font_bytes, settings)
        .map_err(|err| format!("failed to parse {}: {err}", font_path.display()))
}

#[derive(Clone, Copy)]
struct FontCandidate {
    label: &'static str,
    path: &'static str,
    collection_index: u32,
}

fn system_font_candidates() -> Vec<FontCandidate> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "windows")]
    {
        candidates.extend_from_slice(&[
            FontCandidate {
                label: "Yu Gothic UI",
                path: r"C:\Windows\Fonts\YuGothM.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Yu Gothic",
                path: r"C:\Windows\Fonts\YuGothR.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Meiryo",
                path: r"C:\Windows\Fonts\meiryo.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Segoe UI",
                path: r"C:\Windows\Fonts\segoeui.ttf",
                collection_index: 0,
            },
            FontCandidate {
                label: "MS Gothic",
                path: r"C:\Windows\Fonts\msgothic.ttc",
                collection_index: 0,
            },
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        candidates.extend_from_slice(&[
            FontCandidate {
                label: "Hiragino Sans",
                path: "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Hiragino Sans",
                path: "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Arial Unicode",
                path: "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
                collection_index: 0,
            },
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        candidates.extend_from_slice(&[
            FontCandidate {
                label: "Noto Sans CJK JP",
                path: "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                collection_index: 0,
            },
            FontCandidate {
                label: "Noto Sans CJK JP",
                path: "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
                collection_index: 0,
            },
            FontCandidate {
                label: "Noto Sans JP",
                path: "/usr/share/fonts/truetype/noto/NotoSansJP-Regular.ttf",
                collection_index: 0,
            },
            FontCandidate {
                label: "DejaVu Sans",
                path: "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                collection_index: 0,
            },
        ]);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = &mut candidates;
    }

    candidates
}
