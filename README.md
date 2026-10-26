# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

## Running

You'll need [Node.js](https://nodejs.org) with [pnpm](https://pnpm.io), [Rust](https://rustup.rs), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```powershell
pnpm install
pnpm tauri dev
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
