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
  fixed system paths outside any portable REAPER folder. On Windows, close
  Surge XT before you install or update it: RABBIT doesn't start its
  installer while Surge XT is open, and says so.
- **app2clap** *(Windows only, opt-in, standard REAPER installations only)* —
  [app2clap](https://app2clap.jantrid.net/), a CLAP plugin by Jamie Teh
  (jcsteh, of OSARA/NVDA fame) that captures audio from other applications
  and brings it into REAPER — or any CLAP host — as a plug-in you insert on
  a track. RABBIT tracks the
  [rolling `snapshots` release](https://github.com/jcsteh/app2clap/releases/tag/snapshots),
  installs `app2clap.clap` into the per-user CLAP folder
  (`%LOCALAPPDATA%\Programs\Common\CLAP`) with no elevation required, and
  keeps it up to date. Disabled on portable REAPER targets because the CLAP
  folder lives outside any portable REAPER folder.

Surge XT and app2clap are grouped under **Additional software** in the
wizard — extras that aren't tied to REAPER itself. Both are disabled on
portable REAPER targets, since they install to fixed per-user/system
locations rather than into the portable REAPER folder.

### Language packs

RABBIT can also translate **REAPER itself** (not just its own wizard), by
installing a community language pack into `<resource>/LangPack/`. Three are
available today, and all of them translate the SWS extension as well:

- **Spanish** — maintained by Javier Robledo for the Spanish REAPER
  community and published on
  [reaperespa.com](https://www.reaperespa.com/). REAPER and SWS are the
  same translation either way, but **OSARA** has two Spanish translations,
  chosen by the pack's file name: installed as `es_ES.ReaperLangPack` you
  get *REAPER Accesible español*, and as `es_MX.ReaperLangPack` you get
  *Team PMA*'s. Pick one from the **Spanish OSARA translation** dropdown on
  the packages page (enabled once the Spanish pack is selected), or pass
  `--package-variant langpack-es=pma` on the command line. Switching reinstalls under the new name and removes the old file.
- **German** — maintained by MrData and published in the
  [REAPER Stash](https://stash.reaper.fm/v/26248/), installed as
  `de_DE.ReaperLangPack`.
- **French** — maintained by Lee Julien and Pierre-Marie Curt for
  [ReaperAccessible](https://reaperaccessible.fr/), the French
  screen-reader community, and published in their
  [ReaPack repository](https://github.com/reaperaccessible/rap_fr).
  Installed as `fr_CA.ReaperLangPack`, which is what makes **OSARA** load
  its `fr_CA` translation: that one is all but complete, while OSARA's
  `fr_FR` translation still leaves about a third of its messages in
  English. The pack itself is the same standard French under either name —
  only OSARA's own strings differ, and a few of them are Canadian
  (*muté*, *soloté*).

The pack matching the language RABBIT is running in is suggested and
ticked for you; packs for other languages are listed but unticked. Nothing
is offered in English, since REAPER is already English.

If you don't want it, untick it and RABBIT remembers: the pack stays listed
and tickable, but it won't be ticked for you again on this REAPER install.
That covers updates too — if you installed a pack, later stopped using it,
and turn down its next update, RABBIT takes that as a no and stops offering
it. Tick it again at any point and RABBIT forgets the refusal. What is *not*
a refusal is a pack that is simply installed and up to date: it sits
unticked because there is nothing to do, and RABBIT reads that as silence
rather than a decision. The choice is stored per install (in
`RABBIT/install-state.json`), so you can say no on one REAPER and yes on
another.

You can install as many language packs as you like — REAPER keeps them all
in `LangPack/` — and switch between them later from REAPER's own
preferences. Only one is *active* at a time, so if you install several,
pick which one REAPER starts in from the **REAPER language after
installation** dropdown on the packages page (or `--reaper-language
<package>` on the command line). Installing a pack never removes another
one; the only file RABBIT replaces is the one it installed for that same
pack, e.g. when you switch Spanish translations.

The Spanish and German packs are published without version numbers, so
RABBIT identifies them by what the server reports about the file (its ETag
or last-modified date and size, falling back to the file's contents): it
notices when a translator publishes a new file and offers it as an update,
without RABBIT needing a new release. The French pack does carry a version
(`7.75.1` at the time of writing), read from its ReaPack index. Your choice
of Spanish translation is remembered, so an update keeps the variant you
picked instead of reverting to the default.

Beyond installing packages, RABBIT can also apply small post-install
configuration tweaks:

- **Add the REAPER Accessibility ReaPack repository to ReaPack**
  (`https://github.com/Timtam/reapack/raw/master/index.xml`). When
  ReaPack is part of your install plan or already on disk, this step is
  ticked by default; if the repository is already configured in your
  `reapack.ini`, the step shows as *already applied* and is skipped.
  Idempotent and safe to re-run.
- **Set REAPER's language** to one of the language packs you're
  installing, by writing it into `reaper.ini`. A single step covers every
  language — which one it activates comes from the dropdown above. The step
  is tied to the packs you tick: with none ticked it is greyed out, because
  the dropdown it reads from is empty and there would be nothing to
  activate. Tick a language pack and the step ticks itself, ready to be
  unticked if you want the files installed without changing the language
  REAPER is in; untick the last pack and the step clears itself again. A
  pack merely sitting on disk from an earlier run doesn't count — a run
  that installs nothing language-related leaves your current language
  alone. (On the command line the step is still reachable directly with
  `--config-step set-reaper-language`, which activates an already-installed
  pack.) Only the one setting is touched; the rest of `reaper.ini` is
  preserved byte for byte, including its original text encoding.

Built with screen reader users in mind: keyboard-first wizard, native
controls, NVDA/JAWS/Narrator/VoiceOver tested, English, German, Spanish, French
+ Italian UI out of the box. No console window, no installer, no settings file
— one executable you can run from any folder and delete when you're done.

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

If you rename it, keep `rabbit` at the start of the name, in capitals or not,
with nothing, a dash, a space or an opening parenthesis after it, such as
`rabbit`, `Rabbit-cli` or `rabbit(2)`. The last dot in the name and whatever
follows it don't count, so `rabbit.old` works too. Only a copy named like that
asks macOS for an administrator's password or moves anything to the Trash. A
copy with any other name can't install or uninstall Surge XT, and can't
uninstall REAPER.

## Use it

Launch the downloaded executable. The wizard walks you through:

1. **Pick a REAPER target** — RABBIT detects existing standard installs
   automatically; pick "portable" if you want a self-contained REAPER folder.
2. **RABBIT checks for the latest versions** of REAPER and the accessibility
   packages.
3. **Pick the packages** you want installed or updated, plus any
   *configuration* steps (e.g. adding the REAPER Accessibility ReaPack
   repository). Sensible defaults are already checked. The page has two
   tabs, **Install or update** and **Uninstall**, and opens with focus on the
   tabs: press Tab to reach the list, and Shift+Tab from the list to get
   back to the tabs. On a Mac the tabs can take focus only with keyboard
   navigation on (System Settings > Keyboard > Keyboard navigation; on
   macOS 11 and 12, System Preferences > Keyboard > Shortcuts > Use
   keyboard navigation to move focus between controls); without it, the
   page opens in the list.
4. **Review and install.** RABBIT downloads, verifies, and installs everything
   without further prompts.

When it finishes, you can launch REAPER straight from the wizard or open the
saved report.

### Uninstalling from the wizard

To remove packages instead, switch to the **Uninstall** tab on the package
page, tick what should go, and press Next. Nothing is ticked to begin with.

- **Switching tabs:** on Windows, Ctrl+Tab or Ctrl+Page Down from the tabs or
  anything on them, and Ctrl+Shift+Tab or Ctrl+Page Up to go back; the arrow
  keys work while the tabs have focus. On macOS, Ctrl+Tab and Ctrl+Shift+Tab
  anywhere on the page, the arrow keys while the tabs have focus, or
  VoiceOver's own navigation of the tab group. VoiceOver mentions Ctrl+Tab
  the first time the page opens.
- **What RABBIT can remove:** OSARA, SWS, ReaPack, ReaKontrol, FFmpeg,
  app2clap and the JAWS scripts (all three Windows only), the language packs,
  Surge XT and REAPER (see below). On Windows that means a standard REAPER;
  on a Mac, the REAPER app of a standard or a portable REAPER. Every other
  installed package is listed too, with the reason it can't be ticked.
- **What is kept:** your settings, such as SWS's grooves, ReaPack's
  repositories and ReaKontrol's FX maps, unless you tick **Also remove
  settings** below the list. `reaper.ini`, `reaper-kb.ini`, your key maps and
  everything ReaPack installed always stay, and so do the key map backups
  OSARA made and OSARA's settings, which live in `reaper-extstate.ini` beside
  every other extension's. Script files of the JAWS scripts that RABBIT can't
  be sure the installer added, and those changed since, always stay too.
  Everything that goes, settings removed with that option included, moves
  into a backup set under `RABBIT\backups` rather than being deleted. Surge XT
  and REAPER are the exception, and so are the JAWS scripts' uninstaller
  folder and registry entry; see below.
- **Starting it:** the review page says what each package loses and keeps.
  Enter doesn't start an uninstall there; press **Uninstall**. REAPER must be
  closed, and for app2clap, the JAWS scripts, Surge XT and REAPER itself
  every REAPER on the computer. On a Mac, uninstalling REAPER needs every
  REAPER closed only when moving its app takes an administrator's password;
  otherwise closing that REAPER is enough.
- **Surge XT and REAPER (Windows):** their own uninstallers remove them, with
  administrator rights. Windows asks for your permission once, before
  anything is removed, however many of them you ticked; if you don't allow
  it, nothing is uninstalled at all. RABBIT asks you once more before it
  starts, with **No** selected, because a backup can't bring them back. Close
  Surge XT and other music programs first. Surge XT goes for every music
  program on the computer; your patches in `Documents\Surge XT` and the fonts
  it installed stay. REAPER's uninstaller removes the program, and REAPER's
  file associations and ReaRoute for every REAPER on the computer; your
  settings and extensions in `%APPDATA%\REAPER` stay, and so do your
  projects. The extensions work again once you install REAPER again. REAPER
  always goes last. If an uninstaller takes long, the progress page says
  RABBIT is still waiting: it may be showing a question in a window of its
  own, which Alt+Tab finds. If it has finished but its files are still
  there, the progress page says RABBIT is waiting for them: close the music
  program that has the package open. A removal that finishes only after a
  restart says so on the last page. A portable REAPER stays yours to remove
  by hand.
- **The JAWS scripts (Windows):** RABBIT removes them from a standard
  REAPER. Their two plug-in files in `%APPDATA%\REAPER\UserPlugins` move into
  the backup set, and so do the script files RABBIT saw their installer add
  to JAWS's settings folder, `%APPDATA%\Freedom Scientific\JAWS`. Script
  files RABBIT can't be sure the installer added stay, such as those already
  there when RABBIT ran the installer, and so do those that changed since,
  such as `reaper.jkm` with your own key assignments. RABBIT records which
  script files the installer adds when it runs the installer itself, and
  only from this version on, so the script files of an install made before,
  or without RABBIT, stay too. The row's details, and **Show details** on the
  last page, say how to remove the script files that stay by hand. The
  installer also left an uninstaller folder,
  `%ProgramFiles(x86)%\Reaper_JawsScripts`, and an entry in the registry.
  While either is there, RABBIT removes them after the files, with
  administrator rights: Windows asks for your permission once, before
  anything is removed, together with Surge XT and REAPER if you ticked them
  too, and if you don't allow it, nothing is uninstalled. Review says so,
  and RABBIT asks you once more before it starts, with **No** selected,
  because a backup can't bring that folder and entry back. Once both are
  gone, for example after you restored the scripts from a backup, there is
  no permission prompt and no question. If that folder or entry isn't
  exactly as the installer leaves it, RABBIT changes nothing: the row can't
  be ticked, and its details say why and how to remove the scripts by hand.
  JAWS can keep running. If it has one of the script files open, nothing of
  the scripts is removed: wait a moment and uninstall again, and restart
  JAWS first if that doesn't help. If RABBIT has no record of installing the
  scripts and finds none of their files for your account, another account
  most likely installed them: the row can't be ticked, and its details say
  to uninstall them from that account. From a portable REAPER, RABBIT says
  to uninstall the scripts from the standard REAPER, and how to remove them
  by hand.
- **REAPER on a Mac:** RABBIT moves the REAPER app to the Trash, from
  Applications or from a portable REAPER folder. Your settings and extensions
  stay, in `~/Library/Application Support/REAPER` or in the portable folder,
  and so do your projects. The extensions work again once you install REAPER
  again. If your account may move the app, it goes to the Trash with no
  password, after everything else in the run. If only an administrator may
  move it, for example because another account owns it, it sits directly in
  Applications, and your Trash is a folder of your own on the same disk,
  macOS asks for an administrator's password and RABBIT moves it into your
  Trash, with the date and time added to its name, such as
  `REAPER 2026-11-07 15.30.12.app`. It keeps the owner it had, so Finder
  offers no Put Back for it, and emptying the Trash, or dragging it back to
  Applications, may ask for an administrator's password. An app RABBIT can't
  move at all, such as a Locked one, or one only an administrator may move
  that misses any of these conditions, can't be ticked; its details say how
  to move it to the Trash in Finder. RABBIT asks you once more before it
  starts, with **No** selected.
- **Surge XT on a Mac:** Surge XT has no uninstaller of its own there, so
  RABBIT deletes it for good with an administrator's password: its apps, its
  plug-ins in every format and its factory data in
  `/Library/Application Support/Surge XT`. Surge XT goes for every music
  program on the Mac. macOS's installer then forgets Surge XT's receipts, and
  RABBIT has macOS refresh the list of Audio Units that music programs read.
  Your patches in your Documents folder and your Surge XT settings in the
  Library folder of your home folder stay, even with **Also remove
  settings**. A copy of Surge XT you put somewhere else, such as among the
  plug-ins in the Library folder of your home folder, stays too. Close
  Surge XT, Surge XT Effects, `surge-xt-cli` and every REAPER first. RABBIT
  asks you once more before it starts, with **No** selected, because a
  backup can't bring it back. A portable REAPER can't remove Surge XT. If
  the removal stops partway, the last page says so, and where it stopped
  when the administrator command said. **Show details** lists what is gone
  and what is still there. The next time you start RABBIT, Surge XT is still
  on the **Uninstall** tab, so you can uninstall it again.
- **The password window (Mac):** macOS asks once for an administrator's name
  and password, before anything is removed, however many packages need it. It
  may offer Touch ID instead, and it may name osascript, the tool RABBIT asks
  through. RABBIT says when the window is coming. If it is still waiting after
  30 seconds, it says so again, because the window may be behind RABBIT.
  VoiceOver reads both. If you cancel the window, nothing is uninstalled. If
  the window fails instead, for example after three wrong passwords or with a
  name that isn't an administrator's, nothing is uninstalled either, and the
  last page says RABBIT couldn't get administrator rights.
- **If macOS refuses, or RABBIT runs as root (Mac):** if macOS refuses to
  move or delete something, with or without a password, turn on RABBIT under
  System Settings > Privacy & Security > App Management, then quit RABBIT,
  start it again and uninstall again. Start RABBIT as yourself, not with
  sudo: running as root, it uninstalls nothing.
- **Getting a package back:** start RABBIT again and tick it on the **Install
  or update** tab, which no longer ticks a package you removed by itself. To
  put back the very files that went instead, **Show details** on the last
  page lists a `rabbit restore-backup` command for each package whose files
  went into a backup set (see [Uninstall](#uninstall) under command-line
  usage). Surge XT and REAPER don't go into one: install them again. The
  JAWS scripts' files do, but their uninstaller folder and registry entry
  don't, so the command brings back only the files. On a Mac, you can also
  drag REAPER back from the Trash until you empty it; if an administrator's
  password moved it there, dragging it back may ask for one again. A
  package you put back from the Trash, or installed again yourself,
  gets its updates offered as usual: when one is out, RABBIT ticks it.
  Language packs are the exception: one you removed or turned down stays
  unticked.

### Switching the language

Use the language picker at the bottom of the window. Currently bundled
languages: English (United States), Deutsch (Deutschland), español (España),
Français (France), Italiano (Italia) and Português (Brasil). RABBIT auto-picks
your OS language on first launch when a translation is available — regional
variants fall back to the bundled one, so a Mexican Spanish (es-MX) system gets
the Spanish UI, and a European Portuguese (pt-PT) one the Brazilian Portuguese
UI.

### Expert mode: pre-release builds

The wizard has a hidden expert mode for people who test pre-release software.
It installs builds that are unsupported and can break REAPER or its
accessibility at any time. Keep it away from a setup you depend on.

To turn it on, press **Ctrl+Shift+E** (**Cmd+Shift+E** on macOS) on the first
page and confirm. To have it on from the start, launch RABBIT with the
environment variable `RABBIT_EXPERT=1`:

```
$env:RABBIT_EXPERT=1; .\RABBIT.exe                      # Windows, PowerShell
RABBIT_EXPERT=1 /Applications/Rabbit.app/Contents/MacOS/rabbit   # macOS
```

The window title then ends in "Expert mode", and two choices appear on the
first page below the REAPER installation:

- **REAPER builds**: regular releases, or the development builds from
  [landoleet.org](https://www.landoleet.org/). RABBIT installs whatever
  landoleet currently offers, whether that is a development build, a release
  candidate or a release.
- **OSARA builds**: regular snapshots, or the test build of an open OSARA
  pull request. RABBIT fetches the list from GitHub when expert mode comes on,
  with the newest pull request (the highest number) first.
  These builds are unreviewed, unsigned code. GitHub deletes them after 90
  days, and Windows security software is more likely to flag them (see
  [below](#if-antivirus-software-blocks-an-install-windows)).

On the Packages page, a package coming from a pre-release build says so in its
row, for example "(development build)".

Expert mode is never saved. It lasts until RABBIT closes, and pressing the
same keys again on the first page turns it off. Leaving it has a consequence.
The next install without expert mode takes every package that came from a
pre-release build back to its regular release, and the row says "back to the
regular release". The same thing happens to an OSARA test build once its pull
request is merged or closed, or GitHub has deleted the build. The mode can only
be changed on the first page, never while an install is running.

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

# ReaPack shows a donation notice on first run, so installing it
# unattended means acknowledging that notice up front:
RABBIT setup --resource-path "%APPDATA%\REAPER" --package reapack \
     --accept-reapack-donation-notice --apply
```

The CLI is dry-run by default; pass `--apply` to actually make changes.
`--save-report` writes a JSON report next to the resource path so you have a
record of what was installed, and `--json` prints the result as JSON for
scripting. A few other flags are worth knowing: `--allow-reaper-running`
proceeds even though REAPER is open (files it holds open may fail to be
replaced), `--preserve-osara-keymap` and `--replace-osara-keymap` decide
what happens to your key map when OSARA is installed (without either,
RABBIT replaces it only when OSARA is new or the key map is still OSARA's
own, as the wizard does), and `--stage-unsupported`
downloads the packages RABBIT can't install automatically and leaves them
in the cache for you to run yourself instead of skipping them. Every flag
is documented in `RABBIT <command> --help`.

`setup` also accepts `--config-step <id>` (repeatable) and
`--skip-config-step <id>` for the post-install configuration tweaks.
With no flags, the recommended steps whose dependencies are satisfied
(and that aren't already applied) run automatically; pass an explicit
list to opt in to a specific subset, or `--skip-config-step` to opt
out of one. **Set REAPER's language** is the exception: it only runs by
default when the run actually asks for a language pack, so a machine that
merely has one on disk does not get its interface language changed. Ask
for it with `--config-step set-reaper-language` to switch to a pack you
installed earlier. Step ids today are
`reapack-add-reaper-accessibility-remote`,
`reapack-add-reaper-accessible-fr-remote`,
`reapack-add-reaper-accessible-en-remote`, and
`set-reaper-language` (one step for every language; it depends on at
least one language pack being installed or in the plan).

Language packs install like any other package, e.g.:

```
RABBIT install-extension --package langpack-de --resource-path "%APPDATA%\REAPER" --apply
RABBIT setup --resource-path "%APPDATA%\REAPER" --package langpack-es \
     --package langpack-de --reaper-language langpack-de --apply

# Pick Team PMA's Spanish translation (es_MX) over the default es_ES:
RABBIT setup --resource-path "%APPDATA%\REAPER" --package langpack-es \
     --package-variant langpack-es=pma --reaper-language langpack-es --apply
```

`--reaper-language` only accepts a language pack, and lists the ones it
knows if you mistype it. Unlike the wizard, the CLI never records a package
you left out as "don't offer this again". It works the other way round,
though: a package you uninstalled, or turned down in the wizard, is skipped
by `apply-packages` and `setup` with a note, and naming it with `--package`
in a run that installs takes it off that list, so later runs offer it again.
A package you uninstalled but put back yourself, from the Trash or with its
own installer, isn't skipped: those commands update it as usual.

### Pre-release builds

The CLI needs no expert mode to install pre-release builds. Ask for a channel
per package with `--package-channel`. It works with `latest`, `artifacts`,
`download`, `plan`, `install-extension`, `apply-packages` and `setup`.

```
# REAPER's development build from landoleet.org:
RABBIT install-extension --package reaper --resource-path C:\REAPER \
     --package-channel reaper=dev --apply

# The test build of OSARA pull request 1454:
RABBIT install-extension --package osara --resource-path "%APPDATA%\REAPER" \
     --package-channel osara=pr:1454 --apply

# Back to the regular release:
RABBIT apply-packages --resource-path C:\REAPER --package-channel reaper=stable --apply

# See what's out on a channel without installing anything:
RABBIT latest --package-channel reaper=dev
```

`RABBIT packages` lists each package's channels. Unlike the wizard outside
expert mode, the CLI keeps a package on the channel it was last installed
from until you pass `<package>=stable`, so a later `apply-packages` keeps a
development REAPER on development builds. OSARA pull request builds come
through [nightly.link](https://nightly.link/), because GitHub hands workflow
builds only to signed-in users. When a pull request is merged or closed, or
has no build left, RABBIT installs the regular snapshot and prints a warning
saying so. It never counts a network failure as a missing build, so an outage
can't quietly downgrade anything.

RABBIT asks GitHub's API about several packages, and without signing in
GitHub allows 60 such requests an hour for each internet connection.
Following OSARA pull request builds takes several more on every run, so
testers can run into that limit. Version checks then fail with GitHub's "rate
limit exceeded" error, and the wizard can't list OSARA's pull request builds.
To lift the limit to 5,000 an hour, set the environment variable
`GITHUB_TOKEN` to a GitHub personal access token before starting RABBIT. The
wizard and the CLI both use it. RABBIT only reads public data, so a token
without any extra permissions is enough. RABBIT only sends it to
`api.github.com`, and never hands it to OSARA's test builds or to REAPER:

```
$env:GITHUB_TOKEN = "<your token>"; .\RABBIT.exe                        # Windows, PowerShell
GITHUB_TOKEN="<your token>" /Applications/Rabbit.app/Contents/MacOS/rabbit   # macOS
```

### Uninstall

`uninstall` removes packages RABBIT knows how to take off again: today OSARA,
SWS, ReaPack, ReaKontrol, FFmpeg, app2clap and the JAWS scripts (all three
Windows only), the language packs, Surge XT and REAPER. On Windows, Surge XT
and a standard REAPER come off through their own uninstallers, and the JAWS
scripts only from a standard REAPER. On a Mac, RABBIT deletes Surge XT for
good and moves the REAPER app of a standard or a portable REAPER to the Trash.
`RABBIT components` says for each package whether RABBIT can
uninstall it, and if not, why. Name every package with `--package`; there is
no "remove everything". Like the other commands, it only shows what it would
do until you add `--apply`:

```
# See what would go, what stays, and anything that would stop it:
RABBIT uninstall --resource-path "%APPDATA%\REAPER" --package sws

# Uninstall SWS and the German language pack:
RABBIT uninstall --resource-path "%APPDATA%\REAPER" --package sws \
     --package langpack-de --apply

# On a Mac, see what uninstalling Surge XT and REAPER would do:
RABBIT uninstall --resource-path "$HOME/Library/Application Support/REAPER" \
     --package surge-xt --package reaper
```

- **What goes:** the files RABBIT installed and the files the package is known
  to install. A file RABBIT installed that changed since stays, and the report
  says so. For OSARA, that is its plug-in, its translations, the copy of its
  key map in `KeyMaps` and, on Windows, its uninstaller; then the `osara`
  folder if nothing else is left in it. On Windows, OSARA also comes off
  Windows' list of installed programs, but only if that entry points into the
  REAPER you named: there is one OSARA entry for the whole computer, and one
  that belongs to another REAPER stays. The report says which. For the JAWS
  scripts, it is their two plug-in files and the script files RABBIT
  recorded their installer adding, each script file only while it still
  holds what the installer wrote (see below).
- **What stays:** your settings, such as SWS's grooves and `S&M.ini`,
  ReaPack's repositories and its list of what it installed, and ReaKontrol's
  FX maps. Add `--remove-settings` to remove those too. `reaper.ini`,
  `reaper-kb.ini`, your key maps and everything ReaPack installed always stay,
  even files ReaPack installed among a package's settings. So do the key map
  backups OSARA made, and OSARA's settings, which live in
  `reaper-extstate.ini` beside every other extension's. Script files of the
  JAWS scripts that RABBIT can't be sure the installer added, and those
  changed since, stay too, even with `--remove-settings`.
- **Nothing is deleted for good,** except Surge XT, REAPER on Windows, and
  the JAWS scripts' uninstaller folder and registry entry (see below). A
  REAPER app on a Mac goes to the Trash. Other removed files move into a
  backup set under `RABBIT\backups`, and so do settings removed with
  `--remove-settings`. The run saves a report under `RABBIT\logs`. If REAPER
  used the language pack you removed, RABBIT sets REAPER back to English.
- **REAPER must be closed.** For REAPER on Windows, Surge XT, app2clap and the
  JAWS scripts, every REAPER on the computer must be closed, portable ones
  included. The same goes for REAPER on a Mac when the administrator command
  (see below) moves its app. Unlike the commands that install, `uninstall` has
  no `--allow-reaper-running`: a running REAPER holds its extensions' files
  open, so they can't be moved, and it writes `reaper.ini` again when it
  closes, which would undo the language reset. The dry run checks this too.
- **All or nothing up front.** If any package you named can't be uninstalled,
  or one of its files can't be read, `--apply` changes nothing at all and
  says why. For a portable REAPER on Windows and a REAPER app on a Mac that
  RABBIT can't move, it prints how to remove them by hand. So it does for the
  JAWS scripts when you name a portable REAPER, which can't remove them (name
  the standard REAPER instead), when RABBIT doesn't trust their uninstaller
  folder or registry entry, and when it has no receipt for them and finds
  none of their files for your account.
- **Surge XT and REAPER on Windows** come off through their own uninstallers,
  which need administrator rights. The dry run names the packages that need
  them and the uninstaller it would run. With `--apply`, Windows asks for
  permission once for the whole run, before anything is removed; declined,
  nothing is uninstalled and the exit code is 1. RABBIT runs only an
  uninstaller that Windows' list of programs installed for every user names
  and that sits where it belongs: Surge XT's in
  `C:\ProgramData\Surge XT\uninstall`, with nothing beside it but its own
  files, and REAPER's in the REAPER folder you named under Program Files.
  Otherwise it says why and changes nothing; you can still uninstall the
  program in Windows Settings. Surge XT's patches in `Documents\Surge XT` and
  its fonts stay, and RABBIT removes the links into its data folder that its
  uninstaller leaves behind, and its vendor folders if they are left empty.
  REAPER's uninstaller keeps `%APPDATA%\REAPER`; if `reaper.ini` or RABBIT's
  receipt is gone afterwards anyway, RABBIT stops, writes its receipt again
  and says so. An uninstaller still open after ten minutes stops the run; it
  may be waiting for an answer in a window of its own. What these
  uninstallers remove can't be restored from a backup.
- **The JAWS scripts** come off only from a standard REAPER, whose resource
  folder is `%APPDATA%\REAPER`. Their two plug-in files in `UserPlugins` move
  into the backup set, and so do the script files RABBIT recorded their
  installer adding to the language folders in
  `%APPDATA%\Freedom Scientific\JAWS`, each only while it still holds what
  the installer wrote. A script file that was there before the installer
  ran, or that changed since, stays, and the report lists it. RABBIT records
  which script files the installer adds when it runs the installer itself,
  from this version on; the script files of installs made before, or without
  RABBIT, stay. The report says how to remove the script files that stay by
  hand. Once every file is in the backup set, RABBIT removes what the
  installer left for its own uninstaller, with administrator rights: the
  folder `%ProgramFiles(x86)%\Reaper_JawsScripts` with its `Uninstall.dat`
  and `Uninstall.exe`, and the registry entry
  `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts`.
  The dry run lists each of them and names the scripts among the packages
  that need administrator rights. Windows asks for permission before
  anything is removed, in the run's one prompt, the same one Surge XT and
  REAPER use; declined, nothing is uninstalled and the exit code is 1. This
  happens only while the folder or the entry is there: once both are gone,
  the scripts come off with no prompt. A backup can't bring the folder and
  entry back. RABBIT removes them only when they are exactly as the
  installer leaves them: the folder owned by an administrator, with nothing
  else in it, and the entry naming that folder, with no values or subkeys
  but those the installer writes. Otherwise it changes nothing, prints why
  and how to remove the scripts by hand, and exits with 2. JAWS itself may
  keep running. If it has a script file open, the package fails with
  everything put back: wait a moment, then run `RABBIT uninstall` again, and
  restart JAWS first if that doesn't help.
- **REAPER on a Mac** goes to the Trash, from a standard or a portable
  REAPER, and its resource folder stays. The dry run says whether it needs a
  password. If your account may move the app, RABBIT moves it to the Trash
  with no password, after every other package. If only an administrator may
  move it and it sits directly in `/Applications`, the administrator command
  (below) moves it into your Trash, under its name with the date and time
  added, such as `REAPER 2026-11-07 15.30.12.app`; every REAPER must then be
  closed. It keeps the owner it had, so emptying the Trash or dragging it back
  to `/Applications` may ask for an administrator's password, and Finder
  offers no Put Back for it. An app RABBIT can't move at all, such as a Locked
  one, one that isn't the REAPER app RABBIT expects, or one only an
  administrator may move that isn't directly in `/Applications`, is on
  another disk than your Trash or would go to a Trash that isn't a folder of
  your own, is blocked: RABBIT changes nothing, prints how to move it to the
  Trash in Finder and exits with 2.
- **Surge XT on a Mac** has no uninstaller of its own, so the administrator
  command deletes it for good: its two apps, its plug-ins (VST3, AU, CLAP and
  LV2) and its factory data in `/Library/Application Support/Surge XT`, the
  VST3 plug-ins last. It then has macOS's installer forget Surge XT's receipts
  that the installer still lists, and RABBIT has macOS refresh the list of
  Audio Units that music programs read. Your patches in your Documents
  folder, your settings in
  `~/Library/Application Support/Surge Synth Team/Surge XT` and any copy of
  its factory data you put in `~/Library/Application Support/Surge XT` stay,
  even with `--remove-settings`. RABBIT removes Surge XT only from the places
  its installer puts it: a copy you put somewhere else, such as in
  `~/Library/Audio/Plug-Ins`, stays, and RABBIT doesn't list it. Surge XT,
  Surge XT Effects, `surge-xt-cli` and every REAPER must be closed. A backup
  can't bring Surge XT back; install it again instead. If the command stops
  partway, the report says what is gone and what is still there, the exit
  code is 1, and RABBIT still finds Surge XT, so you can run
  `RABBIT uninstall` again.
- **The administrator command (Mac).** Everything that needs an
  administrator on a Mac goes into one command, and it is the run's first
  change. The dry run lists what root would get: each path it deletes for
  good, each installer receipt it forgets, the Trash it moves REAPER into, and
  the programs it first checks are closed. With `--apply`, RABBIT says on
  stderr when macOS's password window is coming, and again if it is still
  open after 30 seconds: it may be behind the app you run RABBIT from, such as
  Terminal. macOS asks once for an administrator's name and password, in a
  window that may name osascript. If you cancel it, nothing is uninstalled and
  the exit code is 1. The same goes for a window that fails, for example after
  three wrong passwords or with a name that isn't an administrator's: RABBIT
  says it couldn't get administrator rights. If one of the programs that must
  be closed was started while the window was open, the command changes
  nothing. If macOS refuses a step, turn on the app you run RABBIT from, such
  as Terminal, under System Settings > Privacy & Security > App Management,
  then quit that app, open it again and run `RABBIT uninstall` again. Run
  RABBIT as yourself, not with sudo: `--apply` as root changes nothing and
  exits with 1. Run the dry run as yourself too: under sudo it describes
  root's run, not yours.
- **Ctrl+C stops between packages.** The package being removed finishes, and
  the rest stay installed. Press Ctrl+C again to stop at once; the files
  already moved stay in the backup set, where `restore-backup` finds them. On
  a Mac the administrator command counts as one package here, even when it
  removes more than one. RABBIT looks for Ctrl+C one last time a moment
  before it says the password window is coming. Pressed before then, Ctrl+C
  stops the run with nothing changed. Pressed later, even while RABBIT is
  still saying it, the first Ctrl+C doesn't reach the window or root's steps:
  the window opens anyway, and RABBIT stops once the command is done. Cancel
  the window to change nothing. A second Ctrl+C closes the password window if
  it is still open, then ends RABBIT. If root's steps are already running,
  they may go on after RABBIT has ended, and RABBIT saves no report; what
  they delete is gone for good. Run the dry run again to see what is left,
  and run `RABBIT uninstall` again to remove the rest.
- **Removed packages stay removed.** The wizard no longer ticks them, and
  `apply-packages` and `setup` skip them with a note. Name one with
  `--package` to install it again. If you put one back yourself, from the
  Trash or with its own installer, RABBIT offers its updates as usual.
  Language packs are the exception: one you removed stays unticked.

`--json` prints the plan in a dry run and the report with `--apply`, both with
message codes instead of sentences. The exit code follows one table for the
dry run and `--apply`:

| Exit code | Meaning |
|---|---|
| 0 | Every package was removed (or, in a dry run, can be), or isn't installed, so a script can run twice. Anything of a package RABBIT still finds afterwards is listed, never deleted. A removal that finishes only after a restart also exits with 0, and says to restart. |
| 1 | A removal failed or was stopped, the Windows permission prompt was declined, or macOS's password window was cancelled or failed. The failed package's files went back where they were, except: what its own uninstaller on Windows already removed; what RABBIT already removed of the JAWS scripts' uninstaller folder and registry entry, which the report lists as far as RABBIT knows it; what the administrator command on a Mac already deleted, which the report lists; an app macOS may already have moved to the Trash; and any file that couldn't be moved back, which stays in the backup set and is named in the report. Packages removed before it stay removed. Also when an error stopped RABBIT before anything changed, such as another RABBIT working on the same REAPER, or `--apply` run as root on a Mac; the message says which. |
| 2 | A package you named can't be uninstalled. Nothing changed. |

To bring a package back, find the uninstall's backup set and restore just that
package from it:

```
RABBIT backups --resource-path "%APPDATA%\REAPER"
RABBIT restore-backup --resource-path "%APPDATA%\REAPER" \
     --backup-id uninstall-unix-1234567890 --package sws --apply
```

Its files come back, its settings too if `--remove-settings` removed them. It
goes back into RABBIT's records and is no longer skipped, and REAPER's language
goes back to it if the uninstall changed that and you haven't picked another
language since. Without `--package`, every package in the set comes back. An
uninstall that stopped partway through a package restores the same way. A
package the set records with none of its files in it, as when they were already
gone when it was uninstalled, has nothing to bring back: `--package` says so,
and restoring the whole set leaves it out and names it, so RABBIT doesn't count
it as installed again. Two limits: a package you installed again after
uninstalling it can't be restored from the old set, because the newer copy is
in the way (uninstall it first); and a removal that a backup can't hold, by
Surge XT's or REAPER's own uninstaller on Windows or by the administrator
command deleting Surge XT on a Mac, can't be restored: install the package
again instead. A REAPER app RABBIT moved to the Trash isn't in a backup set
either: until you empty the Trash, you can drag it back from there. If the
administrator command moved it, macOS may ask for an administrator's password
for that. A restored OSARA isn't back on Windows' list of installed programs;
installing OSARA again puts it there. On a Mac, run `restore-backup --apply` as
yourself: run as root, as with `sudo`, it refuses and changes nothing, as
`uninstall --apply` does.

`--package jaws-scripts` brings back the JAWS scripts' plug-in files and the
script files that went, but not their uninstaller folder and registry entry,
which no backup holds. If RABBIT installed the scripts and their plug-in files
haven't changed since, RABBIT lists them as installed again, and as long as
that folder and entry stay gone, uninstalling them again asks for no
permission. Scripts installed without RABBIT aren't listed again: RABBIT found
them through that folder and entry. A script file goes back only into JAWS's
settings folder: if `%APPDATA%\Freedom Scientific\JAWS` is gone, or the way
from there down to the file leads through a link or junction, the restore
changes nothing and says why. If that stops the restore of a whole set,
restore the set's other packages one at a time with `--package`.

A restore never copies RABBIT's records (its receipt,
`RABBIT\install-state.json`) back from a backup: that would undo every change
to them since the backup was made, uninstalls included. Restoring an
uninstall's backup puts back only each package's own entry, as above. So RABBIT
doesn't restore a backup that holds a copy of its records, such as one an older
version of RABBIT made when it restored an update's backup. The copy stays in
the backup, and the message names its folder. Backups from updates, which hold
the files an update replaced, can't be restored yet either, and neither can the
backups that hold only a copy of RABBIT's records. Nothing changes, and each of
these backups stays as it is.

### Maintain

```
RABBIT backups --resource-path "%APPDATA%\REAPER"          # list backup sets
RABBIT restore-backup --resource-path "%APPDATA%\REAPER" \
     --backup-id uninstall-unix-1234567890 --apply         # restore one set
```

### Update RABBIT itself

```
RABBIT self-update check                       # see if a new RABBIT is out
RABBIT self-update apply --restart             # update + relaunch
```

The GUI does this automatically on startup; the CLI commands are there for
unattended environments and CI.

### If antivirus software blocks an install (Windows)

An install can fail with *"Windows security software blocked …"*. Microsoft
Defender sometimes flags a legitimately code-signed third-party installer it
hasn't seen widely yet — OSARA's development snapshots, rebuilt on every
change, are the usual case — and quarantines it mid-install. This is a false
positive: the installer is signed by its publisher, and RABBIT downloads every
package over HTTPS from the publisher's own release page and verifies it
against the publisher's checksum before touching it.

To finish the installation, do **one** of the following:

- Open **Windows Security → Virus & threat protection → Protection history**,
  find the blocked item, choose **Allow**, then run RABBIT again.
- Add RABBIT's download folder to **Virus & threat protection → Manage
  settings → Exclusions**.
- Install that one package by hand from the publisher's own page, then re-run
  RABBIT for the rest.

RABBIT will never turn your virus protection off, and you should not need to:
disabling real-time protection exposes the whole machine, and any installer
that switches it off is indistinguishable from malware. If a RABBIT release
itself is flagged, please report it — and consider submitting the file to
Microsoft as a false positive at <https://www.microsoft.com/en-us/wdsi/filesubmission>.

## Reports and logs

Every installation produces a JSON report under `<resource>/RABBIT/logs/`.
Surge XT's Windows installer writes its own log there too
(`surge-xt-installer-<stamp>.log`), and RABBIT names it when that install fails.
Backups go to `<resource>/RABBIT/backups/<timestamp>/`. The download cache is a
`rabbit-cache` folder in your temporary folder, usually `%TEMP%\rabbit-cache` on
Windows and `$TMPDIR/rabbit-cache` on macOS. It can be deleted safely at any time, and
the CLI's `--cache-dir` points a single run somewhere else.

## Made with the help of AI

RABBIT was built with the help of AI coding assistants. Much of the code,
tests, and documentation in this repository was written collaboratively
with large language models, then reviewed and maintained by its human
author. We mention this openly so users and contributors know what went
into the project.

## Development

See [DESIGN.md](./DESIGN.md) for the full architecture and design rules. To
build from source you need Rust 1.99 or newer. The wxDragon GUI
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
  + SWS + ReaKontrol downloads. Started by hand (workflow_dispatch), it also
  runs the Trash smoke test: a throwaway bundle goes to the runner's Trash
  the way an uninstall moves REAPER.app. That test is ignored and moves
  nothing unless `RABBIT_TRASH_SMOKE=1` is set, so ordinary test runs never
  touch the Trash.
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
