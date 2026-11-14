use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

#[derive(Clone, Copy)]
pub struct Palette {
    pub name: &'static str,
    pub background: &'static str,
    pub foreground: &'static str,
    pub ansi: [&'static str; 16],
}

pub const DARK: Palette = Palette {
    name: "dark",
    background: "#15171c",
    foreground: "#d6d9e0",
    ansi: [
        "#1d2027", "#e5636b", "#8ccf7e", "#e5c07b", "#6cb6ff", "#d38aea", "#5fc9d6", "#c8ccd4", "#5c6370", "#ff7b84",
        "#a6e39a", "#f2d28e", "#8fc8ff", "#e2a6f3", "#7fdde8", "#f4f5f7",
    ],
};

pub const LIGHT: Palette = Palette {
    name: "light",
    background: "#fbfbfa",
    foreground: "#24292f",
    ansi: [
        "#24292f", "#cf222e", "#116329", "#9a6700", "#0969da", "#8250df", "#1b7c83", "#6e7781", "#8c959f", "#a40e26",
        "#1a7f37", "#7d4e00", "#218bff", "#a475f9", "#3192aa", "#ffffff",
    ],
};

fn named(color: Color) -> Option<usize> {
    Some(match color {
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        _ => return None,
    })
}

fn indexed(n: u8) -> String {
    let n = usize::from(n);
    if n >= 232 {
        let v = 8 + (n - 232) * 10;
        return format!("#{v:02x}{v:02x}{v:02x}");
    }
    let n = n - 16;
    let level = |c: usize| if c == 0 { 0 } else { 55 + c * 40 };
    format!("#{:02x}{:02x}{:02x}", level(n / 36), level(n / 6 % 6), level(n % 6))
}

fn css(color: Color, palette: &Palette) -> Option<String> {
    match color {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Indexed(n) if n < 16 => Some(palette.ansi[usize::from(n)].to_owned()),
        Color::Indexed(n) => Some(indexed(n)),
        other => named(other).map(|i| palette.ansi[i].to_owned()),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn cell_style(cell: &ratatui::buffer::Cell, palette: &Palette) -> String {
    let mut fg = css(cell.fg, palette).unwrap_or_else(|| palette.foreground.to_owned());
    let mut bg = css(cell.bg, palette);
    if cell.modifier.contains(Modifier::REVERSED) {
        let back = bg.unwrap_or_else(|| palette.background.to_owned());
        bg = Some(fg);
        fg = back;
    }
    let mut style = format!("color:{fg};");
    if let Some(bg) = bg {
        style.push_str(&format!("background:{bg};"));
    }
    if cell.modifier.contains(Modifier::BOLD) {
        style.push_str("font-weight:700;");
    }
    if cell.modifier.contains(Modifier::DIM) {
        style.push_str("opacity:.55;");
    }
    if cell.modifier.contains(Modifier::ITALIC) {
        style.push_str("font-style:italic;");
    }
    if cell.modifier.contains(Modifier::UNDERLINED) {
        style.push_str("text-decoration:underline;");
    }
    style
}

pub fn page(buffer: &Buffer, palette: &Palette, cursor: Option<(u16, u16)>) -> String {
    let mut rows = String::new();
    for y in 0..buffer.area.height {
        let mut row = String::new();
        let mut run = String::new();
        let mut current: Option<String> = None;
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            let mut style = cell_style(cell, palette);
            if cursor == Some((x, y)) {
                style.push_str(&format!("background:{};color:{};", palette.foreground, palette.background));
            }
            if current.as_ref() != Some(&style) {
                if let Some(previous) = current.take() {
                    row.push_str(&format!("<span style=\"{previous}\">{}</span>", escape(&run)));
                }
                run.clear();
                current = Some(style);
            }
            let symbol = cell.symbol();
            run.push_str(if symbol.is_empty() { "" } else { symbol });
        }
        if let Some(previous) = current {
            row.push_str(&format!("<span style=\"{previous}\">{}</span>", escape(&run)));
        }
        rows.push_str(&format!("<div class=r>{row}</div>"));
    }
    format!(
        "<!doctype html><meta charset=utf-8><style>html,body{{margin:0;background:{bg}}}\
         .t{{display:inline-block;padding:14px 16px;background:{bg};color:{fg};\
         font:15px/19px Menlo,'SF Mono',monospace;white-space:pre}}.r{{height:19px}}\
         span{{display:inline}}</style><div class=t>{rows}</div>",
        bg = palette.background,
        fg = palette.foreground,
    )
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier, Style};

    use super::*;

    #[test]
    fn markup_in_cells_is_escaped_and_colors_resolve_through_the_palette() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        buffer.set_string(0, 0, "<b>&", Style::new().fg(Color::Red).add_modifier(Modifier::BOLD));
        let html = page(&buffer, &DARK, None);
        assert!(html.contains("&lt;b&gt;&amp;"), "{html}");
        assert!(html.contains("color:#e5636b;font-weight:700;"), "{html}");
        assert_eq!(indexed(22), "#005f00");
        assert_eq!(indexed(240), "#585858");
    }
}
