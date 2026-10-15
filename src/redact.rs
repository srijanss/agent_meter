//! Scrubs credentials and personal identifiers from provider text before it is
//! parsed, logged, or shown (agents_plugin_spec.md privacy rules).

const MARK: &str = "[redacted]";
/// Whole words (split on `_`, `-`, `.`) that mark a key as secret-bearing.
const SECRET_KEY_WORDS: [&str; 9] =
    ["token", "key", "apikey", "secret", "password", "passwd", "session", "sessionid", "credential"];
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
    let line = redact_json_pairs(&redact_assignments(line));
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

/// Replaces the whole value of every secret `name=value` assignment. Quoted
/// values run to the matching quote; unquoted ones to whitespace or a
/// separator, so inner punctuation never splits a secret.
fn redact_assignments(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let is_name_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.');
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        out.push(chars[i]);
        i += 1;
        if chars[i - 1] != '=' {
            continue;
        }
        let name_end = out.len() - 1;
        let name_start = out[..name_end]
            .char_indices()
            .rev()
            .take_while(|&(_, c)| is_name_char(c))
            .last()
            .map_or(name_end, |(at, _)| at);
        if !is_secret_key(&out[name_start..name_end]) {
            continue;
        }
        let Some(&first) = chars.get(i).filter(|c| !c.is_whitespace()) else { continue };
        let quote = matches!(first, '"' | '\'').then_some(first);
        let body_start = i + usize::from(quote.is_some());
        let body_end = chars[body_start..]
            .iter()
            .position(|&c| match quote {
                Some(q) => c == q,
                None => c.is_whitespace() || matches!(c, ',' | ';' | ')' | '"' | '\''),
            })
            .map_or(chars.len(), |n| body_start + n);
        if body_end > body_start {
            out.extend(quote);
            out.push_str(MARK);
            i = body_end;
        }
    }
    out
}

/// Byte offset of the first `"` in `s` that is not escaped by a backslash.
fn closing_quote(s: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(i),
            _ => {}
        }
    }
    None
}

/// Replaces the string value of every `"secret_key": "value"` pair.
fn redact_json_pairs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('"') {
        let after_open = &rest[open + 1..];
        let Some(close) = closing_quote(after_open) else { break };
        let key = &after_open[..close];
        let tail = &after_open[close + 1..];
        let colon = tail.trim_start();
        let value = colon.strip_prefix(':').map(str::trim_start);
        match value {
            Some(v) if is_secret_key(key) && v.starts_with('"') => {
                let value_start = line.len() - v.len() + 1;
                let value_len = closing_quote(&v[1..]).unwrap_or(v.len() - 1);
                out.push_str(&line[line.len() - rest.len()..value_start]);
                out.push_str(MARK);
                // Consume the closing quote too so it is not re-read as the
                // opening quote of the next key.
                let value_end = (value_start + value_len + 1).min(line.len());
                out.push_str(&line[value_start + value_len..value_end]);
                rest = &line[value_end..];
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
    let secret_start = if is_email(core) || is_uuid(core) || core.starts_with("sk-") {
        Some(prefix_len)
    } else {
        None
    };
    match secret_start {
        Some(start) => format!("{}{MARK}{}", &word[..start], &word[core_end..]),
        None => word.to_string(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let mut word = String::new();
    let mut prev_lower = false;
    for c in key.chars() {
        let separator = matches!(c, '_' | '-' | '.');
        // A lower-to-upper step starts a new word (`accessToken`).
        if separator || (prev_lower && c.is_uppercase()) {
            if SECRET_KEY_WORDS.contains(&word.as_str()) {
                return true;
            }
            word.clear();
        }
        if !separator {
            word.extend(c.to_lowercase());
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
    }
    SECRET_KEY_WORDS.contains(&word.as_str())
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
