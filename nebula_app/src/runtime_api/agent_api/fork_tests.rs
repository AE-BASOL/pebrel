use super::*;

fn request(source: u64, target: &std::path::Path) -> RuntimeCommand {
    RuntimeCommand::AgentFork {
        window_id: Some(7),
        source_pane_id: Some(source),
        source_cwd: None,
        name: "synthetic-reviewer".into(),
        kind: crate::ai_agents::AgentKind::Codex,
        session_id: None,
        command: "codex".into(),
        branch: Some("nebula/file-must-not-fork".into()),
        base: None,
        path: Some(target.into()),
        allow_dirty_source: false,
    }
}

#[test]
fn file_and_error_snapshot_cannot_start_a_worktree_transaction() {
    if git_fixture::isolated_child(
        module_path!(),
        "file_and_error_snapshot_cannot_start_a_worktree_transaction",
    ) {
        return;
    }
    let repository = test_git_repository();
    let target = repository.path().join("must-not-be-created");
    for kind in ["document", "code", "image", "file_error"] {
        let hub = RuntimeHub::new();
        let mut state = snapshot(RuntimeTaskState::Idle);
        let pane = &mut state.windows[0].tabs[0].panes[0];
        pane.kind = kind.into();
        // Deliberately forged cwd proves rejection precedes Git preparation.
        pane.cwd = repository.path().to_string_lossy().into_owned();
        pane.file = Some(tabs::FileInfo {
            path: repository.path().join("fixture.rs").to_string_lossy().into_owned(),
            remote: false,
            dirty: false,
            saving: false,
            ready: kind != "file_error",
            revision: None,
        });
        hub.publish(state);
        let error = agent_api::prepare_dispatch_command(request(3, &target), &hub).err().unwrap();
        assert_eq!(error.code, "unsupported_pane");
        assert_eq!(error.details.unwrap()["kind"], kind);
        assert!(!target.exists());
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(repository.path()).args([
            "show-ref",
            "--verify",
            "--quiet",
            "refs/heads/nebula/file-must-not-fork",
        ]);
        let status = crate::platform::process::hidden_command(&mut command).status().unwrap();
        assert!(!status.success(), "rejected source cannot leave a branch");
    }
    git_fixture::completed("file_and_error_snapshot_cannot_start_a_worktree_transaction");
}

#[test]
fn old_snapshot_without_kind_still_prepares_a_terminal_fork() {
    if git_fixture::isolated_child(
        module_path!(),
        "old_snapshot_without_kind_still_prepares_a_terminal_fork",
    ) {
        return;
    }
    let repository = test_git_repository();
    let target = repository.path().join("old-terminal-fork");
    let mut state = serde_json::to_value(snapshot(RuntimeTaskState::Idle)).unwrap();
    let pane = &mut state["windows"][0]["tabs"][0]["panes"][0];
    pane["cwd"] = json!(repository.path());
    pane.as_object_mut().unwrap().remove("kind");
    pane.as_object_mut().unwrap().remove("file");
    let hub = RuntimeHub::new();
    hub.publish(serde_json::from_value(state).unwrap());
    let (prepared, transaction) =
        agent_api::prepare_dispatch_command(request(3, &target), &hub).unwrap();
    assert!(matches!(prepared, RuntimeCommand::AgentStart { worktree: Some(_), .. }));
    assert!(target.exists());
    transaction.unwrap().rollback().unwrap();
    assert!(!target.exists());
    git_fixture::completed("old_snapshot_without_kind_still_prepares_a_terminal_fork");
}
