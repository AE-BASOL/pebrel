//! Synthetic, noninteractive launchers installed in an owned test directory.

use std::path::Path;

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
        let source =
            std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("system directory"))
                .join("System32/cmd.exe");
        std::fs::copy(source, program).expect("copy synthetic fixture executable");
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

pub(crate) fn queued_args(marker: &str) -> Vec<String> {
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
    return vec![
        "/d".into(),
        "/q".into(),
        "/v:on".into(),
        "/c".into(),
        format!(
            "echo {marker}:%PEBREL_512_FROZEN%:%CD%&echo 512-PANE:%PEBREL_PANE_ID%:%NEBULA_PANE_ID%:%PEBREL_PROCESS_ID%&\
         set /p \"line=%CD%>\"&echo 512-INPUT:!line!&\
         if \"!line!\"==\"echo synthetic-queued-request\" (echo synthetic-queued-request&echo 512-QUEUED-EXECUTED)&set /p stop="
        ),
    ];
}

pub(crate) fn queued_command() -> &'static str {
    #[cfg(unix)]
    return "printf synthetic-queued-request";
    #[cfg(windows)]
    return "echo synthetic-queued-request";
}
