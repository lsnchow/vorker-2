use crate::theme::colorize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichContext {
    Normal,
    Review,
}

pub fn style_line(line: &str, context: RichContext, color: bool) -> String {
    if !color {
        return line.to_string();
    }

    let line = style_code_quote_line(line, context, color);
    let line = style_fenced_code_line(&line, context, color);
    let line = style_diff_line(&line, color);
    let line = style_review_labels(&line, context, color);
    let line = style_severity(&line, color);
    style_inline_code(&line, color)
}

fn style_code_quote_line(line: &str, context: RichContext, color: bool) -> String {
    if context != RichContext::Review {
        return line.to_string();
    }

    let trimmed = line.trim_start();
    let Some((line_number, rest)) = trimmed.split_once('|') else {
        return line.to_string();
    };
    if line_number.trim().parse::<usize>().is_err() {
        return line.to_string();
    }

    let indent_len = line.len().saturating_sub(trimmed.len());
    let indent = &line[..indent_len];
    let prefix = format!("{indent}{line_number}|");
    format!(
        "{} {}",
        colorize(&prefix, "gray", color),
        style_code_tokens(rest.trim_start(), color)
    )
}

fn style_diff_line(line: &str, color: bool) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        return colorize(line, "green", color);
    }
    if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        return colorize(line, "red", color);
    }
    if trimmed.starts_with("@@") {
        return colorize(line, "brightMagenta", color);
    }
    line.to_string()
}

fn style_fenced_code_line(line: &str, context: RichContext, color: bool) -> String {
    if context != RichContext::Review {
        return line.to_string();
    }

    let trimmed = line.trim_start();
    if !line.starts_with("    ") && !line.starts_with('\t') {
        return line.to_string();
    }
    let Some(content) = line.get(4..) else {
        return line.to_string();
    };
    if let Some((line_number, _)) = trimmed.split_once('|')
        && line_number.trim().parse::<usize>().is_ok()
    {
        return line.to_string();
    }

    format!(
        "{}{}",
        colorize("    ", "gray", color),
        style_code_tokens(content, color)
    )
}

fn style_review_labels(line: &str, context: RichContext, color: bool) -> String {
    if context != RichContext::Review {
        return line.to_string();
    }

    for label in [
        "Location:",
        "Recommendation:",
        "Coaching:",
        "Patch direction:",
        "Next Steps",
        "Summary",
    ] {
        if let Some(rest) = line.strip_prefix(label) {
            return format!("{}{}", colorize(label, "brightMagenta", color), rest);
        }
    }

    line.to_string()
}

fn style_severity(line: &str, _color: bool) -> String {
    let mut output = line.to_string();
    for (tag, bg) in [
        ("[CRITICAL]", "bgRed"),
        ("[HIGH]", "bgOrange"),
        ("[MEDIUM]", "bgYellow"),
        ("[LOW]", "bgGray"),
    ] {
        if output.contains(tag) {
            output = output.replacen(tag, &highlight_badge(tag, bg), 1);
            break;
        }
    }
    output
}

fn highlight_badge(tag: &str, bg: &str) -> String {
    let code = match bg {
        "bgRed" => "41",
        "bgOrange" => "48;5;130",
        "bgYellow" => "43",
        _ => "100",
    };
    format!("\u{1b}[1;{code};97m{tag}\u{1b}[0m")
}

fn style_inline_code(line: &str, color: bool) -> String {
    let mut output = String::new();
    let mut segments = line.split('`').peekable();
    let mut in_code = false;

    while let Some(segment) = segments.next() {
        if in_code {
            if segments.peek().is_none() {
                output.push('`');
                output.push_str(segment);
            } else {
                output.push_str("\u{1b}[48;5;238m\u{1b}[38;5;229m");
                output.push_str(&style_code_tokens(segment, color));
                output.push_str("\u{1b}[0m");
            }
        } else {
            output.push_str(segment);
        }
        in_code = !in_code;
    }

    if color { output } else { line.to_string() }
}

fn style_code_tokens(code: &str, color: bool) -> String {
    let mut styled = code.to_string();
    for keyword in ["return", "if", "else", "except", "def", "class", "try"] {
        styled = styled.replace(keyword, &colorize(keyword, "brightMagenta", color));
    }
    styled
}
