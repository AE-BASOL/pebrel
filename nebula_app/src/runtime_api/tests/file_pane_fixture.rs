//! Fresh-process authority for Workspace tests that otherwise load user paths.

use std::path::PathBuf;

// Only a fresh child constructs Workspace: settings, SSH discovery and cached
// data paths all see synthetic roots before their first access. The parent
// environment is never mutated, including when these tests run concurrently.
pub(crate) fn isolated_child(test_module: &str, name: &str) -> bool {
    let (_, module) = test_module.split_once("::").expect("Rust test module path");
    let test = format!("{module}::{name}");
    if std::env::var("PEBREL_DEFERRED_ACTION_CHILD").as_deref() == Ok(test.as_str()) {
        let home = PathBuf::from(std::env::var_os("PEBREL_TEST_SSH_HOME").unwrap());
        assert!(!home.join(".ssh/config").exists());
        assert!(crate::ssh::ssh_config_path().is_none());
        assert!(crate::ssh::ssh_config_hosts().is_empty());
        assert_eq!(nebula_settings::RawSettings::load().bool_on("ai_hooks"), Some(false));
        assert_eq!(nebula_settings::RawSettings::load().bool_on("auto_check_updates"), Some(false));
        return false;
    }
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home");
    let config = directory.path().join("config");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&config).unwrap();
    // The Windows WSL hook installer resolves wsl.exe independently of the
    // requested missing launcher. Disable it through its real owned settings.
    std::fs::write(config.join("pebrel_settings.txt"), "ai_hooks=0\nauto_check_updates=0\n")
        .unwrap();
    let marker = directory.path().join("completed");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    crate::runtime_api::tests::git_fixture::configure(&mut command, directory.path());
    let output = command
        .args(["--exact", &test, "--nocapture"])
        .env("PEBREL_DEFERRED_ACTION_CHILD", &test)
        .env("PEBREL_DEFERRED_ACTION_MARKER", &marker)
        .env_remove("PEBREL_CONFIG_FILE")
        .env_remove("NEBULA_CONFIG_FILE")
        .env_remove("PEBREL_GPUI_CONFIG")
        .env_remove("NEBULA_GPUI_CONFIG")
        .env_remove("NEBULA_GPUI_OPEN_DOC")
        .env_remove("SSH_AUTH_SOCK")
        .env("PEBREL_TEST_SSH_HOME", &home)
        .env("PEBREL_CONFIG_DIR", &config)
        .env("NEBULA_CONFIG_DIR", &config)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("APPDATA", &config)
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(marker).unwrap(), name, "child must execute its assertions");
    true
}

pub(crate) fn completed(name: &str) {
    std::fs::write(std::env::var_os("PEBREL_DEFERRED_ACTION_MARKER").unwrap(), name).unwrap();
}
