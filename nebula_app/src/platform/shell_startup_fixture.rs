//! Synthetic, noninteractive launchers installed in an owned test directory.

use std::path::{Path, PathBuf};

#[cfg(windows)]
fn native_cmd() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").expect("system directory"))
        .join("System32/cmd.exe")
}

pub(crate) fn retry_paths(directory: &Path) -> (PathBuf, PathBuf) {
    #[cfg(windows)]
    return (native_cmd(), directory.join("repairable-cwd"));
    #[cfg(unix)]
    return (directory.join("repairable-shell"), directory.to_path_buf());
}

pub(crate) fn repair_retry(program: &Path, cwd: &Path) {
    #[cfg(windows)]
    {
        assert!(program.is_file());
        std::fs::create_dir(cwd).unwrap();
    }
    #[cfg(unix)]
    {
        assert!(cwd.is_dir());
        install(program);
    }
}

pub(crate) fn install(program: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // A relocated signed macOS system binary may be killed by platform
        // policy. The owned script delegates only the explicit fixture args.
        std::fs::write(program, "#!/bin/sh\nexec /bin/sh \"$@\"\n").unwrap();
        std::fs::set_permissions(program, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    {
        std::fs::copy(native_cmd(), program).expect("copy synthetic fixture executable");
    }
}

pub(crate) fn waiting_args(marker: &str) -> Vec<String> {
    #[cfg(unix)]
    return vec![
        "-c".into(),
        format!("printf '{marker}:%s:%s\n' \"$PEBREL_512_FROZEN\" \"$PWD\"; read stop"),
    ];
    #[cfg(windows)]
    return vec![
        "/d".into(),
        "/q".into(),
        "/c".into(),
        format!("echo {marker}:%PEBREL_512_FROZEN%:%CD%&set /p stop="),
    ];
}

pub(crate) fn exit_args(status: u32) -> Vec<String> {
    #[cfg(unix)]
    return vec!["-c".into(), format!("exit {status}")];
    #[cfg(windows)]
    return vec!["/d".into(), "/q".into(), "/c".into(), format!("exit {status}")];
}

pub(crate) fn queued_args(_directory: &Path, marker: &str) -> Vec<String> {
    #[cfg(unix)]
    return vec![
        "-c".into(),
        format!(
            "printf '{marker}:%s:%s\\n' \"$PEBREL_512_FROZEN\" \"$PWD\"; \
         printf '512-PANE:%s:%s:%s\\n' \"$PEBREL_PANE_ID\" \"$NEBULA_PANE_ID\" \"$PEBREL_PROCESS_ID\"; \
         printf '\\033]133;A\\007$ \\033]133;B\\007'; \
         IFS= read -r line; \
         if [ \"$line\" = 'printf synthetic-queued-request' ]; then \
           printf 'synthetic-queued-request\\n512-QUEUED-EXECUTED\\n'; \
         fi; read stop"
        ),
    ];
    #[cfg(windows)]
    {
        // The script lives in the owned parent of repairable-cwd, so this fixed
        // relative command needs no absolute-path quoting at the CMD /c boundary.
        let script = _directory.join("retry-queued.cmd");
        let batch = format!(
            r#"@echo off
echo {marker}:%PEBREL_512_FROZEN%:%CD%
echo 512-PANE:%PEBREL_PANE_ID%:%NEBULA_PANE_ID%:%PEBREL_PROCESS_ID%
set "line="
set /p "line=%CD%>"
echo 512-INPUT:!line!
if "!line!"=="echo synthetic-queued-request" (
  echo synthetic-queued-request
  echo 512-QUEUED-EXECUTED
)
echo 512-BATCH-WAIT-STOP
set /p stop=
"#
        );
        std::fs::write(&script, batch.replace('\n', "\r\n")).unwrap();
        vec!["/d".into(), "/q".into(), "/v:on".into(), "/c".into(), r"..\retry-queued.cmd".into()]
    }
}

pub(crate) fn queued_command() -> &'static str {
    #[cfg(unix)]
    return "printf synthetic-queued-request";
    #[cfg(windows)]
    return "echo synthetic-queued-request";
}
