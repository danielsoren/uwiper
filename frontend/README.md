# Frontend

React 19 + TypeScript, styled with Tailwind CSS 4, state in zustand. This folder is the Vite root (`index.html` lives here); the build goes to `../dist`, where `backend/tauri.conf.json` picks it up.

## Folders

| Folder | What goes there | Rule of thumb |
| --- | --- | --- |
| `app/` | Entry point, app shell, `index.css` with the Tailwind theme | Only one of each |
| `pages/` | One component per sidebar section | Composes components; no reusable pieces here |
| `components/<area>/` | App-specific pieces: `layout/`, `apps/`, `settings/`, `cleanup/`… | Knows about the store, API or app data |
| `ui/` | Primitives, one per file: `Button`, `Switch`, `Segmented`, `Pill`, `ListRow`… | Knows nothing about UWiper; import from `ui/` (the barrel) |
| `store/` | `useAppStore` (zustand), `hooks.ts`, `selectors.ts` | State and data loading only |
| `lib/` | `api.ts` (Rust commands), `types.ts`, `format.ts`, `cn.ts`, sample data | No React components |

If a new piece is generic enough to live in another app, it's a primitive (`ui/`). Otherwise it goes to `components/`, in a subfolder named after the area that uses it.

## Styling

- Design tokens live in `app/index.css`: colors (`fg`, `fg-2…4`, `line`, `fill`, status colors), the type scale (`text-micro` 11px … `text-hero` 56px) and radii (`rounded-control`, `rounded-tile`, `rounded-card`).
- The accent color and the window material are runtime CSS variables (`--accent`, `--surface-*`), set from Settings by `useThemeSync`. Use them through `bg-accent`, `text-accent-soft`, `bg-sidebar`, `bg-main`.
- Combine classes with `cn()` from `lib/cn.ts`. It knows the custom scales, so `cn('text-micro', 'text-fg-3')` keeps both.
- Prefer theme tokens over arbitrary values. An arbitrary value (`text-[11.5px]`) is fine for a one-off that matches the macOS spacing.

## Data

Every Rust command is wrapped in `lib/api.ts`. Pages read data with `useResource('apps' | 'cleanup' | …)`, which loads it the first time it's needed and caches it for the session; `reload()` fetches it again.

Outside Tauri (`npm run dev` in a browser) the same calls return sample data from `lib/fixtures.ts`, and the sidebar says so.
