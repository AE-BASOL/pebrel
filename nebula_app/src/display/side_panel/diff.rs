//! Read-only, bounded Git patches for a frozen repository identity.
use super::GitLocation;
use crate::git_worktree::status::{STATUS_ARGS, parse_status};
use crate::runtime_exec::{PaneExecContext, execute};
use std::path::{Component, Path};
use std::time::{Duration, Instant};

const LIMIT: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(25);

#[cfg(test)]
pub(crate) mod test_fixture;
#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffKey {
    pub location: GitLocation,
    pub path: String,
    pub staged: bool,
}

pub(crate) struct DiffPatch {
    pub root: String,
    pub text: String,
    pub truncated: bool,
    pub lossy: bool,
}

impl DiffKey {
    pub(crate) fn label(&self) -> String {
        match &self.location {
            GitLocation::Local { root } => format!("{} / {}", root.display(), self.path),
            GitLocation::Wsl { distro, root } => format!("{distro}:{root} / {}", self.path),
        }
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}

pub(crate) fn load(key: &DiffKey) -> Result<DiffPatch, String> {
    if !valid_path(&key.path) {
        return Err("Invalid repository-relative path".into());
    }
    let mut options = nebula_terminal::tty::Options::default();
    #[cfg(test)]
    let test_configuration = test_fixture::GitTestConfig::new();
    #[cfg(test)]
    options.env.extend(test_configuration.environment());
    let cwd = match &key.location {
        GitLocation::Local { root } => {
            options.working_directory = Some(root.clone());
            root.to_str().ok_or("Repository path is not UTF-8")?.to_owned()
        },
        GitLocation::Wsl { distro, root } => {
            options.shell = Some(nebula_terminal::tty::Shell::new(
                "wsl.exe".into(),
                vec!["--distribution".into(), distro.clone()],
            ));
            root.clone()
        },
    };
    if cwd.contains('\0') {
        return Err("Invalid repository location".into());
    }
    let context = PaneExecContext::from_pty_options(&options).for_git();
    let started = Instant::now();
    let run =
        |root: &str, args: Vec<String>, no_index: bool| -> Result<serde_json::Value, String> {
            let remaining = DEADLINE.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err("Git diff timed out".into());
            }
            let mut argv = vec![
                "git".into(),
                "--no-pager".into(),
                "--literal-pathspecs".into(),
                "--no-optional-locks".into(),
                "-c".into(),
                format!("safe.directory={root}"),
                "-C".into(),
                root.into(),
            ];
            argv.extend(args);
            let result =
                execute(context.clone(), cwd.clone(), argv, remaining.as_millis() as u64, LIMIT)
                    .map_err(|error| error.message)?;
            if result["timed_out"] == true {
                return Err("Git diff timed out".into());
            }
            let exit = result["exit_code"].as_i64();
            if exit != Some(0) && !(no_index && exit == Some(1)) {
                return Err(result["stderr"].as_str().unwrap_or("Git diff failed").to_owned());
            }
            Ok(result)
        };
    let complete = |result: &serde_json::Value| -> Result<String, String> {
        if result["capture"]["stdout"]["truncated"] == true
            || result["capture"]["stdout"]["encoding"] != "utf-8"
        {
            return Err("Git repository/status output exceeds the supported UTF-8 capture".into());
        }
        Ok(result["stdout"].as_str().unwrap_or_default().to_owned())
    };
    // Porcelain paths are rooted at the repository, even when the drawer follows a subdirectory.
    let reported =
        complete(&run(&cwd, vec!["rev-parse".into(), "--show-toplevel".into()], false)?)?;
    let root = reported.strip_suffix('\n').unwrap_or(&reported);
    let root = if matches!(&key.location, GitLocation::Local { .. })
        && crate::platform::Platform::current() == crate::platform::Platform::Windows
    {
        root.strip_suffix('\r').unwrap_or(root)
    } else {
        root
    }
    .to_owned();
    if root.is_empty() || root.contains('\0') {
        return Err("Invalid Git repository root".into());
    }
    let mut status_args: Vec<_> = STATUS_ARGS.iter().map(|s| (*s).to_owned()).collect();
    status_args.push("--untracked-files=all".into());
    let status = complete(&run(&root, status_args, false)?)?;
    let status = parse_status(&status).map_err(str::to_owned)?;
    let entry = status.entries.iter().find(|entry| entry.path == key.path);
    let Some(entry) =
        entry.filter(|entry| if key.staged { entry.staged() } else { entry.unstaged() })
    else {
        return Ok(DiffPatch { root, text: String::new(), truncated: false, lossy: false });
    };
    let no_index = entry.index == '?' && !key.staged;
    let mut args = vec![
        "diff".into(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
        "--no-color".into(),
        "--find-renames".into(),
    ];
    if no_index {
        args.push("--no-index".into());
    } else {
        args.push("--relative".into());
        if key.staged {
            args.push("--cached".into());
        }
    }
    args.push("--".into());
    if no_index {
        // Git's no-index parser treats this literal as a missing file on all
        // hosts, including Git for Windows. It is not a config-file device path.
        args.push("/dev/null".into());
    } else if let Some(original) = &entry.original {
        if !valid_path(original) {
            return Err("Invalid original Git path".into());
        }
        args.push(original.clone());
    }
    // Unlike pathspecs, no-index operands treat a bare dash as standard input.
    args.push(if no_index && key.path == "-" { "./-".into() } else { key.path.clone() });
    let result = run(&root, args, no_index)?;
    Ok(DiffPatch {
        root,
        text: result["stdout"].as_str().unwrap_or_default().to_owned(),
        truncated: result["capture"]["stdout"]["truncated"] == true,
        lossy: result["capture"]["stdout"]["encoding"] != "utf-8",
    })
}
