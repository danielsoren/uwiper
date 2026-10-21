# UWiper

**Ultimate Wiper** – a Windows app for removing programs completely and seeing everything they leave behind: files, registry keys, services, scheduled tasks, startup entries and caches.

UWiper follows its own rule: it is portable, keeps everything in the folder you put it in and never writes to AppData, ProgramData or the registry unless you switch something on in Settings – and then it tells you exactly where.

## Download

Get **UWiper.exe** from the [latest release](https://github.com/danielsoren/uwiper/releases/latest), put it in any folder and run it. That folder is where UWiper keeps its settings and data; to remove UWiper, delete the folder.

It needs the WebView2 runtime, which Windows 10 and 11 already include. The exe isn't code-signed yet, so SmartScreen may warn on first launch: **More info → Run anyway**.

## Status: v0.1 – read-only

v0.1 shows, it doesn't remove anything yet. It's safe to run on your main PC. See [CHANGELOG.md](CHANGELOG.md).

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

## Project layout

```
frontend/            React + TypeScript UI (see frontend/README.md)
  app/               entry point, app shell, Tailwind theme
  pages/             one file per sidebar section
  components/        app-specific pieces, grouped by area
  ui/                primitives: Button, Switch, Segmented, Pill, …
  store/             zustand store, hooks and selectors
  lib/               API wrappers, types, formatting, sample data
backend/             Rust + Tauri
  src/apps.rs        installed apps (Uninstall registry keys)
  src/startup.rs     Run keys, Startup folders, StartupApproved state
  src/services.rs    services and their binaries
  src/tasks.rs       scheduled task XML under System32\Tasks
  src/cleanup.rs     cache and temp file locations
  src/paths.rs       where UWiper keeps its own data
  src/settings.rs    settings.toml
  src/system.rs      elevation, Mica, Explorer, "Start with Windows"
  tauri.conf.json    app and window configuration
assets/              logo source (icons are generated from it)
```

Stack: [Tauri 2](https://v2.tauri.app), Rust, React 19, Vite, Tailwind CSS 4, zustand.

## Development

Requirements: Node.js 22+, Rust (the version in `rust-toolchain.toml`), and on Windows the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev     # the real app (Windows)
npm run dev           # the UI alone in a browser, with sample data
```

`npm run dev` works on any OS and is handy for UI work – a banner in the sidebar marks the sample data. Debug builds keep their data in `backend/target/dev-home`; set `UWIPER_HOME` to use another folder.

Checks that CI runs:

```sh
npm run typecheck
cd backend && cargo clippy --all-targets -- -D warnings && cargo test
```

To regenerate the app icons after changing the logo: `npx tauri icon assets/logo.svg -o backend/icons`, then keep only `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` and `icon.ico`.

### Releasing

1. Add a section for the version to `CHANGELOG.md` and bump the version in `package.json`, `backend/Cargo.toml` and `backend/tauri.conf.json`.
2. Tag it: `git tag v0.2.0 && git push origin v0.2.0` – or run the **Build** workflow on `main` from the Actions tab with **release** ticked, and it tags the commit itself.

CI builds on Windows and publishes a GitHub release with `UWiper.exe` and a portable zip, using the CHANGELOG section as the notes.
