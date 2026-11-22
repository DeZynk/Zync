# Zync

Zync is a lightweight Minecraft launcher for Windows built with Rust (Tauri v2) and React. It compiles into a single portable executable (`zync.exe`) while keeping user configuration separate in `zync-config.json`.

## Features

- **Portable single binary**: All frontend assets and backend logic are packed into a single executable (~5 MB).
- **External configuration**: Reads and writes `zync-config.json` next to the executable on startup.
- **Instance management**: Isolated game directories per instance with shared assets and libraries to save disk space.
- **Mod loaders**: Supports Vanilla, Fabric, Forge, NeoForge, and Quilt.
- **Download center**: Install game versions, Eclipse Adoptium Java runtimes (Java 8, 17, 21), mods, resource packs, modpacks, and shaders from Modrinth and CurseForge.
- **Smart Java matching**: Automatically selects the required Java version based on the target Minecraft version, and launches via `javaw.exe` for proper discrete GPU detection.
- **Authentication**: Supports Microsoft account sign-in (device code flow) and offline profiles.
- **Internationalization**: Built with `i18next` (English included by default, extensible via locale JSON files).

## Building from Source

### Requirements

- Rust toolchain (`rustup`, MSVC target)
- Node.js 20+
- Visual Studio C++ Build Tools (Windows)

### Development

```bash
npm install
npm run tauri dev
```

### Release Build

To build the standalone portable `.exe` without an installer bundle:

```bash
npx tauri build --no-bundle
```

The output binary will be located at `src-tauri/target/release/zync.exe`.

## License

MIT License. See [LICENSE](LICENSE) for details.
