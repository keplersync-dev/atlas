# Atlas

Keep your friends on the same mods. Atlas is a Windows companion app for Minecraft: Java Edition that keeps a modpack in sync between friends, alongside the launcher you already use (Prism Launcher, Modrinth App, CurseForge, the Minecraft Launcher).

Atlas isn't a launcher. Your launcher still installs and runs the game; Atlas keeps the mods and configs in your instances the same as your friends'.

## How it works

1. Atlas finds the instances your launchers already have.
2. Share one with friends or a group.
3. They accept the invite and pick a launcher. It installs the game, and Atlas fills in the mods and configs.
4. When you change the pack and publish, everyone gets the update.

## Features

- **Required, optional or only me.** Choose per file what friends get. Optional mods are theirs to take or leave.
- **Safe updates.** Atlas waits for the game to close, backs up what it replaces, and never touches your worlds or your own extras.
- **Undo.** Roll an instance back to any of its last three backups.
- **Mods from Modrinth and CurseForge**, downloaded from the source. Files on neither can be uploaded, with your permission.
- **Friends and groups.** Add friends with a friend code or link, and see who's online.
- **Works signed out.** Sign in with Minecraft or Discord only for sharing.

## Download

Download from the [releases page](../../releases/latest), or from [KeplerSync](https://keplersync.app). Atlas updates itself.

## Building from source

You need [Node.js](https://nodejs.org), [pnpm](https://pnpm.io) and [Rust](https://rustup.rs).

```powershell
pnpm install
pnpm tauri dev     # run with hot reload
pnpm tauri build   # build an installer
```

## Show your support

Please consider supporting the project by starring the repository, or by donating via Ko-fi!

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O4O113VGTE)

## Support

If you need help or have any questions, open a [GitHub issue](../../issues) or email <support@keplersync.app>.

## License

[GPL-3.0](LICENSE)

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
