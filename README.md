# Atlas

**Keep your friends on the same mods.** Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends. It works alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher) instead of replacing it.

One player shares an instance; their friends add it to their own launcher in a click, and get every change after that as an update they can review first. Atlas backs up before it changes anything and never touches worlds, settings or the files a friend added themselves.

> **Status:** in development. The desktop app finds your launchers, tracks their instances, scans and identifies their mods, and has a complete local sync engine (updates, backups, restore, crash recovery). Accounts, friends and sharing need the Atlas server, which isn't built yet; until then the social parts of the UI run on mock data. See [the build plan](docs/tauri-plan.md#20-build-order).

## What it does

- **Tracks instances** from Prism Launcher / MultiMC, the Modrinth App, CurseForge and the Minecraft Launcher, or any game folder you point it at. Atlas reads them; the launcher stays in charge of the game.
- **Knows what's inside:** every mod, resource pack, shader pack and data pack, with names and icons from the files themselves and from Modrinth. It notices when you change something in your launcher.
- **Shares with friends and groups** (coming with the server): each file can be shared as required, optional (friends choose), or kept to yourself.
- **Applies updates safely:** waits for the game to close, backs up what it will change, verifies every download, and puts the folder back exactly as it was if anything goes wrong, even after a crash.

Atlas doesn't launch Minecraft, download the game, or host mods that are on Modrinth or CurseForge: it links to them.

## Built with

[Tauri 2](https://tauri.app) (Rust) and [SvelteKit](https://svelte.dev) (Svelte 5, TypeScript) with Tailwind CSS 4. The local database is SQLite via [sqlx](https://github.com/launchbadge/sqlx).

## Running it

Needs [Node.js](https://nodejs.org) with [pnpm](https://pnpm.io), [Rust](https://rustup.rs), and the [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/) (WebView2 and the MSVC build tools).

```powershell
pnpm install
pnpm tauri dev
```

By default the UI runs on **mock data** (`src/lib/api/mock.ts`), so every screen works without a backend. To use the real Rust backend, set `VITE_ATLAS_API`:

```powershell
$env:VITE_ATLAS_API = "tauri"; pnpm tauri dev
```

or put `VITE_ATLAS_API=tauri` in `.env.local`. Mock scenarios (signed out, offline, empty, slow, failing) are listed in [docs/data-plan.md](docs/data-plan.md#mock-scenarios).

Atlas keeps its own data in `%APPDATA%\dev.keplerhaloxx.atlas` (database, download cache, backups, logs). Don't open `atlas.db` with other tools while Atlas is running.

## Tests

```powershell
cd src-tauri
cargo test                                # unit + end-to-end tests over the real IPC
cargo clippy --all-targets -- -D warnings
cd ..
pnpm check                                # TypeScript / Svelte
```

`src-tauri/tests/contract.rs` checks that every call the UI makes reaches a Rust command that accepts its arguments. A few opt-in tests read your real launchers (they write nothing):

```powershell
cargo test --lib real_launchers -- --ignored --nocapture
```

## Project layout

```
src/                  SvelteKit UI
  lib/types.ts        every shape the UI receives: the contract with the backend
  lib/api/            api.ts (all calls), mock.ts, tauri.ts
  tabs/, components/  screens and pieces
src-tauri/src/        Rust backend
  commands/           one Tauri command per API call
  launchers/          reading each launcher's instances
  scan/               file scanning, hashing, metadata, icons, folder watching
  resolve/            identifying files on Modrinth
  sync/               manifests, update planning, applying, backups, recovery
docs/                 the plans: start with data-plan.md
```

## Docs

- [docs/data-plan.md](docs/data-plan.md): what Atlas stores and why, the manifest format, how sync works.
- [docs/backend-plan.md](docs/backend-plan.md): the server (accounts, friends, sharing, storage).
- [docs/tauri-plan.md](docs/tauri-plan.md): the desktop backend, phase by phase.

## License

[GPL-3.0](LICENSE). You can use, study, change and share Atlas; copies and forks you distribute must stay open source under the same license.

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
