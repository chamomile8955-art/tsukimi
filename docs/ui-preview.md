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

All resolutions use the same logical-pixel geometry and desktop density.
Resizing or maximizing does not switch between compact and enlarged controls.
Painted sizes (including padding and borders) are 50-pixel navigation tabs,
60-pixel tools, 70-pixel previous/next controls, 80-pixel playback buttons,
28-pixel window controls, and 68-pixel theme swatches. These trim about 10%
from the previous enlarged controls. The explicit playback icon is 40 pixels.
`src/ui_density.rs` owns these dimensions and tests the shared CSS declarations.
Fonts use 150% of the native base font once at the root: a 12-pixel native
font becomes 18 pixels, with 16.2-pixel poster metadata. The UI audit checks
computed navigation fonts as well as button bounds in both themes.
Posters are 276 x 414 and backdrops are 400 x 225 logical pixels. Both themes,
main settings, and player utility pages share these metrics. Surface radii
remain 14 pixels. Below 1100 logical pixels, the source sidebar becomes an
overlay and initially hides so the single-row library toolbar has enough room;
its header button can still open it. Player panel dismissal retains its
separate 500sp breakpoint.
Poster requests use DPI-specific pixel bounds (400 x 600 at 1x, 800 x 1200
at 2x, 1200 x 1800 at 3x), with separate thumbnail/banner/backdrop profiles.
Resolution is part of the cache key, so old 300 x 300 thumbnails are not reused.
Monitor scale changes reload images; the decoded static-image LRU has a
128 MiB pixel budget in addition to its entry limit (visible images remain
owned by their widgets). Image sharpness still depends on the server's source.
Windows embeds a PerMonitorV2 DPI manifest (with a PerMonitor fallback), so GTK
can redraw for each monitor's scaling without an additional DPI transform.
Native work areas are converted using the surface's fractional scale exactly
once. System font and accessibility settings remain respected. The physical
size and fraction of the display occupied still depend on display dimensions
and the user's OS scaling; resolution alone cannot determine either.

The placement tests cover 1080p, 1440p, 1600p and 4K work areas at 100%, 125%,
150%, 175% and 200% scaling. On Windows, also move an open window between
monitors with different DPI settings and check the main UI, settings, and
player popovers. This mixed-monitor test requires actual Windows hardware.

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

## Native UI Regression Audit

Desktop control targets share CSS variables: navigation tabs are 50 pixels,
tool buttons are 60 pixels, the primary playback control is 80 pixels, and
traffic lights are 28 pixels. Frame radii remain unchanged. Player utility
panels always use a dark palette, independent of the application theme.

Media sizes are 276 x 414 for posters, 400 x 225 for backdrops,
290 x 290 for square covers, 550 x 103 for banners, and 195 x 195 for people.
Media cards and list/episode thumbnails retain these dimensions at all window
sizes, before and after binding/rebinding. Ellipsized
labels and intrinsic image sizes do not enlarge cards; album details retain
a 290-pixel square cover.

Movie and series detail backdrops fill the first content viewport, including
after resizing. The title and playback actions remain over the backdrop;
recommendations and other details begin below it and remain scrollable.

With a desktop session and the normal native build dependencies installed:

```sh
just ui-audit 3  # Dark theme
just ui-audit 2  # Light theme
```

To capture a 4K / 200% preview, run each theme with
`TSUKIMI_UI_AUDIT_4K=1 just ui-audit 3` (or `2`). Images are written to
`target/ui-audit/4k`. The audit forces the large main window to 1920 x 1080
logical pixels and renders its GTK scene at 2x into 3840 x 2160 PNGs. Dialogs
and narrow-window test cases retain their normal size and are centered on
the same canvas. These are local GTK renders with fixtures, not Windows
desktop captures. This mode does not alter production window sizing.

The audit builds fresh resources and a development executable, then exercises
the real application in isolated preview mode. A loopback-only mock server
supplies media fixtures; accounts use in-memory settings and fixture caches
remain under `target/ui-audit/`. No saved accounts or real servers are used.

Coverage includes all five settings categories and their lower content, narrow
settings, version preferences and their editor, home/favorites/recommend/search,
populated and empty search results, the four library tabs, filters and their
selection page, the unified library toolbar at normal and narrow sizes with
grid/list and sort-order changes, movie/series/album/person details, metadata/image/refresh/identify/
missing-episode dialogs, image editing and search, add-server and server
management, and all three player utility panels at normal and narrow sizes,
including the lower player settings. It also switches themes while a player
panel is open.

Assertions check live frame radii, dialog bounds, painted button sizes (including
CSS padding and borders), navigation height and label fit,
filter-title fit, poster and disabled-action text colors, action hover backgrounds,
single-row library toolbar alignment, group spacing and unclipped navigation,
window-control size and circular borders, 68-pixel circular theme selectors in
both selected states, media-card dimensions, square album covers, available icons,
full-viewport detail backdrops and visible first-screen playback actions,
unchanged control geometry across 1152 x 720, 1280 x 800 and 1440 x 900 windows,
active player tabs, fixed dark player surfaces and their text contrast,
and that the loaded styles and main-window template match their build sources.
PNG snapshots are saved alongside the fixture cache for visual inspection. The
audit exits automatically; assertion failures return a nonzero exit status.
The preferences-window controls use GTK-rendered close buttons during the audit
so macOS also exercises the Windows-style title-button sizing and alignment.

This is a UI smoke test, not a substitute for real-server integration tests,
video/audio playback, or verification on each supported operating system.
Optional `TSUKIMI_UI_AUDIT_TRACE=1` prints native focus/layout diagnostics.

## Local Player Playback Audit

```sh
just local-player-audit
```

Requires a native desktop, ffmpeg and the regular build dependencies. Generates
silent video/audio fixtures under `target/local-player-audit`, then launches the
real local player through the same filename/GApplication-open path used by file
associations. It checks clicked-file selection, sibling filtering and natural
ordering, previous/next, Unicode/space-containing paths, playlist activation,
automatic next-video, final EOF, invalid files, reopening from zero, and closing
an empty player without a stalled event thread. Screenshots
and red/green pixel checks verify actual libmpv rendering plus the playlist,
settings and media-info panels. It asserts that the Jellyfin client was never
initialized, ignores an explicitly requested disk log, and checks temporary
runtime cleanup after exit. Normal settings are never read or changed.

The Windows workflow additionally tests per-user association registration and
unregistration. Explorer/default-app selection and real audio output still
require Windows hardware; the silent fixture is not an audible quality test.
