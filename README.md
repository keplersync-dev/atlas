# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

**Status:** in development. Launcher detection, mod scanning, Minecraft and Discord sign-in, friends with live presence, groups, friend and group links (`atlas://`), sharing instances (publish, accept, update, with uploads for files that aren't on Modrinth or CurseForge), CurseForge matching and automatic updates work.

## Running

You'll need [Node.js](https://nodejs.org) with [pnpm](https://pnpm.io), [Rust](https://rustup.rs), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```powershell
pnpm install
pnpm tauri dev
```

Signing in needs an Atlas server. Development builds use one at `http://localhost:8080` and release builds `https://keplersync.app`; point the app at another with `ATLAS_SERVER_URL`:

```powershell
$env:ATLAS_SERVER_URL = "https://atlas.example.com"; pnpm tauri dev
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
