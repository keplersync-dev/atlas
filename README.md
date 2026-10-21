# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

**Status:** in development. Launcher detection, mod scanning and the local sync engine work; accounts, friends and sharing are still to come.

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

## Tests

```powershell
cd src-tauri
cargo test
```

## License

[GPL-3.0](LICENSE)

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
