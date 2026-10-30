# Discord Cache Recovery

Find and recover images, GIFs, videos and emojis that are still sitting in your Discord cache,
even after the original message, link or GIF was deleted.

## What it does

Discord keeps a local copy of the media you've seen. This app reads that cache and shows
everything in a searchable gallery:

- **Real file names and links.** Attachments show their original file name, and every file
  shows the link it was downloaded from.
- **Dates.** When an attachment was posted (decoded from Discord's IDs) and when your Discord
  downloaded it.
- **Sorted by kind.** Attachments, GIFs, emojis, stickers, avatars, server icons and embeds.
- **Search and filter** by name, link, channel ID, type and date. Name channels yourself to
  find things by where they were sent.
- **Export** a single file or a whole selection to any folder. Exported files keep the
  original name and their date.
- Supports Discord, PTB, Canary and Development on Windows, macOS and Linux (including
  Flatpak and Snap). You can also open a cache folder you copied somewhere else.

## Is it safe? Can it get my account banned?

The app works **entirely offline**. It never logs in, never uses your account or token,
never talks to Discord's servers and never modifies Discord. It only reads files Discord
already stored on your computer, the same way a file browser would.

## Download

Get the installer for your system from the
[latest release](https://github.com/Azmekk/discord-cache-recovery-tool/releases/latest):

| System | File |
|---|---|
| Windows | `.msi` or `-setup.exe` |
| macOS (Intel and Apple Silicon) | `.dmg` |
| Linux | `.AppImage`, `.deb` or `.rpm` |

The macOS build isn't signed with an Apple developer certificate. If macOS says the app is
damaged, run `xattr -cr "/Applications/Discord Cache Recovery.app"` once.

## FAQ

### I can't find the image or GIF I'm looking for.
The app can only recover what is still in the cache. Discord removes old entries as the cache
fills up, and clearing the cache deletes everything. Try turning off **Group sizes**, and check
**Show Discord's own interface images** in case it's filed there.

### Does it know which server or favorite GIF tab something came from?
Not directly. That information isn't stored with the cached files. Attachments include the
channel ID, so you can label channels yourself (right-click a channel in Discord with
Developer Mode on, then Copy Channel ID).

## Development

Requires Node 22+, Rust (stable) and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev
```

`npm run dev` runs the UI alone in a browser with generated sample data. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the cache is parsed.

### Releasing

Push a version tag; GitHub Actions builds every platform and publishes the release:

```bash
./release.sh v4.0.0
```

## Collaborators

[@ggatesdew](https://github.com/ggatesdew) - Testing on macOS

## License

MIT, see [LICENSE](LICENSE).
