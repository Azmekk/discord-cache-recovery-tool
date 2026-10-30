// Shared by every download page: finds the latest GitHub release, works out
// which installer suits this visitor, and fills in the page.
//
// Markup it understands:
//   <a data-download="primary">      best installer for this visitor (href + text)
//   <span data-download-label>       e.g. "Download for Windows"
//   <span data-download-detail>      e.g. "v4.0.0 · 64-bit installer · 3.1 MB"
//   <span data-version>              e.g. "v4.0.0"
//   <a data-download="windows-x64">  one link per target (see TARGETS below)
//
// Pages can also call DiscordCacheRecovery.downloads (a promise) for the raw data.
// Until a release is published every link points at the Releases page.
(function () {
  const REPO = 'Azmekk/discord-cache-recovery-tool';
  const RELEASES = `https://github.com/${REPO}/releases/latest`;

  // id → how to recognise its file among the release assets.
  const TARGETS = [
    { id: 'windows-x64', os: 'windows', label: 'Windows', detail: '64-bit installer', match: /_x64-setup\.exe$/ },
    { id: 'windows-arm64', os: 'windows', label: 'Windows (ARM)', detail: 'ARM64 installer', match: /_arm64-setup\.exe$/ },
    { id: 'mac-arm64', os: 'mac', label: 'macOS (Apple Silicon)', detail: 'M1 and later', match: /_aarch64\.dmg$/ },
    { id: 'mac-x64', os: 'mac', label: 'macOS (Intel)', detail: 'Intel Macs', match: /_x64\.dmg$/ },
    { id: 'linux-appimage', os: 'linux', label: 'Linux AppImage', detail: 'Any distro, x64', match: /_amd64\.AppImage$/ },
    { id: 'linux-appimage-arm64', os: 'linux', label: 'Linux AppImage (ARM)', detail: 'Any distro, ARM64', match: /_aarch64\.AppImage$/ },
    { id: 'linux-deb', os: 'linux', label: 'Debian / Ubuntu', detail: '.deb, x64', match: /_amd64\.deb$/ },
    { id: 'linux-rpm', os: 'linux', label: 'Fedora / RHEL', detail: '.rpm, x64', match: /\.x86_64\.rpm$/ },
  ];

  const OS_NAMES = { windows: 'Windows', mac: 'macOS', linux: 'Linux' };

  async function detect() {
    const ua = navigator.userAgent;
    const os = /Windows/.test(ua) ? 'windows' : /Mac/.test(ua) ? 'mac' : /Linux|X11/.test(ua) && !/Android/.test(ua) ? 'linux' : null;
    let arm = /arm|aarch64/i.test(ua);
    try {
      // Chromium-based browsers can tell ARM from x64; others can't.
      const hints = await navigator.userAgentData?.getHighEntropyValues(['architecture']);
      if (hints?.architecture) arm = hints.architecture === 'arm';
    } catch {}
    // Safari never reports the CPU; nearly every Mac sold since 2020 is Apple Silicon.
    if (os === 'mac') arm = true;
    return { os, arm };
  }

  function pick(os, arm) {
    const id = {
      windows: arm ? 'windows-arm64' : 'windows-x64',
      mac: arm ? 'mac-arm64' : 'mac-x64',
      linux: arm ? 'linux-appimage-arm64' : 'linux-appimage',
    }[os];
    return TARGETS.find((t) => t.id === id);
  }

  function size(bytes) {
    return bytes ? `${(bytes / 1048576).toFixed(1)} MB` : '';
  }

  async function load() {
    const [{ os, arm }, release] = await Promise.all([
      detect(),
      fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: 'application/vnd.github+json' } })
        .then((r) => (r.ok ? r.json() : null))
        .catch(() => null),
    ]);

    const assets = release?.assets ?? [];
    const targets = TARGETS.map((t) => {
      const asset = assets.find((a) => t.match.test(a.name));
      return { ...t, url: asset?.browser_download_url ?? RELEASES, file: asset?.name ?? null, size: size(asset?.size) };
    });
    const best = os && pick(os, arm);
    const primary = best ? targets.find((t) => t.id === best.id) : null;

    return {
      version: release?.tag_name ?? null,
      releasesUrl: RELEASES,
      repoUrl: `https://github.com/${REPO}`,
      os,
      osName: os ? OS_NAMES[os] : null,
      primary,
      targets,
    };
  }

  function fill(d) {
    const label = d.primary ? `Download for ${d.osName}` : 'Download';
    const detail = [d.version, d.primary?.detail, d.primary?.size].filter(Boolean).join(' · ');
    document.querySelectorAll('[data-download="primary"]').forEach((a) => (a.href = d.primary?.url ?? d.releasesUrl));
    document.querySelectorAll('[data-download-label]').forEach((el) => (el.textContent = label));
    document.querySelectorAll('[data-download-detail]').forEach((el) => (el.textContent = detail || 'All platforms on GitHub'));
    document.querySelectorAll('[data-version]').forEach((el) => (el.textContent = d.version ?? ''));
    for (const t of d.targets) {
      document.querySelectorAll(`[data-download="${t.id}"]`).forEach((a) => (a.href = t.url));
    }
  }

  const downloads = load();
  window.DiscordCacheRecovery = { downloads, TARGETS, REPO };
  const run = () => downloads.then(fill);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run);
  else run();
})();
