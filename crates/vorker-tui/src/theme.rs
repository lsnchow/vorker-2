pub const TITLE_ART: [&str; 6] = [
    "██╗   ██╗ ██████╗ ██████╗ ██╗  ██╗███████╗██████╗",
    "██║   ██║██╔═══██╗██╔══██╗██║ ██╔╝██╔════╝██╔══██╗",
    "██║   ██║██║   ██║██████╔╝█████╔╝ █████╗  ██████╔╝",
    "╚██╗ ██╔╝██║   ██║██╔══██╗██╔═██╗ ██╔══╝  ██╔══██╗",
    " ╚████╔╝ ╚██████╔╝██║  ██║██║  ██╗███████╗██║  ██║",
    "  ╚═══╝   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝",
];

pub fn colorize(text: &str, tone: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }

    let code = match tone {
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "brightGreen" => "92",
        "brightRed" => "91",
        "gray" => "90",
        "brightBlack" => "90",
        "magenta" => "35",
        "brightMagenta" => "95",
        "white" => "97",
        "black" => "30",
        "bgRed" => "41",
        "bgGreen" => "42",
        "bgMagenta" => "45",
        "bgGray" => "100",
        _ => "0",
    };

    format!("\u{1b}[{code}m{text}\u{1b}[0m")
}

pub fn emphasize(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    format!("\u{1b}[1m{text}\u{1b}[0m")
}

pub fn highlight(text: &str, enabled: bool, background: &str, foreground: &str) -> String {
    if !enabled {
        return text.to_string();
    }

    let background_code = match background {
        "bgRed" => "41",
        "bgGreen" => "42",
        "bgMagenta" => "45",
        "bgGray" => "100",
        _ => "49",
    };
    let foreground_code = match foreground {
        "brightRed" => "91",
        "yellow" => "33",
        "black" => "30",
        "brightGreen" => "92",
        "green" => "32",
        "gray" => "90",
        "white" => "97",
        _ => "39",
    };

    format!("\u{1b}[1;{background_code};{foreground_code}m{text}\u{1b}[0m")
}

pub fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
            continue;
        }
        result.push(ch);
    }

    result
}

pub fn visible_length(text: &str) -> usize {
    strip_ansi(text).chars().count()
}

pub fn truncate(text: &str, max_length: usize) -> String {
    if visible_length(text) <= max_length {
        return text.to_string();
    }
    if max_length <= 3 {
        return ".".repeat(max_length);
    }
    let mut output = String::new();
    for ch in strip_ansi(text).chars().take(max_length.saturating_sub(3)) {
        output.push(ch);
    }
    output.push_str("...");
    output
}

pub fn pad(text: &str, width: usize) -> String {
    let mut output = text.to_string();
    let missing = width.saturating_sub(visible_length(text));
    output.push_str(&" ".repeat(missing));
    output
}

pub fn fit(text: &str, width: usize) -> String {
    pad(&truncate(text, width), width)
}

pub fn hard_wrap(text: &str, max_length: usize) -> Vec<String> {
    let clean = strip_ansi(text);
    if clean.chars().count() <= max_length {
        return vec![clean];
    }

    let mut lines = Vec::new();
    let mut remaining = clean;

    while remaining.chars().count() > max_length {
        let slice: String = remaining.chars().take(max_length).collect();
        if let Some(break_at) = slice.rfind(' ')
            && break_at >= (max_length * 3 / 5)
        {
            lines.push(remaining[..break_at].to_string());
            remaining = remaining[break_at + 1..].to_string();
            continue;
        }
        lines.push(slice);
        remaining = remaining.chars().skip(max_length).collect();
    }

    if !remaining.is_empty() {
        lines.push(remaining);
    }

    lines
}
