# Changelog

## [0.3.0-beta.1] – 2026-10-22

Leftovers of apps that are long gone, and a lot more junk and caches. Still a beta – quarantine stays on by default.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v0.3.0-beta.1-portable.zip** – the same, in a folder.

### New

- 👻 **Entries without an owner.** The Leftovers page now scans for everything apps that are gone left behind:
  - folders in AppData, ProgramData and Program Files, and settings in the registry, that no installed app claims;
  - apps still listed as installed whose uninstaller and folder are gone;
  - shortcuts and Start menu folders pointing nowhere;
  - PATH and App Paths entries pointing nowhere;
  - startup entries, services and tasks whose programs are missing.

  Missing programs and shortcut targets are certain (High), empty folders and keys likely (Medium). Unclaimed folders and keys, PATH entries whose folder doesn't exist yet and apps whose uninstaller alone is gone are only possible (Low – never picked for you). Every find shows when it last changed.
- 🧹 **More junk and caches.**
  - Any Chromium or Firefox-based browser is found by its folder layout, not just a list of names.
  - App caches are found by folder name, Microsoft Store apps included.
  - Known caches for Telegram, Steam, Teams, Epic Games, Battle.net, Adobe media, Opera, Yandex Browser, Vivaldi, Go, Bun, Maven, Electron, JetBrains IDEs and Unreal Engine.
  - Also the Recycle Bin (off by default), Windows logs and the Windows internet cache.
- 📜 **MIT license.**

### Better

- Lists refresh as soon as anything is removed or restored, and data folders get an Open button.
- Settings is simpler: the «Outside the UWiper folder» section and the Start with Windows option are gone. UWiper keeps nothing outside its folder, and an old startup entry is removed.
- Cleanup and Quarantine rows expand with an animation.
- Notes line up with warnings; System's path column lines up across rows.
- App icons: MSI product icons, and the app's exe one folder down (Adobe Acrobat and others that showed a letter).

## [0.2.0-beta.1] – 2026-10-21

The first version that removes things. **It's a beta** – quarantine is on by default, so everything UWiper removes can be put back. Please keep it on while you try this version.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v0.2.0-beta.1-portable.zip** – the same, in a folder, if you prefer a zip.

Settings and data stay next to `UWiper.exe`. For a full clean, use **Restart as administrator** in the sidebar.

### New

- 🗑️ **Uninstall completely.** Runs the app's own uninstaller (MSI included), notices when it's done, then scans for leftovers: its folder, data folders and registry keys named after it, shortcuts, services, scheduled tasks and startup entries. Each find has a confidence level, and you choose what goes. A System Restore point can be created first.
- ♻️ **Quarantine.** Removed files are moved aside, registry keys and services exported to `.reg`, tasks saved as XML. Restore a whole batch in one click; batches are deleted for good after the number of days set in Settings.
- 🧹 **Cache cleaning.** Pick categories on the Cleanup page and clean them. Files in use are skipped.
- 👻 **Remove broken entries.** Startup entries, services and scheduled tasks that point to missing files can be moved to quarantine from the Leftovers page.
- 🛡️ **Safety rules.** Windows and Microsoft components, shared folders, other apps' folders, UWiper's own data and your «Never touch» list are never removed.
- 📝 **Log.** Every change is written to a daily log in UWiper's logs folder.

### Better

- Apps show their real icons and their real size right away – the larger of the measured folder and what the installer reported – instead of changing when selected.
- Loading states are skeletons shaped like the content, and opening Settings no longer stalls the window.
- New logo and app icon.
- Header actions sit on the right, Settings has the same width as other pages, and text is vertically centered on Windows (font metrics fix).
- Translucent window lets more of the Mica backdrop through.

### Not yet

- Driver Store packages, firewall rules and PATH entries move to v0.3.

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
- **Settings** – where UWiper keeps its data, everything it writes outside its folder (only «Start with Windows», off by default), safety and scan options, accent color, Windows- or macOS-style window buttons, Mica on Windows 11.
- Starts without administrator rights; **Restart as administrator** unlocks the parts that need them, like scheduled tasks.

### Coming in 0.2

Clean uninstall with a leftover scan, quarantine with one-click restore, cache cleaning and Driver Store packages.
