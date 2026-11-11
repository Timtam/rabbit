# RABBIT Design

Status: revised 2026-05-07

RABBIT means "REAPER Accessibility Bootstrap & Bundle Installation Tool". Its
job is to install and update REAPER, OSARA, SWS, ReaPack, ReaKontrol, FFmpeg
(Windows only, opt-in), JAWS-for-REAPER scripts (Windows only, when JAWS is
detected), apply small post-install configuration tweaks (e.g. adding the
REAPER Accessibility ReaPack repository to ReaPack), and later additional
packages, while keeping the workflow usable with screen readers on Windows
and macOS.

The audience is REAPER users — including users who are not Rust developers,
accessibility experts, or installer engineers. RABBIT must therefore choose
sensible defaults, hide implementation detail, and avoid asking the user
questions they cannot reasonably answer.

## Product Goals

- One executable. RABBIT ships as a single self-contained binary per platform.
  Run with no arguments → graphical wizard; run with any argument or `--help` →
  CLI. There is no separate `rabbit-cli` binary, no helper executable, no
  installer, no companion resource folder.
- On Windows the executable does not pop up a console window when launched as
  a GUI; the same binary still attaches to the parent console when run from a
  command prompt with arguments.
- Release builds are optimized for file size first, then runtime speed
  (`opt-level = "z"`, fat LTO, single codegen unit, stripped symbols, panic
  abort) so a download stays small for users on metered or slow connections.
- The wizard is short, opinionated, and free of jargon. RABBIT picks the
  defaults a non-technical user would otherwise have to research, and presents
  results in plain terms ("REAPER 7.69 installed", "OSARA up to date") rather
  than internal mechanics ("detector: rabbit-receipt", "confidence: High",
  "PlannedAutomationKind::VendorInstaller").
- Install into an existing standard REAPER installation, into an existing
  portable REAPER installation, or set up REAPER plus the selected packages
  from scratch.
- Fully automate installation and update of REAPER, OSARA, SWS, ReaPack,
  ReaKontrol, the Windows-only JAWS-for-REAPER scripts, and (Windows-only,
  opt-in) FFmpeg's shared runtime libraries for REAPER's video decoder,
  without asking the user to run vendor installers or copy files manually
  in the normal flow.
- Apply small post-install **configuration steps** alongside the package
  install — e.g. adding RABBIT's REAPER Accessibility ReaPack repository to
  ReaPack's `reapack.ini`. Steps surface in the same wizard tree under a
  separate "Configuration" group, run after the package install pipeline
  finishes, and are idempotent (re-running shows them as "already applied"
  and skips them).
- Update REAPER and selected packages when newer versions are available.
- Detect installed versions where technically possible and clearly report
  "installed, version unknown" where it is not reliable.
- Prefer user-level installation paths for extensions so admin rights are not
  needed unless installing REAPER itself into a protected location.
- Preserve user configuration by default where possible. When OSARA is
  selected, RABBIT replaces the active key map with the OSARA key map, after
  backing up `reaper-kb.ini`, only when that loses nothing of the user's:
  OSARA is new, or `reaper-kb.ini` still matches the installed OSARA key map
  (`KeyMaps/OSARA.ReaperKeyMap`, compared before the update overwrites it).
  Otherwise the key map stays and OSARA's "Merge OSARA key map" action brings
  the new bindings in. The Packages step shows an explicit checkbox so the
  user sees and can override the choice; the CLI does the same with
  `--replace-osara-keymap` and `--preserve-osara-keymap`.
- Make every user-visible string localizable from the beginning. Embedded
  locales today: en-US, de-DE, es-ES, fr-FR, it-IT and pt-BR.
- Build Windows and macOS artifacts automatically for every push in GitHub
  Actions so every commit can be tested from real binaries. Artifacts are the
  raw single-file binaries; no zipping.
- Publish signed release artifacts through a GitHub release pipeline so tagged
  versions become downloadable binaries with checksums and update metadata.
- Let RABBIT detect when a newer RABBIT version has been released and update
  itself with as little user interaction as practical while preserving
  accessibility and platform trust requirements.

## Source-Backed Facts

- REAPER's own download page exposes the current version and platform-specific
  downloads. On 2026-04-23 it listed REAPER 7.69, released 2026-04-12.
  Source: <https://www.reaper.fm/download.php>
- REAPER's user guide says the resource path is shown from "Options > Show
  REAPER resource path in explorer/finder" and that Windows settings are not
  stored in the Windows Registry. Source:
  <https://dlz.reaper.fm/userguide/ReaperUserGuide728c.pdf>
- REAPER portable installs are based on a folder containing `reaper.ini`.
  On Windows the installer has a "Portable install" option; on macOS the guide
  describes creating a folder with `reaper.ini` and copying the app there.
  Source: <https://www.reaper.fm/userguide/ReaperUserGuide681c.pdf>
- OSARA installs into the REAPER user configuration/resource path, not the
  REAPER program directory. Its Windows installer writes files into
  `UserPlugins`, `KeyMaps`, and `osara/locale`; the standard Windows installer
  also writes an uninstall registry `DisplayVersion`. Source:
  <https://raw.githubusercontent.com/jcsteh/osara/master/installer/osara.nsi>
- OSARA's macOS installer uses
  `~/Library/Application Support/REAPER` for a standard installation and asks
  for the folder containing portable REAPER for portable installs. Source:
  <https://raw.githubusercontent.com/jcsteh/osara/master/installer/mac/Install%20OSARA.js>
- OSARA's update code embeds `OSARA_VERSION` into the extension binary and
  compares it with the snapshot update JSON. Source:
  <https://raw.githubusercontent.com/jcsteh/osara/master/src/updateCheck.cpp>
- SWS 2.14.0.7 is the latest stable version shown on the SWS site at the time
  of this design. The SWS changelog says SWS can be installed either by the
  traditional installer or via ReaPack 1.2.4.4 or newer. Sources:
  <https://sws-extension.org/> and <https://sws-extension.org/whatsnew.php>
- SWS embeds Windows version resources using `SWS_VERSION` and
  `SWS_VERSION_STR`. Source:
  <https://raw.githubusercontent.com/reaper-oss/sws/master/version.rc2.in>
- ReaPack's user guide says to install by placing the downloaded platform file
  into REAPER's `UserPlugins` directory and restarting REAPER. Source:
  <https://reapack.com/user-guide>
- ReaPack 1.2.6 is the latest stable release shown by ReaPack at the time of
  this design. Source: <https://reapack.com/>
- ReaPack builds its version from `Extensions/ReaPack.ext`, embeds Windows file
  version resources, and registers installed packages in
  `ReaPack/registry.db`. Sources:
  <https://raw.githubusercontent.com/cfillion/reapack/master/Extensions/ReaPack.ext>,
  <https://raw.githubusercontent.com/cfillion/reapack/master/src/buildinfo.rc>,
  and <https://raw.githubusercontent.com/cfillion/reapack/master/src/registry.cpp>
- ReaKontrol is a REAPER extension for Native Instruments Komplete Kontrol
  keyboards. Its site says the current version is 2026.2.16.100, it requires
  REAPER 6.37 or later, and there is no installer: on Windows and macOS the
  user installs it by copying the downloaded file into REAPER's `UserPlugins`
  folder. The site also documents `reaKontrol/fxMaps` under the REAPER resource
  path for additional mapping files. Sources:
  <https://reakontrol.jantrid.net/> and
  <https://github.com/jcsteh/reaKontrol>
- wxWidgets can use native controls on Windows and has MSAA accessibility
  support through `wxAccessible`, but that documented custom accessibility class
  is Windows/MSAA-specific. Sources:
  <https://wxwidgets.org/docs/faq/windows/> and
  <https://wxd.sourceforge.net/wxWidgets-2.6/docs/html/wx/wx_wxaccessible.html>
- wxDragon is a Rust wxWidgets toolkit/wrapper. Its docs describe native
  Windows, macOS, and Linux support, a Rust widget API, and XRC support through
  `include_xrc!`. Source: <https://docs.rs/wxdragon/latest/wxdragon/>
- AccessKit is Rust accessibility infrastructure for custom-rendered UI
  toolkits and supports Windows/macOS adapters. Source: <https://accesskit.dev/>
- The REAPER Accessibility Hoard is a public file server that hosts the
  JAWS-for-REAPER scripts under
  <https://hoard.reaperaccessibility.com/Custom%20actions,%20Scripts%20and%20jsfx/Windows%20Scripts/JAWS%20Scripts%20by%20Snowman/>.
  It is the rejetto/HFS server (`https://github.com/rejetto/hfs`), which exposes
  a documented public REST API: `POST /~/api/get_file_list` returns a JSON
  directory listing including each entry's name, size, and modified time, which
  RABBIT can use as the latest-version provider for the JAWS scripts.
- REAPER's video decoder loads FFmpeg's shared runtime libraries
  (`avformat-XX.dll`, `avcodec-XX.dll`, ...) from `<resource>/UserPlugins/`.
  REAPER 7.66 added support for FFmpeg 8.x; RABBIT pins to the highest
  stable major REAPER's decoder is known to support and bumps that pin as
  new majors are added. Sources for shared Windows builds: Gyan.dev
  publishes a fixed `ffmpeg-release-full-shared.7z` redirector that always
  points at the current stable, with a sibling `*.ver` plain-text endpoint
  exposing the version. tordona/ffmpeg-win-arm64 publishes per-tag GitHub
  releases of ARM64 shared builds (`ffmpeg-<ver>-full-shared-win-arm64.7z`).
  Sources: <https://www.gyan.dev/ffmpeg/builds/> and
  <https://github.com/tordona/ffmpeg-win-arm64/releases>. No equivalent
  scriptable source of shared `dylib` builds for macOS exists today —
  OSXExperts.net and evermeet.cx ship static binaries only, so the Mac
  side is deferred until a shared-build source materializes.
- FFmpeg's vanilla Windows resource script (`fftools/fftoolsres.rc`)
  attaches only a manifest, not a `VERSIONINFO` block, so probing the PE
  resource of `ffmpeg.exe` returns nothing useful. The reliable
  cross-vendor signal for the FFmpeg release version of an
  externally-installed copy is parsing the first line of
  `ffmpeg.exe -version`'s stdout (`ffmpeg version <version>
  Copyright (c) ...`).
- ReaPack stores its enabled remotes in `<resource>/reapack.ini` under a
  `[remotes]` section. Each remote is a numbered key
  (`remote<N>=<name>|<url>|<enabled>|<autoinstall>`) with `size=` declaring
  the count. ReaPack treats `size` as authoritative and assigns indices
  in addition order, so an idempotent "add this remote" upsert appends at
  the next index when the URL isn't already present.
- Surge XT is a free hybrid synthesizer from the
  [Surge Synth Team](https://surge-synthesizer.github.io/). RABBIT tracks
  the rolling nightly channel at `surge-synthesizer/surge` releases tag
  `Nightly` rather than the stable `surge-synthesizer/releases-xt` repo
  because the most recent stable (`1.3.4`, 2024-08-11) is the project's
  *de facto* abandoned channel — Surge XT now ships through nightlies.
  The nightly tag is static; the rolling build identity lives in the
  asset filenames (`surge-xt-<platform>-NIGHTLY-<YYYY-MM-DD>-<short-sha>-…`).
  The Windows installer is Inno Setup (`surge-xt-win64-…-setup.exe`,
  silent flags `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART
  /NOCLOSEAPPLICATIONS /TYPE=full /ALLUSERS /LOG=<file>`). RABBIT doesn't
  let Inno close programs through Windows' Restart Manager: Inno registers
  every file it would install that matches Surge's `CloseApplicationsFilter`,
  the fonts in `C:\Windows\Fonts` that Surge's uninstaller leaves behind
  included, so a program holding one of them made setup stop with exit code
  5, and closing programs could close a screen reader. Instead the install
  doesn't start while `Surge XT.exe` or `Surge XT Effects.exe` runs (the
  manifest's `blocking_programs`, which the manifest check keeps free of
  screen readers). The log goes to
  `<resource>/RABBIT/logs/surge-xt-installer-<stamp>.log`, passed without
  quotes of its own: both `Command` and the elevated start quote an argument
  with spaces as one word, and Inno drops quotes anywhere in a parameter.
  RABBIT makes the folder first, as the user, and leaves `/LOG=` out when it
  can't. Accepted risk: Inno writes the log after its own permission prompt,
  possibly as another administrator account, and refuses to start (exit
  code 1) when it can't create the log, as with a roaming AppData folder
  redirected to a share that account can't write to. The user then gets the
  general advice and no log is named. RABBIT starts the installer without
  elevation, so declining Inno's own permission prompt ends it with code 2,
  not 1223. Because Inno rolls back before it ends with any code other than
  0, that code fails the package even when an earlier install's files pass
  verification. A non-zero exit code is reported in plain words, with what
  Inno's codes 2 (in practice a declined prompt), 5, 7 and 8 mean and the
  log's path. The nightly channel no longer ships a Win32 setup.exe and has
  no native ARM64 setup.exe, so Windows-on-arm hosts run the win64
  installer under x64 emulation (`ArchitecturesAllowed=x64compatible`).
  The macOS DMG (`surge-xt-macOS-…-dmg`) wraps a `productbuild`
  distribution `.pkg` whose sub-pkgs install the VST3, AU, CLAP, LV2,
  Standalone, and Effects formats under `/Library/Audio/Plug-Ins/<format>/`
  and the Resources sub-pkg under `/Library/Application Support/Surge XT/`.
  Both paths require admin rights, which is why RABBIT only offers Surge
  XT on standard REAPER targets — portable installations can't host the
  system-wide factory data anyway.
- macOS elevation in `rabbit-platform::elevation` raises the native
  AuthorizationServices dialog by wrapping the elevated command in
  `osascript -e 'do shell script "…" with administrator privileges'`.
  We picked that over `SMJobBless` because the latter would require a
  separate signed helper bundle RABBIT doesn't ship, and over plain
  `sudo` because `sudo` can't read a password from a GUI parent
  process. The osascript route is the same path Apple's own installer
  uses for GUI-driven elevation and is screen-reader friendly. Its
  consumers are the Surge XT `MountDiskImageAndRunPkgInstaller` runner and
  an uninstall's one administrator command (see Safety Behavior). The root
  shell behind
  `do shell script` would split and expand an unquoted path, so every
  word is POSIX single-quoted and the whole command is then put into one
  AppleScript string, after a `cd /`. Nothing is written to a script
  file, which another process of the same user could rewrite before root
  runs it. `run_administrator_shell` puts several commands behind one
  prompt the same way and reports which one failed. Neither raises a
  prompt from a test build or from a program not named like RABBIT:
  `is_rabbit_file_stem` wants the file stem to be `rabbit`, ignoring case,
  then nothing, `-`, a space or `(`, and never a cargo test binary's
  `<crate>-<16 hex digits>`. The same check guards `move_to_trash` and
  `refresh_audio_units`, so no test moves or ends anything of the user's.
  Every name RABBIT ships under passes, the app's inner `rabbit` included;
  a bare binary renamed to anything else can't install Surge XT, and can't
  uninstall it or REAPER.

## Recommended Technical Direction

Use Rust for the core application, package engine, and primary UI. The UI
should prefer wxDragon so RABBIT can stay in one Rust codebase while still using
mature wxWidgets-backed native controls.

Recommended implementation:

- `rabbit-core` in Rust: detection, manifests, downloads, verification, install
  planning, backups, receipts, localization lookup, and logging.
- `rabbit-platform` in Rust: Windows/macOS native API isolation (file-version
  probes, registry probes, keychain/codesign, locale probe, plist parsing,
  disk-image mounting). One-way dependency from `rabbit-core` to
  `rabbit-platform`.
- `rabbit` in Rust (single binary, formerly `rabbit-cli` + `rabbit-ui-wxdragon`): the
  user-facing entry point. Built with the GUI feature on by default so a release
  binary can launch the wizard. The `main` function dispatches by argv:
  - no arguments → run the wxDragon wizard
  - any arguments or `--help` → run the CLI subcommand parser
  This single-binary model removes the duplicate distribution shape and makes
  it easy for users to memorize "the file is `RABBIT.exe`/`RABBIT`".
- Build release artifacts as self-contained executables wherever the platform
  allows. Embed required UI text, default localization resources, package
  metadata, and small static assets into the binary. Do not require a RABBIT
  installer for normal use.

### Single-Binary Argv Dispatch

Top-level `main`:

1. Read `std::env::args_os()`.
2. If exactly one argument is present (the program name itself), launch the
   GUI. On Windows, the binary is built with `#![windows_subsystem = "windows"]`
   so no console pops up; the GUI path uses `AttachConsole(ATTACH_PARENT_PROCESS)`
   only when the user explicitly asks for a banner/version on stdout.
3. Otherwise, hand the full argv to the CLI parser, which on Windows attaches
   to the parent console (or allocates one) so help output and command
   results are visible.

The CLI subcommand surface stays roughly what `rabbit-cli` exposes today;
moving it under the same crate is a packaging change, not a feature change.

### Release Build Profile

Release builds are tuned for binary size:

```toml
[profile.release]
opt-level = "z"     # optimize for size
lto = "fat"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

`debug-assertions` stays off in release. The CI release-mode artifact check
ensures the produced binary stays a single file with no neighboring DLL/dylib
dependency for the normal launch path.

This keeps the important logic and UI integration in Rust and avoids maintaining
a separate C++/Objective-C/C ABI UI shell. The tradeoff is that wxDragon is a
younger Rust layer over a mature toolkit, so RABBIT should keep the GUI thin and
well tested. If wxDragon blocks required accessibility behavior, the fallback
should be a direct wxWidgets shell with the same view-model boundary rather than
rewriting the installer engine.

## Primary Automation Requirement

Full unattended installation is part of the product definition, not a stretch
goal. For the first-class supported package set of REAPER, OSARA, SWS,
ReaPack, and ReaKontrol, RABBIT should converge on one shared unattended
execution path used by both the GUI and CLI.

Design rules:

- The normal supported path must not stop at "download and tell the user what to
  do next" for REAPER, OSARA, SWS, ReaPack, or ReaKontrol.
- For executable installers, RABBIT itself must download, verify, launch, wait
  for completion, evaluate exit status, and validate the installed result in the
  same run.
- The supported flow must not require the user to manually open an `.exe`,
  `.pkg`, `.app`, disk image, or extracted archive and click through its setup
  UI on their own.
- Any manual-attention flow for those packages is a temporary implementation
  gap, not acceptable steady-state product behavior.
- The GUI wizard and CLI must call the same package execution engine so
  unattended behavior is consistent and testable.
- RABBIT should prefer direct verified file installation for extensions when that
  is technically reliable, because it is more deterministic and accessible than
  driving third-party interactive installers.
- When RABBIT must use a vendor installer, it should do so with documented or
  validated silent arguments, explicit exit-code handling, integrity checks, and
  a post-install verification pass.
- "Run upstream installer" in the package model means RABBIT invokes the installer
  itself as part of the installation operation. It does not mean "download the
  installer and ask the user to run it manually later".
- If a package cannot currently be installed unattended on a platform, RABBIT
  should mark that as unsupported for that build/platform combination, not treat
  permanent manual installation as the finished design.

## RABBIT Portability

RABBIT should behave like a portable utility. A user should be able to download a
single executable, run it from any writable folder, complete the REAPER setup or
update, and remove the executable afterward. The executable may create cache,
log, backup, report, and receipt files in explicit RABBIT locations under the
selected REAPER resource path or user cache directory, but those files are
operation data, not files required to start RABBIT.

Distribution goals:

- Windows: a single signed executable. No MSI, no setup program, no companion
  DLLs in the default launch path. The same executable serves both UI and CLI
  users via argv dispatch.
- macOS: a single signed and notarized executable. Use an `.app` bundle
  layout only if macOS GUI launch policy forces it for a given
  wxWidgets/wxDragon shape; in that case the bundle stays self-contained with
  no separate RABBIT installer, and the CLI command-line surface still works
  against the binary inside the bundle.
- Release artifact names follow `rabbit-<version>-<os>-<arch>[.exe]`
  (e.g. `rabbit-0.2.0-windows-x86_64.exe`, `rabbit-0.2.0-macos-universal`).
  macOS uses the literal `universal` token because the bare binary is a fat
  Mach-O with both `aarch64` and `x86_64` slices — one artifact runs natively
  on both Apple Silicon and Intel hosts. This makes successive downloads
  distinguishable on disk, calls out the build's architecture explicitly, and
  works around the macOS-binary-without-extension ambiguity. Users may rename the file after downloading; the self-update
  apply step swaps in place under whatever filename the running binary has,
  not under the downloaded file's name.
- CI artifacts and GitHub release assets are the raw single-file binaries plus
  per-file SHA-256 sums. No `.zip` wrapper is required for single-file
  artifacts; release notes link directly to the per-platform binaries.
- Do not require separate locale files, XRC files, icons, package manifests,
  or certificates beside the executable for the default experience.
- Store downloads in the normal RABBIT cache directory and allow the cache to be
  deleted safely.
- Store install receipts, backups, and reports in the selected REAPER resource
  path so they travel with portable REAPER installations when possible.
- Do not write RABBIT program settings to the Windows Registry. If user
  preferences become necessary, keep them optional and user-scoped.

Implementation rules:

- Use `include_str!`, `include_bytes!`, or generated Rust data for required
  built-in resources.
- Treat external locale/resource directories as optional developer or advanced
  override paths, not runtime requirements.
- Keep wxDragon/wxWidgets deployment self-contained for release builds. The
  release check must verify that launching RABBIT does not depend on DLLs or
  dylibs sitting beside the executable unless the platform's GUI framework
  requires a signed app bundle layout.
- Any temporary extraction of embedded helpers must go to a temporary directory,
  be integrity-checked, and be cleaned up best-effort.
- The same portability rule applies to self-update: updated binaries should
  replace the old RABBIT binary or app bundle in place, not install a separate
  long-lived updater application.

## CI/CD And Release Delivery

RABBIT should have first-class delivery automation from the beginning. The design
target is that every push produces testable platform artifacts and every tagged
release produces end-user release assets plus update metadata.

GitHub Actions build pipeline for every push:

- Trigger on every push and pull request.
- Build RABBIT on at least:
  - `windows-latest` for the Windows executable
  - `macos-latest` for the macOS app or executable artifact
- Run the normal Rust checks on both platforms:
  - formatting
  - unit/integration tests
  - release-mode build
- Build the distributable artifact shape, not only debug binaries:
  - Windows: a single `RABBIT.exe`
  - macOS: a single `RABBIT` executable (or a self-contained signed `.app`
    bundle if macOS GUI launch policy requires it for the chosen
    wxWidgets/wxDragon shape)
- Upload build artifacts to the workflow run so every push has downloadable
  test binaries. Single-file artifacts upload as the bare file; no `.zip`
  wrapper is added on top of an already-single-file binary.
- Publish per-file SHA-256 sums alongside the artifacts so testers can
  verify what they ran.

Live-upstream smoke pipeline:

- A separate scheduled workflow runs each day against the real upstream
  download URLs so vendor-side changes (relocated assets, new archive
  layouts, expired redirectors) trip CI before they trip a user. The
  current workflow is `macos-smoke.yml`, which exercises the OSARA / SWS /
  ReaKontrol / REAPER macOS install paths on `macos-latest`. The
  Windows-side smoke (FFmpeg via Gyan + tordona, plus the existing
  Windows install paths) is on the open-questions list — adding it
  requires a parallel `windows-smoke.yml` and `windows_upstream_smoke.rs`
  test file.

GitHub release pipeline:

- Trigger on a version tag such as `vX.Y.Z`, or on an explicit release workflow
  dispatch that creates the tag as part of the release process.
- Rebuild Windows and macOS release artifacts from the tagged commit in a clean
  GitHub Actions environment.
- Produce release attachments:
  - Windows artifact
  - macOS artifact
  - SHA-256 checksums
  - machine-readable release/update manifest
- Apply code signing where available:
  - Windows Authenticode signing for `RABBIT.exe`
  - macOS code signing and notarization for the app bundle or executable
- Generate release notes from a changelog or tag diff, with manual override for
  accessibility-relevant release notes.
- Publish the GitHub Release only after artifacts, checksums, signing, and
  update metadata are complete.

Release metadata:

- Each published release should expose enough machine-readable metadata for
  RABBIT self-update, including:
  - semantic version
  - release channel (`stable`, later optionally `beta`)
  - publish timestamp
  - per-platform download URL
  - expected SHA-256
  - minimum supported previous RABBIT version if a breaking updater transition is
    ever needed
- The release workflow should emit a stable JSON manifest asset or equivalent
  update feed derived from the GitHub Release so the updater does not need to
  scrape human-written release notes.

Suggested workflow layout:

```text
.github/
  workflows/
    ci.yml
    release.yml
```

## RABBIT Self-Update

RABBIT should be able to update itself from GitHub Releases with minimal user
interaction while staying accessible and verifiable.

Updater design goals:

- Check for RABBIT updates separately from package updates for REAPER and its
  extensions.
- Use the GitHub release/update manifest as the authoritative source for the
  latest RABBIT version and platform artifact URL.
- Compare versions using strict semantic versioning for RABBIT itself.
- Default behavior should be:
  - detect newer RABBIT release
  - present a short accessible prompt
  - download in the background after confirmation
  - apply the update with one restart/replace step
- Advanced later option:
  - support a user preference for automatically downloading stable RABBIT updates
    and applying them on the next restart

Updater flow:

1. On startup, or on explicit `Check for RABBIT updates`, fetch the signed or
   checksum-validated release manifest from the configured GitHub release
   channel.
2. If the current RABBIT version is already current, report that plainly.
3. If a newer version exists, show:
   - current version
   - available version
   - release channel
   - short release notes or a link to them
4. After confirmation, download the platform artifact to a temporary update
   staging directory.
5. Verify:
   - HTTPS transport
   - expected SHA-256 from release metadata
   - Windows signature or macOS code signing/notarization where applicable
6. Stage the replacement.
7. Replace RABBIT with the new version using the smallest platform-appropriate
   restart flow.
8. Relaunch the updated RABBIT instance and confirm the new version.

Platform update strategy:

- Windows single-executable build:
  - RABBIT cannot replace its own running `.exe` in place.
  - Stage the new executable beside the current one or in a temporary
    directory.
  - Launch a very small temporary updater helper process or script whose only
    job is to wait for RABBIT to exit, swap the executable, and relaunch RABBIT.
  - The helper must be ephemeral, integrity-checked, and cleaned up best-effort.
- macOS app bundle build:
  - Stage the new signed/notarized app bundle in a temporary directory.
  - After RABBIT exits, replace the existing bundle atomically where possible and
    relaunch it.
  - Preserve the app bundle path so Dock aliases and user expectations do not
    break.

Updater safety rules:

- Never apply a RABBIT self-update while a package installation/update operation
  is running.
- Never replace RABBIT with an unsigned or checksum-mismatched artifact.
- Keep one rollback copy of the previously running RABBIT binary or bundle until
  the first successful restart of the updated version.
- Log the update attempt and result in a plain text and machine-readable report.
- If self-update fails, RABBIT must continue running the existing version and
  report the error without leaving itself half-replaced.

CLI and UI expectations:

- CLI should expose explicit commands later such as:
  - `rabbit self-update check`
  - `rabbit self-update apply`
- GUI should expose:
  - automatic startup check
  - manual `Check for RABBIT updates`
  - accessible progress/status during download and replacement

## Accessibility Rules

- Use native controls only: buttons, check boxes, radio buttons, list views,
  tree views, tab controls, text fields, progress controls, and standard
  dialogs.
- A tab control is wxNotebook: SysTabControl32 on Windows, NSTabView on
  macOS, which screen readers present as tabs. Its keyboard path: on
  Windows, Ctrl+Tab and Ctrl+Shift+Tab or Ctrl+Page Down and Ctrl+Page Up
  from inside it, and the arrow keys on its tab strip; on macOS, physical
  Ctrl+Tab and Ctrl+Shift+Tab, which RABBIT reads itself because wxWidgets
  takes Command for Control there, the arrow keys on its tab strip, and
  VoiceOver's own navigation of the tab group. A moment after the Packages
  page first opens, VoiceOver says once that Control-Tab switches tabs. The
  tab strip is a Tab stop on both: Tab from it enters the selected tab,
  Shift+Tab from the tab's list comes back to it, and Tab from the last
  button goes round to it. wxMSW does this itself. wxOSX's notebook passes
  its focus straight on into the selected tab and its Tab order skips the
  strip, so on macOS RABBIT keeps focus on the strip and handles those
  three keys itself. An NSTabView takes focus only with Full Keyboard
  Access (keyboard navigation in System Settings); without it, Tab skips
  the strip and Ctrl+Tab is how to switch. After a switch on macOS, focus
  moves into the new tab and RABBIT announces the tab's name. Two Windows
  quirks of the native tab control are documented rather than worked
  around: Ctrl+Tab does nothing from the wizard's buttons, which sit
  outside the tab control, and plain Page Up or Page Down on a checkbox or
  button inside a tab switches tabs. That switch moves focus into the new
  tab's list, whose label names the tab, and Review names the mode and the
  ticks the run leaves out.
- Avoid custom drawing, canvas controls, owner-drawn lists, and grid controls in
  the main workflow.
- Every control needs an accessible name, role, state, keyboard focus, and
  visible label.
- Every screen must work by keyboard only, with a predictable tab order and
  mnemonic accelerators.
- On macOS, wxWidgets gives each scrolling wizard page real scroll bar
  controls, which Tab stopped on. RABBIT takes them out of the Tab order;
  VoiceOver still reaches them with its own keys.
- Progress must be exposed as both a progress bar and a text status line.
- Errors must be plain text, selectable/copyable, and available in a final
  report.
- The test matrix for every release must include NVDA and Narrator on Windows,
  plus VoiceOver on macOS. JAWS should be included before public beta.

## User Workflow

The UI is a short, jargon-free wizard. Defaults are chosen so a non-technical
user can finish the install by pressing Next a few times.

1. Target
   - One concise control: pick a detected REAPER, or pick "Install or update a
     portable REAPER folder" and choose the folder.
   - Detected installations show simply as "REAPER 7.69 in `<path>`" — no
     architecture, no detection-confidence label, no evidence list. Those
     remain available in the saved report and from the CLI for advanced
     users.

2. Version check
   - A dedicated progress page with a progress bar, one status line ("Checking
     OSARA…") and a hidden error region that only appears if the package plan
     can't be built.
   - When the checks finish, the wizard moves on to the package list by
     itself, even if some of them failed. A package whose check failed gets a
     row that can't be ticked, with a short reason, and Review carries a note
     with the full error; every other package installs or updates as usual.
     Only a failure to build the package plan at all keeps the wizard on this
     page, with the error lines shown, so the user can go back or quit.
     Failed checks only ever affect installs: the Uninstall tab's rows never
     depend on them.

3. Packages
   - Two tabs, **Install or update** and **Uninstall**. The page opens on
     Install or update with focus on the tab strip, so screen readers read
     the tab and its position first, and Tab moves into the package list.
     Where macOS can't focus the tab strip (an NSTabView takes focus only
     with Full Keyboard Access), focus goes to the install list and RABBIT
     announces the tab's name instead. Either way, a moment after the first
     arrival in a run, VoiceOver says that Control-Tab switches tabs. The
     hint waits so that VoiceOver's reading of the new focus can't cut it
     off; if the page is gone by then, the next arrival tries again. RABBIT
     changes the tab selection by itself only on the arrival from the
     version check, once the page shows: wxOSX moves focus into a tab on
     every selection change, even on a hidden page, so the rebuild after a
     run leaves it alone. On Windows that rebuild clears a list's selection
     before deleting its rows: wxMSW answers comctl32's notice that the
     selected row went by focusing the list, hidden or not, and after an
     uninstall that took focus off the Done page. After either run the
     rebuild also happens before Done shows and takes focus, so Done's
     status is the last focus change. Back from Review or from the ReaPack
     page lands on the strip as well, on Windows and on macOS with Full
     Keyboard Access: showing the page refocuses the control it last had
     focus in, which is the notebook, even if the user left from the list.
     In uninstall mode, Back from Review puts focus back in the Uninstall
     list instead, on every platform. On macOS, adding the first tab while
     the window is built also counts as a selection change, and that put
     the start focus in the hidden Install list: VoiceOver began at the top
     of the window and Tab did nothing. So after building all pages, RABBIT
     focuses the target choice again on macOS, and the wizard opens there.
     Each list's caret goes to its top row the first time the list gets
     focus after the page opens. The tab showing when Next is
     pressed decides what the run does; install and uninstall never mix.
     The Uninstall tab lists every installed package under the same groups,
     with headings that can't be ticked, nothing ticked by default, and rows
     RABBIT can't remove shown but impossible to tick, saying why. An empty
     list says so in its details text. On Windows the sentence is also a row
     without a checkbox; macOS shows no row, because a row there has either
     a checkbox or a disclosure triangle, and says the sentence with the
     tab's name instead. Its rows come from the detection the version check
     already ran, which asks only which files exist and how big they are,
     and on macOS who may move an app. There a row's details say how the
     package goes: to the Trash as the user, into the user's Trash through
     the administrator command, or deleted for good by it. An app RABBIT
     can't move is shown but can't be ticked, and its details say why and
     how to move it in Finder: Command-Delete, and for a locked app Get
     Info's Locked first. On Windows the JAWS scripts' row adds, while the
     uninstaller folder their installer left in Program Files (x86) or its
     registry entry is there, that only an administrator may remove those, so
     Windows asks, and that a backup can't bring them back; that line stands
     in for the backup line when none of the scripts' files are left to move.
     Its notes say when RABBIT has a receipt but no record of which files the
     installer added, and in every removable state how to remove by hand the
     script files that stay. When RABBIT doesn't trust that folder or entry,
     or has no receipt and finds neither of the scripts' DLLs for this
     account, the row can't be ticked and reads "remove it by hand"; its
     details say why (what RABBIT didn't trust, or that the account that
     installed the scripts should uninstall them), then give the manifest's
     by-hand steps. On a portable target the row reads like that of any
     package a portable REAPER can't have, and the same steps follow the
     details' sentence that only the standard REAPER removes the scripts.
     Below its list, an unticked **Also remove settings** checkbox with a
     note on what it would remove works like the OSARA key map checkbox:
     disabled and out of the Tab order until a ticked package has settings
     RABBIT may remove, and unticked again when none has. On macOS,
     VoiceOver is told when a tick makes the option available or not, since
     that happens away from the row. Everything below describes the Install
     or update tab.
   - Tree view with two top-level groups: **Packages** and
     **Configuration**. Packages contains REAPER, OSARA, SWS, ReaPack,
     ReaKontrol, FFmpeg (Windows only), JAWS scripts (Windows only), and
     later additions; each row reads as plain text "OSARA — installed
     2024.3.6, latest 2026.2.16, will update". Configuration contains
     opt-in post-install tweaks (currently: "Add the REAPER Accessibility
     ReaPack repository to ReaPack"); rows that are already applied or
     whose dependency package isn't in the install plan are disabled with
     a short status tag in the row label
     (`(already applied)`, `(requires ReaPack)`).
   - Parent group nodes show a half-checked / tristate state when only
     some of their children are ticked. Toggling the parent propagates to
     every actionable leaf below it; clicks and Space keystrokes on
     not-actionable leaves are pre-empted before native processing so
     screen readers don't announce a phantom "checked" state that would
     immediately revert. The checkboxes are real native controls
     (`TVS_CHECKBOXES` on Windows with a custom three-image state list
     for the tristate parent, `wxDataView` toggle renderer with a custom
     `CustomDataViewTreeModel` on macOS/Linux) so screen readers
     announce them as proper checkboxes, not as "graphic" or
     "static text".
   - The selected row's details pane shows the package's localized
     description (one or two plain-language sentences explaining what the
     package is and why a user might want it). The same description is
     exposed by the CLI via `rabbit packages`, so users can read about
     every package before deciding to install it.
   - Defaults: install or update missing/outdated recommended accessibility
     packages, plus apply every recommended Configuration step whose
     dependency is satisfied and which isn't already applied.
   - OSARA key map: an explicit checkbox stays on the Packages page so the
     user sees and controls what RABBIT will do with `reaper-kb.ini`.
     Default is replace-with-backup for a new OSARA and for a key map that
     is still the installed OSARA key map (scripts, order and binding
     comments aside), and keep for any other key map. The short note next
     to the checkbox says which case applies in the active locale. The CLI
     makes the same choice unless `--replace-osara-keymap` or
     `--preserve-osara-keymap` is passed.

4. ReaPack donation acknowledgement (only when ReaPack is being installed
   or updated this run)
   - Dedicated wizard page that mirrors the donation hint from
     <https://reapack.com/>. Plain language, in the active locale:
     ReaPack is free software, donations are optional, here is the
     donation URL.
   - A focusable link to <https://reapack.com/donate> and an explicit
     checkbox or "I understand, continue" control. Continue is disabled
     until the acknowledgement is set. Going back to step 3 clears the
     acknowledgement so the user always re-confirms after changing the
     selection.
   - In the CLI, the same acknowledgement is surfaced as an interactive
     prompt before staging. For unattended use the user passes
     `--accept-reapack-donation-notice`; without it, `apply-packages` /
     `setup` refuses to stage ReaPack and exits non-zero with a clear
     message that points at the flag.
   - Skipped automatically on runs where ReaPack is neither installed nor
     updated.

5. Review
   - Short plain-language summary: target path, packages to be installed or
     updated, an indication that backups will be made if any existing files
     will be replaced. No backup-file paths, no admin-prompt enumeration, no
     planned-execution metadata in the GUI summary; all of that is still
     written to the saved report and exposed by the CLI.
   - In uninstall mode the heading says "Ready to uninstall", and the
     summary has one line per package with what goes and what stays (or,
     with Also remove settings ticked, which settings go too), the
     warnings, a note naming what is ticked on the other tab and left out
     (on the Install or update tab, only ticks RABBIT didn't set itself,
     such as an update it ticked by default), and that the packages can be
     installed again. When a ticked package's own uninstaller removes it
     (on Windows, Surge XT or a standard REAPER), or the elevated helper
     removes the uninstaller folder and registry entry the JAWS scripts'
     installer left, the summary also says that Windows asks for permission
     once, before anything is removed, and that nothing is uninstalled
     without it. For a vendor's uninstaller it also says to answer a window
     from that uninstaller if one appears, and asks to close other music
     programs. The JAWS scripts' own line leaves the folder and entry out,
     so for them the summary adds the question's sentence: a backup can
     bring back their files but not that folder and entry, or, with none of
     their files left to move, can't bring the scripts back. On macOS,
     when the administrator command has a package to remove, the summary
     says once, naming every such package, that macOS asks once for an
     administrator's name and password before anything is removed, that it
     may offer Touch ID, that the window may name osascript, the tool RABBIT
     asks through, and that cancelling the window uninstalls nothing. A
     package that goes to the Trash gets a line saying it can be dragged
     back from there until the Trash is emptied; one the administrator
     command moves there, that it keeps its owner, so Finder offers no Put
     Back and emptying the Trash may ask for a password. When that command
     deletes a package for good, the summary also asks to close other music
     programs. It doesn't ask this for an app the command moves to the
     Trash, since no other program uses that app. The Uninstall button is
     enabled whenever something removable is ticked, whatever the install
     plan says. Uninstalling is never one key press away: focus goes to the
     summary, there is no default button, and Enter starts nothing. A run
     with a step a backup can't undo, such as a vendor's uninstaller, the
     macOS administrator command, a move to the Trash or the elevated
     helper's removal of the JAWS scripts' uninstaller folder and registry
     entry, or a package whose manifest asks for it, gets a Yes/No question
     with No focused first. For a package that goes to the Trash the
     question says it can be dragged back from there instead of saying a
     backup can't bring it back. For the JAWS scripts it names only the
     folder and entry while any of their files go into the backup set, and
     once both are gone it isn't asked at all.
     Back returns to the Uninstall tab's list, never to the ReaPack
     donation page.

6. Progress
   - Single progress bar plus one current-step line. A "Show details" toggle
     reveals the underlying log for users who want it; collapsed by default.
   - Before an uninstall starts, the Uninstall button refuses with a message
     while something the uninstall preflight needs closed is running: the
     target's REAPER, every REAPER for a package that needs a standard
     REAPER (such as app2clap, which any REAPER can load, or the JAWS
     scripts), and the programs a package's plan names. JAWS itself is
     never one of them: it is the user's screen reader, and the scripts'
     plan names no program to close. On macOS a REAPER.app the
     administrator command moves needs every REAPER closed too, because
     root's `pgrep` can't tell one REAPER from another; when no other
     ticked package (such as Surge XT) needs every REAPER closed, the
     message says so. It uses the engine's own process checks, so the two
     agree, and there is no way to go ahead anyway.
   - In uninstall mode the heading says "Uninstalling", focus moves to the
     gauge, and the gauge counts one step per package, so it never parks
     halfway. The status line names each package as its removal starts and
     says whether it went; both are spoken on macOS. A vendor's uninstaller
     gets its own line ("Running Surge XT's own uninstaller…"), and once it
     has run for 30 seconds the status and the log say RABBIT is still
     waiting and that it may be asking a question in another window, found
     with Alt+Tab. When an Inno uninstaller has ended but its files are
     still there 30 seconds later, a line says RABBIT is waiting for them
     and to close the music program that has the package open. Each line is
     spoken on macOS; on Windows, which has no announcement, focus moves to
     the log the first time each happens in a run, so the screen reader
     reads it once. On macOS, right before the password window takes
     focus, the status line and the log say, and VoiceOver hears, that
     macOS now asks once for an administrator's name and password, which
     packages the administrator command uninstalls, and that cancelling
     uninstalls nothing. The window itself keeps macOS's own text and names
     osascript, not RABBIT: RABBIT adds no `with prompt` text, so this is
     the only word from RABBIT the user gets. After 30 seconds without an
     answer a second line says RABBIT is still waiting and the window may be
     behind RABBIT. The command runs as one, so afterwards each of its
     packages gets its lines together: the log says "The administrator
     command has finished with Surge XT", and the status line says whether
     it went. An app the user moves to the Trash gets "Moving REAPER to the
     Trash…". Which progress events are spoken is
     decided by one exhaustive match, so a new event can't be added without
     that decision. Closing the window asks "Stop uninstalling?" with No
     focused, and the run stops after the step it is on: one package, or on
     macOS the administrator command, which covers several. A vendor's
     uninstaller always runs to its end.

7. Done
   - One sentence summarizing success or failure.
   - Buttons: Launch REAPER and Open resource folder. There is no Save
     report button: every run saves its report under `<resource>/RABBIT/logs/`
     by itself. The signature-verification counts, lock-file paths, and
     similar diagnostics stay in that saved report.
   - After an uninstall the status names the packages that went, says why
     any stayed (in the user's language for REAPER or another RABBIT being
     open, a file in use, too little room for the backup, every way a
     vendor's uninstaller can end and every way the JAWS scripts' cleanup
     can end; otherwise the engine's own sentence), and that a package comes
     back by ticking it on the Install or update tab in a new run. A JAWS
     script file in use says JAWS or another program has it open, to wait
     and run RABBIT again, and to restart JAWS first if that doesn't help,
     rather than to close a program, which would be the user's screen
     reader. After a cleanup that was refused, failed or lost, the reason is
     followed by the manifest's steps to remove the scripts by hand. A
     removal that finishes after a restart says to restart the computer,
     and a declined permission prompt, or on macOS a cancelled
     password window, says plainly that nothing was uninstalled. After a
     macOS removal Done also says what moved to the Trash and what it is
     called there now; where the administrator command stopped or what it
     left, and that a tree it stopped in while deleting may be only partly
     there. When no package went but the command removed part of one, the
     first line says RABBIT removed only part of it rather than that
     nothing was uninstalled, since that part can't be put back. Done also
     names installer receipts macOS still lists, which do no harm, and
     every program the command looked for when it found one of them
     running, or couldn't check, and so changed nothing; and when
     macOS refused a step, it says to turn RABBIT on under System Settings >
     Privacy & Security > App Management, quit it, start it again and
     uninstall again. Show details
     lists the files
     removed, kept and left over, the backup set, and one `rabbit
     restore-backup` command per removed package whose files went into
     that set: never for one a vendor's uninstaller, the Trash or the
     administrator command took. For the JAWS scripts it also lists what
     the elevated helper removed with administrator rights, each file, the
     folder and the registry entry as regedit names it, and what of the
     folder or entry is still there, also after a cleanup that failed part
     way. For a removed package only, it lists the script files kept
     because they were there before the installer ran or their records
     disagree, then the note on removing by hand the script files that
     stayed: while the package is still there, all of its files are. The
     restore command for the JAWS scripts brings back their files, not the
     folder and entry. The saved report is the engine's `uninstall-<time>`
     report, which the details point to; the wizard writes one of its own
     only for a run the engine refused before it began, except a macOS run
     refused because RABBIT runs as root, whose report would be root's file
     in the user's resource folder. Both tabs are then rebuilt from fresh
     detection.

Streamlining rules (apply when adding new wizard text):

- Default to one sentence per element. If a control needs more, the second
  sentence belongs in the saved report, not the wizard.
- Never expose internal identifiers (detector names, automation kinds, plan
  action enums, lock-file paths, SHA-256 prefixes) to the GUI user.
- Power-user output (full plan, full execution log, verdicts) stays
  reachable through the CLI and the saved report.

## Installation Discovery

Represent every candidate as:

```text
Installation {
  kind: Standard | Portable,
  app_path: PathBuf,
  resource_path: PathBuf,
  version: Option<Version>,
  architecture: Option<Architecture>,
  writable: bool,
  confidence: High | Medium | Low,
  evidence: Vec<Evidence>
}
```

Windows standard detection:

- Resource path: `%APPDATA%\REAPER`.
- App candidates:
  - `%ProgramFiles%\REAPER\reaper.exe`
  - `%ProgramFiles(x86)%\REAPER\reaper.exe`
  - uninstall registry entries, treated only as hints.
- Version: Windows file version metadata where available; otherwise optional
  runtime probe.

Windows portable detection:

- User-selected folder is authoritative.
- Auto-detected portable candidates must contain `reaper.exe` and `reaper.ini`.
- Resource path is the portable folder.

macOS standard detection:

- App candidates:
  - `/Applications/REAPER.app`
  - `/Applications/REAPER64.app`
  - `/Applications/REAPER-ARM.app`
- Resource path: `~/Library/Application Support/REAPER`.
- Version: app bundle `Info.plist` where available; otherwise optional runtime
  probe.

macOS portable detection:

- User-selected folder is authoritative.
- Candidate folder should contain a REAPER app bundle and normally `reaper.ini`.
  The app bundle is an `.app` directly in the folder, with "reaper" in its
  own name, ignoring case. With more than one, detection, the preflight, the
  wizard and an uninstall all pick the same: `REAPER.app`, then
  `REAPER64.app`, then `REAPER-ARM.app`, then any other by name, so Review
  and the run name the same app.
- Resource path is the selected folder.

REAPER runtime probe:

- Optional later enhancement: launch REAPER in a controlled way with a temporary
  ReaScript that calls `reaper.GetAppVersion()`. This is more accurate than
  binary metadata but must never be used silently if it would disturb the user
  or open a visible DAW session unexpectedly.

## Version Detection Strategy

There is no single generic, external "REAPER extension version" API that RABBIT
should rely on. Detection must be package-specific and confidence-scored.

| Component | Primary detector | Fallback detector | Confidence notes |
| --- | --- | --- | --- |
| REAPER | executable/app metadata or optional `GetAppVersion()` runtime probe | presence of app only | Runtime probe is most accurate, metadata is good enough for normal installs. |
| OSARA | RABBIT receipt after RABBIT-managed install; Windows standard uninstall `DisplayVersion` | binary string scan for embedded `OSARA_VERSION`; presence of `reaper_osara*` | Portable/mac installs do not have a universal external version registry. Binary scan is useful but should be marked best-effort. |
| SWS | RABBIT receipt; Windows PE `ProductVersion`; ReaPack registry if installed by ReaPack | binary metadata/string scan; presence of `reaper_sws*` | Prefer ReaPack registry for ReaPack-managed SWS. |
| ReaPack | RABBIT receipt; Windows PE `ProductVersion`; ReaPack self-entry in `ReaPack/registry.db` after first launch | presence of `reaper_reapack*` | The registry DB may not exist until ReaPack has run inside REAPER. |
| ReaKontrol | RABBIT receipt after RABBIT-managed install | best-effort binary metadata if available; presence of `reaper_kontrol*` | No installer or registry-based detector is expected; validate binary metadata quality during implementation. |
| FFmpeg | RABBIT receipt after RABBIT-managed install; `ffmpeg.exe -version` parsed from stdout's first line | StringFileInfo `ProductVersion` of an FFmpeg executable (only present when a vendor patches it in); libavformat-major filename heuristic (`avformat-62.dll` → FFmpeg 8.0.0, mapped via `lib_major - 54`) | Vanilla FFmpeg attaches only a manifest to its binaries; `VS_FIXEDFILEINFO` of `ffmpeg.exe` carries libavutil's version, not the FFmpeg release. Spawning `ffmpeg.exe -version` is the only reliable vendor-independent way to recover the patch level for an externally-installed copy; the libavformat-major fallback recovers only the major when no executable is present. |
| Surge XT | RABBIT receipt after RABBIT-managed install (records the `NIGHTLY-YYYY-MM-DD-sha` token verbatim) | `read_file_version_parts` on the system VST3 bundle — Windows PE `VERSIONINFO` of `Contents\x86_64-win\Surge XT.vst3` inside `<CommonProgramFiles>\VST3\Surge Synth Team\Surge XT.vst3\`; macOS `CFBundleShortVersionString` of `/Library/Audio/Plug-Ins/VST3/Surge XT.vst3/Contents/Info.plist` | The vendor-installed binary still carries the most recent *stable* semver (e.g. `1.3.4`) — Surge XT doesn't stamp nightlies with their date+sha. Receipt-driven detection produces the exact nightly token; the file-metadata fallback reports the stable semver, which compares lower than any `NIGHTLY-…` under `Version::cmp_lenient` so the planner naturally schedules an Update. On macOS a VST3 bundle whose version can't be read still counts as installed, with no version and low confidence: the uninstall deletes that bundle last with `rm -R`, which may take its `Info.plist` before it stops, so a removal stopped there leaves Surge XT detected and offered again. On Windows a bundle without a version is still not detected. |
| ReaPack packages | `ReaPack/registry.db` table `entries.version` | none | This is the best source for packages ReaPack knows about. |

RABBIT should keep its own receipt in each REAPER resource path:

```text
<resource_path>/RABBIT/install-state.json
```

The receipt should record package id, installed version, source URL, SHA-256,
installed files, backup files, install time, RABBIT version, and target
architecture. This is authoritative only for files RABBIT installed. If the user
later changes files manually, RABBIT should show that the receipt and disk state
do not match.

The receipt also records what a later uninstall needs in order to undo only
RABBIT's own changes: what each configuration step found or changed
(`applied_configuration`: the ReaPack repository and whether RABBIT added it,
or the `langpack=` value the language step replaced). A package the user
removes through RABBIT goes into `declined_packages`, the same set that holds
the packages they left unticked. Both readers honour the set for every package
that isn't installed: the wizard doesn't tick its Install row, and
`apply-packages` and `setup` without `--package` leave it out and say so. A
package in the set that is installed again, say dragged back from the Trash or
installed by hand, gets its updates as usual: the wizard ticks its Update row,
and those commands keep it in their default list. "Installed" means what each
reader plans from: the wizard's plan, and for the CLI the detection its run
plans from. Only a `remember_opt_out` package (the language packs) keeps its
refusal while installed, so a language pack whose update was turned down isn't
ticked again. Leaving a row unticked only records a "no" for packages that set
`remember_opt_out`, so an optional package skipped once still has its updates
offered. Only the user's own verdict takes a package out of the set again:
ticking it in the wizard, naming it with `--package` in a CLI run that installs
(`setup`, `apply-packages` or `install-extension`), or restoring it with
`restore-backup --package`. An install that comes without a verdict, such as a
package the CLI picked from its default list, leaves the set alone. These
fields are optional and left out when empty, so older RABBIT versions still
read the file. OSARA's key map is always kept when OSARA is removed, so the
receipt doesn't track a key map backup.

Two packages can look odd on Windows. The uninstall of FFmpeg and app2clap
removes only the files RABBIT's receipt lists, plus `app2clap.clap` for
app2clap. It also keeps an FFmpeg file that changed since RABBIT installed it.
But any `avformat*`, `avcodec*` or similar DLL in `UserPlugins` counts as
FFmpeg, and any `app2clap*.clap` in the CLAP folder counts as app2clap. So
such a file still counts as the package right after the removal, and the
uninstall reports it as left behind. The package's Update row is then ticked,
unless RABBIT can read the file's version and it is already the latest.
Without a receipt, RABBIT can't read app2clap's version at all. This is left
as it is: such a file is most likely the user's own, and RABBIT can't tell it
from one put back by hand.

Some vendor installers write where no receipt path reaches. For those, the
manifest's `uninstall` block lists the folders under `install_snapshot`:
RABBIT lists them right before the installer runs and again afterwards, and
the package's receipt entry records every file the installer added or changed
there, with its hash and size (`vendor_added_files`, kept apart from
`installed_files` so deleting one of these files doesn't make detection think
the package is gone). Files that were there before and didn't change are not
recorded. A changed file is marked `existed_before`, because what it held may
have been the user's, unless an earlier install recorded it and it still held
exactly that. A reinstall keeps the record and adds to it. Nothing below a
folder RABBIT couldn't list before the run is recorded, links are never
followed, and nothing here can fail an install. Only the JAWS scripts declare
folders so far: everything in each JAWS version's `Settings` folder under
`%APPDATA%\Freedom Scientific\JAWS`, where the language folders are, and
`%APPDATA%\REAPER\UserPlugins`. The JAWS scripts' uninstall removes the files
recorded as new directly in a folder below a `Settings` folder, such as a
language folder, as long as they are unchanged (see Safety Behavior); files
deeper down stay.

The JAWS scripts' installer always runs elevated, and JAWS keeps running
meanwhile and creates files of its own in the same folders, which are new
too. So after an installer that runs elevated on Windows, a new file is
recorded only when Administrators, SYSTEM or TrustedInstaller own it: the
elevated installer's new files normally belong to Administrators, JAWS's to
the user. Any other answer, a failure to read the owner included, leaves the
file out. The owner is read at install time, while it still tells who
created the file: a restore copies files back, and the copies belong to
whoever restored them, so a check at uninstall time would keep every restored
file. A changed file is recorded as above whoever owns it, since writing over
a file keeps its owner. Owners don't always tell the two apart: with UAC
turned off both run with full administrator rights, and a policy can make the
account that created a file its owner even when it ran elevated. The rule
then records every new file, as without it, or leaves the installer's files
out too. Compared with no rule, it only ever leaves files out: it records
fewer files, never more, so an uninstall removes fewer files, never more. A
new file it leaves out that an earlier install recorded keeps that record, so
it goes only if it still holds exactly what that install left.

## Package Model

Use a manifest-driven package system so future packages can be added without
rewriting the installer engine.

> **TODO (docs): properly document the manifest schema.** The manifest
> (`crates/rabbit-core/embedded/packages/builtin-packages.json`) has grown a
> data-driven `github_release` block (spike branch
> `spike/data-driven-github-provider`) that lets a GitHub-release plugin be
> added/maintained with a manifest entry alone — no Rust. Once that direction
> is adopted, write a proper schema reference covering: every `PackageSpec` /
> `EmbeddedPackageSpec` field; the `github_release` block (`repo`, `release`
> = `"latest"` | `{ "tag": … }`, `version_from` = `tag_name` | `asset_name`,
> the `assets` selectors with `arch` / `name_prefix`+`name_suffix` / `exact_name`,
> `artifact_kind`, `install_destination`); which packages stay "snowflakes"
> (bespoke provider enums) and why; and a worked "add a new package" example.
> Keep it close to the manifest (e.g. a `builtin-packages.md` next to the JSON)
> so contributors find it.

```text
PackageSpec {
  id,
  display_name_key,
  display_description_key,
  package_kind,
  required,
  recommended,
  requires_user_acknowledgement,
  min_reaper_version,
  supported_platforms,
  supported_architectures,
  latest_version_provider,
  artifact_provider,
  detectors,
  install_steps,
  uninstall,
  backup_policy
}
```

Every package also carries a required `uninstall` block saying what removing
it means. It holds a list of plans, and the first plan matching the platform
and the target (`standard` or `portable`) wins. Each plan picks one strategy:
`remove_files` (RABBIT moves the package's own files into a backup set, or
on macOS whole bundles to the Trash or through the administrator command),
`vendor_uninstaller` (RABBIT runs the vendor's uninstaller silently and checks
the result), `manual` (RABBIT changes nothing and explains how to remove it by
hand) or `not_supported` (RABBIT changes nothing and says why). Paths in a
plan start with a token such as `{resource}`, `{clap_dir}` or
`{program_files}`, or, in macOS plans only, are absolute paths below
`/Applications`, `/Library/Application Support` or a
`/Library/Audio/Plug-Ins` format folder. The rules are checked when the
manifest loads: every supported platform and target has a plan and every
plan is reachable, each strategy has the fields it needs, removals that need
administrator rights never go into the user's backups, and no path may reach
into `{resource}/RABBIT`. `rabbit uninstall` carries out `remove_files`
and `vendor_uninstaller` plans (see Safety Behavior), and so does the wizard's
Uninstall tab. OSARA, SWS, ReaPack, ReaKontrol, FFmpeg, app2clap, the JAWS
scripts and the language packs have `remove_files` plans. FFmpeg and the
language packs set `requires_receipt`, because a copy RABBIT has no receipt
for is most likely the user's own; the others name their files, so a copy
without a receipt can go too, unless ReaPack still manages it. OSARA's
Windows plan also has a `registry_cleanup` entry: once the files are gone,
the engine removes `HKCU\...\Uninstall\OSARA`, but only while its
`UninstallString` names a program directly in this resource folder's `osara`
folder, compared ignoring case and by real path, so an entry that belongs to
another REAPER stays. The entry is read only when the run gets there, never
while the plan is built, and one that can't be read or removed is a problem
in the report, not a reason to put the files back. A portable REAPER on
Windows has a `manual` plan whose note holds the instructions. The JAWS
scripts' plan is described below. On Windows, Surge XT (Inno Setup) and a
standard REAPER (NSIS) have `vendor_uninstaller` plans with
`elevation: required` and `backup: none`. Each names its uninstaller only
through an HKLM `locate` value, where it must sit
(`must_be_in`: Surge's exact `{program_data}/Surge XT/uninstall` folder holding
only `unins000.*`, REAPER's install folder strictly below `{program_files}`,
whose HKLM `InstallLocation` must equal `{app_dir}`), its silent arguments,
a `timeout_seconds` of 600, and `verify_gone` paths that must be gone
afterwards. REAPER's also lists 1223 in `success_exit_codes`: the NSIS UAC
plugin (v0.2.4) its uninstaller bundles sets that exit code whenever it runs
elevated, as it always does under the helper, and REAPER's script never
sets it back, so the code says nothing and `verify_gone` and the registry
entry decide; a run that ends so with REAPER still there is reported like
exit 0, as finished but still installed, never as an error code. A plan
that lists such codes must have `verify_gone`, and may not list 0 or, for
NSIS, 1. Surge's `remove_leftovers` lists the `SurgeXTData` junctions,
removed as links, then its vendor folders, only when empty. They go also
when Windows finishes the removal at the next restart, since nothing would
remove them afterwards; a vendor folder still holding a bundle that waits
for the restart isn't empty and stays. Surge's
`kept_user_data` is `{documents}/Surge XT`. REAPER's `kept_user_data` is
`{resource}`, shown as kept and never removed; the engine's warning about
installed packages that depend on REAPER says its extensions stay and work
again once REAPER is back, so the manifest adds no warning of its own. A
vendor plan's summary says whom the uninstaller takes the package from, so
Surge XT gets no separate machine-wide warning.

On macOS neither has an uninstaller RABBIT can use, so both have
`remove_files` plans whose `remove.trees` name whole bundles. A tree leaves
whole, to the Trash or through the run's administrator command, never into a
backup set (see Safety Behavior). REAPER's plan holds for standard and
portable targets alike, with `elevation: if_needed` and `backup: to_trash`.
Its one tree is `{app_path}`, which must end in `.app`, contain "REAPER" and
carry the bundle ID `com.cockos.reaper`, and its `kept_user_data` is
`{resource}`, which always stays. Surge XT's has
`elevation: required` and `backup: none`, and lists eleven trees: its two
apps first, then its Audio Unit, CLAP and LV2 bundles,
`/Library/Application Support/Surge XT`, and its two VST3 bundles last, the
`Surge XT.vst3` that detection reads at the very end. Its
`blocking_programs` are `Surge XT`, `Surge XT Effects` and `surge-xt-cli`,
which ships inside the app. `macos_pkg_forget` spells out the installer
receipt IDs of today's Surge XT (`org.surge-synth-team.…`) and those Surge
XT 1.0.x left behind (`com.surge-synth-team.…`). A `refresh_audio_units`
post step follows, and its `kept_user_data` lists the patches under
`{documents}` and the settings and any copy of the factory data under
`{home}/Library/Application Support`. All of them are shown as kept and
never removed, not even with Also remove settings ticked or
`--remove-settings`, since the administrator command never takes the user's
data. `requires_standard_install` keeps a portable target away from it, as on
Windows. The checks when the manifest loads keep root narrow. A
`remove_files` plan with any elevation is macOS only and may name nothing
to remove but `remove.trees`: never files one by one, never anything a
receipt names. Windows' one exception, a plan-level `machine_cleanup` (the
JAWS scripts', below), isn't plan elevation: that plan stays
`elevation: never` with `backup: move`, its files go into a backup set as
the user, and only the cleanup runs in the elevated helper. An elevated
plan's `kept_user_data` is kept from root by the planner, not by these
checks: when the administrator command takes the package, that data is only
listed as kept, while an `if_needed` app that goes to the Trash as the user
lets Also remove settings move it into a backup set first.
`elevation: required` deletes for good, so it needs
`backup: none`, and each of its trees must be an absolute macOS path
directly in `/Applications`, `/Library/Application Support` or one of the
`/Library/Audio/Plug-Ins` format folders (VST3, Components, CLAP, LV2),
never a token such as `{home}` or `{resource}`; one in `/Applications` must
be an `.app`. `elevation: if_needed` is only for one `{app_path}` tree with
`must_end_with: ".app"` and a `bundle_id`, on a `to_trash` plan; a
`to_trash` plan moves exactly one tree, and a `bundle_id` is allowed only
there, since the administrator command's deletes never read it. Program
names in a plan for macOS hold only letters, digits, spaces, `_` and `-`,
since they become a pattern for `pgrep`. Receipt IDs, allowed only on an
`elevation: required` plan, start with a letter or digit and hold only
letters, digits, `.`, `_` and `-`. `rabbit packages`
prints each package's plan per platform and target, and `rabbit components`
says for each package whether RABBIT can uninstall it, and if not, why.

The JAWS scripts' Windows plan is a `remove_files` plan with
`elevation: never` and `backup: move`, for a standard REAPER only
(`requires_standard_install`). A standard REAPER's resource folder is
`{appdata}/REAPER`, where the scripts' installer writes whatever the target.
`remove.paths` names both DLLs in `{appdata}/REAPER/UserPlugins`; the receipt
lends the 64-bit one its hash. `remove.vendor_added_files` names the pattern
`{appdata}/Freedom Scientific/JAWS/*/Settings/*/*`, the files directly in
any folder below a JAWS version's `Settings` folder, such as a language
folder (files deeper down are never removed), and the key of a note on
removing by hand the script files that stay. The plan-level
`machine_cleanup` names what the installer leaves for its own uninstaller,
which removes nothing and which RABBIT never runs: the folder
`{program_files_x86}/Reaper_JawsScripts`; its files in the order they are
deleted, `Uninstall.dat`, then `Uninstall.exe`, which detection looks for;
the HKLM key `Reaper_JawsScripts` in the 32-bit view, with its two values,
`UninstallDirectory` naming the folder and `Uninstall.dat` naming that file;
and the key of the steps to remove the scripts by hand.
`{program_files_x86}` is Windows' known folder for 32-bit programs
(`FOLDERID_ProgramFilesX86`), never read from the environment. Unlike
`{program_files}`, which stands for both Program Files folders, it is that
one folder, so a folder of the same name in the native Program Files is
never considered. `machine_wide_warning: false` turns off the engine's own
sentence that a package needing a standard REAPER serves every REAPER and
other programs, which is wrong for scripts only JAWS reads. On a portable
target the plan is out of reach, since it removes something, and the
planner shows the `machine_cleanup`'s by-hand steps instead.

The checks when the manifest loads keep both new parts narrow.
`vendor_added_files` is only for a Windows `remove_files` plan of a package
that needs a standard REAPER, with at least one pattern and a by-hand note
key. Each pattern starts with `{appdata}` or `{localappdata}` and spells out
the folder after it, which may not be `REAPER`: that rule, not the snapshot
roots, keeps the patterns out of the resource folder, since the JAWS scripts'
roots include `{appdata}/REAPER/UserPlugins`. No name in a pattern is RABBIT
or ends in a dot or a space, which Windows drops. Each pattern lies below an
`install_snapshot` root, with at least one more name, written the same way up
to there ignoring case, a `*` in the root matching only a `*`: RABBIT
records added files only below those roots. A `*` may stand for a whole name
at any level. `machine_cleanup` is only for a Windows `remove_files` plan
with `elevation: never` and `backup: move` of a package that needs a standard
REAPER. Its folder is one exact folder below `{program_files_x86}`; its
files are plain names, each once ignoring case; its key is in HKLM, in the
32-bit view; each of its values names the folder or one of its files, and
exactly one names the folder, through which RABBIT checks the folder; and it
has a by-hand key. `machine_wide_warning: false` is only for a package that
needs a standard REAPER.

Every package ships a `display_description_key` that resolves to a one- or
two-sentence localized description of what the package is and why a user
might want it. The wizard surfaces the description in the package details
pane (the same spot that today shows the handling/version block); the CLI
exposes it via `rabbit packages` so non-technical users can read about
REAPER, OSARA, SWS, ReaPack, ReaKontrol, and the JAWS scripts before
deciding what to install.

`requires_user_acknowledgement` is set on packages whose upstream policy or
licensing posture asks for an explicit acknowledgement before install — see
the ReaPack donation rule below. Default is `false`. When set, RABBIT must
not start the install for that package until the user has confirmed the
package-specific acknowledgement message in the GUI (a dedicated wizard
page) or in the CLI (an interactive prompt or an explicit
`--accept-<package>-notice` flag for unattended runs).

Initial package kinds:

- `reaper_app`: vendor installer, dmg/app copy, or portable creation.
- `user_plugin_binary`: copy one or more extension binaries into `UserPlugins`.
  Some packages ship a single extension DLL (OSARA, SWS, ReaPack, ReaKontrol);
  others ship a runtime bundle whose entire `bin/` folder lands in
  `UserPlugins` (FFmpeg). The install pipeline supports both shapes via the
  artifact kinds `ExtensionBinary` (raw single-DLL download), `Archive` (zip
  carrying a single matching binary), `DiskImage` (macOS DMG), and
  `SevenZipArchive` (the FFmpeg `.7z` whose every `bin/<file>` is flattened
  into `UserPlugins`). A single receipt covers all extracted files for the
  multi-file case.
- `keymap`: copy into `KeyMaps`, optionally replace `reaper-kb.ini` with
  backup.
- `reapack_package`: install/update through ReaPack later, once ReaPack is
  present and REAPER has been launched.
- `screen_reader_scripts`: a screen reader's scripts, which their own
  installer puts into the user's screen-reader settings (the JAWS scripts'
  installer picks the JAWS version and language folder under
  `%APPDATA%\Freedom Scientific\JAWS`, such as `2025\Settings\enu`).
  Platform-gated: a package of this kind only appears in the wizard when the
  relevant screen reader is available on the host.

For the initial supported package set, RABBIT should implement these unattended
strategies:

- REAPER Windows standard install:
  - download the official installer,
  - verify signature and version,
  - invoke it itself with unattended arguments for standard installation,
  - wait for completion and treat non-zero or unexpected exit codes as failure,
  - verify `reaper.exe` and the target resource path after completion.
- REAPER Windows portable install:
  - either invoke the official installer itself with unattended portable-install
    arguments or use another validated vendor-supported unattended method,
  - wait for completion and treat non-zero or unexpected exit codes as failure,
  - verify both `reaper.exe` and `reaper.ini` in the selected portable folder.
- REAPER macOS standard install:
  - download the official disk image or app distribution,
  - verify signature/notarization,
  - mount or extract it non-interactively,
  - copy/install REAPER unattended into `/Applications` or the chosen target,
  - verify the final app bundle and version.
- REAPER macOS portable install:
  - create the portable folder layout unattended,
  - place the REAPER app bundle there using a verified unattended copy flow,
  - create or preserve `reaper.ini` as required for portable mode.
- OSARA:
  - install unattended by either invoking a validated silent installer path or
    reproducing the upstream file layout directly into the selected REAPER
    resource path,
  - manage the keymap behavior as a RABBIT choice: replacement plus backup
    when it loses nothing of the user's, keep otherwise, and either one on
    request.
- SWS:
  - install unattended by placing the correct verified binary into
    `UserPlugins` for the selected REAPER architecture.
- ReaPack:
  - install unattended by placing the correct verified binary into
    `UserPlugins` for the selected REAPER architecture.
  - mark the package with `requires_user_acknowledgement = true`. Whenever
    a RABBIT run would install or update ReaPack, the wizard must show a
    dedicated confirmation page before the Review step, and the CLI must
    prompt before staging the artifact. The page reproduces the donation
    hint visible on <https://reapack.com/> in the active locale: ReaPack is
    free software released under the terms of the LGPL, but its author
    Christian Fillion accepts donations at <https://reapack.com/donate> to
    support continued development. The page links to the donation URL,
    states clearly that donating is optional and that no donation is
    required to use ReaPack or RABBIT, and only enables Continue once the
    user has explicitly acknowledged the notice (a checkbox or focused
    button in the GUI; an interactive prompt or
    `--accept-reapack-donation-notice` flag in the CLI for unattended
    runs). Skip the page entirely on runs where ReaPack is neither being
    installed nor updated.
- ReaKontrol:
  - install unattended by placing the correct verified binary into
    `UserPlugins` for the selected REAPER architecture or platform,
  - preserve existing `reaKontrol/fxMaps` user mapping data and treat it as
    user content, not package-owned files.
- JAWS scripts (Windows only):
  - latest-version + artifact provider: HFS REST API at
    `hoard.reaperaccessibility.com`. RABBIT calls
    `POST /~/api/get_file_list` with the JAWS scripts' folder and picks the
    `.exe` installer with the highest version in its file name. Its download
    URL comes from the same listing.
  - install by running the scripts' own NSIS installer with administrator
    rights, interactively rather than with `/S`: a plugin inside it opens a
    dialog that silent mode hides without dismissing, so a silent run hangs.
    The installer picks the JAWS version and language folder itself and
    copies the script files into
    `%APPDATA%\Freedom Scientific\JAWS\<version>\Settings\<language>`, such
    as `2025\Settings\enu`. It always puts `Reaper_ComAccess32.dll` and
    `Reaper_ComAccess64.dll` into `%APPDATA%\REAPER\UserPlugins`, whatever
    the target, and leaves `Uninstall.dat` and `Uninstall.exe` in
    `%ProgramFiles(x86)%\Reaper_JawsScripts`, with an entry
    `Reaper_JawsScripts` in HKLM's 32-bit list of installed programs, through
    which detection finds the scripts. RABBIT then checks that the 64-bit
    DLL is there and, where that entry was there before, that the installer
    wrote `Uninstall.exe` anew.
  - the package only appears in the wizard package list on Windows when JAWS
    is detected on the host. macOS users never see the JAWS scripts row.
  - RABBIT copies no script file itself, so it backs none up: the installer
    writes over them in place. The receipt names the 64-bit DLL, and the
    install snapshot records which files the installer added or changed in
    the JAWS settings folders and in `UserPlugins` (see the receipt above),
    for the uninstall.
- FFmpeg (Windows only, opt-in):
  - latest-version provider: Gyan.dev's `*.ver` plain-text endpoint at
    `https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z.ver`,
    a single line of UTF-8 with the current stable release version.
  - artifact provider fans out per architecture:
    - x64 → Gyan.dev's `ffmpeg-release-full-shared.7z` redirector. The URL
      is fixed; the version pulled from the `*.ver` endpoint is the
      authoritative version stamp.
    - ARM64 / ARM64-EC → highest stable tag from
      `tordona/ffmpeg-win-arm64`'s GitHub releases (skipping the
      `daily-autobuild-…`/`latest` rolling tags) whose major matches the
      supported FFmpeg major; asset
      `ffmpeg-<ver>-full-shared-win-arm64.7z`.
  - install unattended by extracting every file under the archive's `bin/`
    folder into the selected REAPER resource path's `UserPlugins`
    (skipping `include/`, `lib/`, `doc/`, …). The user's FFmpeg-aware
    plugins (REAPER's video decoder) load from the same folder. A single
    RABBIT receipt records every extracted file so future detection
    rounds-trip cleanly.
  - opt-in by default (`recommended = false`); doesn't surprise users who
    don't need video decoding.
  - macOS is filtered out at the manifest level until a scriptable
    shared-`dylib` source materializes. OSXExperts.net and evermeet.cx
    ship static binaries only, which can't satisfy REAPER's video decoder.
  - external-install detection prefers spawning
    `<UserPlugins>/ffmpeg.exe -version` (with the
    `CREATE_NO_WINDOW` flag on Windows so users don't see a console
    flash) and parsing the first stdout line; falls back to the
    libavformat-major filename heuristic when no executable is present.

## Configuration Steps

Configuration steps are post-install actions that run *after* the
package install pipeline finishes, typically to wire newly-installed
packages into REAPER's per-target config files. They are
manifest-driven so future steps can be added without rewriting the
engine.

```text
ConfigurationStep {
  id,
  display_name_key,
  display_description_key,
  recommended,
  requires_package_id,   // dependency package id, or None
  kind                   // tagged enum of step kinds (see below)
}
```

Initial step kinds:

- `AddReapackRemote { name, url }`: append a remote into ReaPack's
  `<resource>/reapack.ini` `[remotes]` section. Idempotent on URL —
  re-running is a no-op when the URL is already listed; the
  appended index honours ReaPack's `size=` field so it lands at the
  next slot ReaPack itself would have written.

The single shipped step today is the REAPER Accessibility ReaPack
remote (`https://github.com/Timtam/reapack/raw/master/index.xml`),
recommended when ReaPack is part of the install plan or already on
disk.

Wizard surface:

- Configuration steps appear in their own "Configuration" group at
  the top level of the Packages tree, sibling to the "Packages"
  group. Each step's row label is its localized display name; rows
  whose dependency package isn't installed or queued show
  `(requires <package>)` and rows whose effect is already on disk
  show `(already applied)`. Both states render the row as disabled
  with the checkbox forced unchecked, and click/Space input on those
  rows is pre-empted before native processing reaches the
  TVS_CHECKBOXES auto-cycle so screen readers don't announce a
  phantom toggle.
- Toggling the dependency package live (e.g. unticking ReaPack)
  re-evaluates the configuration row's availability and, if the row
  becomes actionable again, restores its recommended-default tick.
- The row's accessible description and the wizard's details pane
  carry the longer "Unavailable: requires X to be installed."
  / "Already applied on this REAPER target." sentence so the user
  hears the full reason on focus.

CLI surface:

- `rabbit setup` accepts `--config-step <id>` (repeatable allowlist)
  and `--skip-config-step <id>` (skip-list overlaid on the
  recommended-default set). With no flags, every recommended step
  whose dependency is satisfied and which isn't already applied runs
  automatically.
- `apply_configuration_step` returns a per-step report whose status
  is one of `Applied`, `Skipped`, `SkippedDependencyMissing`, or
  `DryRun`; the report stitches into the same `SetupReport` JSON
  the package install pipeline writes.

Detection:

- "Already applied" detection is per-`kind`. For
  `AddReapackRemote`, RABBIT scans the `[remotes]` section of
  `<resource>/reapack.ini` for an entry whose URL field matches.
  Missing `reapack.ini` reads as "not applied". The detection is
  cheap enough to run during model build and after every target
  reselection, so the wizard's "(already applied)" tag stays in sync
  with whichever target the user picks.

## Install Targets

Resource path layout:

```text
<resource_path>/
  reaper.ini
  reaper-kb.ini
  UserPlugins/
  KeyMaps/
  ReaPack/
    registry.db
  reaKontrol/
    fxMaps/
  osara/
    locale/
  RABBIT/
    install-state.json
    logs/
    backups/
```

Extension files:

- OSARA Windows: install all available supported OSARA DLLs, matching the
  upstream installer behavior:
  - `UserPlugins/reaper_osara32.dll`
  - `UserPlugins/reaper_osara64.dll`
  - `UserPlugins/reaper_osara_arm64ec.dll` where supported
- OSARA macOS:
  - `UserPlugins/reaper_osara.dylib`
- SWS:
  - install the binary matching the selected REAPER architecture, unless the
    upstream package explicitly supports installing multiple side-by-side
    architecture files.
- ReaPack:
  - install the binary matching REAPER architecture, not merely the operating
    system architecture.
- ReaKontrol Windows:
  - `UserPlugins/reaper_kontrol.dll`
- ReaKontrol macOS:
  - `UserPlugins/reaper_kontrol.dylib`
- ReaKontrol support data:
  - preserve `reaKontrol/fxMaps/` and any user-created map files during
    install and update.
- FFmpeg Windows (x64 and ARM64; macOS not yet supported):
  - every file under the upstream archive's `bin/` directory is flattened
    into `UserPlugins/`. For a current Gyan / tordona FFmpeg 8.x build
    that's `avcodec-62.dll`, `avdevice-62.dll`, `avfilter-9.dll`,
    `avformat-62.dll`, `avutil-60.dll`, `postproc-58.dll`,
    `swresample-6.dll`, `swscale-9.dll`, `ffmpeg.exe`, `ffprobe.exe`,
    `ffplay.exe`. Keeping the executables alongside the libs lets the
    detection pass invoke `ffmpeg.exe -version` for the authoritative
    release version on subsequent runs.

## Update Flow

1. Discover installation candidates.
2. Detect current component state.
3. Refresh latest-version metadata from providers.
4. Build an install plan.
5. Show the plan and require confirmation.
6. Download artifacts into the RABBIT cache:
   - Windows: `%LOCALAPPDATA%\RABBIT\cache`
   - macOS: `~/Library/Caches/RABBIT`
7. Verify artifacts:
   - HTTPS only.
   - SHA-256 when known.
   - Authenticode signature for Windows executables and DLLs where available.
   - macOS code signing/notarization checks where available.
8. Ensure REAPER is not running.
9. Create backups.
10. Apply changes unattended:
   - invoke verified silent REAPER install steps where required,
   - launch installer executables or equivalent package routines directly from
     RABBIT where the package model says so,
   - copy verified extension files directly where possible,
   - use temp files and atomic rename where possible.
11. Write receipt and report.
12. Verify the final installed state against the plan and report any mismatch as
    an installation failure, not a silent warning.

## Safety Behavior

- If REAPER is running, stop before changing extension files and ask the user to
  close it.
- When OSARA is selected, back up `reaper-kb.ini` and replace it with the OSARA
  key map by default only when OSARA is new or `reaper-kb.ini` still matches the
  installed OSARA key map; otherwise keep it, and point at OSARA's "Merge OSARA
  key map" action. The user can override either way with the Packages checkbox
  or `--replace-osara-keymap` / `--preserve-osara-keymap`.
- Do not overwrite or delete user-created `reaKontrol/fxMaps` content during a
  ReaKontrol install or update.
- RABBIT's own file installs and uninstalls back up the files they replace
  or remove under `RABBIT/backups/<id>/`, named by what made them:
  `unix-<s>` for an update, `uninstall-unix-<s>`,
  `restore-unix-<s>-before-restore`, and `unattended-unix-<s>-<n>` for the
  key map OSARA's key map replaces, or `-2` and so on when a set already has
  that id. Earlier versions also wrote an `unattended-unix-<s>-<n>` set
  holding only a copy of the receipt after vendor installers ran
  (`unattended-receipt-update`); RABBIT writes none any more, and no longer
  copies the receipt into an update's set either. Vendor installers, and
  OSARA's macOS archive, overwrite files with no backup.
  Files from inside the resource folder keep their relative path in the
  set; files from outside it (app2clap's CLAP folder) go under
  `external/<n>/`, beside a `<file>.rabbit-origin` note of their original
  path. `<n>` is one above the highest number the set's manifest or its
  `external` folder already uses, so an outside file never lands on
  another's place, whatever a put-back took out of the set. When the
  `external` folder can't be listed, the file is refused, and an origin
  note or a copy is never written over a file the set already holds,
  inside the resource folder too.
  Each manifest entry is saved, through a temp file and a rename,
  before its copy starts and marked done after, so an interrupted run
  still leaves a set whose manifest names every copy in it. A set an
  update wrote isn't restored yet (see below). Every path a set records to
  its copy is absolute, so a set written through a relative
  `--resource-path` restores from any folder; older sets that recorded
  relative paths are read by their `RABBIT/backups/<id>` part. A file from
  outside the resource folder records its absolute original path, and a
  file inside it its path relative to the resource folder. Restore puts
  each file back at its recorded path and takes the install lock. Outside
  the resource folder it writes only below a folder RABBIT installs into
  itself (the CLAP folder), for any package, or where one of the entry's
  own package's `vendor_added_files` patterns matches; another package's
  file never goes there. For a pattern,
  the folder before its first `*` (`%APPDATA%\Freedom Scientific\JAWS` for
  the JAWS scripts) must exist, nothing that exists below it on the way to
  the file may be a link or junction, the file included, and the deepest of
  those that exists must lie, by its real path, inside that folder's real
  path. That folder itself, and what lies above it, may be reached through
  a link. Below the CLAP folder the same holds, except that the folder may
  be missing: nothing below it can be a link then, and the restore makes it
  again, as an install would. Below either, the file goes back to that
  folder joined with the names below it, the same path the check looked
  at, never the recorded spelling, which may be a `\\?\` one. Inside the
  resource folder, the file's own place must not be a link, and the
  deepest part of the way to it that exists must lie, by its real path,
  inside the resource folder's real path and outside `RABBIT`'s, as
  uninstall's guard has it; a link that stays inside is followed.
  Anywhere, a name with a `:`, or on Windows one ending in a dot or space
  or naming a device such as `CON` or `COM1.clap`, is refused, since the
  file system could open it as another name or as the device. A set that
  names one place twice, by names the file system reads as the same, is
  refused: the second file would replace the first, and the before-restore
  set would lose the file that was there. The temporary file a restore
  writes first is made new, so a link at its name stops the restore, and
  only a regular file at the target is backed up. A path refused stops the
  restore before anything changes, with the reason, such as that JAWS's
  folder is gone; in a whole set, the message adds that the set's other
  packages can be restored one at a time with `--package` when another
  package has a file in the set. An entry whose copy doesn't come back is
  left out and never refused, since nothing is written for it.
- Restore uses a set id only when it is one plain name, with no `:`,
  slash or NUL, and on Windows no trailing dot or space and no device name
  such as `CON`. It must also equal, byte for byte, a folder `read_dir`
  lists in `RABBIT/backups`. When only the case differs, the error names
  the set that is there, and so it does when the id names a set once its
  dots and spaces at the end are gone, as when it was copied with the
  period after it; neither is restored. `RABBIT`, `RABBIT/backups` and the
  set must be real folders. Restore and `rabbit backups` never follow a
  link or junction there or inside a set: a link or a badly named folder
  is listed with a warning and never looked into. So no run writes a set
  while `RABBIT` or `RABBIT/backups` is a link. An uninstall refuses the
  whole run before anything changes (the `backup-folder` check), and so
  does an install: the install preflight has the same check, failing in a
  dry run too, so setup's first step, which makes RABBIT's folders, stops
  before it makes any; the package operation (the wizard's install,
  `setup`, `apply-packages`) and `install-extension --apply` check it
  before the lock or any download. Should the folder become a link in
  between, the package operation usually finds it before the writer does:
  it runs the install preflight again for each extension file it installs,
  and that stops the rest of the run with the `backup-folder` error, as a
  REAPER started mid-run does; what installed before it stays installed
  and in the receipt. Past those checks, the writer refuses every write to
  a set while `RABBIT`, `RABBIT/backups` or the set's own folder is a link
  (`BackupPathIsLink`), its first write and every later one, before
  anything is written. Then an update stops before it replaces the file,
  and its staged copy goes again. OSARA's key map replacement leaves
  `reaper-kb.ini` as it is, while OSARA itself is installed: its item says
  only that OSARA was installed, not that the key map was replaced, and
  the receipt doesn't list `reaper-kb.ini` among OSARA's files. A restore
  stops at the first file it would back up, with the files before it
  restored, as on any failed copy. An uninstall's package fails, and what
  of it already moved goes back where it can.
- A restore never copies a receipt file over `RABBIT/install-state.json`:
  an old copy would undo every change to the receipt since, uninstalls
  included. Restore puts no file from a set back anywhere in `RABBIT/`,
  under any name Windows or macOS would open as it: in another case, and on
  Windows with trailing dots or spaces, or a `:` stream. Only the merge
  below changes the receipt. Before anything changes, in a dry run too, it
  refuses a set an update wrote (`install-replacement`), whole or with
  `--package`, because its files belong to the receipt entry from before the
  update, until RABBIT can go back to an earlier version by changing only
  that package's entry. It also refuses a set that holds only a receipt copy
  (`unattended-receipt-update`), and any other set that holds a copy of the
  receipt, by its manifest or, without one, by its layout, such as the set a
  restore of an update's set wrote in earlier versions. The copy stays in
  the set for anyone who needs it by hand. No run copies the receipt into a
  set any more.
- What a set records besides its files, in fields that are optional in the
  manifest, so `schema_version` stays 1, older manifests load and restore as
  before, and older versions ignore the fields:
  - Every set: `created_unix`, the seconds since 1970 (UTC) when its folder
    was made. `created_at` stays the id RABBIT asked for, which the folder
    lacks when it got `-2`.
  - An update's set (`install-replacement`): `replaced`, for each package
    whose files it holds, the package's receipt entry from before the
    update, or none when the receipt had none (a copy RABBIT didn't
    install), and the version that replaced it. Each record is saved before
    the package's first file is copied in, so the set never holds a file
    without it; when that first copy fails, the record goes again, and so
    does a set left empty. One install call writes one set: the wizard one
    per package, `install-extension` one for all its packages. The records
    aren't in `packages`, which older versions read as an uninstall's
    whatever the set's reason: they would merge the old entry back while
    only the replaced files came back.
  - An uninstall's set: in `packages`, each package's receipt entry, the
    `langpack=` value (below), and `with_settings`, whether this uninstall
    moved the package's settings into the set: `false` when it records the
    package, `true` once a settings file has moved, and `false` again when
    a failed package's settings all went back. `None` in older sets.
  - A key map set (`osara-keymap-replacement`): `keymap_origin`, what
    `reaper-kb.ini` was, read before OSARA's install, which overwrites the
    OSARA key map it is compared with: `own-keymap`, `unchanged-osara`,
    `default-keys` (a `reaper-kb.ini` without key bindings or custom
    actions, which still goes into a set) or `unknown` (no OSARA key map to
    compare with, or one of the two unreadable). A value a later version
    writes reads as `unknown`.
  - A before-restore set: its files only.
- Restoring a set an uninstall wrote merges each package's recorded entry
  into the current receipt instead, the package comes off
  `declined_packages`, and the `langpack=` value its uninstall replaced goes
  back, but only while `reaper.ini` still says `<>`. `restore-backup
  --package` does this for one package of the set. A package installed
  again since its uninstall is refused, with nothing changed, because its
  newer receipt entry describes newer files; so is `--package` for any other
  set, which records no receipt entry in `packages`. An update's set keeps
  its entries in `replaced` instead, which no restore merges: it is refused
  whole and with `--package` (`EarlierVersionNotYet`). A receipt entry that is
  still the very one the set recorded is no reinstall: the uninstall stopped
  before it reached the receipt (RABBIT was ended halfway through the
  package, or a failed package's files couldn't all go back), or an earlier
  restore from the set put the entry back. Its files are restored and the
  entry stays as it is.
- An uninstall records each package before it moves anything, and keeps the
  record when it found none of the package's files to move, or stopped
  before any copy of them finished. A package has a file in the set when
  the restore would put back one of its entries, by the rule it follows for
  every entry: a finished entry, or a pending one whose copy is complete.
  `--package` for a package with none is refused (`NothingOfItInSet`),
  before anything changes and in a dry run too. A whole-set restore leaves
  such a package out: its entry isn't merged, it stays on
  `declined_packages`, its `langpack=` value isn't put back, and the report
  names it under `skipped_packages`. A newer receipt entry for it doesn't
  count as a reinstall, so it refuses nothing, and neither does a place one
  of its entries couldn't go back to. A set with no file at all to put back
  is refused like `--package`, as a set holding only a receipt copy is,
  rather than reported as restored. This is decided before the lock. Under
  it, the manifest is read again and must be the same, and the plan must
  agree, or the restore stops with nothing changed, as when an uninstall
  wrote more of the set in between.
- A real restore first copies each file it replaces into a set of its own,
  `restore-unix-<s>-before-restore` (or `-2` and so on), written like any
  other, and its report names that set as `before_restore_set_id`.
  Restoring it puts those files back; files the restore added, the
  receipt and REAPER's language stay as they are. A restore that replaced
  nothing writes no set and names none, and neither does a dry run. Every
  real restore that finishes saves its report as
  `RABBIT/logs/restore-backup-unix-<s>.json` and `.txt` (or `-2` and so on,
  never over another report), still under the install lock, as every real
  uninstall does. One stopped by an error
  saves none yet; `rabbit backups` still lists its before-restore set. A
  dry run saves nothing; `--save-report` saves its report under a free name
  the same way, and `--report-path` writes where it says. An uninstall's
  report is saved the same way, never over another.
- A real restore on macOS refuses when RABBIT itself runs as root, as under
  `sudo`, as an uninstall does: what it puts back, the before-restore set
  and the receipt would all be root's. The refusal comes after the set is
  found and the set and package refusals, before the REAPER check, the
  install lock and any change. A dry run still works. The check comes
  through the restore's environment, so tests run it on every platform.
- `rabbit backups` lists the sets through one overview in the core
  (`backup_overview`), which the wizard's Backups tab is to share. It turns
  sets into backups: one per package an uninstall recorded; one per package
  of an update's set, from its file entries and its `replaced` records; one
  key map, found by an entry whose original is the resource folder's
  `reaper-kb.ini`, whatever made the set; one per package a restore's set
  recorded as replaced; one for each other set, set without a manifest,
  damaged set, unfinished deletion in `RABBIT/backups-deleting`, and link
  or badly named folder; one for a set's files that no backup above holds;
  and one for all sets that hold only a copy of the receipt. Backups are
  grouped as uninstalled software, earlier versions,
  key maps and other backups, newest first inside a group, those without a
  date last and the records last of all. A set's date is its manifest's
  `created_unix`, else the seconds in its `created_at`, else, without a
  manifest, the seconds in its name; every id form counts, `-2` suffixes
  included. Its size comes from a walk by each entry's own metadata, which
  follows no link and counts files with no entry, so it is what deleting it
  frees; a backup's size adds up its manifest entries. Each backup gets a
  status (can restore, stopped partway, already back, installed again, not
  complete, nothing in it, version unknown, another of its exclusive group
  installed, part missing, records unreadable, not here, not yours, not a
  backup), and earlier versions are judged by the rules of the swap that is
  to restore them, although `restore-backup` refuses them for now. The
  overview reads the manifests, the receipt once, an older update set's
  copy of the receipt, and sizes; it never hashes a file. "Not complete"
  and "already back" compare the hashes the manifests and the receipt
  recorded and the sizes on disk; the restore checks the files themselves.
  Where a file goes back is judged from the paths' text by the restore's
  own rules, and only such a place is looked at: a path outside the
  resource folder and the folders RABBIT installs into, such as one on
  another computer, is never opened. A backup with a file a restore would
  refuse names it, and gets no restore command. A key map is restored on
  the command line only with its whole set, so only while it is all that
  comes back of the set. On macOS a set whose owner differs
  from that of `RABBIT/backups` is another account's. Each set also gets a
  fingerprint, its manifest's length and modified time or, without one, its
  file count and size, for a later run to compare under the lock. Local time
  comes from the operating system on Windows and macOS, UTC elsewhere.
- `rabbit install-extension --apply` takes the install lock across its
  download, install and receipt change, as the package operation does, so
  it never writes files, a backup set or the receipt while another RABBIT
  installs, uninstalls or restores on the same REAPER.
- Uninstall removes only regular files a RABBIT receipt lists with a hash,
  files the package manifest names, and files the receipt records the
  package's installer adding where the plan's `vendor_added_files` patterns
  match them (below). A macOS plan's `remove.trees` are different: those
  bundles leave whole, by the routes described below, and are never hashed,
  put into a backup set or put back. `reaper.ini`, every `reaper-*.ini` such
  as `reaper-kb.ini` and `reaper-extstate.ini` (where every extension keeps
  its settings), copies of them, and everything in `{resource}/RABBIT` always
  stay, and every path passes a guard against links, `..` and whole folders,
  once when the plan is built and again right before it moves. Before anything
  moves, every file of every package is read and hashed, as the dry run does,
  so a file that can't be read stops the whole run with nothing changed. A
  file only the receipt names that changed since RABBIT installed it stays and
  is reported. The move reuses that hash only for a file last changed well
  before it was read and whose size and modification time are still the same;
  any other file is hashed again, so FFmpeg's 250 MB is read once. While
  ReaPack is installed, a settings file its records claim stays, even with
  `--remove-settings`. Files move into one backup set per run, by rename on
  the same drive, or from another drive as a copy that is checked before the
  original is deleted, after a free-space check. A file another program holds
  open is in use, for every package, with nothing moved: on the same drive
  Windows refuses the rename with a sharing or lock violation, and from
  another drive the copy goes again when the original can't be deleted. The
  package's files then go back, and the message asks to close that program;
  for a file the package's installer added, which only the JAWS scripts have
  and JAWS itself may hold open, it says to wait and run RABBIT again, and to
  restart JAWS first if that doesn't help, rather than to close the user's
  screen reader. The target's REAPER must be closed, and every REAPER for
  packages that serve the whole computer; those checks run again before each
  package, or on macOS for all the packages of the administrator command
  together, right before its window. A package that serves the whole computer
  can't be removed from a portable target when its plan removes anything; a
  `manual` or `not_supported` plan says the same on every target. One
  requested package that can't be removed makes the whole run change nothing.
  When a package's removal fails, its files go back and its receipt entry
  stays; when it works, the entry goes and the package joins
  `declined_packages`. The set records each package's receipt entry, and the
  `langpack=` value REAPER had when the removal set it back to no language
  pack, never a copy of `reaper.ini`. A removed package's report item names
  the set only when the set records the package and holds a file of it that
  a restore would put back, by the restore's own rule (see above). One that
  put nothing there is removed with no set named, even when other packages
  of the run filled one: a package of the administrator command whose trees
  were gone before root's turn, one whose apps were gone before their turn
  to go to the Trash and whose settings stayed out of the set, or one that
  moved no file. A set that ends the run holding only package records, with
  no file and no copy of the receipt, is removed again, so a run whose
  packages all moved nothing names no set at all.
- A plan's `vendor_added_files` patterns pick files from the receipt's
  `vendor_added_files` records, never from a folder listing. A receipt is a
  file anyone can edit, so only a plain absolute path counts, and only where
  a pattern matches it: exactly as many names as the pattern, literal names
  compared ignoring case, and no name holding a `:`, which would name an
  alternate data stream. Records outside every pattern are passed by
  without a word, such as the DLLs the same snapshot records in
  `UserPlugins`. One file may be recorded under two spellings, so the
  records are grouped by path, ignoring case. A file any record marks
  `existed_before` stays and is listed as kept, because it was there before
  the installer ran and may be the user's own; so does one whose records
  disagree on its hash. Neither goes even with `--remove-settings`. Any other
  file still there is a candidate with the hash its records agree on, and
  the run checks that hash right before the move, so a file changed since,
  such as `reaper.jkm` with the user's keys, stays and is reported, like a
  file only the receipt names. Every candidate passes the guard below the
  folder before the pattern's first `*` (`%APPDATA%\Freedom Scientific\JAWS`
  for the JAWS scripts), so a junction there that leads elsewhere blocks the
  whole package. No folder goes with the files, not even an empty one: JAWS
  made most of the language folders, and nothing RABBIT still has can tell
  which ones the installer made, since a restore changes owners. With a
  receipt that records no file under the patterns as new from the
  installer, as from an install made before RABBIT recorded them, a note
  says all those files stay; in every state the package can be removed in,
  another note says how to remove by hand the files that stay. The backup
  set keeps these files under `external/<n>/`, like every file from outside
  the resource folder, and a restore puts them back only under the package's
  own patterns (see above).
- A package whose vendor's uninstaller removes it (Surge XT and a standard
  REAPER, on Windows) is removable only once RABBIT found that uninstaller
  through HKLM and every trust check passed, read only, while the plan is
  built. Once every check of the run has passed and before the first
  change, the run starts one hidden elevated helper, RABBIT itself under a
  hidden command, for every such package and every planned machine cleanup
  (below): the run's only Windows permission prompt. Declined, every
  package is Cancelled and nothing changes; if the helper can't start, the
  packages that need it fail and nothing changes either. The helper takes
  orders only from the RABBIT that started it, over a named pipe both ends
  check by process ID, runs only the packages on its command line, each
  once and in order, from the built-in manifests, and repeats every check
  itself from HKLM and the real folders, never from HKCU or a per-user
  folder: over-the-shoulder elevation runs it as another account. Its
  machine-wide folders don't come from its environment either; ProgramData
  is read from HKLM with Windows' own drive, and Program Files (x86) is
  Windows' known folder. rabbit.exe takes the DLLs it imports from System32
  only (`/DEPENDENTLOADFLAG:0x800`), and the helper loads later ones the
  same way, so a DLL put next to a portable RABBIT in Downloads can't run
  with administrator rights. Replacing rabbit.exe itself stays possible
  for whoever can write to its folder; the permission prompt then shows no
  verified publisher. REAPER's NSIS uninstaller runs from
  a copy in a new folder under ProgramData that only administrators and
  SYSTEM can write to, checked against the original by hash while both are
  locked; Surge's Inno uninstaller runs in place after its folder is checked
  again, holding nothing but administrator-owned `unins000.*` files. Neither
  vendor signs its uninstaller, so there is no signature to check. Every
  REAPER on the computer must be closed, and the programs the plan names;
  REAPER is always removed last. The status follows the uninstaller and
  fresh detection: exit 0, or a code the plan's `success_exit_codes` lists,
  with everything gone is Removed; with files
  Windows deletes at the next restart, RemovedPendingRestart, even while
  detection still finds the package through those very files; with
  leftovers, which are listed and never deleted, RemovedWithLeftovers; a
  package still detected otherwise is Failed and keeps its receipt entry;
  NSIS's exit 1 is Cancelled; any other code is Failed. Afterwards the
  receipt follows detection: the entry goes, and the package joins
  `declined_packages`, only once the package is gone. An uninstaller still
  open after `timeout_seconds` is left running, the package Failed and the
  run stopped; so is one RABBIT lost track of after it started, because the
  helper stopped answering. Either may still be at work, so the receipt
  stays as it was. A working folder RABBIT couldn't remove, or a Windows
  list it couldn't read afterwards, is reported as a problem; a list it
  couldn't read is never taken to mean the package is still listed. If
  `reaper.ini` or RABBIT's
  receipt is gone after a vendor's uninstaller ran, the run stops, the
  package is Failed, and the receipt is written again from the copy the run
  holds in memory. The helper is told to quit at the end of every run.
  Nothing these uninstallers remove can be restored from a backup.
- The helper has a second kind of plan: a `machine_cleanup`, which deletes
  what an installer left in machine-wide places to find its own uninstaller,
  in place of running one. Only the JAWS scripts have one. While the plan is
  built, `VendorLookup::machine_cleanup` checks the folder and the key read
  only, the way the helper checks them again: the folder must be exactly the
  one the manifest names below `{program_files_x86}`, which comes from
  Windows' known folder, never from the environment or the registry; if it is
  there, it is reached through no link, owned by Administrators, SYSTEM or
  TrustedInstaller, and holds nothing but the manifest's `files`, each a
  regular file with such an owner. The key is read in HKLM's 32-bit view only;
  if it is there, it has no subkeys and only the manifest's values, each a
  plain `REG_SZ` naming exactly its expected path ignoring case, and the value
  naming the folder is among them. A value that holds no text counts as one
  RABBIT doesn't expect, and a `REG_EXPAND_SZ` as one that points elsewhere.
  There is no signature check: the scripts' `Uninstall.exe` isn't signed, and
  nothing runs it; ownership stands in, as for vendor uninstallers. A check
  that fails makes the package CleanupNotTrusted, with the reason and the
  manifest's by-hand steps, and nothing of it is planned; so does a lookup
  that can't look, such as `NothingListed`. With neither folder nor key there,
  no cleanup is planned: no prompt, no question, and the files still go. With
  no receipt and none of the files the plan names for this account (without a
  receipt no script file is a candidate, so that means neither DLL), the
  package is NoFilesForThisAccount and left alone, because the folder and key
  then most likely belong to another account's install, whose files would be
  left without them; with a receipt, the cleanup is planned even when no file
  is left. A planned cleanup sets the plan item's `machine_cleanup` and
  `confirm` and makes `needs_elevated_helper()` true, so it shares the run's
  one permission prompt with vendor uninstallers. Its route stays the backup
  set, `unbacked` stays false, and `requires_elevation`, which means macOS's
  administrator command, stays false. RABBIT sends the helper only the package
  ID; the helper takes the folder, files and key from its own built-in
  manifest, only for a standard REAPER, and answers with `cleanup-finished`
  instead of `finished`. The cleanup runs only after every file of the package
  is in the backup set, so nothing a backup can't bring back happens unless
  all of that worked. It checks everything again, then deletes the files in
  the manifest's order, the folder once it is empty and the key last, and
  stops at the first failure. So the key outlives the rest, and
  `Uninstall.exe`, which detection looks for with the key, goes last of the
  files; with a receipt, a package whose cleanup failed has its files back and
  stays detected through the receipt, so the uninstall can be run again.
  Something already gone counts as removed. Right before each file and the
  folder it checks again that the folder is reached through no link and, for a
  file, that an administrator still owns it; a refusal there while nothing is
  gone yet is a refusal, later a failed step. Refused or lost puts every file
  back, fails the package and stops the run: something changed after planning,
  or what the helper did is unknown. Failed puts the files back and keeps the
  receipt entry, even when something is already gone, and the run goes on;
  what is already gone and what is still there are in the report. With nothing
  there any more, the package is removed as if no cleanup were planned. When
  the cleanup finishes and removed anything, the package counts as removed:
  its receipt entry goes, and a receipt that can't be saved then is a problem
  in the report, with nothing put back, and the receipt in memory stays
  without the entry so a later package's save writes that too. The dry run
  asks no helper and lists what the plan found. Every delete with
  administrator rights goes through `VendorEnvironment`'s three delete
  methods, `remove_file_no_follow`, `remove_empty_dir` and
  `delete_uninstall_entry`: the lookup's read-only environment refuses all
  three, the test environment deletes only inside its fixture folder and never
  in HKLM, the cleanup refuses Windows' real list in rabbit-core's test
  builds, the in-process test helper refuses machine-wide folders outside the
  temporary folder, and `rabbit-platform`'s registry delete accepts only
  HKLM's real list or an HKCU root `Software\RABBIT-test-*`, and refuses HKLM
  in its own tests. A backup can't bring the folder and key back.
  `restore-backup` brings back the files, and the receipt entry when there was
  one. Detection then finds the scripts through that entry, as long as the
  64-bit DLL has the size the entry records, and while the folder and key stay
  gone a later uninstall needs no prompt. Without an entry nothing finds the
  restored files: the scripts' only other detector needs the key and the
  `Uninstall.exe` the cleanup removed.
- On macOS a plan's trees leave whole. Surge XT's, its bundles and
  `/Library/Application Support/Surge XT`, are deleted for good by the
  run's administrator command. REAPER.app leaves by one of three routes,
  decided while the plan is built from file facts read as the user
  (`tree_route`): to the Trash as the user, into the user's Trash through
  the run's administrator command, or by hand. The read before the run,
  and a dry run, check each tree with the guard and see that it is there,
  but never list, hash or walk it, and the space check and the write
  rehearsal count only files that still go into a backup set. The routes
  decide the run's order: the packages of the administrator command
  first, in removal order, REAPER among them when only root can move its
  app; then the rest in removal order, REAPER last. The report, and the
  progress page as the run reaches each package, follow that order.
  Review, and the list the progress page starts with, name the ticked
  packages in the Uninstall tab's order.
- A REAPER.app this account may move goes to the Trash as the user, with
  no password, through `NSFileManager`'s `trashItemAtURL`. "May move" is
  read the way macOS's Trash function needs it: the right to take the
  name out of its folder (macOS's own `_DELETE_OK` check, which covers
  ACLs, the sticky rule and the folder's flags), to add names to that
  folder and search it, and to write the bundle, whose `..` changes. Right
  before it moves, the guard and that question run again, and the answer
  must still be the user. It counts as moved only once its old place is
  empty and macOS named where it went; a refusal is TrashFailed, anything
  less certain TrashUnclear and stops the run, and whatever of the package
  went into the backup set goes back on either. An app only root may move
  goes into the user's Trash through the administrator command, but only
  when it sits directly in `/Applications`, has the bundle ID the manifest
  names, neither it nor `/Applications` is Locked, and the user's Trash,
  found from the passwd entry and never from `$HOME`, is a real folder the
  user owns on the app's volume. Anything else is left to
  the user, with the reason and Finder steps, and can't be ticked: a
  Locked app, another bundle ID or none, an app only root may move
  anywhere else, no Trash RABBIT can check, a Trash on another volume, or
  a target app path that isn't there. The resource folder always stays.
- On macOS the run's one administrator command is its first change.
  `osascript` asks for the password and runs root's shell in one blocking
  call, and a second call asks again, so every root step of the run goes
  into one call, made after every check RABBIT makes as the user and
  before anything else changes. Before it, as the user: the
  cancel token; the process checks for these packages; the administrator
  guard on every path root deletes or moves; for REAPER's app, the
  question of who may move it, which must still say only root, and the
  check of the user's Trash; `pkgutil --pkgs`, for the receipts still
  listed; and the steps themselves, which must be ones the administrator
  shell takes. Anything that stops the run there stops it with nothing
  changed and no window. When everything root would delete or move is
  gone already and none of these packages' receipts is still listed,
  there is no window either; a receipt still listed, or a listing that
  failed while a package has receipts to forget, still opens one for
  `pkgutil --forget`. A cancelled window cancels every package of the
  run, those that needed no administrator included, and changes nothing;
  a window that fails, or a command that never ran, fails the packages
  that needed it, leaves the rest unreached and changes nothing either.
  Root gets only these steps, each program by its absolute path on the
  sealed system volume:
  1. `/usr/bin/pgrep -x -i -- <names>`, which must find nothing: the plans'
     `blocking_programs`, plus `REAPER` and `reaper64` when a package needs
     every REAPER closed (Surge XT, or a REAPER.app root moves), in case one
     was started while the window was open.
  2. For each package that deletes, `/bin/rm -R -f -x <path>` for each
     tree in manifest order, exactly the path the guard checked moments
     before, then `/usr/sbin/pkgutil --forget <id>` for each of its
     receipts the listing still showed (all of them when listing failed).
     A forget that fails doesn't stop the command.
  3. For a REAPER.app only root can move, `/bin/test -d` and
     `/bin/test ! -L` on the user's Trash, then `/bin/mv -h -n` of the app
     into it under a fresh name: its own name with the local date and time
     from just before the password window, such as
     `REAPER 2026-11-07 15.30.12.app`, so it never takes the name of
     something already there.
  Every word is POSIX single-quoted inside one AppleScript string, with no
  script file, no `with prompt` text and the output silenced. A step that
  fails, other than a forget, stops the command with its own status, 101
  for the first step, so RABBIT knows where it stopped; statuses end at
  254, which caps one command at 154 steps. RABBIT waits for `osascript`
  on a helper thread, so it can say after 30 seconds that it is still
  waiting.
- After the command, the filesystem decides every package's status, read
  with `lstat` as the user; the exit status only says where it stopped. A
  package whose paths are all gone is Removed, whatever the status,
  unless it is an app root was to move whose move may not have run and
  that RABBIT can't see at its destination: that is TrashUnclear, since
  something else may have moved it. One partly gone is Failed, with what
  is gone and what is left; one with nothing gone is Failed if its step
  stopped the command, and the packages after it weren't reached.
  `rm -R -f` deletes all it can inside a tree before it fails on a
  folder, so a tree it stopped in may be partly gone though still there,
  and the report says so. Root's move into the Trash is claimed only when
  something is at the destination and nothing of the app's name is inside
  it, because BSD `mv` moves into a folder already there. A package not
  fully removed stops the run there; nothing else has changed, since the
  command came first. What root removed never goes back. Receipt
  entries follow fresh detection, the installer's receipts are listed again
  and any still listed are reported as harmless, and once anything of a
  package is gone its `refresh_audio_units` step ends the user's
  `AudioComponentRegistrar` so hosts read the Audio Unit list again; a
  refresh that fails is never a problem. "Operation not permitted" from a
  step, which root gets only from macOS's own protections such as App
  Management, or a refused move to the Trash, adds the advice to turn RABBIT
  on under System Settings > Privacy & Security > App Management (from the
  CLI, the app it runs in, such as Terminal).
- Root never deletes or moves anything reached through a link, a folder
  another volume is mounted on, or anything outside the fixed system
  folders. The one other path it is given is the user's Trash, where
  REAPER.app goes. RABBIT checks it as the user just before the window,
  with one `lstat` of the Trash itself but not of the folders above it: a
  real folder, not a link, owned by the user, on the app's volume. Root
  checks it again with `test -d` and `test ! -L` before the move. Before
  the window, every path root deletes or moves must sit directly in its
  system folder (`/Applications` for REAPER.app), spelt exactly as that
  folder and its name, so a trailing slash that would make `lstat` and
  `rename` follow a link never gets in; be a real folder, not a link or a
  file; be reached through real folders only, each read with `lstat` from
  `/` down; be on the same volume as the folder holding it, or it is
  refused as `MountPoint`, because `rm -x` stays out of a volume mounted
  below the folder it is given but empties one mounted on that folder;
  and be valid UTF-8. Surge XT's trees go in a fixed order, so that a
  command that stops partway leaves something the next run can finish.
  Its apps go first: App Management can refuse even root at another
  developer's app, and the command then stops before any plug-in. Its
  VST3 bundle goes last: detection finds Surge XT by it, so a command
  that stops partway leaves Surge XT detected, with its receipt entry,
  and offered again.
- A real macOS uninstall refuses before anything, the install lock
  included, when RABBIT itself runs as root, as under `sudo`: the checks
  of what the user may do, the Trash and the backup set would all be
  root's. A dry run still works. The wizard saves no report for that
  refusal, and the CLI says so in words of its own and exits with 1.
- A real run takes the install lock even where the resource folder doesn't
  exist yet, as for a REAPER.app that was never opened, which creates the
  folder: the app outside it can still go, and the receipt and the report
  are saved there. Folders made only for the lock go again when the run
  changed nothing, so a mistyped `--resource-path` leaves no trace.
- `rabbit uninstall` only shows what it would do unless given `--apply`, and
  needs every package named with `--package`; there is no "remove
  everything". Its dry run names the packages that need administrator
  rights and says Windows asks once, or on macOS that macOS asks once, in a
  window that may name osascript. Its dry run runs the same checks, a
  running REAPER included, so it says what `--apply` would say. A macOS dry
  run also lists, below each package, the steps root would get, from the
  same checks, and names the programs root first checks are closed; it
  records them in the report and never runs them. For REAPER's move it
  shows only the Trash folder, since a real run names the app by its own
  time, taken just before the password window. With `--apply` on macOS,
  the CLI says on stderr when the password window is coming and when it is
  still open after 30 seconds, because the window names osascript and the
  CLI otherwise says nothing until the run is over. `rabbit uninstall` has
  no `--allow-reaper-running`, unlike every command that installs: a
  running REAPER holds its extensions' files open and writes `reaper.ini`
  again when it closes. Its exit code is 2
  when a package asked for can't be uninstalled and nothing changed, 1 when a
  removal failed or was stopped or the permission prompt was declined (on
  macOS, the password window cancelled), and 0
  otherwise, packages that aren't installed and removals that finish after a
  restart included. Package settings stay unless `--remove-settings` is
  given; then they move into the same backup set. Those of a package that a
  vendor's uninstaller or, on macOS, the administrator command removes stay
  even then and are only listed as kept, since neither puts anything into a
  backup set. With `--apply`, the first Ctrl+C asks the run to stop before
  the next step, through the same cancel token the wizard uses: the next
  package, or on macOS the administrator command, which covers several. A
  second Ctrl+C ends RABBIT at once. On macOS `osascript` runs in a
  process group of its own, so Terminal's first Ctrl+C reaches only
  RABBIT's group and can't end the password window or root's steps
  halfway; RABBIT reads the stop once `osascript` has returned. While
  `osascript` runs, the second Ctrl+C is passed on to its group as SIGINT
  before RABBIT ends, so no password window is left open with nobody
  waiting for its answer. Root's shell in that group gets it only where
  macOS lets RABBIT signal it.
- Keep a machine-readable operation report and a plain text report.
- Treat non-writable targets as a planning error before downloading anything.
- Do not request elevation unless the selected REAPER app install target
  requires it, or a selected vendor uninstaller, machine cleanup of an
  uninstaller folder or registry entry that is still there (the JAWS
  scripts'), machine-wide removal, or REAPER.app that only an administrator
  can move to the Trash does.
- Do not delete unknown files during update. Only remove files listed in a RABBIT
  receipt or explicitly owned by the package manifest.
- Closing the window during an install or an uninstall stops the run; it
  does not abandon it. Ask first, with **No** as the focused button, then
  let the pipeline stop at its next package boundary so every guard unwinds
  the way it does on a clean finish: the install lock is released,
  extraction temp dirs are removed, and the run writes a report saying which
  packages landed and which were never reached. Whatever step is running
  when the user confirms runs to completion — an elevated vendor installer
  has no halfway point to return to, and an uninstall finishes the package
  it started, or on macOS the administrator command — so the wizard says it
  is finishing that step rather than
  pretending the stop is instant.
- Refuse to close at all while RABBIT is replacing its own program files.
  That swap has no unwind path, so there is no answer that makes quitting
  safe; say so instead of offering a choice that cannot be honoured.

## Localization

Use message IDs from the first commit. Recommended Rust-side choice: Fluent via
`fluent-rs`. Required built-in locales should be embedded into the executable.
During development and for advanced overrides, RABBIT may also read locale files
like:

```text
locales/
  en-US/
    rabbit.ftl
```

Rules:

- No string concatenation for user-visible messages.
- The default release must work without a `locales/` directory.
- Include translator comments for placeholders.
- Localize logs that are shown to users, but keep an internal structured event
  code for support.
- Include accessible names, descriptions, dialog titles, and button labels in
  localization.
- Keep package display names localizable, but package IDs stable and
  untranslated.

## Suggested Repository Structure

```text
Cargo.toml
crates/
  rabbit-core/
  rabbit-platform/
  rabbit/                # the single user-facing binary (CLI + GUI dispatch)
ui/
  wxdragon/
    xrc/
locales/
  en-US/
  de-DE/
  es-ES/
  fr-FR/
  it-IT/
  pt-BR/
docs/
  architecture/
tests/
```

`rabbit-core` has no GUI dependency. `rabbit-platform` isolates Windows/macOS
APIs. The `rabbit` binary crate depends on both and contains:

- `cli/` — the clap-based subcommand parser (former `rabbit-cli` content).
- `ui/` — the wxDragon wizard (former `rabbit-ui-wxdragon` content), behind a
  `gui` Cargo feature for dev-loop builds that skip native deps.
- `main.rs` — the argv dispatcher that picks GUI or CLI mode.

This layout lets another native UI shell replace the wxDragon module without
touching the package engine. Files under `locales/` and `ui/` are
source/development assets; release builds embed required resources instead
of shipping those directories beside the executable.

## Testing Strategy

Automated tests:

- manifest parsing
- version comparison
- path discovery from fake filesystem fixtures
- ReaPack SQLite registry parsing
- receipt read/write
- install planning
- backup and rollback
- embedded-resource availability without external files
- release packaging checks for accidental runtime file dependencies
- GitHub Actions workflow validation for push builds and tagged releases
- release-manifest generation and checksum publication
- RABBIT self-update version comparison and channel selection
- self-update staging, verification, rollback, and restart handoff logic

Manual accessibility tests:

- Windows 11 with NVDA
- Windows 11 with Narrator
- Windows 11 with JAWS before beta
- macOS current release with VoiceOver

Install tests:

- clean Windows standard install
- clean Windows portable install
- clean macOS standard install
- clean macOS portable install
- update from older REAPER plus older extensions
- unattended REAPER Windows standard install end-to-end
- unattended REAPER Windows portable install end-to-end
- unattended REAPER macOS standard install end-to-end
- unattended REAPER macOS portable install end-to-end
- unattended OSARA install end-to-end
- unattended SWS install end-to-end
- unattended ReaPack install end-to-end
- unattended ReaKontrol install end-to-end
- GitHub Actions push build produces downloadable Windows and macOS artifacts
- GitHub release workflow publishes release assets, checksums, and update
  metadata
- launch RABBIT from a temporary folder with no neighboring resource files
- existing user key map preserved
- OSARA key map replacement with backup
- ReaPack already installed with populated registry
- existing `reaKontrol/fxMaps` user maps preserved
- extension installed manually with unknown version
- RABBIT self-update from one released version to the next on Windows
- RABBIT self-update from one released version to the next on macOS

## Open Questions

- Select and validate the exact unattended invocation strategy for the REAPER
  Windows installer for both standard and portable targets, including exit
  codes, logging, and upgrade behavior.
- Select and validate the exact unattended install strategy for REAPER on macOS:
  mounted DMG copy flow, packaged installer flow, or another vendor-supported
  non-interactive path.
- Confirm SWS and ReaPack macOS binaries expose reliable version metadata
  outside ReaPack's registry DB. If not, RABBIT receipts and ReaPack DB should be
  treated as the reliable sources.
- Confirm whether ReaKontrol release binaries expose reliable version metadata
  on Windows and macOS. If not, RABBIT receipts plus package-file presence should
  be treated as the reliable sources.
- Decide whether the RABBIT update feed should be a GitHub release asset JSON
  generated by `release.yml`, a repository-hosted appcast/manifest file, or
  both.
- Validate the exact Windows self-update replacement mechanism for a running
  single executable: temporary helper executable, script, or another minimal
  relaunch approach.
- Validate the exact macOS self-update replacement mechanism for a signed and
  notarized app bundle without breaking code signing, quarantine, or app path
  stability.
- Decide how stable and beta RABBIT release channels should be represented in the
  GitHub release/update metadata and in the UI.
- Decide whether first-version RABBIT should install SWS directly from SWS
  release assets or through an unattended ReaPack-driven path after ReaPack is
  present. The design target remains unattended either way.
- Build a small wxDragon proof of concept and test it with NVDA, Narrator, and
  VoiceOver before expanding it into the full wizard.
- Verify whether wxDragon exposes the wxWidgets accessibility hooks RABBIT needs
  directly. If not, document the smallest upstream contribution or local wrapper
  needed for accessible names, descriptions, roles, and state.
- Verify wxDragon/wxWidgets release packaging on Windows and macOS can meet the
  one-download, no-RABBIT-installer goal without sacrificing code signing,
  notarization, or screen-reader behavior.
- Add a Windows-side live-upstream smoke pipeline analogous to
  `macos-smoke.yml`: download FFmpeg per arch (Gyan x64, tordona ARM64),
  extract the `bin/` payload into a temp UserPlugins, assert the receipt
  round-trips, and round-trip the detection back to the upstream version.
  Until that lands, FFmpeg's per-arch resolver is exercised only by unit
  tests against fixture release-list bodies, not against real GitHub /
  Gyan responses.
- Find a scriptable source of shared (`dylib`) FFmpeg builds for macOS so
  the FFmpeg row can be enabled there too. OSXExperts.net and
  evermeet.cx ship static binaries only, which can't satisfy REAPER's
  video decoder. Candidate fallbacks include bridging to Homebrew
  (detect `brew` on `$PATH` and run `brew install ffmpeg` on opt-in) or
  hosting our own shared builds.
- Confirm the macOS uninstall on real Macs, Apple Silicon and Intel, with
  VoiceOver. In particular: that each system folder root deletes or moves
  from reports the same volume as the bundles in it, `/Applications` above
  all, since macOS firmlinks it to the data volume, or the `MountPoint`
  check refuses every Surge XT removal and every root move of REAPER.app;
  that root's `pgrep -x` finds `Surge XT Effects`, whose name is exactly
  the 16 characters the kernel keeps of a program's name; and what root's
  `rm` does when a second Ctrl+C ends RABBIT while it runs.
