use agent_meter::account::{mark_active, parse_accounts};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Barrier};

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n\n[accounts.c]\nprovider = \"codex\"\n\n[accounts.d]\nprovider = \"codex\"\n";

#[test]
fn concurrent_marks_for_different_providers_are_not_lost() {
    let dir = std::env::temp_dir().join(format!("agent_meter_serial_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");

    for round in 0..40 {
        fs::write(&path, CONFIG).unwrap();
        let (claude, codex) = if round % 2 == 0 { ("b", "d") } else { ("a", "c") };
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [claude, codex]
            .into_iter()
            .map(|name| {
                let (path, barrier) = (path.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    mark_active(&path, name)
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().expect("each mark succeeds");
        }

        let accounts = parse_accounts(&fs::read_to_string(&path).unwrap(), Path::new("/h")).unwrap();
        let active: Vec<_> = accounts.iter().filter(|a| a.active).map(|a| a.name.as_str()).collect();
        assert_eq!(active, vec![claude, codex], "round {round}: a mark was lost");
    }
    let _ = fs::remove_dir_all(&dir);
}
