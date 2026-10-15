use agent_meter::account::{mark_active, parse_accounts};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG: &str = r#"
[accounts.claude-primary]
provider = "claude"
active = true

[accounts.claude-work]
provider = "claude"
home = "/data/claude-work"

[accounts.codex-main]
provider = "codex"
active = true
"#;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn active_names(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap();
    let mut names: Vec<String> = parse_accounts(&text, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .filter(|a| a.active)
        .map(|a| a.name)
        .collect();
    names.sort();
    names
}

#[test]
fn marking_moves_active_within_its_provider_only() {
    let dir = scratch("mark");
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();

    mark_active(&path, "claude-work").expect("marks");

    assert_eq!(active_names(&path), vec!["claude-work", "codex-main"]);
    // Only the config file exists: no temp file left, nothing else written.
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    // Homes survive the rewrite.
    let text = fs::read_to_string(&path).unwrap();
    let work = parse_accounts(&text, Path::new("/home/u")).unwrap();
    assert_eq!(
        work.iter().find(|a| a.name == "claude-work").unwrap().home,
        PathBuf::from("/data/claude-work")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unknown_account_leaves_the_file_untouched() {
    let dir = scratch("unknown");
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();

    assert!(mark_active(&path, "nope").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), CONFIG);
    let _ = fs::remove_dir_all(&dir);
}
