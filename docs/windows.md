# Windows portable build

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
