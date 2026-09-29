# UI preview mode

Tsukimi can open its real main-window template without connecting to a server
or reading saved accounts:

```sh
tsukimi --ui-preview
```

The same mode can be enabled for an IDE launch configuration:

```text
TSUKIMI_UI_PREVIEW=1
```

Preview mode uses GLib's in-memory settings backend. It therefore starts with
an empty Sources list and the existing **No Server Selected** page, and does
not read or write saved accounts, routes, window state, or other GSettings
data. Server restore, network requests, and background-image restore are
skipped. The real window template, CSS, top navigation, menu, settings button,
and window controls are still used.

The preview uses a separate GTK application ID, so it can run alongside a
normal Tsukimi instance without activating or reusing that real session.

## Source-tree preview

On a development machine with the normal Meson dependencies installed:

```sh
just preview
```

This performs an incremental development build/install under
`build/dev-prefix/`; it does not create a portable ZIP or release package.

To start with GTK Inspector:

```sh
just preview-inspector
```

For an existing Windows portable build, run from PowerShell:

```powershell
.\tsukimi.exe --ui-preview
```

To start GTK Inspector on Windows:

```powershell
$env:GTK_DEBUG = "interactive"
.\tsukimi.exe --ui-preview
```

`GTK_DEBUG=interactive` normally opens Inspector at startup. If it has been
closed, press `Ctrl+Shift+D` while the Tsukimi window is focused to reopen it.

Remove the environment variable afterward if desired:

```powershell
Remove-Item Env:GTK_DEBUG
```

## Startup Window Checks

Normal and preview launches use the same 1152 x 720 logical-pixel size. On
smaller displays, the window fits within the work area with a 24-pixel margin
per edge. Saved maximized/fullscreen states do not change the next launch.
The splash and main window use the same placement policy and monitor.

On macOS and Windows, centering uses the native work area, excluding the Dock,
menu bar, or taskbar. GTK owns DPI-aware sizing; native placement only moves
the window. On Wayland, the compositor retains control of placement.

For release UI checks, launch in both themes, resize, close, and launch again.
Verify that the splash and main window start at the same size and position,
that settings remain readable at 760 x 720 and 500 x 600, and that playback
controls retain identical geometry after changing the application theme.

## Inspecting and Editing CSS

In GTK Inspector:

1. Use the object picker and click the widget in the Tsukimi window.
2. Use **Objects** to inspect the widget hierarchy and template classes.
3. Use **CSS Nodes** and **CSS Properties** to see matching selectors and
   computed values.
4. Use the global **CSS** editor to paste temporary rules. Changes apply
   immediately and are not written to the repository.
5. Copy successful rules into `resources/style.css`, then rerun
   `just preview` so the updated resource bundle is rebuilt.

GTK Inspector availability depends on the GTK runtime included with the build.
