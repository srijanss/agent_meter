//! Scrubs credentials and personal identifiers from provider text before it is
//! parsed, logged, or shown (agents_plugin_spec.md privacy rules).

const MARK: &str = "[redacted]";
/// Whole words (split on `_`, `-`, `.`) that mark a key as secret-bearing.
const SECRET_KEY_WORDS: [&str; 10] = [
    "token", "key", "apikey", "secret", "password", "passwd", "session", "sessionid", "credential",
    "pin",
];
const SECRET_HEADERS: [&str; 3] = ["authorization", "cookie", "set-cookie"];

/// Returns `text` with emails, org/account UUIDs, API tokens, secret-looking
/// `key=value` pairs and credential header lines replaced by `[redacted]`.
pub fn redact(text: &str) -> String {
    // Pairs may span lines (`"token":\n "abc"`), so scan the whole text first.
    redact_json_pairs(text).split('\n').map(redact_line).collect::<Vec<_>>().join("\n")
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
            push_redacted_word(&mut out, &line[word_start..i]);
            out.push(c);
            word_start = i + c.len_utf8();
        }
    }
    push_redacted_word(&mut out, &line[word_start..]);
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
        // `\"abc\"` inside a JSON string: the delimiters are escaped quotes, so
        // keep them intact (and the surrounding JSON valid) around the marker.
        if first == '\\' && chars.get(i + 1) == Some(&'"') {
            let body_start = i + 2;
            let body_end = (body_start..chars.len().saturating_sub(1))
                .find(|&j| chars[j] == '\\' && chars[j + 1] == '"')
                .unwrap_or(chars.len());
            if body_end > body_start {
                out.push_str("\\\"");
                out.push_str(MARK);
                i = body_end;
                continue;
            }
        }
        let quote = matches!(first, '"' | '\'').then_some(first);
        let body_start = i + usize::from(quote.is_some());
        let mut escaped = false;
        let body_end = chars[body_start..]
            .iter()
            .position(|&c| match quote {
                // A backslash escapes the next char, so `\"` never closes.
                Some(q) => {
                    let closes = !escaped && c == q;
                    escaped = !escaped && c == '\\';
                    closes
                }
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

/// Byte length of the JSON array/object at the start of `s`, honoring strings
/// and escapes. An unbalanced container extends to the end of `s`.
fn container_len(s: &str) -> usize {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + c.len_utf8();
                }
            }
            _ => {}
        }
    }
    s.len()
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
            Some(v) if is_secret_key(key) && v.starts_with(['{', '[']) => {
                // Whole array/object value: redact it entirely, unbalanced
                // input is redacted to its end rather than left visible.
                let value_start = line.len() - v.len();
                out.push_str(&line[line.len() - rest.len()..value_start]);
                out.push_str(&format!("\"{MARK}\""));
                rest = &line[value_start + container_len(v)..];
            }
            Some(v) if is_secret_key(key) && !v.is_empty() => {
                // Bare scalar (number, bool, null): swap for a string marker so
                // the JSON stays parseable.
                let value_start = line.len() - v.len();
                let len = v.find([',', '}', ']', '\n']).unwrap_or(v.len());
                let bare = v[..len].trim_end();
                out.push_str(&line[line.len() - rest.len()..value_start]);
                out.push_str(&format!("\"{MARK}\""));
                rest = &line[value_start + bare.len()..];
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

/// Appends `word` to `out`, swapping a secret-looking core (email, UUID,
/// `sk-` token) for the marker and keeping its wrapping punctuation.
fn push_redacted_word(out: &mut String, word: &str) {
    let is_wrapper = |c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')');
    let trimmed_start = word.trim_start_matches(is_wrapper);
    let prefix_len = word.len() - trimmed_start.len();
    let core = trimmed_start.trim_end_matches(is_wrapper);
    if !core.is_empty() && (is_email(core) || is_uuid(core) || core.starts_with("sk-")) {
        out.push_str(&word[..prefix_len]);
        out.push_str(MARK);
        out.push_str(&word[prefix_len + core.len()..]);
    } else {
        out.push_str(word);
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
