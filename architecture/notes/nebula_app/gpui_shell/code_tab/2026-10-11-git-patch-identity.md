# Frozen Git patch identity and read-only ownership

## Status

Proposed for maintainer review.

## Context

Issue [#538](https://github.com/Kuddev/pebrel/issues/538) requests both the
changed-file hierarchy and a corresponding diff when a changed file is selected.
The hierarchy change preserves file identity; opening the normal document still
cannot show index-versus-worktree provenance, deleted paths, or binary patches.

## Evidence

- [Git row routing](../../../../../nebula_app/src/gpui_shell/workspace/vcs_panel/rows.rs)
  previously selected on click and opened the document or merge editor on Enter.
- [Runtime process capture](../../../../../nebula_app/src/runtime_exec.rs)
  already owns argv boundaries, parallel bounded pipe draining, deadlines,
  process-group termination and reaping for host and WSL commands.
- [Shared status parsing](../../../../../nebula_app/src/git_worktree/status.rs)
  consumes complete NUL records and preserves a renamed file's original path.

## Decision

Capture repository location, repository-relative path and staged comparison from
the rendered changed-file row. One CodeTab tool owns each exact patch key. A
mouse click displays that tool while leaving row focus intact; Enter transfers
focus. The native second click continues to open the ordinary file, or the
existing conflict merge tool, without creating a second copy of the patch.
Localized row hints and accessibility labels distinguish Click/Enter diff from
double-click file editing, and conflict hints also name the Resolve merge action.
Enabled row action buttons stop propagation after their own operation: staging
must not also open a patch, and Resolve must not be overwritten by parent row
activation. Keyboard Unstage retains the already selected patch without opening
another tool.

The first real cwd-switch control exposed that mouse-down redraws rebuilt row
callbacks using the new followed cwd while the old Git snapshot stayed visible.
Each actual host/WSL status snapshot now carries its typed origin; row callbacks
use that snapshot origin even when a new refresh has changed the panel root.
Local file selection and double-click paths use that same snapshot root.
Parser-only synthetic snapshots fall back to the VcsList's retained root.

The patch child owns its asynchronous completion and generation. Refresh clears
old content, replaces the completion task and rejects a stale generation. Closing
the entity drops its UI completion; any already running blocking read remains
bounded by one shared 25-second deadline across repository/status/diff commands.
It never owns or starts a PTY.

Git reads use literal argv pathspecs and the bounded process authority. Resolve
the repository root before comparing status paths so a followed subdirectory
cannot redirect a root-relative filename. Rename comparisons include both paths.
Untracked files use Git's no-index comparison and accept its documented exit 1.
Git and Git for Windows parse the literal `/dev/null` as a missing comparison
side in [their no-index authority](https://github.com/git-for-windows/git/blob/main/diff-no-index.c);
this is separate from opening a null device as a configuration file. WSL executes
the guest Git comparison through the existing process adapter.
A zero-byte untracked file still has a new-file mode patch; an empty comparison
means the selected change is now clean. A file named `-` is passed as `./-` to
no-index because a bare dash selects standard input rather than that file.
External diff/textconv commands are disabled. Capture is limited to 1 MiB per
pipe, and incomplete/lossy patch text has a visible warning.

The tool has no TextFileView, draft, save command, document reload path or durable
session leaf. Its CodeTab path is display metadata only and cannot match an
ordinary file. Virtual patch rows display additions, deletions and hunk headers;
long lines are split into bounded shaping chunks. Copy preserves the captured
patch, including the explicit limitations of truncated/lossy capture. The shared
per-control CopyFeedback owner changes the button to a check plus Copied label
only after issuing a valid clipboard write, refreshes its bounded expiry on repeat,
and clears on refresh/error. Releasing the patch also releases that timer.
The API reports acceptance as `()`, so this does not assert OS clipboard delivery.

## Rejected alternatives

- Loading diffs through a fake file path would hand synthetic content to file
  save/dirty and runtime file-read authorities.
- Running Git while rendering would repeat I/O on every repaint and let a
  followed-directory change reinterpret an already displayed row.
- An arbitrary double-click timer would disagree with the native click counter
  and require another window-owned cancellation service.
- Unbounded `Command::output` would bypass existing deadline and pipe ownership.

## Consequences

Index and worktree comparisons of the same path remain distinct tools. Refresh
is explicit. Empty comparisons, binary messages and command errors remain
visible. Copying an incomplete patch cannot reconstruct omitted bytes.

## Validation

[Reader fixtures](../../../../../nebula_app/src/display/side_panel/diff/tests.rs)
cover synthetic index/worktree changes, Unicode, leading dashes, nested cwd,
rename/delete/untracked/empty/binary/conflict cases, traversal/NUL rejection and
capture limits. [GPUI controls](../../../../../nebula_app/src/gpui_shell/workspace/vcs_panel/diff_tests.rs)
exercise real clicks, Enter, native double-click counts, focus, Copy, Refresh,
read-only behavior, captured repository identity and large virtualized patches.
Synthetic Git commands and asynchronous panel reads use owned empty regular
configuration/attributes files and an empty hooks directory outside repositories.
The test-only context retains those resources until child reaping; it suppresses
signing, fsmonitor and inherited command-line config without process-wide changes.
GPUI controls run in child processes with an owned settings directory and a wired
SSH home resolver. They assert the synthetic host/path before any Workspace
constructor, rather than trusting an unconnected environment name. A full
Workspace renders the real source editor, proves its Save key works, then mounts
the patch and proves the same key cannot write a hidden dirty neighbor. Its actual
Close action also proves copy feedback ownership is released. The other controls
cover repeat/expiry with the test clock and preserve the copy button geometry.
No native user credential or personal SSH file is accessed by these local fixtures.
Execution results must be recorded in the PR after the pinned checks run; the
fixtures alone do not establish native WSL acceptance or a visual review.

## Supersedes

None. This stacks on the changed-file hierarchy implementation.

## Revisit when

The process authority exposes cancellable read workers, or maintainers choose a
dedicated diff tab/session kind, editable staging hunks or side-by-side rendering.

The exact-process UI driver writes fifteen disabled hook/update/restore/resume/refresh settings into every private child config directory. Before constructing a Workspace, each child verifies the effective loaded settings as well as the synthetic SSH path and host. The outer queue fixture is separately prepared; its settings cannot substitute for child configuration.
