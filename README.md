# RABBIT — REAPER Accessibility Bootstrap & Bundle Installation Tool

RABBIT sets up a fully accessible REAPER on Windows and macOS in a few clicks.
Instead of hunting through download pages, copying files into the right
folders, and fighting installers that fight your screen reader, you launch one
small program and it does the work.

RABBIT installs and keeps up to date:

- **REAPER** — the DAW itself
- **OSARA** — the screen-reader extension that makes REAPER usable with
  NVDA, JAWS, Narrator, and VoiceOver
- **SWS** — the popular SWS Extension
- **ReaPack** — REAPER's package manager
- **ReaKontrol** — Native Instruments Komplete Kontrol support
- **JAWS-for-REAPER scripts** *(Windows only, when JAWS is detected)*
- **FFmpeg** *(Windows only, opt-in)* — the shared FFmpeg runtime
  (`avformat`, `avcodec`, …) that REAPER's video decoder loads from
  `UserPlugins`. Pulled from
  [Gyan.dev](https://www.gyan.dev/ffmpeg/builds/) on x64 and
  [tordona/ffmpeg-win-arm64](https://github.com/tordona/ffmpeg-win-arm64)
  on ARM64. Pinned to the latest stable FFmpeg major REAPER's video
  decoder is known to support (currently 8.x). Unticked by default so
  it doesn't surprise users who don't need video.
- **Surge XT** *(opt-in, standard REAPER installations only)* — the
  free open-source hybrid synthesizer from the
  [Surge Synth Team](https://surge-synthesizer.github.io/). RABBIT
  runs the vendor installer (Inno Setup on Windows,
  productbuild-wrapped `.pkg` on macOS) under elevation so the VST3,
  CLAP, AU (macOS only) and standalone formats land system-wide for
  REAPER and other DAWs to pick up. Tracks the
  [rolling nightly channel](https://github.com/surge-synthesizer/surge/releases/tag/Nightly)
  rather than the official 1.3.4 release (which is from August 2024
  and the project effectively ships through nightlies). Hidden on
  portable REAPER targets because Surge XT's factory data lives at
  fixed system paths outside any portable REAPER folder.

Beyond installing packages, RABBIT can also apply small post-install
configuration tweaks. Today there's one such step:

- **Add the REAPER Accessibility ReaPack repository to ReaPack**
  (`https://github.com/Timtam/reapack/raw/master/index.xml`). When
  ReaPack is part of your install plan or already on disk, this step is
  ticked by default; if the repository is already configured in your
  `reapack.ini`, the step shows as *already applied* and is skipped.
  Idempotent and safe to re-run.

Built with screen reader users in mind: keyboard-first wizard, native
controls, NVDA/JAWS/Narrator/VoiceOver tested, German + English UI out of the
box. No console window, no installer, no settings file — one executable you
can run from any folder and delete when you're done.

## Download

Pick the file that matches your machine. These links always point at the
latest release — bookmark or share them freely:

- **Windows (Intel/AMD 64-bit)**:
  [rabbit-windows-x86_64.exe](https://github.com/Timtam/rabbit/releases/latest/download/rabbit-windows-x86_64.exe)
- **Windows (ARM 64-bit)**:
  [rabbit-windows-aarch64.exe](https://github.com/Timtam/rabbit/releases/latest/download/rabbit-windows-aarch64.exe)
- **macOS (universal — Apple Silicon + Intel)** — recommended:
  [rabbit-macos-universal.app.zip](https://github.com/Timtam/rabbit/releases/latest/download/rabbit-macos-universal.app.zip)
- **macOS bare binary** (CLI use):
  [rabbit-macos-universal](https://github.com/Timtam/rabbit/releases/latest/download/rabbit-macos-universal)

To pin a specific version (or download SHA-256 sums for verification),
browse the [GitHub Releases
page](https://github.com/Timtam/rabbit/releases) — every release also
publishes versioned filenames (`rabbit-<version>-windows-x86_64.exe`,
etc.) alongside per-asset `.sha256` files.

On Windows, place the downloaded executable wherever you like (Desktop,
Downloads, a USB stick) and double-click it. You can rename it to `RABBIT.exe`
if you prefer — RABBIT still updates itself in place under whatever filename you
chose.

### macOS first launch

RABBIT for macOS is signed with an Apple Developer ID and notarized by Apple.
Unzip `rabbit-<version>-macos-universal.app.zip` and double-click `Rabbit.app`
— it launches normally on first run, with no quarantine workaround needed.
Self-updates keep working under the same bundle identity.

The bare `rabbit-<version>-macos-universal` download is a plain Mach-O CLI
executable (no `.app` wrapper). After downloading, run `chmod +x` and invoke
it from Terminal.

## Use it

Launch the downloaded executable. The wizard walks you through:

1. **Pick a REAPER target** — RABBIT detects existing standard installs
   automatically; pick "portable" if you want a self-contained REAPER folder.
2. **RABBIT checks for the latest versions** of REAPER and the accessibility
   packages.
3. **Pick the packages** you want installed or updated, plus any
   *configuration* steps (e.g. adding the REAPER Accessibility ReaPack
   repository). Sensible defaults are already checked.
4. **Review and install.** RABBIT downloads, verifies, and installs everything
   without further prompts.

When it finishes, you can launch REAPER straight from the wizard or open the
saved report.

### Switching the language

Use the language picker at the bottom of the window. Currently bundled
languages: English (United States) and Deutsch (Deutschland). RABBIT auto-picks
your OS language on first launch when a translation is available.

## Command-line usage

The same `RABBIT.exe` / `RABBIT` executable also exposes a CLI when invoked with
arguments. Run `RABBIT --help` for the full list. The most useful commands grouped
by what they do:

### See what you have

```
RABBIT detect                                  # list detected REAPER installs
RABBIT detect --portable C:\REAPER             # also probe a portable folder
RABBIT components --resource-path "%APPDATA%\REAPER"
RABBIT latest                                  # show latest upstream versions
```

### Plan an install or update

```
RABBIT plan --resource-path "%APPDATA%\REAPER"
RABBIT plan --resource-path "%APPDATA%\REAPER" --online
RABBIT preflight --resource-path "%APPDATA%\REAPER"
```

`plan` prints what RABBIT *would* do for the given REAPER target. Add
`--online` to compare detected versions against the live upstream feeds.

### Install and update

```
# One-shot setup of a portable REAPER + accessibility packages:
RABBIT setup --resource-path C:\REAPER --portable --apply

# Update or install one specific package:
RABBIT install-extension --package osara --resource-path "%APPDATA%\REAPER" --apply

# Install/update everything that needs it for an existing REAPER:
RABBIT apply-packages --resource-path "%APPDATA%\REAPER" --apply
```

The CLI is dry-run by default; pass `--apply` to actually make changes.
`--save-report` writes a JSON report next to the resource path so you have a
record of what was installed.

`setup` also accepts `--config-step <id>` (repeatable) and
`--skip-config-step <id>` for the post-install configuration tweaks.
With no flags, all recommended steps whose dependencies are satisfied
(and that aren't already applied) run automatically; pass an explicit
list to opt in to a specific subset, or `--skip-config-step` to opt
out of one. The only step today is
`reapack-add-reaper-accessibility-remote`. Run `RABBIT --help` for the
full set of flags.

### Maintain

```
RABBIT backups --resource-path "%APPDATA%\REAPER"          # list rollback sets
RABBIT restore-backup --resource-path "%APPDATA%\REAPER" \
     --backup-id unix-1234567890 --apply                  # roll back one set
```

### Update RABBIT itself

```
RABBIT self-update check                       # see if a new RABBIT is out
RABBIT self-update apply --restart             # update + relaunch
```

The GUI does this automatically on startup; the CLI commands are there for
unattended environments and CI.

## Reports and logs

Every installation produces a JSON report under `<resource>/RABBIT/logs/`.
Backups go to `<resource>/RABBIT/backups/<timestamp>/`. The download cache lives
in `%LOCALAPPDATA%\RABBIT\cache` (Windows) or `~/Library/Caches/RABBIT` (macOS) and
can be deleted safely at any time.

## Made with the help of AI

RABBIT was built with the help of AI coding assistants. Much of the code,
tests, and documentation in this repository was written collaboratively
with large language models, then reviewed and maintained by its human
author. We mention this openly so users and contributors know what went
into the project.

## Development

See [DESIGN.md](./DESIGN.md) for the full architecture and design rules. To
build from source you need a recent stable Rust toolchain. The wxDragon GUI
feature on Windows additionally needs the Visual Studio C++ build tools, an
LLVM `libclang.dll` discoverable through `LIBCLANG_PATH`, and Ninja on
`PATH`.

```
cargo fmt
cargo test --workspace
.\scripts\build-wxdragon-test.ps1            # Windows GUI smoke build
```

CI lives under `.github/workflows/`:

- `ci.yml` — formatting, tests, and release-mode artifacts on every push.
- `macos-smoke.yml` — daily live-upstream smoke against real REAPER + OSARA
  + SWS + ReaKontrol downloads.
- `release.yml` — builds tagged `v*` releases (Windows `.exe`, macOS bare
  binary, macOS `.app.zip`), publishes the GitHub Release with checksums and
  the self-update manifest. macOS bundles are Developer ID signed and
  notarized when the `MACOS_*` signing secrets are configured, falling back to
  ad-hoc signing + the `Open Me First.command` helper otherwise.
- `macos-signing-smoke.yml` — manual (workflow_dispatch) smoke test of the
  macOS signing + notarization + stapling path. Builds the universal bundle,
  signs/notarizes/staples it, verifies the result on the runner, and uploads
  the signed bundle without publishing. Run it to validate signing before
  tagging a release.

Issues, pull requests, and translation contributions welcome — RABBIT is for
the REAPER accessibility community first.
