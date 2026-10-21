# UWiper

**Ultimate Wiper** – a Windows app for removing programs completely and seeing everything they leave behind: files, registry keys, services, scheduled tasks, startup entries and caches.

UWiper follows its own rule: it is portable, keeps everything in the folder you put it in and never writes to AppData, ProgramData or the registry unless you switch something on in Settings – and then it tells you exactly where.

## Status: v0.1 – read-only

v0.1 shows, it doesn't remove anything yet. It's safe to run on your main PC.

| Area | v0.1 |
| --- | --- |
| Apps | Installed apps from the Windows Uninstall registry keys (64-bit, 32-bit, per user), size on disk, location, uninstall command |
| Leftovers | Startup entries, services and scheduled tasks that point to files that no longer exist |
| Cleanup | Measures temp files, Windows Update cache, crash dumps, shader and thumbnail caches, browser caches, developer caches (npm, pnpm, Yarn, pip, NuGet, Cargo, Gradle) and app caches |
| System | Startup items (Run keys + Startup folders, with Task Manager's enabled/disabled state), services, scheduled tasks |
| Settings | Where UWiper stores its data, everything it writes outside its folder, safety and scan options, appearance |

Some areas need administrator rights (for example scheduled tasks). UWiper starts without them and offers **Restart as administrator**.

## Roadmap

- **v0.2** – clean uninstall: run the app's own uninstaller, find what it left behind with confidence levels, move it to quarantine (files plus `.reg` backups), one-click restore. Cache cleaning. Driver Store packages.
- **v0.3** – leftover scan for apps that were removed earlier, Store/MSIX apps, firewall rules, PATH entries.
- **v0.4** – install tracking: snapshot what an installer creates, for an exact uninstall later.

## Running it

Grab `UWiper-portable.zip` from the latest [Build](../../actions/workflows/build.yml) run, unzip it wherever you like and start `UWiper.exe`. It needs the WebView2 runtime, which Windows 10 and 11 already include.

Everything UWiper keeps ends up next to `UWiper.exe`:

```
UWiper\
  UWiper.exe
  settings.toml     your settings, plain text
  webview\          the UI engine's profile
  quarantine\       created in v0.2, location configurable
```

To remove UWiper, delete the folder (turn off *Start with Windows* first if you enabled it).

## Development

Requirements: Node.js 22+, Rust (stable), and on Windows the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev     # the real app (Windows)
npm run dev           # the UI alone in a browser, with sample data
```

`npm run dev` works on any OS and is handy for UI work – a banner in the sidebar marks the sample data. Debug builds keep their data in `src-tauri/target/dev-home`; set `UWIPER_HOME` to use another folder.

Checks that CI runs:

```sh
npm run typecheck
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

### Layout

```
src/                 React + TypeScript UI
  pages/             one file per sidebar section
  components/        shared UI (sidebar, switches, icons, logo)
  state/store.tsx    app-wide state and data loading
  lib/api.ts         typed wrappers around the Rust commands
src-tauri/src/       Rust backend
  apps.rs            installed apps (Uninstall registry keys)
  startup.rs         Run keys, Startup folders, StartupApproved state
  services.rs        services and their binaries
  tasks.rs           scheduled task XML under System32\Tasks
  cleanup.rs         cache and temp file locations
  paths.rs           where UWiper keeps its own data
  settings.rs        settings.toml
  system.rs          elevation, Mica, Explorer, "Start with Windows"
```

Stack: [Tauri 2](https://v2.tauri.app), Rust, React 19, Vite.
