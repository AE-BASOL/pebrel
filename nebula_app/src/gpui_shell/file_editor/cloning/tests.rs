use super::*;
use gpui::{TestAppContext, VisualTestContext};
use gpui_component::Root;

fn duplicate(
    source: &Entity<TextFileView>,
    cx: &mut TestAppContext,
) -> (Entity<TextFileView>, VisualTestContext) {
    let snapshot = cx.update(|cx| source.read(cx).duplicate_snapshot(cx));
    let mut file = None;
    let (_, window) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| TextFileView::from_duplicate(snapshot, window, cx));
        file = Some(view.clone());
        Root::new(view, window, cx)
    });
    window.run_until_parked();
    (file.unwrap(), window.clone())
}

fn modifier() -> &'static str {
    if crate::platform::Platform::current() == crate::platform::Platform::MacOS {
        "cmd"
    } else {
        "ctrl"
    }
}

fn focus_source(file: &Entity<TextFileView>, window: &mut VisualTestContext) {
    window.update(|window, cx| {
        file.update(cx, |file, cx| {
            file.preview = false;
            file.input.read(cx).focus_handle(cx).focus(window, cx);
        });
    });
}

#[gpui::test]
fn mixed_duplicate_document_preserves_draft_independent_undo_and_revision_conflict(
    cx: &mut TestAppContext,
) {
    let name = "mixed_duplicate_document_preserves_draft_independent_undo_and_revision_conflict";
    if crate::runtime_api::tests::file_pane_fixture::isolated_child(module_path!(), name) {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.md");
    let original = "# Original\n\nSaved content\n";
    let draft = "# Draft\n\nUnsaved content\n";
    std::fs::write(&path, original).unwrap();
    let (source, mut source_window) = super::super::tests::open(path.clone(), cx);
    focus_source(&source, &mut source_window);
    source_window.simulate_keystrokes(&format!("{}-a", modifier()));
    source_window.simulate_input(draft);
    source_window.run_until_parked();
    let (copy, mut copy_window) = duplicate(&source, cx);
    copy.read_with(&copy_window, |file, cx| {
        assert_eq!(file.draft(cx), draft);
        assert!(file.dirty && !file.loading && !file.saving);
        assert_eq!(file.document.as_ref().unwrap().text, original);
        let source = source.read(cx);
        assert_ne!(source.input.entity_id(), file.input.entity_id());
        assert_ne!(source.preview_images.entity_id(), file.preview_images.entity_id());
        assert!(!Rc::ptr_eq(&source.blocks, &file.blocks));
        assert!(!Rc::ptr_eq(&source.inline_views, &file.inline_views));
    });
    focus_source(&copy, &mut copy_window);
    copy_window.simulate_keystrokes(&format!("{}-z", modifier()));
    copy_window.run_until_parked();
    assert_eq!(copy.read_with(&copy_window, |file, cx| file.draft(cx)), original);
    assert_eq!(source.read_with(&source_window, |file, cx| file.draft(cx)), draft);
    copy_window.simulate_keystrokes(&format!("{}-shift-z", modifier()));
    copy_window.run_until_parked();
    assert_eq!(copy.read_with(&copy_window, |file, cx| file.draft(cx)), draft);
    copy_window.simulate_keystrokes(&format!("{}-a", modifier()));
    copy_window.simulate_input("Copy's independent draft");
    copy_window.run_until_parked();
    copy_window.update(|_, cx| copy.update(cx, |file, cx| file.save(cx).detach()));
    copy_window.run_until_parked();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "Copy's independent draft");
    source_window.update(|_, cx| source.update(cx, |file, cx| file.save(cx).detach()));
    source_window.run_until_parked();
    source.read_with(&source_window, |file, cx| {
        assert!(file.dirty && !file.saving);
        assert_eq!(file.draft(cx), draft);
        assert!(matches!(file.notice, Some((Message::EditorConflict, None))));
    });
    assert_eq!(std::fs::read_to_string(path).unwrap(), "Copy's independent draft");
    crate::runtime_api::tests::file_pane_fixture::completed(name);
}

#[gpui::test]
fn mixed_duplicate_code_has_independent_input_and_undo_baseline(cx: &mut TestAppContext) {
    let name = "mixed_duplicate_code_has_independent_input_and_undo_baseline";
    if crate::runtime_api::tests::file_pane_fixture::isolated_child(module_path!(), name) {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.rs");
    std::fs::write(&path, "fn original() {}\n").unwrap();
    let (source, mut source_window) = super::super::tests::open(path, cx);
    focus_source(&source, &mut source_window);
    source_window.simulate_keystrokes(&format!("{}-a", modifier()));
    source_window.simulate_input("fn unsaved() {}\n");
    source_window.run_until_parked();
    let (copy, mut copy_window) = duplicate(&source, cx);
    focus_source(&copy, &mut copy_window);
    copy_window.simulate_keystrokes(&format!("{}-z", modifier()));
    copy_window.run_until_parked();
    assert_eq!(copy.read_with(&copy_window, |file, cx| file.draft(cx)), "fn original() {}\n");
    assert_eq!(source.read_with(&source_window, |file, cx| file.draft(cx)), "fn unsaved() {}\n");
    source_window.simulate_keystrokes(&format!("{}-z", modifier()));
    source_window.run_until_parked();
    assert_eq!(source.read_with(&source_window, |file, cx| file.draft(cx)), "fn original() {}\n");
    crate::runtime_api::tests::file_pane_fixture::completed(name);
}

#[gpui::test]
fn mixed_duplicate_preserves_read_only_and_error_without_reloading_loaded_revision(
    cx: &mut TestAppContext,
) {
    let name = "mixed_duplicate_preserves_read_only_and_error_without_reloading_loaded_revision";
    if crate::runtime_api::tests::file_pane_fixture::isolated_child(module_path!(), name) {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("readonly.txt");
    std::fs::write(&path, "Read only revision").unwrap();
    let writable_permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut permissions = writable_permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let (source, _) = super::super::tests::open(path.clone(), cx);
    std::fs::set_permissions(&path, writable_permissions).unwrap();
    std::fs::remove_file(&path).unwrap();
    let (copy, mut copy_window) = duplicate(&source, cx);
    focus_source(&copy, &mut copy_window);
    copy_window.simulate_keystrokes(&format!("{}-a", modifier()));
    copy_window.simulate_input("Must not replace readonly source");
    copy_window.run_until_parked();
    copy.read_with(&copy_window, |file, cx| {
        assert!(file.document.as_ref().unwrap().read_only);
        assert_eq!(file.draft(cx), "Read only revision");
        assert!(!file.dirty && !file.loading);
    });
    let (missing, _) = super::super::tests::open(path, cx);
    let (missing_copy, missing_window) = duplicate(&missing, cx);
    missing_copy.read_with(&missing_window, |file, _| {
        assert!(file.document.is_none() && !file.loading && !file.saving);
        assert!(matches!(file.notice, Some((Message::EditorReadFailed, Some(_)))));
    });
    crate::runtime_api::tests::file_pane_fixture::completed(name);
}

#[gpui::test]
fn mixed_duplicate_loading_file_has_its_own_completion_owner(cx: &mut TestAppContext) {
    let name = "mixed_duplicate_loading_file_has_its_own_completion_owner";
    if crate::runtime_api::tests::file_pane_fixture::isolated_child(module_path!(), name) {
        return;
    }
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        super::super::init(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("loading.rs");
    std::fs::write(&path, "fn loaded_independently() {}\n").unwrap();
    let mut files = None;
    let (_, window) = cx.add_window_view(|window, cx| {
        let source = cx.new(|cx| TextFileView::new(path.clone(), window, cx));
        assert!(source.read(cx).loading);
        let snapshot = source.read(cx).duplicate_snapshot(cx);
        let copy = cx.new(|cx| TextFileView::from_duplicate(snapshot, window, cx));
        assert!(copy.read(cx).loading);
        source.update(cx, |file, cx| file.detach_host(cx));
        files = Some((source, copy.clone()));
        Root::new(copy, window, cx)
    });
    window.run_until_parked();
    let (source, copy) = files.unwrap();
    copy.read_with(window, |file, cx| {
        assert!(!file.loading && !file.saving);
        assert_eq!(file.draft(cx), "fn loaded_independently() {}\n");
        assert_ne!(source.read(cx).input.entity_id(), file.input.entity_id());
        assert!(source.read(cx).document.is_none() && !source.read(cx).loading);
        assert!(file.document.is_some() && file.operation.is_none());
    });
    crate::runtime_api::tests::file_pane_fixture::completed(name);
}
