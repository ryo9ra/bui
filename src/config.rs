//! User configuration loaded from `$XDG_CONFIG_HOME/bui/config.toml`
//! (or `~/.config/bui/config.toml`).
//!
//! Schema (all keys optional, all sections optional):
//!
//! ```toml
//! [theme]
//! current_branch = "green"      # named color or "#rrggbb"
//! remote_branch  = "cyan"
//! merged_tag     = "darkgray"
//! worktree_tag   = "cyan"
//! spinner        = "yellow"
//! ```
//!
//! Missing keys fall through to the defaults below.

use std::path::PathBuf;

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub theme: Theme,
    pub fetch: FetchConfig,
    pub worktree: WorktreeConfig,
}

#[derive(Clone, Debug, Default)]
pub struct FetchConfig {
    /// Pass `--prune-tags` (in addition to `--prune`) to `git fetch`.
    /// Off by default: deleting local tags that aren't on the remote can
    /// surprise users who keep local-only tags. Set to `true` if you only
    /// keep pushed tags.
    pub prune_tags: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WorktreeConfig {
    /// Optional root directory under which new worktrees are placed by
    /// default. The `W` 2-step prompt prefills step 2 with
    /// `<root>/<branch>` when set, or `../wt-<leaf>` when not.
    pub root: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub current_branch: Color,
    pub remote_branch: Color,
    pub merged_tag: Color,
    pub worktree_tag: Color,
    pub spinner: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            current_branch: Color::Green,
            remote_branch: Color::Cyan,
            merged_tag: Color::DarkGray,
            worktree_tag: Color::Cyan,
            spinner: Color::Yellow,
        }
    }
}

/// Returns the loaded config, or `Config::default()` if no file exists or
/// the file is malformed. Errors are silent on purpose — a corrupt config
/// shouldn't prevent the TUI from launching.
pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    load_from_str(&text)
}

pub fn load_from_str(text: &str) -> Config {
    match toml::from_str::<RawConfig>(text) {
        Ok(raw) => raw.into_config(),
        Err(_) => Config::default(),
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("bui").join("config.toml"));
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("bui")
            .join("config.toml"),
    )
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    theme: RawTheme,
    fetch: RawFetch,
    worktree: RawWorktree,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawTheme {
    current_branch: Option<String>,
    remote_branch: Option<String>,
    merged_tag: Option<String>,
    worktree_tag: Option<String>,
    spinner: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawFetch {
    prune_tags: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawWorktree {
    root: Option<String>,
}

impl RawConfig {
    fn into_config(self) -> Config {
        let defaults = Theme::default();
        let t = self.theme;
        Config {
            theme: Theme {
                current_branch: parse_color_or(t.current_branch.as_deref(), defaults.current_branch),
                remote_branch: parse_color_or(t.remote_branch.as_deref(), defaults.remote_branch),
                merged_tag: parse_color_or(t.merged_tag.as_deref(), defaults.merged_tag),
                worktree_tag: parse_color_or(t.worktree_tag.as_deref(), defaults.worktree_tag),
                spinner: parse_color_or(t.spinner.as_deref(), defaults.spinner),
            },
            fetch: FetchConfig {
                prune_tags: self.fetch.prune_tags.unwrap_or(false),
            },
            worktree: WorktreeConfig {
                root: self.worktree.root,
            },
        }
    }
}

fn parse_color_or(s: Option<&str>, fallback: Color) -> Color {
    s.and_then(parse_color).unwrap_or(fallback)
}

pub(crate) fn parse_color(s: &str) -> Option<Color> {
    let lower = s.trim().to_lowercase();
    match lower.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ if lower.starts_with('#') && lower.len() == 7 => parse_hex(&lower),
        _ => None,
    }
}

fn parse_hex(s: &str) -> Option<Color> {
    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
    let b = u8::from_str_radix(&s[5..7], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_colors() {
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("Cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
        assert_eq!(parse_color("dark_grey"), Some(Color::DarkGray));
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(0xff, 0x88, 0x00)));
        assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn rejects_garbage_input() {
        assert_eq!(parse_color("not-a-color"), None);
        assert_eq!(parse_color("#xyz"), None);
        assert_eq!(parse_color("#ff"), None);
    }

    #[test]
    fn empty_string_yields_defaults() {
        let c = load_from_str("");
        assert_eq!(c.theme.current_branch, Color::Green);
        assert_eq!(c.theme.remote_branch, Color::Cyan);
    }

    #[test]
    fn partial_overrides_keep_other_defaults() {
        let c = load_from_str(
            r#"
            [theme]
            current_branch = "red"
        "#,
        );
        assert_eq!(c.theme.current_branch, Color::Red);
        // Untouched key keeps its default.
        assert_eq!(c.theme.remote_branch, Color::Cyan);
    }

    #[test]
    fn invalid_color_string_falls_back() {
        let c = load_from_str(
            r#"
            [theme]
            current_branch = "not-a-color"
        "#,
        );
        assert_eq!(c.theme.current_branch, Color::Green);
    }

    #[test]
    fn malformed_toml_yields_defaults() {
        let c = load_from_str("not = toml = at all");
        assert_eq!(c.theme.current_branch, Color::Green);
    }

    #[test]
    fn fetch_section_defaults_to_no_prune_tags() {
        let c = load_from_str("");
        assert!(!c.fetch.prune_tags);
    }

    #[test]
    fn worktree_root_is_none_by_default() {
        let c = load_from_str("");
        assert!(c.worktree.root.is_none());
    }

    #[test]
    fn worktree_root_can_be_overridden() {
        let c = load_from_str(
            r#"
            [worktree]
            root = "~/wt"
        "#,
        );
        assert_eq!(c.worktree.root.as_deref(), Some("~/wt"));
    }

    #[test]
    fn fetch_prune_tags_can_be_enabled() {
        let c = load_from_str(
            r#"
            [fetch]
            prune_tags = true
        "#,
        );
        assert!(c.fetch.prune_tags);
    }

    #[test]
    fn hex_in_config_resolves_to_rgb() {
        let c = load_from_str(
            r##"
            [theme]
            spinner = "#00ffaa"
        "##,
        );
        assert_eq!(c.theme.spinner, Color::Rgb(0, 0xff, 0xaa));
    }
}
