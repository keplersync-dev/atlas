# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

**Status:** in development. Launcher detection, mod scanning, Discord sign-in, friends with live presence, and sharing instances (publish, accept, update) work. Uploading files that aren't on Modrinth or CurseForge, and groups, are still to come.

## Running

You'll need [Node.js](https://nodejs.org) with [pnpm](https://pnpm.io), [Rust](https://rustup.rs), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```powershell
pnpm install
pnpm tauri dev
```

The UI runs on mock data by default. To use the real backend:

```powershell
$env:VITE_ATLAS_API = "tauri"; pnpm tauri dev
```

Signing in needs an Atlas server. Point the app at one with `ATLAS_SERVER_URL`:

```powershell
$env:VITE_ATLAS_API = "tauri"; $env:ATLAS_SERVER_URL = "http://localhost:8080"; pnpm tauri dev
```

To try two accounts on one PC, start a second copy with its own data folder and sign-in while the first is running (development builds only). Run it from a copy of the exe, so it doesn't stop `pnpm tauri dev` from rebuilding:

```powershell
Copy-Item src-tauri\target\debug\atlas.exe src-tauri\target\debug\atlas-2.exe
$env:ATLAS_PROFILE = "2"; $env:ATLAS_SERVER_URL = "http://localhost:8080"; src-tauri\target\debug\atlas-2.exe
```

## Tests

```powershell
cd src-tauri
cargo test
```

`protocol/` holds the types and rules the app shares with the server (manifests, API bodies, friend codes). Test it with `cargo test` in that folder.

## License

[GPL-3.0](LICENSE)

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
