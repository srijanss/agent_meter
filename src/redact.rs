//! Scrubs credentials and personal identifiers from provider text before it is
//! parsed, logged, or shown (agents_plugin_spec.md privacy rules).

const MARK: &str = "[redacted]";
const SECRET_KEY_HINTS: [&str; 5] = ["token", "key", "secret", "password", "session"];
const SECRET_HEADERS: [&str; 3] = ["authorization", "cookie", "set-cookie"];

/// Returns `text` with emails, org/account UUIDs, API tokens, secret-looking
/// `key=value` pairs and credential header lines replaced by `[redacted]`.
pub fn redact(text: &str) -> String {
    text.split('\n').map(redact_line).collect::<Vec<_>>().join("\n")
}

fn redact_line(line: &str) -> String {
    if let Some((name, _)) = line.split_once(':') {
        if SECRET_HEADERS.contains(&name.trim().to_ascii_lowercase().as_str()) {
            return format!("{}: {MARK}", name.trim());
        }
    }
    line.split(' ').map(redact_word).collect::<Vec<_>>().join(" ")
}

fn redact_word(word: &str) -> String {
    let is_wrapper = |c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')');
    let trimmed_start = word.trim_start_matches(is_wrapper);
    let prefix_len = word.len() - trimmed_start.len();
    let core = trimmed_start.trim_end_matches(is_wrapper);
    if core.is_empty() {
        return word.to_string();
    }
    let core_end = prefix_len + core.len();
    let secret_start = match core.split_once('=') {
        Some((key, _)) if is_secret_key(key) => Some(prefix_len + key.len() + 1),
        _ if is_email(core) || is_uuid(core) || core.starts_with("sk-") => Some(prefix_len),
        _ => None,
    };
    match secret_start {
        Some(start) => format!("{}{MARK}{}", &word[..start], &word[core_end..]),
        None => word.to_string(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    SECRET_KEY_HINTS.iter().any(|hint| lowered.contains(hint))
}

fn is_email(s: &str) -> bool {
    match s.split_once('@') {
        Some((local, domain)) => !local.is_empty() && domain.contains('.') && !domain.starts_with('.'),
        None => false,
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}
