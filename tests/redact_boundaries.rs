use agent_meter::redact::redact;

#[test]
fn keeps_ordinary_assignments_that_merely_contain_key_letters() {
    for text in ["monkey=12", "keyboard=us", "turkey=yes", "tokenizer=bpe"] {
        assert_eq!(redact(text), text, "{text} must be preserved");
    }
}

#[test]
fn still_redacts_real_secret_keys() {
    for key in ["token", "api_key", "access_token", "client_secret", "password", "session_id", "apikey", "API-KEY"] {
        let out = redact(&format!("{key}=hunter2"));
        assert!(!out.contains("hunter2"), "{key}: {out}");
    }
}

#[test]
fn preserves_plain_text_and_empty_values_exactly() {
    for text in ["Current session: 34% used", "name=  spaced", "token=", "a=b c=d"] {
        assert_eq!(redact(text), text);
    }
}
