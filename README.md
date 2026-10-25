# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

**Status:** in development. Launcher detection, mod scanning, Minecraft and Discord sign-in, friends with live presence, groups, friend and group links (`atlas://`), sharing instances (publish, accept, update, with uploads for files that aren't on Modrinth or CurseForge), CurseForge matching and automatic updates work.

## Running

You'll need [Node.js](https://nodejs.org) with [pnpm](https://pnpm.io), [Rust](https://rustup.rs), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```powershell
pnpm install
pnpm tauri dev
```

Signing in needs an Atlas server. Development builds use one at `http://localhost:8080`; point the app at another with `ATLAS_SERVER_URL`:

```powershell
$env:ATLAS_SERVER_URL = "https://atlas.example.com"; pnpm tauri dev
```

To try two accounts on one PC, run a second copy with its own data folder and sign-in next to `pnpm tauri dev` (development builds only). It restarts whenever `pnpm tauri dev` rebuilds the app, so both stay on the same code:

```powershell
pnpm dev:profile
```

`pnpm dev:profile 3` starts a third, and so on.

## Tests

```powershell
cd src-tauri
cargo test
```

`protocol/` holds the types and rules the app shares with the server (manifests, API bodies, friend codes). Test it with `cargo test` in that folder.

## Releasing

Releases are built by GitHub Actions ([release.yml](.github/workflows/release.yml)) and installed apps update themselves from the latest one.

Once, in the repository's settings on GitHub:

- **Secrets:** `TAURI_SIGNING_PRIVATE_KEY` (the contents of the private key from `pnpm tauri signer generate`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- **Variables:** `ATLAS_SERVER_URL`, the server release builds connect to.

The matching public key goes in `plugins.updater.pubkey` in [tauri.conf.json](src-tauri/tauri.conf.json). Keep the private key backed up: without it, installed copies can't be updated.

For each release:

1. Set `version` in [src-tauri/Cargo.toml](src-tauri/Cargo.toml) and commit.
2. Tag it and push the tag:

   ```powershell
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. When the workflow finishes, a draft release with the installer is waiting on GitHub. Write what changed, then publish it. Installed copies offer the update within a few hours, or the next time they start.

When a server change needs a newer app, release the app first, then set `MIN_CLIENT_VERSION` on the server to that version. Older copies are then asked to update.

## License

[GPL-3.0](LICENSE)

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
