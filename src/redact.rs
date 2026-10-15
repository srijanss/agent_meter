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
    let line = redact_json_pairs(line);
    let is_delimiter = |c: char| c.is_whitespace() || matches!(c, '{' | '}' | '[' | ']' | ':');
    let mut out = String::with_capacity(line.len());
    let mut word_start = 0;
    for (i, c) in line.char_indices() {
        if is_delimiter(c) {
            out.push_str(&redact_word(&line[word_start..i]));
            out.push(c);
            word_start = i + c.len_utf8();
        }
    }
    out.push_str(&redact_word(&line[word_start..]));
    out
}

/// Replaces the string value of every `"secret_key": "value"` pair.
fn redact_json_pairs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('"') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('"') else { break };
        let key = &after_open[..close];
        let tail = &after_open[close + 1..];
        let colon = tail.trim_start();
        let value = colon.strip_prefix(':').map(str::trim_start);
        match value {
            Some(v) if is_secret_key(key) && v.starts_with('"') => {
                let value_start = line.len() - v.len() + 1;
                let value_len = v[1..].find('"').unwrap_or(v.len() - 1);
                out.push_str(&line[line.len() - rest.len()..value_start]);
                out.push_str(MARK);
                rest = &line[value_start + value_len..];
            }
            _ => {
                let consumed = open + 1 + close + 1;
                out.push_str(&rest[..consumed]);
                rest = &rest[consumed..];
            }
        }
    }
    out.push_str(rest);
    out
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
        Some((key, value)) if is_secret_key(key) => {
            let unquoted = value.trim_start_matches(is_wrapper);
            if unquoted.is_empty() {
                None
            } else {
                // Keep the opening quote so the closing one stays balanced.
                Some(prefix_len + key.len() + 1 + (value.len() - unquoted.len()))
            }
        }
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
