pub const TITLE_ART: [&str; 5] = [
    "__     ______  ____  _  ________ ____",
    "\\ \\   / / __ \\/ __ \\/ |/ / ____/ __ \\",
    " \\ \\ / / / / / /_/ /    / __/ / /_/ /",
    "  \\ V / /_/ / _, _/ /|  / /___/ _, _/",
    "   \\_/\\____/_/ |_/_/ |_/_____/_/ |_|",
];

pub fn colorize(text: &str, _tone: &str, _enabled: bool) -> String {
    text.to_string()
}

pub fn emphasize(text: &str, _enabled: bool) -> String {
    text.to_string()
}

pub fn highlight(text: &str, _enabled: bool, _background: &str, _foreground: &str) -> String {
    text.to_string()
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
    let mut output = String::new();
    for ch in strip_ansi(text).chars().take(max_length.saturating_sub(1)) {
        output.push(ch);
    }
    output.push('…');
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
