use agent_meter::models::{parse_model_profiles, Harness, ModelProvider};

const CONFIG: &str = r#"
[accounts.work]
provider = "claude"

[models.fast]
harness = "pi"
model = "gpt-6-astra"
provider = "openai"

[models.deep]
harness = "claude"
model = "claude-opus-5-5"
thinking = "high"
provider = "anthropic"
"#;

#[test]
fn model_tables_become_named_profiles_sorted_by_name_and_other_tables_are_ignored() {
    let profiles = parse_model_profiles(CONFIG).unwrap();
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["deep", "fast"]);

    let deep = &profiles[0];
    assert_eq!(deep.harness, Harness::Claude);
    assert_eq!(deep.model, "claude-opus-5-5");
    assert_eq!(deep.thinking.as_deref(), Some("high"));
    assert_eq!(deep.provider, ModelProvider::Anthropic);

    let fast = &profiles[1];
    assert_eq!((fast.harness, fast.provider), (Harness::Pi, ModelProvider::Openai));
    assert_eq!(fast.thinking, None);

    assert_eq!(parse_model_profiles("[accounts.a]\nprovider = \"claude\"\n").unwrap(), vec![]);
}

#[test]
fn invalid_profiles_are_rejected_without_echoing_their_values() {
    let bad = |body: &str| parse_model_profiles(&format!("[models.x]\n{body}")).unwrap_err().to_string();
    let ok = "harness = \"pi\"\nmodel = \"m\"\nprovider = \"openai\"\n";

    assert!(parse_model_profiles(&format!("[models.x]\n{ok}")).is_ok());
    for body in [
        "harness = \"pi\"\nprovider = \"openai\"\n",                                  // no model
        "harness = \"pi\"\nmodel = \"m\"\n",                                          // no provider
        "model = \"m\"\nprovider = \"openai\"\n",                                     // no harness
        "harness = \"vim\"\nmodel = \"m\"\nprovider = \"openai\"\n",                  // unknown harness
        "harness = \"pi\"\nmodel = \"m\"\nprovider = \"mystery\"\n",                  // unknown provider
        "harness = \"pi\"\nmodel = \"\"\nprovider = \"openai\"\n",                    // empty model
        "harness = \"pi\"\nmodel = \"m\\nx\"\nprovider = \"openai\"\n",               // control char
        &format!("{ok}thinking = \"\"\n"),
        &format!("{ok}api_key = \"sk-secret-123\"\n"),                                // credentials / unknown keys
    ] {
        let message = bad(body);
        assert!(!message.contains("sk-secret-123") && !message.contains("mystery"), "{message}");
    }
    assert!(parse_model_profiles("[models]\nx = 3\n").is_err());
    assert!(parse_model_profiles("not toml [").is_err());
}
