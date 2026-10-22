# Changelog

## [1.0.0] – 2026-10-22

The first full release. An independent safety audit of 0.5 went through everything UWiper offers to remove; every finding is fixed here.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v1.0.0-portable.zip** – the same, in a folder.

### Safer cleanup

- Steam, Epic Games and Battle.net: only the cache folders of their built-in browsers are cleaned. Sign-ins, cookies and history stay.
- Cookies, history, site storage, sign-ins and browser settings are never cleaned, whatever category a folder comes from.
- Nothing is cleaned through a link or junction, at the folder itself or any folder above it.
- Crash reports and Windows Update downloads are off by default. Temp keeps files changed in the last day.
- The report tells files in use, files Windows refused and other failures apart.

### More accurate leftovers

- A folder on the Desktop is never a leftover, whatever its name – only shortcuts there are. A Start menu folder is offered whole only when it holds nothing but shortcuts.
- Folders with saved games are always Low.
- Services, Microsoft Store apps and running programs count as owners. Shared registration keys such as ASIO are never offered.
- Low finds now say UWiper couldn't match them, not that nothing uses them.
- A scheduled task counts as broken only when every one of its actions runs a program that's gone.
- Only High finds are picked to start with. Settings that still had the old High & Medium default move to High only.

### Removing and restoring

- Every find is checked again right before it's removed. Anything that changed since the scan is skipped with a note to scan again.
- Quarantine backs an item up and records it on disk before changing anything. If the record can't be written, nothing more is removed. The next start finishes whatever a crash interrupted.
- Restore leaves things alone that are there again (a reinstalled app's settings, service or task) and keeps the quarantined copy.

### Drivers and install tracking

- Driver Store locations come from Windows' own SetupAPI, so every package is found and paired with its other versions. Extension packages and packages UWiper can't locate show Unknown and are never removed.
- Install tracking marks something as the installer's only when something ties it to the install. An app's data folder that changed after the install is Medium at uninstall time.

### Fixed

- The step numbers in Installs sit in the middle of their circles.
- The middle window button turns into Restore when the window is maximized.

## [0.5.0-beta.1] – 2026-10-22

Install tracking: UWiper notes what an installer adds, so uninstalling the app later removes all of it.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v0.5.0-beta.1-portable.zip** – the same, in a folder.

### New

- 📦 **Track an install.** On the new Installs page:
  1. UWiper takes a snapshot.
  2. You run the installer – pick it in UWiper or start it yourself.
  3. You click Done, and UWiper shows what's new: folders, files, shortcuts, registry keys, its entry in installed apps, services, scheduled tasks, startup entries, PATH entries and driver packages.
  4. You untick anything that isn't the app's and save the trace.

  Tracking survives closing UWiper or restarting Windows. It also looks at new folders at the root of the system drive (`C:\xampp`), new dot-folders in your profile and new environment variables – those are never ticked for you.
- 🎯 **Accurate by design.**
  - UWiper records what an installer adds at the top two levels of each place, never anything deeper inside a folder that was already there – other apps running at the same time write there too.
  - Something only counts as new when it was also made after tracking started.
  - What appeared inside an existing folder, loose new files, drivers, and services or tasks that don't run anything the installer added are left unticked.
  - Runtimes installed along with the app (Visual C++ and the like) are kept out of its trace, since other apps may need them. You can pick which app the install was for.
  - A trace remembers what was in each folder it made. If other apps add to that folder later, only the app's own part is offered for removal.
  - An app that updated itself under a new name or id is still recognized as the same app.
- 🔒 **Private.**
  - The snapshot on disk holds only salted hashes of names.
  - Traces hold names of files, folders, registry keys and services – never what's in them.
  - Both stay in UWiper's folder.
- 🧹 **Uninstalling a tracked app** starts its leftover scan from the trace, and **Remove what's left** cleans up after an app that's already gone. Both go through quarantine as always.
- Apps marks tracked apps and shows when the install was tracked.

## [0.4.0-beta.1] – 2026-10-22

Startup entries, services and scheduled tasks can now be switched, and old driver packages removed.

### Download

- **UWiper.exe** – the app itself. Put it in any folder and run it.
- **UWiper-v0.4.0-beta.1-portable.zip** – the same, in a folder.

### New

- ⚡ **Switch what starts with Windows.** On the System page:
  - startup entries turn on and off with the same switch Task Manager uses – the entry itself stays;
  - services can be set to Automatic, Automatic (delayed), Manual or Disabled;
  - scheduled tasks turn on and off.

  Windows' own entries, services and tasks stay read-only – also the ones outside the Windows folder, like Defender – and every change is written to the log.
- 🧩 **Old driver packages.** A new Drivers tab lists the third-party packages in the Driver Store with their version, date, size and the device they drive. Every driver update leaves the previous version behind, and Windows never removes it. UWiper picks the older versions nothing uses while a device runs the newest one. They go to quarantine as an export and come back into the Driver Store with Restore. Packages a device, a service or a printer still uses are never removed, and printer drivers are never picked for you.

### Better

- Overview's cache rows open Cleanup at that group, and «Start with Windows» opens the Startup tab.
- Button text sits exactly in the middle, warnings line up with their button, and the macOS window buttons light up one at a time, only under the pointer.
- Messages from Windows tools come through readable on non-English Windows.

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
