# Pulsar-Dialog

Dialogs and context menus for Pulsar, drawn with the engine's own UI framework
([WGPUI](https://github.com/Far-Beyond-Pulsar/WGPUI) and
[WGPUI-Component](https://github.com/Far-Beyond-Pulsar/WGPUI-Component)), so
they look like the rest of the engine instead of the operating system's
message boxes.

A dialog wears the user's saved Pulsar theme, and is dark when there is none.

## Why a helper process

Many of Pulsar's questions are asked when the engine cannot draw yet: before
it has a window, or because the graphics or shader setup is what is wrong (no
discrete GPU, the shader compiler missing). So the dialog is not drawn by the
asking process. The asker sends a request to the `pulsar-dialog` helper, which
draws it with its own GPU context and answers.

```
engine / Hub / installer                  pulsar-dialog (helper)
  pulsar_dialog_protocol::client::show ──►  reads a DialogRequest (JSON, stdin)
        (no GPU, no UI dependency)          draws it in the engine's look
                                  ◄──────  prints a DialogResponse (JSON, stdout)
```

A process that already has a GPUI app (the editor) can skip the helper and use
`pulsar_dialog::open_dialog`.

## Crates

| Crate | What |
|---|---|
| `pulsar_dialog_protocol` | `DialogRequest` / `DialogResponse`, the context-menu request shape, and the client that runs the helper. No UI or GPU dependency: this is what the engine links. Optional `native-fallback` feature shows the platform's own box if the helper cannot run. |
| `pulsar_dialog` | The dialog as a GPUI view, theme selection (read-only), the window size, and `run_blocking` / `open_dialog`. |
| `pulsar-dialog` | The helper executable. |

## Use

```rust
use pulsar_dialog_protocol::{client, DialogRequest, Severity};

let answer = client::show(
    &DialogRequest::new("No Discrete GPU Detected", "Continue using the available GPU?")
        .severity(Severity::Warning)
        .detail("Adapter: Qualcomm Adreno")
        .yes_no()
        .checkbox("Do not ask again", false),
);

if answer.is_yes() {
    // carry on
}
```

`show` returns `Outcome::Unavailable` when nothing could show the dialog (no
helper, no display). Decide what silence means: a prompt that guards a
download reads it as "no".

The helper is found through `PULSAR_DIALOG_EXE`, then beside the running
executable, then on `PATH`. By hand:

```
pulsar-dialog --request-file request.json     # or the JSON on stdin
```

It prints one JSON line (the response) and exits 0.

## Progress windows

A long job (rebuilding the engine) is shown by `pulsar-dialog --progress`: the
host sends a `ProgressRequest` as the first stdin line, then one
`ProgressUpdate` per line (`update`, `failed` with buttons, `done`), and reads
one `ProgressResponse` (`finished`, `answered`, `closed`) when the window ends.
`pulsar_dialog_protocol::progress` wraps that:

```rust
use pulsar_dialog_protocol::progress::{start, ProgressRequest};

let mut window = start(&ProgressRequest::new("Bootstrap Engine", "Rebuilding engine"))?;
window.update(Some("Building release engine"), Some(0.4));
// on success:  window.finish();
// on failure:  let answer = window.fail(Some("Rebuild failed"), "cargo exited with 101", None, buttons);
```

After a failure the window stays until a button is pressed (Enter presses the
default, Escape the cancel). If the host goes away while the job is running,
the window closes. `cargo run --example progress_demo -- fail` shows it.

## Theme

`state.json` in the per-user app-data directory names the saved theme; the
themes are the `*.json` files in its `themes/` folder, the same files the
engine and the Hub use. A request may carry its own `ThemeHint`. The helper
only reads: it never writes `state.json` or the themes folder.

## Status

Built: requests and responses, the client, progress windows, the dialog view (severity icon,
message, detail, checkbox, buttons, Enter and Escape, close-as-answer), theme
selection, the helper.

Planned, in this order:

1. Replace the engine's native popups (`rfd::MessageDialog`): the discrete-GPU
   warning, the DXC setup prompts, the file-association prompts, and the editor's
   confirmations.
2. Context menus: the request shape is in `pulsar_dialog_protocol::menu`; the
   helper answers `None` until the menu window is drawn.
3. A software-rendering fallback in the helper for machines where the GPU
   itself fails.
4. Sizing a dialog to its content (today the height is estimated from the text).

## Building

```
cargo build --release -p pulsar-dialog
cargo test --workspace
```

The GPUI revision is pinned in `Cargo.toml` to the one Pulsar-Native pins; keep
them together.

## License

Apache-2.0.
