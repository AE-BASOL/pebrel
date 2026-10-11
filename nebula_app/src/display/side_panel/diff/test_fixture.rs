//! Owned Git configuration for actual-Git reader and control fixtures.
use std::collections::HashMap;
use std::process::{Command, Stdio};

pub(crate) struct GitTestConfig {
    directory: tempfile::TempDir,
}

impl GitTestConfig {
    pub(crate) fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("hooks")).unwrap();
        for name in ["global", "system", "attributes"] {
            std::fs::write(directory.path().join(name), b"").unwrap();
        }
        Self { directory }
    }

    pub(crate) fn environment(&self) -> HashMap<String, String> {
        let path = |name| self.directory.path().join(name).to_str().unwrap().to_owned();
        let settings = [
            ("core.hooksPath", path("hooks")),
            ("core.attributesFile", path("attributes")),
            ("commit.gpgSign", "false".into()),
            ("tag.gpgSign", "false".into()),
            ("core.fsmonitor", "false".into()),
            ("user.useConfigOnly", "true".into()),
            ("user.name", "Synthetic Git test".into()),
            ("user.email", "fixture@example.invalid".into()),
        ];
        let mut env = HashMap::from([
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), path("global")),
            ("GIT_CONFIG_SYSTEM".into(), path("system")),
            ("GIT_ATTR_NOSYSTEM".into(), "1".into()),
            ("GIT_TEMPLATE_DIR".into(), path("hooks")),
            ("GIT_CONFIG_PARAMETERS".into(), String::new()),
            ("GIT_CONFIG_COUNT".into(), settings.len().to_string()),
        ]);
        for (index, (key, value)) in settings.into_iter().enumerate() {
            env.insert(format!("GIT_CONFIG_KEY_{index}"), key.into());
            env.insert(format!("GIT_CONFIG_VALUE_{index}"), value);
        }
        env
    }

    pub(crate) fn configure(&self, command: &mut Command) {
        for name in [
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
            "GIT_DIR",
            "GIT_COMMON_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG",
            "GIT_SHALLOW_FILE",
            "GIT_SSH",
            "GIT_SSH_COMMAND",
            "GIT_ASKPASS",
            "SSH_ASKPASS",
            "SSH_AUTH_SOCK",
        ] {
            command.env_remove(name);
        }
        command.envs(self.environment()).stdin(Stdio::null());
        crate::platform::process::hidden_command(command);
    }
}
