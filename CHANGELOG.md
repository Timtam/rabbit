# Changelog

All notable changes to RABBIT are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## How to update

Add entries under `## [Unreleased]` as part of each change. Use these
subsection headings (omit any that are empty):

- `### Added` — new features
- `### Changed` — changes to existing behavior
- `### Deprecated` — soon-to-be-removed features
- `### Removed` — now-removed features
- `### Fixed` — bug fixes
- `### Security` — security fixes

To cut a release, run the **prepare-release** workflow from the GitHub
Actions tab (workflow_dispatch) with the version `X.Y.Z` as input. It will:

1. Validate that `[Unreleased]` is non-empty and the tag is unused.
2. Bump the workspace `version` in `Cargo.toml` (and refresh `Cargo.lock`).
3. Insert `## [X.Y.Z] - YYYY-MM-DD` below `## [Unreleased]`, leaving a
   fresh empty `## [Unreleased]` on top.
4. Commit, tag `vX.Y.Z`, push both, and dispatch the release workflow.

The release workflow then extracts the section matching the tag's version
from this file and posts it as the GitHub release body.

## [Unreleased]

### Changed

- macOS: ship a single universal Mach-O instead of separate Apple Silicon
  and Intel builds. The release pipeline now builds both arches and
  `lipo -create`s them into one binary, published as
  `rabbit-<version>-macos-universal` (and `…-universal.app.zip`). The
  self-update manifest's `platforms` map keeps its `macos-aarch64` and
  `macos-x86_64` keys for backward compatibility with already-released
  RABBIT 0.1.0 clients, both pointing at the universal artifact, so
  existing installs migrate to the fat binary on their next self-update
  check without a manual download.

### Fixed

- macOS: install no longer aborts with
  `no artifact found for sws on MacOs/Universal` (and the analogous
  silent ReaPack-arm64 mis-install on Intel hosts) when REAPER is a
  universal Mach-O. The artifact dispatcher now canonicalizes
  `Architecture::Universal` to the host slice on macOS before
  per-package resolvers run, so SWS picks the matching `Darwin-x86_64`
  / `Darwin-arm64` `.dmg` and ReaPack picks the matching
  `reaper_reapack-x86_64.dylib` / `reaper_reapack-arm64.dylib`. On
  Apple Silicon Macs running RABBIT under Rosetta, the dispatcher
  consults `sysctl.proc_translated` and forces the `arm64` slice so
  plug-ins match the `arm64`-native REAPER process rather than the
  `x86_64` translator's view.

- macOS: VoiceOver now reads the German UI with a German voice. The
  bundle previously declared only English (`CFBundleDevelopmentRegion`
  with no `CFBundleLocalizations` and no `.lproj` directories), so
  Cocoa picked the English voice for every accessibility string
  regardless of the in-app language. The bundle now ships
  `CFBundleLocalizations` for `en` and `de`, matching empty
  `en.lproj` / `de.lproj` stubs, and seats `AppleLanguages` from the
  resolved runtime locale before wxDragon brings up Cocoa so the
  override also takes effect for users on an English-language macOS
  who switch RABBIT to German.

## [0.1.0] - 2026-05-09

Initial public release. RABBIT is a REAPER accessibility bootstrap and
bundle installer with a screen-reader-friendly GUI wizard and a matching
CLI, packaged as a single self-contained executable per platform.

### Added

- **Accessibility-first wizard** built on wxDragon: keyboard-first
  navigation, native controls, tested with NVDA, JAWS, Narrator, and
  VoiceOver. Wizard runs when launched without arguments; arguments hand
  off to the CLI.
- **Cross-platform builds**: Windows x86_64, Windows aarch64, macOS
  aarch64, and macOS x86_64. macOS ships as an ad-hoc-signed `.app.zip`
  with an `Open Me First.command` helper that clears Gatekeeper
  quarantine on first launch.
- **Localization**: English (en-US) and German (de-DE) bundled, with
  Fluent-based runtime locale selection. Auto-picks the OS language when
  a translation is available.
- **REAPER discovery**: detects standard installations on Windows and
  macOS, plus user-supplied portable folders. Reports app path, resource
  path, version, architecture, writability, and confidence.
- **Package install / update** for REAPER, OSARA, SWS, ReaPack,
  ReaKontrol, JAWS-for-REAPER scripts (Windows + JAWS only), and FFmpeg
  shared runtime (Windows, opt-in; pulled from gyan.dev on x64 and
  tordona/ffmpeg-win-arm64 on ARM64, pinned to FFmpeg 8.x).
- **Architecture-aware artifact resolution**: selects x86_64 / aarch64 /
  arm64ec / universal builds appropriate to the detected REAPER, with
  per-arch self-update assets in the release manifest.
- **Configuration steps**: post-install tweaks managed alongside package
  installs. Ships with `reapack-add-reaper-accessibility-remote`, which
  adds the REAPER Accessibility ReaPack repository
  (`Timtam/reapack`) and is idempotent on re-runs.
- **Preflight checks** that run before any apply: REAPER process state,
  resource path writability, donation acknowledgement for ReaPack, etc.
  CLI returns a non-zero exit when checks fail.
- **Dry-run by default in the CLI**: every install / update / restore
  command requires `--apply` to make changes. JSON output available on
  every command via `--json`.
- **Backups and rollback**: each install writes a backup set under
  `<resource>/RABBIT/backups/<timestamp>/`; `rabbit backups` lists them
  and `rabbit restore-backup` rolls one back.
- **JSON + text reports** under `<resource>/RABBIT/logs/` for every
  install, update, restore, and setup operation.
- **Self-update**: signed update manifest published alongside each
  GitHub release with per-arch URLs and SHA-256 sums; `rabbit
  self-update check / stage / apply [--restart]` (CLI) and an automatic
  on-startup check (GUI). Handles bare-binary replacement on Windows and
  macOS without the user re-clearing Gatekeeper.
- **Portable runtime**: the binary carries embedded resources (locales,
  package manifest). Cache and lock files live under the OS cache directory by
  default; nothing is left behind in the resource path beyond explicit
  reports and backups.
