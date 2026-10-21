# Changelog

## [0.1.0] – 2026-10-21

First preview. **Read-only: this version shows what's on your PC and never changes anything**, so it's safe to try on your main machine.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v0.1.0-portable.zip** – the same, in a folder, if you prefer a zip.

UWiper is portable: settings and data stay next to `UWiper.exe`. To remove it, delete that folder. It needs the WebView2 runtime, which Windows 10 and 11 already have.

The exe isn't code-signed yet, so Windows SmartScreen may warn on first launch: **More info → Run anyway**.

### What's in it

- **Apps** – installed programs from the Windows Uninstall registry keys (64-bit, 32-bit, per user): size on disk, location, registry entry, uninstall command.
- **Leftovers** – startup entries, services and scheduled tasks that point to files that no longer exist.
- **Cleanup** – measures temp files, Windows Update cache, crash dumps, shader and thumbnail caches, browser caches, developer caches (npm, pnpm, Yarn, pip, NuGet, Cargo, Gradle) and app caches.
- **System** – startup items with Task Manager's enabled/disabled state, services, scheduled tasks.
- **Settings** – where UWiper keeps its data, everything it writes outside its folder (only "Start with Windows", off by default), safety and scan options, accent color, Windows- or macOS-style window buttons, Mica on Windows 11.
- Starts without administrator rights; **Restart as administrator** unlocks the parts that need them, like scheduled tasks.

### Coming in 0.2

Clean uninstall with a leftover scan, quarantine with one-click restore, cache cleaning and Driver Store packages.
