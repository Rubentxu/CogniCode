use mermaid_rs_renderer::{RenderOptions, Theme};

macro_rules! dark_theme {
    (
        primary = $primary:expr,
        text = $text:expr,
        border = $border:expr,
        bg = $bg:expr,
        secondary = $secondary:expr,
        tertiary = $tertiary:expr,
        cluster_bg = $cluster_bg:expr,
        cluster_border = $cluster_border:expr,
    ) => {
        Theme {
            font_family: "'trebuchet ms', verdana, arial, sans-serif".to_string(),
            font_size: 14.0,
            primary_color: $primary.to_string(),
            primary_text_color: $text.to_string(),
            primary_border_color: $border.to_string(),
            line_color: $border.to_string(),
            secondary_color: $secondary.to_string(),
            tertiary_color: $tertiary.to_string(),
            edge_label_background: $bg.to_string(),
            cluster_background: $cluster_bg.to_string(),
            cluster_border: $cluster_border.to_string(),
            background: $bg.to_string(),
            sequence_actor_fill: $bg.to_string(),
            sequence_actor_border: $border.to_string(),
            sequence_actor_line: $border.to_string(),
            sequence_note_fill: $secondary.to_string(),
            sequence_note_border: $border.to_string(),
            sequence_activation_fill: $secondary.to_string(),
            sequence_activation_border: $border.to_string(),
            text_color: $text.to_string(),
            git_colors: Theme::mermaid_default().git_colors,
            git_inv_colors: Theme::mermaid_default().git_inv_colors,
            git_branch_label_colors: Theme::mermaid_default().git_branch_label_colors,
            git_commit_label_color: Theme::mermaid_default().git_commit_label_color,
            git_commit_label_background: Theme::mermaid_default().git_commit_label_background,
            git_tag_label_color: Theme::mermaid_default().git_tag_label_color,
            git_tag_label_background: Theme::mermaid_default().git_tag_label_background,
            git_tag_label_border: Theme::mermaid_default().git_tag_label_border,
            pie_colors: Theme::mermaid_default().pie_colors,
            pie_title_text_size: 25.0,
            pie_title_text_color: $text.to_string(),
            pie_section_text_size: 17.0,
            pie_section_text_color: $text.to_string(),
            pie_legend_text_size: 17.0,
            pie_legend_text_color: $text.to_string(),
            pie_stroke_color: $border.to_string(),
            pie_stroke_width: 1.6,
            pie_outer_stroke_width: 1.6,
            pie_outer_stroke_color: $border.to_string(),
            pie_opacity: 0.85,
        }
    };
}

macro_rules! light_theme {
    (
        primary = $primary:expr,
        text = $text:expr,
        border = $border:expr,
        bg = $bg:expr,
        secondary = $secondary:expr,
        tertiary = $tertiary:expr,
        cluster_bg = $cluster_bg:expr,
        cluster_border = $cluster_border:expr,
    ) => {
        Theme {
            font_family: "'trebuchet ms', verdana, arial, sans-serif".to_string(),
            font_size: 14.0,
            primary_color: $primary.to_string(),
            primary_text_color: $text.to_string(),
            primary_border_color: $border.to_string(),
            line_color: $border.to_string(),
            secondary_color: $secondary.to_string(),
            tertiary_color: $tertiary.to_string(),
            edge_label_background: $bg.to_string(),
            cluster_background: $cluster_bg.to_string(),
            cluster_border: $cluster_border.to_string(),
            background: $bg.to_string(),
            sequence_actor_fill: $bg.to_string(),
            sequence_actor_border: $border.to_string(),
            sequence_actor_line: $border.to_string(),
            sequence_note_fill: $secondary.to_string(),
            sequence_note_border: $border.to_string(),
            sequence_activation_fill: $secondary.to_string(),
            sequence_activation_border: $border.to_string(),
            text_color: $text.to_string(),
            git_colors: Theme::modern().git_colors,
            git_inv_colors: Theme::modern().git_inv_colors,
            git_branch_label_colors: Theme::modern().git_branch_label_colors,
            git_commit_label_color: Theme::modern().git_commit_label_color,
            git_commit_label_background: Theme::modern().git_commit_label_background,
            git_tag_label_color: Theme::modern().git_tag_label_color,
            git_tag_label_background: Theme::modern().git_tag_label_background,
            git_tag_label_border: Theme::modern().git_tag_label_border,
            pie_colors: Theme::modern().pie_colors,
            pie_title_text_size: 25.0,
            pie_title_text_color: $text.to_string(),
            pie_section_text_size: 17.0,
            pie_section_text_color: $text.to_string(),
            pie_legend_text_size: 17.0,
            pie_legend_text_color: $text.to_string(),
            pie_stroke_color: $border.to_string(),
            pie_stroke_width: 1.6,
            pie_outer_stroke_width: 1.6,
            pie_outer_stroke_color: $border.to_string(),
            pie_opacity: 0.85,
        }
    };
}

fn get_theme(name: &str) -> Theme {
    match name {
        "catppuccin-mocha" => dark_theme! {
            primary = "#cba6f7",
            text = "#cdd6f4",
            border = "#585b70",
            bg = "#1e1e2e",
            secondary = "#45475a",
            tertiary = "#313244",
            cluster_bg = "#45475a",
            cluster_border = "#585b70",
        },
        "catppuccin-latte" => light_theme! {
            primary = "#8839ef",
            text = "#4c4f69",
            border = "#bcc0cc",
            bg = "#eff1f5",
            secondary = "#eff1f5",
            tertiary = "#e6e9ef",
            cluster_bg = "#eff1f5",
            cluster_border = "#bcc0cc",
        },
        "dracula" => dark_theme! {
            primary = "#bd93f9",
            text = "#f8f8f2",
            border = "#6272a4",
            bg = "#282a36",
            secondary = "#44475a",
            tertiary = "#282a36",
            cluster_bg = "#44475a",
            cluster_border = "#6272a4",
        },
        "tokyo-night" => dark_theme! {
            primary = "#7aa2f7",
            text = "#a9b1d6",
            border = "#3d59a1",
            bg = "#1a1b26",
            secondary = "#1a1b26",
            tertiary = "#16161e",
            cluster_bg = "#1a1b26",
            cluster_border = "#3d59a1",
        },
        "tokyo-night-light" => light_theme! {
            primary = "#34548a",
            text = "#343b58",
            border = "#9699a3",
            bg = "#e1e2e7",
            secondary = "#e1e2e7",
            tertiary = "#d5d6db",
            cluster_bg = "#e1e2e7",
            cluster_border = "#9699a3",
        },
        "tokyo-night-storm" => dark_theme! {
            primary = "#7aa2f7",
            text = "#a9b1d6",
            border = "#414868",
            bg = "#1f2335",
            secondary = "#1f2335",
            tertiary = "#181825",
            cluster_bg = "#1f2335",
            cluster_border = "#414868",
        },
        "nord" => dark_theme! {
            primary = "#88c0d0",
            text = "#eceff4",
            border = "#4c566a",
            bg = "#2e3440",
            secondary = "#3b4252",
            tertiary = "#2e3440",
            cluster_bg = "#3b4252",
            cluster_border = "#4c566a",
        },
        "nord-light" => light_theme! {
            primary = "#5e81ac",
            text = "#4c566a",
            border = "#81a1c1",
            bg = "#eceff4",
            secondary = "#eceff4",
            tertiary = "#d8dee9",
            cluster_bg = "#eceff4",
            cluster_border = "#81a1c1",
        },
        "github-light" => light_theme! {
            primary = "#0969da",
            text = "#1f2328",
            border = "#8b949e",
            bg = "#ffffff",
            secondary = "#f6f8fa",
            tertiary = "#eaeef2",
            cluster_bg = "#f6f8fa",
            cluster_border = "#8b949e",
        },
        "github-dark" => dark_theme! {
            primary = "#58a6ff",
            text = "#e6edf3",
            border = "#30363d",
            bg = "#0d1117",
            secondary = "#21262d",
            tertiary = "#161b22",
            cluster_bg = "#21262d",
            cluster_border = "#30363d",
        },
        "solarized-light" => light_theme! {
            primary = "#268bd2",
            text = "#586e75",
            border = "#93a1a1",
            bg = "#fdf6e3",
            secondary = "#eee8d5",
            tertiary = "#fdf6e3",
            cluster_bg = "#eee8d5",
            cluster_border = "#93a1a1",
        },
        "solarized-dark" => dark_theme! {
            primary = "#268bd2",
            text = "#839496",
            border = "#586e75",
            bg = "#002b36",
            secondary = "#073642",
            tertiary = "#002b36",
            cluster_bg = "#073642",
            cluster_border = "#586e75",
        },
        "one-dark" => dark_theme! {
            primary = "#61afef",
            text = "#abb2bf",
            border = "#4b5263",
            bg = "#282c34",
            secondary = "#21252b",
            tertiary = "#181a1f",
            cluster_bg = "#21252b",
            cluster_border = "#4b5263",
        },
        "zinc-dark" => dark_theme! {
            primary = "#52525b",
            text = "#fafafa",
            border = "#3f3f46",
            bg = "#18181b",
            secondary = "#27272a",
            tertiary = "#18181b",
            cluster_bg = "#27272a",
            cluster_border = "#3f3f46",
        },
        _ => Theme::modern(),
    }
}

pub fn render_mermaid(mermaid_code: &str, theme_name: &str) -> Result<String, String> {
    let theme = get_theme(theme_name);
    let options = RenderOptions {
        theme,
        layout: mermaid_rs_renderer::LayoutConfig::default(),
    };
    mermaid_rs_renderer::render_with_options(mermaid_code, options)
        .map_err(|e| format!("Render error: {}", e))
}

pub fn list_themes() -> Vec<&'static str> {
    vec![
        "catppuccin-mocha",
        "catppuccin-latte",
        "dracula",
        "tokyo-night",
        "tokyo-night-light",
        "tokyo-night-storm",
        "nord",
        "nord-light",
        "github-light",
        "github-dark",
        "solarized-light",
        "solarized-dark",
        "one-dark",
        "zinc-dark",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_default_theme() {
        let svg = render_mermaid("flowchart LR; A-->B", "default").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_render_tokyo_night_light() {
        let svg = render_mermaid("flowchart LR; A-->B", "tokyo-night-light").unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_render_dracula() {
        let svg = render_mermaid("flowchart LR; A-->B-->C", "dracula").unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_list_themes_count() {
        assert_eq!(list_themes().len(), 14);
    }

    #[test]
    fn test_unknown_theme_falls_back_to_modern() {
        let svg = render_mermaid("flowchart LR; A-->B", "nonexistent").unwrap();
        assert!(svg.contains("<svg"));
    }
}
