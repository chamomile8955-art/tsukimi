# Windows portable build

This fork maintains Windows portable builds only. The artifact is built by
[`.github/workflows/windows.yml`](../.github/workflows/windows.yml). Linux and
macOS users should use the
[official `tsukinaha/tsukimi` repository](https://github.com/tsukinaha/tsukimi).

The Windows ZIP is a portable build. Extract the complete `Tsukimi` directory
to a writable location and run `tsukimi.exe` from that directory. Do not place
the extracted application under `Program Files`, because portable mode must be
able to write beside the executable.

Tsukimi creates and uses these directories relative to `tsukimi.exe`:

| Directory | Contents |
| --- | --- |
| `data/` | Application and third-party per-user data |
| `cache/` | Poster, image, API response, GStreamer, and temporary caches |
| `config/` | GSettings configuration, accounts, and preferences |
| `logs/` | `tsukimi.log` and explicitly requested log files |

The GSettings keyfile is stored at
`config/glib-2.0/settings/keyfile`. Server response and poster caches are stored
below `cache/tsukimi/`. Startup writes the active config, cache, data, log, and
temporary directories to `logs/tsukimi.log`.

Deleting the extracted `Tsukimi` directory removes data created by the portable
application. Linux and macOS builds continue to use their normal platform
directories.

## Local player (no application history)

Opening a video bypasses server startup and opens the existing player directly:

```powershell
.\tsukimi.exe --local-player -- "D:\Movies\Episode 2.mkv"
```

A filename alone also selects this mode. The clicked file starts from zero;
all supported videos in that same folder are added in natural filename order.
Subfolders, music, and subtitles are not playlist entries. Previous/next,
playlist selection, automatic next-video playback, Ctrl+O, and dropping another
video into the player are supported. The main menu also has a local-video entry
that launches a separate player process, without changing the client session.

No accounts are restored, no Jellyfin connection or progress reporting occurs,
and no playback history, position, window state, or disk log is saved. Preferences
in this mode are in memory only. User mpv configuration/scripts, watch-later
resume/save, and disk demuxer caching are disabled. Temporary runtime directories
are isolated and removed on normal exit; an abnormal termination can leave a
temporary directory, but it contains no application playback history.

For Explorer double-click support, run the packaged helper once:

```batch
tools\windows\register-local-player.bat
```

Then right-click a video, choose **Open with > Choose another app**, select
**Tsukimi Local Player**, and choose **Always**. Registration is per-user and
does not require administrator rights or overwrite Windows `UserChoice`.
It also adds `NoRecentDocs` to this player's file class, following
[Microsoft's shell tracking guidance](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shaddtorecentdocs).
System metadata (Explorer activity, Prefetch, antivirus, crash or driver data)
is outside the app's control; this is not a system-wide forensic erasure feature.

To unregister, first select another default player, then run:

```batch
tools\windows\register-local-player.bat -Unregister
```

The helper must remain inside the extracted `tools/windows` folder alongside
its bundled `video-extensions.json`. Moving the portable folder requires
unregistering the old location and registering the new location.

## Display scaling

The executable embeds a PerMonitorV2 DPI-awareness manifest, with PerMonitor
as a fallback, following the
[Windows DPI manifest guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process).
GTK handles the current monitor's scale; the application does not resize its
controls based on display resolution or apply a second scaling factor.
Startup size remains 1152 x 720 logical pixels, reduced only when necessary to
fit the work area. Larger or maximized windows show more content with the same
control, font and poster sizing. Detail backdrops still fill the first viewport.

Different physical screen sizes or OS scaling settings can change how much of
the display the window occupies. Keep OS scaling appropriate for each display;
do not force bitmap scaling through Windows compatibility overrides.

## Cleaning data from older Windows builds

Older Windows builds could store caches below `%LOCALAPPDATA%`, `%APPDATA%`, or
`%TEMP%`, and preferences in the current user's GSettings registry key. Close
Tsukimi, then run one of the cleanup scripts included in `tools/windows/`.

PowerShell, with confirmation:

```powershell
.\tools\windows\clean-tsukimi-data.ps1
```

PowerShell, without confirmation:

```powershell
.\tools\windows\clean-tsukimi-data.ps1 -Force
```

Command Prompt:

```batch
tools\windows\clean-tsukimi-data.bat
```

The scripts list every existing target before asking for confirmation. They
only remove exact Tsukimi names from known user-data locations and the legacy
Tsukimi GSettings registry key. They do not require administrator privileges
and refuse to remove the current portable application tree.

## Files outside the portable directory

Tsukimi redirects the GLib/GTK data, configuration, cache, runtime, and
temporary locations used by the application. GSettings uses its keyfile backend
instead of the Windows registry, and the GStreamer registry is also local.

Windows and third-party system components may still create operating-system
metadata that applications cannot safely redirect, including Prefetch records,
Windows Error Reporting data, recent-file entries, antivirus history, or GPU
driver shader caches. These are not Tsukimi application data. A user-selected
external mpv configuration may also write wherever that configuration directs
mpv; portable mode does not alter mpv playback or configuration behavior.
