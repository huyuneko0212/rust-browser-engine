use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const CONFIG_FILE_NAME: &str = "browser.conf";
const FONT_PATH_KEY: &str = "font.path";

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub font_path: Option<PathBuf>,
}

pub fn app_config() -> &'static AppConfig {
    static CONFIG: OnceLock<AppConfig> = OnceLock::new();
    CONFIG.get_or_init(load_app_config)
}

fn load_app_config() -> AppConfig {
    if let Some(config_path) = find_existing_file(CONFIG_FILE_NAME) {
        let config_text = fs::read_to_string(&config_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
        return parse_config_text(&config_text, &config_path)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", config_path.display()));
    }

    AppConfig::default()
}

fn parse_config_text(config_text: &str, config_path: &Path) -> Result<AppConfig, String> {
    let mut font_path = None;

    for (line_index, raw_line) in config_text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(format!("line {line_number}: expected `key = value`"));
        };

        let key = key.trim();
        let value = parse_value(raw_value.trim(), line_number)?;

        match key {
            FONT_PATH_KEY => font_path = Some(resolve_path(config_path, &value)),
            _ => return Err(format!("line {line_number}: unsupported key `{key}`")),
        }
    }

    Ok(AppConfig { font_path })
}

fn parse_value(raw_value: &str, line_number: usize) -> Result<String, String> {
    if raw_value.is_empty() {
        return Err(format!("line {line_number}: missing value"));
    }

    let value = match (raw_value.strip_prefix('"'), raw_value.strip_suffix('"')) {
        (Some(without_prefix), Some(_)) if raw_value.len() >= 2 => {
            &without_prefix[..without_prefix.len() - 1]
        }
        _ => match (raw_value.strip_prefix('\''), raw_value.strip_suffix('\'')) {
            (Some(without_prefix), Some(_)) if raw_value.len() >= 2 => {
                &without_prefix[..without_prefix.len() - 1]
            }
            _ => raw_value,
        },
    };

    let value = value.trim();
    if value.is_empty() {
        return Err(format!("line {line_number}: empty value"));
    }

    Ok(value.to_string())
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("")
}

fn resolve_path(config_path: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

pub(crate) fn find_existing_file(relative_path: &str) -> Option<PathBuf> {
    for root in search_roots() {
        for dir in root.ancestors() {
            let candidate = dir.join(relative_path);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        roots.push(current_dir);
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let exe_dir = exe_dir.to_path_buf();
            if !roots.contains(&exe_dir) {
                roots.push(exe_dir);
            }
        }
    }

    if roots.is_empty() {
        roots.push(PathBuf::from("."));
    }

    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_font_path_relative_to_config_file() {
        let config = parse_config_text(
            "font.path = assets/fonts/custom.ttf",
            Path::new("project/browser.conf"),
        )
        .unwrap();

        assert_eq!(
            config.font_path,
            Some(Path::new("project").join("assets/fonts/custom.ttf"))
        );
    }

    #[test]
    fn uses_system_font_when_font_path_is_not_set() {
        let config = parse_config_text(
            "# use OS default font",
            Path::new("project/browser.conf"),
        )
        .unwrap();

        assert_eq!(config.font_path, None);
    }

    #[test]
    fn strips_comments_and_quotes() {
        let config = parse_config_text(
            "font.path = \"assets/fonts/PixelMplus12-Bold.ttf\" # comment",
            Path::new("project/browser.conf"),
        )
        .unwrap();

        assert_eq!(
            config.font_path,
            Some(Path::new("project").join("assets/fonts/PixelMplus12-Bold.ttf"))
        );
    }
}
