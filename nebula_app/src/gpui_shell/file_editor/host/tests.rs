use super::*;
use futures::channel::oneshot;
use gpui::{Context, IntoElement, Render, Styled as _, TestAppContext, VisualTestContext};
use gpui_component::Root;
use std::collections::VecDeque;

type ReadResult = std::io::Result<LoadedDocument>;
thread_local! {
    static LOADS: RefCell<VecDeque<(DocumentSource, oneshot::Receiver<ReadResult>)>> = RefCell::default();
}

pub(super) fn take_load(source: &DocumentSource) -> Option<oneshot::Receiver<ReadResult>> {
    LOADS.with(|loads| {
        let (expected, receiver) = loads.borrow_mut().pop_front()?;
        assert_eq!(&expected, source, "fixture must intercept the real source's load");
        Some(receiver)
    })
}

struct Reads;
impl Drop for Reads {
    fn drop(&mut self) {
        LOADS.with(|loads| loads.borrow_mut().clear());
    }
}

fn pending(source: DocumentSource) -> oneshot::Sender<ReadResult> {
    let (sender, receiver) = oneshot::channel();
    LOADS.with(|loads| loads.borrow_mut().push_back((source, receiver)));
    sender
}

impl TextFileView {
    pub(crate) fn queue_remote_test_load(document: crate::ssh_sftp::document::RemoteDocument) {
        let sender = pending(DocumentSource::Remote(document.location.clone()));
        sender.send(Ok(LoadedDocument::Remote(document))).ok().unwrap();
    }
}

fn local(path: &std::path::Path) -> LoadedDocument {
    LoadedDocument::Local(document::Document::load(path).unwrap())
}

fn open(
    source: DocumentSource,
    cx: &mut TestAppContext,
) -> (Entity<TextFileView>, VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        super::super::super::math_view::register(cx);
        super::super::init(cx);
    });
    let mut file = None;
    let (_, window) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| match source {
            DocumentSource::Local(path) => TextFileView::new(path, window, cx),
            DocumentSource::Remote(location) => TextFileView::new_remote(location, window, cx),
        });
        file = Some(view.clone());
        Root::new(view, window, cx)
    });
    window.run_until_parked();
    (file.unwrap(), window.clone())
}

struct FileTransferProbe {
    file: Entity<TextFileView>,
    render_file: bool,
}

impl Render for FileTransferProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        gpui::div().size_full().children(self.render_file.then(|| self.file.clone()))
    }
}

fn transfer_before_draw(
    file: &Entity<TextFileView>,
    source: &mut VisualTestContext,
    cx: &mut TestAppContext,
) -> (VisualTestContext, Entity<FileTransferProbe>) {
    source.update(|window, cx| file.update(cx, |view, cx| view.prepare_transfer(window, cx)));
    // add_window_view draws automatically. Its root retains the editor without
    // rendering it until the test explicitly requests the destination's first draw.
    let holder = cx.new(|_| FileTransferProbe { file: file.clone(), render_file: false });
    let (_, destination) = cx.add_window_view(|window, cx| {
        file.update(cx, |view, cx| {
            view.attach_host(window, cx);
            view.set_render_active(true, window, cx);
        });
        Root::new(holder.clone(), window, cx)
    });
    let destination = destination.clone();
    let source_window = source.update(|window, _| {
        let handle = window.window_handle();
        window.remove_window();
        handle
    });
    assert!(!cx.windows().contains(&source_window));
    assert!(cx.update(|cx| source_window.update(cx, |_, _, _| ())).is_err());
    (destination, holder)
}

fn reveal(holder: &Entity<FileTransferProbe>, destination: &mut VisualTestContext) {
    destination.update(|window, cx| {
        holder.update(cx, |holder, cx| {
            holder.render_file = true;
            cx.notify();
        });
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn transfer(
    file: &Entity<TextFileView>,
    source: &mut VisualTestContext,
    cx: &mut TestAppContext,
) -> VisualTestContext {
    let (mut destination, holder) = transfer_before_draw(file, source, cx);
    reveal(&holder, &mut destination);
    destination
}

fn settle(file: &Entity<TextFileView>, window: &mut VisualTestContext, text: &str) {
    window.run_until_parked();
    file.read_with(window, |view, cx| {
        assert!(!view.loading && view.notice.is_none());
        assert_eq!(view.draft(cx).as_ref(), text);
    });
}

#[gpui::test]
fn pending_local_read_finishes_in_new_host_before_its_first_draw(cx: &mut TestAppContext) {
    let _reads = Reads;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owner.txt");
    std::fs::write(&path, "late local 🌿").unwrap();
    let sender = pending(DocumentSource::Local(path.clone()));
    let (file, mut source) = open(DocumentSource::Local(path.clone()), cx);
    assert!(file.read_with(&source, |view, _| view.loading));
    let (mut destination, holder) = transfer_before_draw(&file, &mut source, cx);
    assert!(
        cx.update(|cx| cx.with_window(file.entity_id(), |window, _| window.window_handle()))
            .is_none(),
        "GPUI still maps the editor to its closed source before destination draw"
    );
    sender.send(Ok(local(&path))).ok().unwrap();
    settle(&file, &mut destination, "late local 🌿");
    assert!(
        cx.update(|cx| cx.with_window(file.entity_id(), |window, _| window.window_handle()))
            .is_none(),
        "completion must apply before the editor's first destination draw"
    );
    reveal(&holder, &mut destination);
    let destination_window = destination.update(|window, _| window.window_handle());
    assert_eq!(
        cx.update(|cx| cx.with_window(file.entity_id(), |window, _| window.window_handle())),
        Some(destination_window)
    );
    destination.update(|window, cx| {
        window.draw(cx).clear(cx);
        file.update(cx, |view, cx| {
            view.input.update(cx, |input, cx| input.replace_all("new owner's draft", window, cx));
        });
    });
    destination.run_until_parked();
    assert!(file.read_with(&destination, |view, _| view.dirty));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "late local 🌿");
}

#[gpui::test]
fn real_sftp_read_result_keeps_remote_identity_after_source_window_closes(cx: &mut TestAppContext) {
    let _reads = Reads;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    // The snapshot is read through the real SFTP codec. Only result delivery is
    // gated, so this test never resolves a user's SSH alias or credentials.
    let document =
        runtime.block_on(crate::ssh_sftp::document::fixture_snapshot("远端正文\r\n".as_bytes()));
    let location = document.location.clone();
    let sender = pending(DocumentSource::Remote(location.clone()));
    let (file, mut source) = open(DocumentSource::Remote(location.clone()), cx);
    let mut destination = transfer(&file, &mut source, cx);
    sender.send(Ok(LoadedDocument::Remote(document))).ok().unwrap();
    settle(&file, &mut destination, "远端正文\n");
    file.read_with(&destination, |view, _| {
        assert!(view.is_remote_location(&location));
        assert!(!view.preview && !view.markdown);
        assert!(matches!(view.document, Some(LoadedDocument::Remote(_))));
    });
}

#[gpui::test]
fn approved_dirty_reload_uses_latest_host_and_rejects_a_later_draft(cx: &mut TestAppContext) {
    let _reads = Reads;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reload.txt");
    std::fs::write(&path, "original").unwrap();
    let first = pending(DocumentSource::Local(path.clone()));
    let (file, mut source) = open(DocumentSource::Local(path.clone()), cx);
    first.send(Ok(local(&path))).ok().unwrap();
    settle(&file, &mut source, "original");
    source.update(|window, cx| {
        window.draw(cx).clear(cx);
        file.update(cx, |view, cx| {
            view.input.update(cx, |input, cx| input.replace_all("approved draft", window, cx));
        });
    });
    source.run_until_parked();
    source.update(|window, cx| file.update(cx, |view, cx| view.request_reload(window, cx)));
    assert!(source.has_pending_prompt());
    let second = pending(DocumentSource::Local(path.clone()));
    let mut destination = transfer(&file, &mut source, cx);
    let discard = destination.update(|_, cx| {
        super::super::super::config::ui_language(cx).text(Message::EditorDiscard).to_owned()
    });
    destination.simulate_prompt_answer(&discard);
    destination.run_until_parked();
    assert!(file.read_with(&destination, |view, _| view.loading));
    std::fs::write(&path, "reloaded in destination").unwrap();
    second.send(Ok(local(&path))).ok().unwrap();
    settle(&file, &mut destination, "reloaded in destination");

    destination.update(|window, cx| {
        window.draw(cx).clear(cx);
        file.update(cx, |view, cx| {
            view.input.update(cx, |input, cx| input.replace_all("first dirty draft", window, cx));
        });
    });
    destination.run_until_parked();
    destination.update(|window, cx| file.update(cx, |view, cx| view.request_reload(window, cx)));
    destination.update(|window, cx| {
        file.update(cx, |view, cx| {
            view.input.update(cx, |input, cx| input.replace_all("newer draft", window, cx));
        });
    });
    destination.simulate_prompt_answer(&discard);
    destination.run_until_parked();
    file.read_with(&destination, |view, cx| {
        assert!(!view.loading && view.dirty);
        assert_eq!(view.draft(cx).as_ref(), "newer draft");
    });
}

#[gpui::test]
fn retired_pane_and_superseded_load_cannot_apply_old_results(cx: &mut TestAppContext) {
    let _reads = Reads;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("replace.txt");
    std::fs::write(&path, "old result").unwrap();
    let old_document = local(&path);
    let first = pending(DocumentSource::Local(path.clone()));
    let (file, mut window) = open(DocumentSource::Local(path.clone()), cx);
    let second = pending(DocumentSource::Local(path.clone()));
    window.update(|window, cx| file.update(cx, |view, cx| view.load(window, cx)));
    window.run_until_parked();
    assert!(first.send(Ok(old_document.clone())).is_err(), "the superseded task is cancelled");
    std::fs::write(&path, "current result").unwrap();
    second.send(Ok(local(&path))).ok().unwrap();
    settle(&file, &mut window, "current result");
    let retired = pending(DocumentSource::Local(path.clone()));
    window.update(|window, cx| file.update(cx, |view, cx| view.load(window, cx)));
    window.run_until_parked();
    window.update(|_, cx| file.update(cx, |view, cx| view.detach_host(cx)));
    window.run_until_parked();
    assert!(retired.send(Ok(old_document)).is_err(), "retiring cancels the pending result");
    file.read_with(&window, |view, cx| {
        assert!(!view.host.is_attached() && !view.loading);
        assert_eq!(view.draft(cx).as_ref(), "current result");
    });
}

#[gpui::test]
fn transfer_stops_old_pointer_job_and_rebuilds_changed_preview_in_destination(
    cx: &mut TestAppContext,
) {
    let _reads = Reads;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preview.md");
    std::fs::write(&path, "# Old heading\n\nText").unwrap();
    let first = pending(DocumentSource::Local(path.clone()));
    let (file, mut source) = open(DocumentSource::Local(path.clone()), cx);
    first.send(Ok(local(&path))).ok().unwrap();
    settle(&file, &mut source, "# Old heading\n\nText");
    source.update(|window, cx| {
        window.draw(cx).clear(cx);
        file.update(cx, |view, cx| {
            view.input
                .update(cx, |input, cx| input.replace_all("# New heading\n\nDraft", window, cx));
            view.start_preview_selection_scroll(window, cx);
            assert!(view.host.selection_scroll.is_some());
        });
    });
    source.run_until_parked();
    assert!(file.read_with(&source, |view, _| view.preview_task.is_some()));
    let mut destination = transfer(&file, &mut source, cx);
    file.read_with(&destination, |view, _| {
        assert!(!view.preview_selection_scroll_active && view.host.selection_scroll.is_none());
        assert!(view.preview_stale);
    });
    destination.executor().advance_clock(Duration::from_millis(300));
    destination.run_until_parked();
    file.read_with(&destination, |view, cx| {
        assert!(!view.preview_stale && view.dirty);
        assert_eq!(view.draft(cx).as_ref(), "# New heading\n\nDraft");
        assert_eq!(view.outline.headings[0].label.as_str(), "New heading");
    });
}
