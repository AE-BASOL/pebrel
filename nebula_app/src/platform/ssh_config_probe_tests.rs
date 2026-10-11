//! Native macOS OpenSSH config-only acceptance with owned inputs.
//! No personal SSH configuration, identity files, agent or connection is used.

use crate::ssh_session::{parse_resolved_config, ssh_config_probe_args};

#[test]
fn jumpserver_usernames_survive_real_openssh_config_only_probes() {
    let directory = tempfile::tempdir().unwrap();
    let include = directory.path().join("included-config");
    let config = directory.path().join("config");
    let control = directory.path().join("control-运维@root");
    std::fs::write(
        &include,
        format!(
            "Match user 运维@root\n HostName 127.0.0.1\n ControlPath \"{}/control-%r\"\n RemoteCommand printf_%r\nHost gateway\n HostName 127.0.0.2\n",
            directory.path().display()
        ),
    )
    .unwrap();
    std::fs::write(&config, format!("Include \"{}\"\n", include.display())).unwrap();
    for (spec, user, host, config) in [
        (
            "jumpserverUser@root@127.0.0.1:2222",
            "jumpserverUser@root",
            "127.0.0.1",
            std::path::Path::new("/dev/null"),
        ),
        (
            "ssh://jumpserverUser@root@[::1]:2222",
            "jumpserverUser@root",
            "::1",
            std::path::Path::new("/dev/null"),
        ),
        (
            "literal%40@root@127.0.0.1:2222",
            "literal%40@root",
            "127.0.0.1",
            std::path::Path::new("/dev/null"),
        ),
        ("ssh://运维@root@[::1]:2222", "运维@root", "::1", std::path::Path::new("/dev/null")),
        ("运维@root@gateway:2222", "运维@root", "127.0.0.1", config.as_path()),
    ] {
        // -G never connects. -F selects only owned input, with all identity
        // and known-host defaults disabled even if this test starts normally.
        let output = std::process::Command::new("/usr/bin/ssh")
            .args(["-F"])
            .arg(config)
            .args([
                "-o",
                "IdentityFile=none",
                "-o",
                "GlobalKnownHostsFile=/dev/null",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-G",
            ])
            .args(ssh_config_probe_args(spec))
            .env_remove("SSH_AUTH_SOCK")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "{spec}: {}", String::from_utf8_lossy(&output.stderr));
        let text = String::from_utf8_lossy(&output.stdout);
        let parsed = parse_resolved_config(spec, &text).unwrap();
        assert_eq!((parsed.user.as_str(), parsed.host.as_str(), parsed.port), (user, host, 2222));
        assert_eq!(parsed.original, spec);
        assert!(!parsed.has_identity_files());
        if host == "127.0.0.1" && !user.is_ascii() {
            assert!(text.lines().any(|line| line == format!("controlpath {}", control.display())));
            assert!(text.lines().any(|line| line == "remotecommand printf_运维@root"));
        }
    }
    for (spec, selected_config, host) in [
        ("jumpserverUser@root@127.0.0.1:2222", std::path::Path::new("/dev/null"), "127.0.0.1"),
        ("ssh://jumpserverUser@root@[::1]:2222", std::path::Path::new("/dev/null"), "::1"),
        ("运维@root@gateway:2222", config.as_path(), "127.0.0.2"),
    ] {
        for user_option in [["-l", "cli@account"], ["-o", "User=cli@account"]] {
            let output = std::process::Command::new("/usr/bin/ssh")
                .arg("-F")
                .arg(selected_config)
                .args([
                    "-o",
                    "IdentityFile=none",
                    "-o",
                    "GlobalKnownHostsFile=/dev/null",
                    "-o",
                    "UserKnownHostsFile=/dev/null",
                ])
                .args(user_option)
                .args(["-p", "2022", "-G"])
                .args(ssh_config_probe_args(spec))
                .env_remove("SSH_AUTH_SOCK")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let parsed =
                parse_resolved_config(spec, &String::from_utf8_lossy(&output.stdout)).unwrap();
            assert_eq!(
                (parsed.user.as_str(), parsed.host.as_str(), parsed.port),
                ("cli@account", host, 2022)
            );
            assert_eq!(parsed.original, spec);
            assert!(!parsed.has_identity_files());
        }
    }
}
