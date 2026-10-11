//! Fresh-process Git authority for worktree transaction regression controls.

pub(crate) fn configure(command: &mut std::process::Command, directory: &std::path::Path) {
    let hooks = directory.join("hooks");
    let templates = directory.join("templates");
    for path in [&hooks, &templates] {
        std::fs::create_dir(path).unwrap();
    }
    let global = directory.join("global.gitconfig");
    let system = directory.join("system.gitconfig");
    let attributes = directory.join("attributes");
    std::fs::write(&system, "").unwrap();
    std::fs::write(&attributes, "").unwrap();
    // JSON string quoting is Git config string quoting for these generated
    // paths; escaped Windows backslashes remain literal path characters.
    let quote = |path: &std::path::Path| serde_json::to_string(&path.to_string_lossy()).unwrap();
    std::fs::write(&global, format!(
        "[core]\n hooksPath = {}\n attributesFile = {}\n[init]\n templateDir = {}\n[commit]\n gpgSign = false\n[tag]\n gpgSign = false\n[credential]\n helper =\n",
        quote(&hooks), quote(&attributes), quote(&templates)
    )).unwrap();
    for (name, _) in std::env::vars_os() {
        let name_text = name.to_string_lossy();
        if name_text.starts_with("GIT_")
            || name_text == "SSH_AUTH_SOCK"
            || name_text == "SSH_ASKPASS"
        {
            command.env_remove(&name);
        }
    }
    command
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_CONFIG_SYSTEM", &system)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
}

pub(super) fn isolated_child(test_module: &str, name: &str) -> bool {
    let (_, module) = test_module.split_once("::").unwrap();
    let test = format!("{module}::{name}");
    if std::env::var("PEBREL_GIT_FIXTURE_CHILD").as_deref() == Ok(test.as_str()) {
        for name in ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"] {
            assert!(std::path::Path::new(&std::env::var_os(name).unwrap()).is_file());
        }
        return false;
    }
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let marker = directory.path().join("completed");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    configure(&mut command, directory.path());
    let output = command
        .args(["--exact", &test, "--nocapture"])
        .env("PEBREL_GIT_FIXTURE_CHILD", &test)
        .env("PEBREL_GIT_FIXTURE_MARKER", &marker)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Git fixture child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(marker).unwrap(), name);
    true
}

pub(super) fn completed(name: &str) {
    std::fs::write(std::env::var_os("PEBREL_GIT_FIXTURE_MARKER").unwrap(), name).unwrap();
}
