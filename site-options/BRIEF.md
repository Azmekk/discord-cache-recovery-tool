# Download site brief (shared by all five design options)

## Product
**Discord Cache Recovery**: a free, open-source (MIT) desktop app for Windows, macOS and Linux
that finds images, GIFs, videos and emojis still sitting in Discord's local cache, even after the
original message, link or favorite GIF was deleted, and lets you search, preview and export them.

**Audience:** ordinary Discord users who lost something: a favorite GIF whose link now 404s, a
photo a friend deleted, an emoji from a server they left. Not developers. Many are nervous about
running an unknown .exe and about getting their Discord account banned.

**The page's job:** get them to download it, and reassure them it's safe. Secondary: show it's
good-looking and capable (screenshot), answer "will it find my thing?" honestly.

**Owner's other sites** (match the quality bar: modern, animated hero, polished, dark is fine):
https://storagemap.vexbyte.com/ and https://meetinghelper.vexbyte.com/ (you can look at them for
quality level; don't copy their look).

## Facts and copy material (accurate: don't invent features)
- Finds media in Discord's cache: attachments, GIFs (Tenor, Giphy), emojis, stickers, avatars,
  server icons and banners, link-preview embeds.
- Real file names and links: attachments show their original file name; every file shows the
  link it was downloaded from. Embeds show the original source link.
- Dates: when an attachment was posted (decoded from Discord's IDs) and when your Discord
  downloaded it.
- Search by name, link or channel ID; filter by kind (attachments, GIFs, emojis…), format (images,
  GIFs, videos, audio), date and Discord install. You can name channels yourself to search by them.
- "Group sizes": Discord caches the same image at several sizes; the app keeps the largest.
- Export one file or a whole selection to any folder; exported files keep their original name
  and date.
- Works with Discord, PTB, Canary and Development; Windows, macOS, Linux (including Flatpak and
  Snap). You can also open a cache folder copied from elsewhere.
- Fast: scans thousands of cached files in seconds without reading whole files.
- **Safe by design:** works entirely offline. Never logs in, never uses your account or token,
  never talks to Discord's servers, never modifies Discord. It only reads files Discord already
  stored on your computer. Open source on GitHub.
- On Windows, quitting Discord fully (tray icon → Quit Discord) before scanning can be needed if
  the app says the cache is locked.
- Honest limits: it can only recover what's still in the cache. Discord removes old entries as
  the cache fills up, and clearing the cache deletes everything. It can't tell which server or
  favorite-GIF tab something came from (attachments do include the channel ID).
- Builds aren't code-signed: Windows SmartScreen → **More info → Run anyway**; macOS "damaged"
  message → run `xattr -cr "/Applications/Discord Cache Recovery.app"` once.
- MIT licensed. Made by Azmek (GitHub: Azmekk). macOS testing by ggatesdew.
- Repo: https://github.com/Azmekk/discord-cache-recovery-tool

Write copy in plain, friendly, specific language from the user's point of view (sentence case,
active voice, no hype words like "revolutionary", no filler). Good headline directions: "Get back
the GIFs Discord forgot", "Your lost Discord media is probably still on your computer". Pick your
own; make it fit your concept.

## Not Discord-branded
Don't use Discord's logo, wordmark, Clyde, or "blurple" (#5865F2) as the brand color, and don't
imply this is a Discord product. Footer must say: "Not affiliated with or endorsed by Discord Inc."

## Required content, in a sensible order
1. Top bar: app icon + name, a couple of anchor links, GitHub link.
2. **Hero** with your concept's animated centerpiece, headline, one-line subhead, primary download
   button, detail line, and a secondary "View on GitHub" link.
3. Show the app: `screenshot.png` (dark, 1320×820) and optionally `screenshot-light.png`. Frame it
   well; it's real UI with generated sample images.
4. What it recovers / features (from the facts above).
5. Safe by design (prominent: this is the #1 worry).
6. How it works, short and honest (it reads Discord's local HTTP cache, pairs each file with the
   link and date stored in the cache index, sorts by kind).
7. Downloads: every platform target as a link, plus the unsigned-build notes.
8. FAQ (4–6 questions: banned?, is it a virus?, why can't I find my GIF?, which server did it come
   from?, does it need Discord closed?, Mac "damaged" message).
9. Footer: MIT license, GitHub, credits, not-affiliated line.

## Technical requirements
- One self-contained file: `site-options/<your-slug>/index.html` with inline CSS and JS. No build
  step, no frameworks. Google Fonts are fine. Avoid other third-party scripts (if you truly need
  one, load it from cdnjs with an exact version).
- Paths (relative to your file): `../../site/downloads.js` (load with `defer`),
  `../../site/icon.svg` (also use as favicon), `../../site/screenshot.png`,
  `../../site/screenshot-light.png`. Use exactly these relative paths.
- Download wiring via `downloads.js` data attributes (read `site/downloads.js` to see them):
  `<a data-download="primary">` with `<span data-download-label>` and `<span data-download-detail>`
  inside or next to it; `<span data-version>`; and one link per target id: `windows-x64`,
  `windows-arm64`, `mac-arm64`, `mac-x64`, `linux-appimage`, `linux-appimage-arm64`, `linux-deb`,
  `linux-rpm`. Give every link a sensible default `href` (the releases page) for no-JS visitors.
- Quality floor: responsive down to 360px with no horizontal scroll; visible keyboard focus;
  `prefers-reduced-motion` respected (animations stop or become static); good contrast; semantic
  HTML; `<title>` and meta description; `<meta name="viewport">`.
- Performance: animations must be cheap. Pause canvas/RAF loops when offscreen
  (IntersectionObserver) or when the tab is hidden. No huge DOM.
- Animation must be **cache-themed** in a way specific to your concept (below), not generic
  particles or gradient blobs.

## Preview and verification
A static server for the repo root is configured in `.claude/launch.json` as **"site"** (port
8788). Start it with the browser tool `preview_start` (name "site"); it's shared with the other
design agents, so if it's already running that's fine; don't stop it when you're done. Your page is at
`http://localhost:8788/site-options/<your-slug>/`. Screenshot desktop (≈1440 wide) and mobile
(375 wide, `resize_window`), scroll through every section, check the console for errors, and fix
what looks off. Critique it like a design lead and remove one accessory. Reset the viewport
(`resize_window` preset "desktop") when done.

## Scope
Only create/edit files inside `site-options/<your-slug>/`. Don't touch anything else (no git
commits, no branch switching, no edits to `site/`, `README.md`, the app, or other options).

## Report back (concise)
Concept in one sentence, palette + fonts, what the hero animation does, anything you're unsure
about, and what you verified visually (desktop + mobile).
