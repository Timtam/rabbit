app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = yes
common-no = no

# What happens to a package in this run, the first thing its row says. An
# unticked row on either tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal.
action-install = Will install
action-update = Will update
action-keep = Won't touch
action-uninstall = Will uninstall
# On the Backups tab: what happens to a backup ticked to restore, or to
# delete. The last is for a backup that is the only copy of what it holds,
# such as software you uninstalled.
action-restore = Will restore
action-delete = Will delete
action-delete-only-copy = Will delete the only copy

package-reaper = REAPER
package-osara = OSARA
package-sws = SWS Extension
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Snowman's JAWS Scripts for REAPER
package-ffmpeg = FFmpeg for Improved Video Support
package-surge-xt = Surge XT
package-app2clap = app2clap
package-langpack-es = REAPER in Spanish
package-langpack-de = REAPER in German
package-langpack-fr = REAPER in French
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Team PMA (es_MX)

package-reaper-description = REAPER is the digital audio workstation that everything else here builds on. RABBIT can install or update REAPER for you.
package-osara-description = OSARA is the open source accessibility extension that makes REAPER usable with screen readers. NVDA, JAWS and Narrator on Windows, VoiceOver on macOS are all widely adopted, some other Windows screen readers might also work. Install OSARA if you rely on a screen reader to use REAPER.
package-sws-description = The SWS Extension is a long-running community-authored pack of extra actions, scripts, and helpers that extend REAPER's features. For the most accessible REAPER setup, whether you're on Windows or Mac, you should install SWS alongside OSARA.
package-reapack-description = ReaPack is an open source package manager. It can be used to search, install, track and update third-party scripts and extensions from inside REAPER itself. Install this if you want to use scripts shared by the REAPER community.
package-reakontrol-description = ReaKontrol provides open source integration for Native Instruments Komplete Kontrol keyboards. Install this if you have an S series MK2, A series, M-32 or Kontrol MK3 keyboard.
package-jaws-scripts-description = Snowman's scripts improve how JAWS handles various windows throughout REAPER, as well as offering extended Braille support and many other features. Note that these scripts are intended to be used alongside OSARA, they're not an alternative to it. For optimal accessibility with JAWS, you should install both.
package-ffmpeg-description = FFmpeg's shared runtime libraries enable REAPER's video decoder to import and play back common video and audio formats. RABBIT installs the BtbN GPL-shared build's bin folder into UserPlugins; the patch level isn't recoverable from the DLL filenames alone, so externally-installed FFmpegs are reported with a `<major>.0.0` placeholder.
package-surge-xt-description = Surge XT is a free, open-source hybrid synthesizer. RABBIT runs the vendor installer for you — it lays down the VST3, CLAP, AU (macOS only), and standalone formats system-wide so REAPER and other DAWs can load Surge XT. Tracks the rolling nightly channel because the most recent stable release (1.3.4) is from August 2024 and the project effectively ships through nightlies now. Standard REAPER installations only: the factory data lives outside any portable REAPER folder.
package-app2clap-description = app2clap is a CLAP plug-in for Windows that captures audio from other applications and brings it into REAPER (or any CLAP host) as a plug-in you insert on a track — handy for recording or processing sound from a browser, media player, or other program. RABBIT downloads the latest build and installs app2clap.clap into your per-user CLAP folder, so no administrator rights are needed. Windows only. Standard REAPER installations only: it installs outside any portable REAPER folder.
package-langpack-es-description = Translates REAPER's own interface into Spanish, including the SWS extension. Maintained by Javier Robledo for the Spanish REAPER community, and distributed through reaperespa.com — RABBIT downloads the current version and installs it as es_ES.ReaperLangPack. OSARA reads that file name to choose its own Spanish translation. After installing, pick the language in REAPER's preferences (or let RABBIT set it for you).
package-langpack-de-description = Translates REAPER's own interface into German, including the SWS extension. Maintained by MrData and published in the REAPER Stash — RABBIT downloads the current version and installs it as de_DE.ReaperLangPack. After installing, pick the language in REAPER's preferences (or let RABBIT set it for you).
package-langpack-fr-description = Translates REAPER's own interface into French, including the SWS extension. Maintained by Lee Julien and Pierre-Marie Curt for ReaperAccessible, the French screen-reader community, and published in their ReaPack repository — RABBIT downloads the current version and installs it as fr_CA.ReaperLangPack. OSARA reads that file name to choose its own French translation, and its fr_CA one is nearly complete where its fr_FR one still leaves about a third of the messages in English; the pack itself is the same standard French either way. After installing, pick the language in REAPER's preferences (or let RABBIT set it for you).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = RABBIT can't uninstall this yet.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Removes the SWS extension.
uninstall-summary-sws-keeps = Your grooves and your SWS settings, such as your cycle actions, stay.
uninstall-summary-reapack-removes = Removes the ReaPack extension.
uninstall-summary-reapack-keeps = Your ReaPack settings and repositories stay, and so does ReaPack's list of what it installed.
uninstall-summary-reakontrol-removes = Removes the ReaKontrol extension.
uninstall-summary-reakontrol-keeps = Your FX maps stay, and RABBIT doesn't touch your Native Instruments software.
uninstall-summary-ffmpeg-removes = Removes the FFmpeg files RABBIT installed in UserPlugins. REAPER can then no longer use FFmpeg to open or render media.
uninstall-summary-ffmpeg-keeps = FFmpeg files that RABBIT has no record of, such as ones left behind by older versions, stay.
uninstall-summary-app2clap-removes = Removes app2clap from your CLAP folder. Every program you use CLAP plug-ins in loses it, not only REAPER.
uninstall-summary-app2clap-keeps = The CLAP folder and the other plug-ins in it stay.
uninstall-summary-langpack-removes = Removes the language pack RABBIT installed. If REAPER uses it, RABBIT sets REAPER back to English.
uninstall-summary-langpack-keeps = Language packs you added yourself stay.
uninstall-summary-osara-removes = Removes the OSARA extension, its translations, and the copy of its key map in the KeyMaps folder.
uninstall-summary-osara-removes-windows = Removes the OSARA extension, its translations, its uninstaller, and the copy of its key map in the KeyMaps folder. If Windows lists this OSARA among your installed programs, RABBIT takes it off that list.
uninstall-summary-osara-keeps = Your key map stays as it is, and so do OSARA's settings and the backups of your key map that OSARA made.
uninstall-summary-surge-removes = Surge XT's own uninstaller removes it for every music program on this computer. A backup can't bring it back.
uninstall-summary-surge-keeps = Your own patches, in the Surge XT folder in your Documents, stay.
uninstall-summary-reaper-windows-removes = REAPER's own uninstaller removes the program. It also removes REAPER's file associations and ReaRoute for every REAPER on this computer.
uninstall-summary-reaper-keeps = Your settings, extensions and projects stay.
# The JAWS scripts: two plug-in files in REAPER's UserPlugins folder, and
# script files their installer adds to JAWS's settings folder, which RABBIT
# removes only while they are as the installer left them. Keep
# %APPDATA%\Freedom Scientific\JAWS as it is.
uninstall-summary-jaws-scripts-removes = Removes the scripts' two plug-in files from REAPER's UserPlugins folder, and any script files RABBIT saw their installer add to JAWS's settings folder.
uninstall-summary-jaws-scripts-keeps = Script files RABBIT can't be sure the installer added, and those changed since, such as your own key assignments, stay in JAWS's settings folder, %APPDATA%\Freedom Scientific\JAWS.
# The same two sentences on macOS, where Surge XT has no uninstaller of its
# own and REAPER is an app. Name the Trash and the Library folder as Finder
# does in your language.
uninstall-summary-surge-macos-removes = Removes Surge XT's apps and plug-ins, and its factory data in the Library folder. A backup can't bring them back.
uninstall-summary-reaper-macos-removes = Moves the REAPER app to the Trash.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = your grooves
uninstall-kept-sws-settings = SWS settings
uninstall-kept-sws-cycle-actions = your cycle actions
uninstall-kept-reapack-settings = ReaPack settings and your repositories
uninstall-kept-reapack-data = ReaPack's list of what it installed, and its download cache
uninstall-kept-reakontrol-fx-maps = your FX maps
uninstall-kept-app2clap-clap-folder = the CLAP folder, which other plug-ins share
uninstall-kept-osara-key-map = your key map
uninstall-kept-osara-replaced-key-map = your key map from before OSARA replaced it
uninstall-kept-osara-merged-key-map = your key map from before OSARA added its keys to it
uninstall-kept-osara-settings = OSARA's settings, in a file every extension shares
uninstall-kept-surge-user-data = your patches and everything else you saved with Surge XT
uninstall-kept-reaper-resource = REAPER's resource folder, with your settings, extensions and key maps
# On macOS, in the Library folder of your home folder.
uninstall-kept-surge-settings-macos = your Surge XT settings
# On macOS, in the Library folder of your home folder: a copy of Surge XT's
# factory data put there by hand, which Surge XT reads instead of the one it
# installs for everyone. Say "factory data" as in
# uninstall-summary-surge-macos-removes.
uninstall-kept-surge-factory-copy-macos = your own copy of Surge XT's factory data

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = If you use OSARA's key map, some of its keys run SWS actions. Those keys stop working until you install SWS again.
uninstall-warning-sws-reapack-scripts = Many REAPER scripts, accessibility scripts among them, need SWS. Scripts you installed with ReaPack may stop working.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Scripts and extensions you installed with ReaPack stay, but nothing keeps them up to date.
# A note on what stays behind after Surge XT is uninstalled.
uninstall-note-surge-fonts-stay = The fonts Surge XT installed stay: its installer marks them never to be removed.
# A note on the JAWS script files that stay after the JAWS scripts are
# uninstalled, and how to remove them by hand. Keep %APPDATA%, the folder
# and file names, and "reaper" (the start of the file names) exactly as they
# are. For the example language folder, use JAWS's folder for your language:
# enu for English, deu for German, esn for Spanish, fra for French, ita for
# Italian.
uninstall-note-jaws-scripts-by-hand = To remove the script files that stay, delete the files whose names start with reaper from the language folders in %APPDATA%\Freedom Scientific\JAWS, such as 2025\Settings\enu. Copy reaper.jkm, reaper.jcf and reaper.ini somewhere else first if you want to keep your key assignments and settings.

# How to remove a package by hand where RABBIT doesn't, shown line by line.
# Keep file names, folder paths and %APPDATA% exactly as they are.
#
# The JAWS scripts' steps, shown where RABBIT can't remove them: from a
# portable REAPER, when it doesn't trust their uninstaller folder or
# registry entry, when it finds none of their files for this account, and
# after an uninstall in which it couldn't remove that folder and entry.
# %ProgramFiles(x86)% is how Windows names the Program Files (x86) folder
# in a path: keep it exactly as it is, and "reaper" too. For the example
# language folder, use JAWS's folder for your language, as in
# uninstall-note-jaws-scripts-by-hand.
uninstall-by-hand-jaws-scripts = To remove the JAWS scripts by hand:
    1. Close REAPER.
    2. Delete Reaper_ComAccess32.dll and Reaper_ComAccess64.dll from %APPDATA%\REAPER\UserPlugins.
    3. Delete the folder %ProgramFiles(x86)%\Reaper_JawsScripts. Windows asks for an administrator's permission.
    4. To remove the script files too, delete the files whose names start with reaper from the language folders in %APPDATA%\Freedom Scientific\JAWS, such as 2025\Settings\enu. Copy reaper.jkm, reaper.jcf and reaper.ini somewhere else first if you want to keep your key assignments and settings.
uninstall-manual-reaper-portable = RABBIT doesn't remove a portable REAPER: REAPER's own files and yours share its folder. To remove it by hand:
    1. Close REAPER.
    2. Copy anything you want to keep out of the portable REAPER folder, such as projects you saved there, your key maps and your settings.
    3. Delete the portable REAPER folder.
    The backups RABBIT made for this REAPER are in that folder too, in RABBIT\backups, and go with it.

# Why RABBIT didn't start a program's own uninstaller, or didn't remove a
# package's uninstaller folder and registry entry. RABBIT does both with
# administrator rights. Nothing has changed at that point. RABBIT shows one
# of these, then uninstall-vendor-refused-nothing-changed, or, for the folder
# and registry entry, the steps to remove the package by hand.
# { $package } is the package's name, such as Surge XT, REAPER or
# JAWS-for-REAPER scripts.
# { $path } is a file, a folder or a value as Windows holds it; keep it as it
# is.
uninstall-vendor-refused-not-listed = Windows doesn't list { $package } among the programs installed for all users, so RABBIT can't find its uninstaller.
uninstall-vendor-refused-unclear-command = RABBIT can't tell which program Windows names as { $package }'s uninstaller: { $path }
uninstall-vendor-refused-not-a-full-path = Windows names { $package }'s uninstaller by a path RABBIT doesn't trust: { $path }
uninstall-vendor-refused-unknown-folder = RABBIT can't find a folder it needs to check { $package }'s uninstaller.
uninstall-vendor-refused-wrong-folder = { $package }'s uninstaller isn't in the folder it belongs in: { $path }
uninstall-vendor-refused-foreign-file = Next to { $package }'s uninstaller is a file that doesn't belong there: { $path }
uninstall-vendor-refused-folder-or-link = Next to { $package }'s uninstaller is a folder or a link that doesn't belong there: { $path }
uninstall-vendor-refused-untrusted-owner = Next to { $package }'s uninstaller is a file an administrator didn't put there: { $path }
uninstall-vendor-refused-install-location-missing = Windows doesn't say which folder { $package } is installed in.
uninstall-vendor-refused-install-location-mismatch = Windows says { $package } is installed in { $path }, not in the REAPER you chose.
uninstall-vendor-refused-not-in-install-folder = { $package }'s uninstaller isn't in its program folder: { $path }
uninstall-vendor-refused-not-below-program-files = { $package } isn't installed in a folder of its own in Program Files: { $path }
uninstall-vendor-refused-copy-differs = The copy RABBIT made of { $package }'s uninstaller isn't the same as the original.
uninstall-vendor-refused-unreadable = RABBIT couldn't read { $path } to check { $package }'s uninstaller.
uninstall-vendor-refused-not-a-vendor-plan = RABBIT has no uninstaller for { $package } that it may run.
uninstall-vendor-refused-unsupported = RABBIT can run { $package }'s uninstaller only on Windows.
# { $path } is the name of a value in the package's registry entry.
uninstall-vendor-refused-unexpected-registry-value = { $package }'s registry entry holds a value RABBIT doesn't expect: { $path }
# { $path } is the text of a value in the package's registry entry.
uninstall-vendor-refused-registry-value-mismatch = { $package }'s registry entry points somewhere RABBIT doesn't expect: { $path }
# The package's registry entry has subkeys, which its installer doesn't write.
uninstall-vendor-refused-registry-key-has-subkeys = { $package }'s registry entry holds more than RABBIT expects.
# { $path } is the package's uninstaller folder in Program Files.
uninstall-vendor-refused-untrusted-folder = An administrator didn't create { $package }'s folder: { $path }
# Follows each of the messages above. { $package } as above.
uninstall-vendor-refused-nothing-changed = Nothing was changed. You can uninstall { $package } in Windows Settings, under Apps, Installed apps.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (not available: { $reason })
wizard-package-row-unavailable-portable = portable REAPER target
wizard-package-row-unavailable-version-check = online version check failed

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package }: the latest-version check failed ({ $message }). Installing or updating this package is disabled for this run.

detect-installed = Installed
detect-not-installed = Not installed
detect-version-unknown = Unknown version
detect-source-receipt = RABBIT receipt
detect-source-files = UserPlugins file presence
detect-source-reapack-registry = ReaPack registry

# $package is the localized package display name.
status-package-installed = { $package } installed

wizard-step-target = Target
wizard-step-version-check = Version check
wizard-step-packages = Packages
wizard-step-reapack-acknowledgement = ReaPack donation
wizard-step-review = Review
wizard-step-progress = Progress
wizard-step-done = Done

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Back
wizard-button-back-mnemonic = B
wizard-button-next = Next
wizard-button-next-mnemonic = N
wizard-button-install = Install
wizard-button-install-mnemonic = I
wizard-button-uninstall = Uninstall
wizard-button-uninstall-mnemonic = U
wizard-button-close = Close
wizard-button-close-mnemonic = C
wizard-button-cancel = Cancel
wizard-button-cancel-mnemonic = C

wizard-target-heading = Choose a task
wizard-target-language-label = Language
wizard-target-language-restart-note = Changing the language restarts RABBIT so the new language can take effect.
wizard-locale-name-en-US = English (United States)
wizard-locale-name-de-DE = German (Germany)
wizard-locale-name-es-ES = Spanish (Spain)
wizard-locale-name-fr-FR = French (France)
wizard-locale-name-it-IT = Italian (Italy)
wizard-locale-name-pt-BR = Portuguese (Brazil)
wizard-target-choice-label = Installation Path
wizard-target-details-label = Target details
wizard-target-empty = No REAPER installation target is selected.
wizard-target-portable-choice = Make or update a portable version of REAPER
wizard-target-portable-folder-label = Portable folder
wizard-target-portable-folder-message = Choose a portable REAPER folder if you already have one, or an empty folder if making a new portable version.
wizard-target-portable-folder-browse-label = Browse…
wizard-target-portable-pending-details = Use the Browse button to set the location of an existing portable version if you have one, or to choose an empty folder if you want to make a new portable version of REAPER.
wizard-target-custom-portable-label = Portable REAPER folder
wizard-target-custom-portable-app-path-label = REAPER application path
wizard-target-custom-portable-path-label = Portable resource path
wizard-target-custom-portable-version-label = REAPER version
wizard-target-custom-portable-writable-label = Writable
wizard-target-custom-portable-note = RABBIT will create the REAPER resource path here if it is missing.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } in { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = REAPER installation path: { $app_path }
    Version: { $version }
    Resource path: { $path }
    Writable: { $writable }

wizard-packages-heading = Choose what to install, update or uninstall
wizard-packages-list-label = Install or update: packages and settings
wizard-packages-row-checked = Checked
wizard-packages-row-unchecked = Unchecked
wizard-packages-choice-shown = New choice below the list: { $choice }.
wizard-packages-choice-hidden = Choice removed: { $choice }.
wizard-packages-tree-group-label = Packages
wizard-additional-software-tree-group-label = Additional software
wizard-language-tree-group-label = Language packs
wizard-configuration-tree-group-label = Configuration

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Install or update
wizard-packages-tab-uninstall = Uninstall
# The third tab, where RABBIT's backups are restored or deleted. Keep it
# short too.
wizard-packages-tab-backups = Backups
# macOS only: VoiceOver says this once, the first time the Packages page opens.
# Name the Tab and Shift keys as Apple's page of Mac keyboard shortcuts does,
# and write Control out in full.
wizard-packages-tabs-switch-hint-macos = Press Control-Tab or Control-Shift-Tab to switch between the tabs.
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Uninstall: software RABBIT can remove
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package }: { $action }. You have { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = RABBIT can't uninstall it yet
wizard-uninstall-row-unavailable-not-supported = RABBIT can't uninstall it
wizard-uninstall-row-unavailable-manual = remove it by hand
wizard-uninstall-row-unavailable-reapack = managed by ReaPack
wizard-uninstall-row-unavailable-reapack-check-failed = ReaPack's records couldn't be checked
wizard-uninstall-row-unavailable-no-receipt = RABBIT didn't install this copy
wizard-uninstall-row-unavailable-no-trusted-uninstaller = no uninstaller RABBIT can trust
wizard-uninstall-row-unavailable-unsafe-path = a file RABBIT never removes
# macOS: the app has to be moved to the Trash in Finder. Name the Trash and
# Finder as macOS does in your language.
wizard-uninstall-row-unavailable-by-hand-macos = move it to the Trash in Finder
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack manages { $package }: it installed { $path } as { $reapack_package }. Uninstall it in ReaPack instead.
wizard-uninstall-details-reapack-check-failed = RABBIT couldn't check ReaPack's records for { $path } ({ $reason }), so it leaves { $package } alone.
wizard-uninstall-details-portable = { $package } serves every REAPER on this computer, so RABBIT removes it only from the standard REAPER, not from a portable one.
wizard-uninstall-details-no-receipt = RABBIT didn't install this copy of { $package }, so it is most likely your own, and RABBIT leaves it alone.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT found no uninstaller for { $package } it can trust.
wizard-uninstall-details-unsafe-path = RABBIT won't remove { $path }. It leaves { $package } alone rather than remove only part of it.
# The details text of an Uninstall row whose package RABBIT leaves alone
# because of the uninstaller folder and registry entry its installer left:
# RABBIT doesn't trust what it found there, or finds none of the package's
# files for this account. The package's steps to remove it by hand follow.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-cleanup-refused = RABBIT doesn't trust what it found in { $package }'s uninstaller folder in Program Files or in its registry entry, so it leaves { $package } alone.
wizard-uninstall-details-no-files-for-this-account = RABBIT doesn't find any of { $package }'s files for your account. If another account installed { $package }, uninstall it from that account; until then RABBIT leaves its uninstaller folder in Program Files and its registry entry alone.
# The details text of a macOS app RABBIT can't move to the Trash, by itself or
# with an administrator's password: first why, then how to move it there in
# Finder. $package is the localized package name, $path the app's full macOS
# path, keep it as it is, and $error macOS's own error message. Name Finder,
# the Trash, File > Get Info and Locked as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does: Finder's
# menus show it only as symbols.
wizard-uninstall-details-by-hand-not-movable-macos = Your account may not move { $path }: it belongs to another account or to macOS, or its folder is one you can't change.
wizard-uninstall-details-by-hand-locked-macos = { $path } is locked.
wizard-uninstall-details-by-hand-not-the-app-macos = { $path } isn't the { $package } app RABBIT expects there, so RABBIT leaves it alone.
wizard-uninstall-details-by-hand-unchecked-macos = RABBIT couldn't check who may move { $path } ({ $error }).
wizard-uninstall-details-by-hand-no-trash-folder-macos = Only an administrator can move { $path }, and your Trash isn't a folder of your own that RABBIT can check.
wizard-uninstall-details-by-hand-other-disk-macos = Only an administrator can move { $path }, and it is on another disk than your Trash.
wizard-uninstall-details-by-hand-finder-macos = To remove { $package }, select it in Finder and press Command-Delete. Finder asks for an administrator's name and password if it needs one.
wizard-uninstall-details-by-hand-unlock-macos = To remove { $package }, select it in Finder and choose File > Get Info. Untick Locked and close that window, then press Command-Delete. Finder asks for an administrator's name and password if it needs one. If Locked isn't ticked, or Finder still can't move it, it has a lock that Finder can't remove.
wizard-uninstall-details-by-hand-not-the-app-finder-macos = If it is the { $package } you want to remove, select it in Finder and press Command-Delete.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT moves the files into a backup rather than deleting them.
wizard-uninstall-details-dependents-stay = These packages install on top of { $package } and stay: { $packages }. They work again once you install { $package } again.
wizard-uninstall-details-machine-wide = { $package } serves every REAPER and every other program on this computer, so they all lose it.
wizard-uninstall-details-reapack-owned-extensions = ReaPack installed these, and they stay without updates once it is gone: { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT couldn't read ReaPack's records to say what it still manages ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT didn't install this copy. It removes the files { $package } is known to install.
wizard-uninstall-details-reapack-orphan = ReaPack once installed { $package }. ReaPack is gone, so RABBIT can remove it; ReaPack's records of it stay.
# A note on a package whose installer added files outside REAPER's folder,
# when RABBIT has no record of which ones. $package is the localized
# package name.
wizard-uninstall-details-no-vendor-record = RABBIT has no record of which files { $package }'s installer added outside REAPER's folder, so all of those stay.
# The details text of an Uninstall row whose package its own uninstaller
# removes, with administrator rights. The second line follows the reason
# RABBIT can't trust a package's own uninstaller; $package is the localized
# package name.
wizard-uninstall-details-vendor = Windows asks for your permission before anything is removed.
wizard-uninstall-details-vendor-by-hand = You can uninstall { $package } yourself in Windows Settings, under Apps, Installed apps.
# The details text of an Uninstall row whose package's installer left an
# uninstaller folder in Program Files and a registry entry, which RABBIT
# removes with administrator rights once the package's files are in the
# backup. It follows wizard-uninstall-details-files, or takes its place
# when none of the package's files are left to move. $package is the
# localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-machine-cleanup = Only an administrator may remove { $package }'s uninstaller folder in Program Files and its registry entry, so Windows asks for your permission before anything is removed. A backup can't bring those back.
# The details text of an Uninstall row on macOS, by how the package goes:
# deleted for good with an administrator's password; to the Trash, moved as
# you; or into your Trash with an administrator's password, because your
# account may not move it. Name the Trash and Put Back as Finder does in your
# language. The password window may offer Touch ID, so don't say a password
# is the only way. The Trash line follows the package's own sentence, which
# already says that it goes to the Trash, so it only says how to get it back.
# The administrator line follows the package's own sentence, which already
# says a backup can't bring it back, so it doesn't say that again.
wizard-uninstall-details-elevated-macos = macOS asks once for an administrator's name and password, before anything is removed. To have it again, you install it again.
wizard-uninstall-details-trash = Until you empty the Trash, you can drag it back from there.
wizard-uninstall-details-trash-elevated-macos = Your account may not move it, so macOS asks once for an administrator's name and password, before anything is removed, and RABBIT then moves it into your Trash. It keeps the owner it had, so Finder offers no Put Back for it, and emptying the Trash may ask for an administrator's password.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = RABBIT found nothing it can uninstall in this REAPER.
wizard-uninstall-empty-details = None of the software RABBIT knows how to uninstall is installed in this REAPER.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = RABBIT couldn't read its records for this REAPER ({ $error }), so it can't list what it could uninstall.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Settings
wizard-uninstall-remove-settings = Also remove settings
wizard-uninstall-settings-note-none-ticked = Tick a package to see which of its settings RABBIT can remove. Unless you tick this option, settings stay.
wizard-uninstall-settings-note-none-removable = None of the ticked packages has settings RABBIT can remove.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = With this ticked, RABBIT also removes these settings. They go into the backup with the rest, so the backup can bring them back.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package }: { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Now available below the list: { $option }.
wizard-uninstall-settings-unavailable = No longer available: { $option }.

# The Backups tab. The label right before its list names the tab first, as
# the other tabs' labels do; screen readers read it when focus enters the
# list. $size is a size, one of the size-* messages, such as "742 MB". The
# -empty form is for a list with no backups.
wizard-backups-list-label = Backups: what RABBIT can restore or delete, { $size } in all
wizard-backups-list-label-empty = Backups: what RABBIT can restore or delete
# The one row, without a checkbox, of a Backups list with nothing in it, and
# its details text. On macOS, VoiceOver says the row's sentence after the
# tab's name instead.
wizard-backups-empty = RABBIT has no backups for this REAPER.
wizard-backups-empty-details = RABBIT makes a backup when it uninstalls a package, when an update replaces a package's files, and when OSARA's key map replaces yours. None of that has happened in this REAPER yet, or its backups were deleted. What a package's own installer or uninstaller, the Trash or an administrator's password changed never has a backup.
# Stands in for the Backups list when RABBIT can't read its backups folder.
# $error is the error message, in English.
wizard-backups-unreadable = RABBIT couldn't read the backups of this REAPER.
wizard-backups-unreadable-details = RABBIT couldn't read its backups folder ({ $error }), so it can't list what it could restore or delete.
# The group headings of the Backups list, rows without a checkbox, then the
# details text each shows. The details say what backups are for, and what
# they never hold.
wizard-backups-group-removed = Software you uninstalled
wizard-backups-group-removed-details = When RABBIT uninstalls a package, it moves the files here instead of deleting them. Each one is the only copy. What a package's own uninstaller, the Trash or an administrator's password removed isn't here.
wizard-backups-group-versions = Earlier versions
wizard-backups-group-versions-details = When an update replaces a package's files, RABBIT keeps the old ones here. Updates by a package's own installer, such as REAPER's, keep none.
wizard-backups-group-keymaps = Key maps
wizard-backups-group-keymaps-details = When OSARA's key map replaces yours, RABBIT keeps the old one here. Usually only the oldest is your own.
wizard-backups-group-other = Other backups
wizard-backups-group-other-details = RABBIT can't restore these here. You can delete them.
# One row of the Backups list, read as a list row. $backup is what the row
# calls the backup (one of the wizard-backups-name-* messages, or a package's
# name and version), $action is action-restore, action-delete,
# action-delete-only-copy or action-keep, $when one of the
# wizard-backups-when-* messages or backup-date-unknown, and $size a size,
# such as "4 MB".
wizard-backups-row = { $backup }: { $action }. { $when }, { $size }
# The row that stands for every old copy of RABBIT's records, read as a list
# row. $action as above, $count a number, the copies, and $size a size.
wizard-backups-row-records = Old copies of RABBIT's records: { $action }. { $count ->
    [one] One copy
   *[other] { $count } copies
}, { $size }
# A backup named in a sentence on Review, so that two backups with the same
# name can be told apart. $backup and $when as in wizard-backups-row.
wizard-backups-row-reference = { $backup } ({ $when })
# Names joined in a sentence: $rest is the names but the last, separated by
# commas, and $last the last one, as in "FFmpeg, ReaPack and SWS".
wizard-backups-list-and = { $rest } and { $last }
# What a row calls a package's backup: its localized name, $package, and the
# version in the backup, $version, a version number.
wizard-backups-name-package = { $package } { $version }
# What a row calls a key map: your own; one whose kind RABBIT didn't record,
# as in backups from earlier versions of RABBIT and those a restore made;
# REAPER's own default keys; an earlier OSARA key map you hadn't changed.
wizard-backups-name-keymap-own = Your key map
wizard-backups-name-keymap = Key map
wizard-backups-name-keymap-default = REAPER's default keys
wizard-backups-name-keymap-osara = An earlier OSARA key map
# What a row calls the other backups. "RABBIT's records" is RABBIT's list of
# what it installed.
wizard-backups-name-records = Old copies of RABBIT's records
wizard-backups-name-records-copy = A copy of RABBIT's records
wizard-backups-name-not-a-backup = Not a backup RABBIT made
wizard-backups-name-deleting = Unfinished deletion
wizard-backups-name-no-manifest = Backup with no record of what it holds
wizard-backups-name-damaged = Damaged backup
wizard-backups-name-before-restore = Files a restore replaced
wizard-backups-name-update-files = Other files an update replaced
wizard-backups-name-uninstall-files = Other files an uninstall moved
wizard-backups-name-keymap-files = Other files kept with a key map
wizard-backups-name-unknown = Backup from another version of RABBIT
# When a backup was made, as its row says it after the action. $date is a
# date and time (backup-date-time), and $version a version number.
# "Replaced by OSARA's" means by OSARA's key map.
wizard-backups-when-uninstalled = Uninstalled { $date }
wizard-backups-when-replaced-by = Replaced by { $version } on { $date }
wizard-backups-when-replaced = Replaced on { $date }
wizard-backups-when-keymap-osara = Replaced by OSARA's on { $date }
wizard-backups-when-keymap-restore = Replaced by a restore on { $date }
wizard-backups-when-made = Made on { $date }
# A date and time, as rows, details and Review say when a backup was made,
# such as "November 9, 2026, 2:05 PM". $year, $month (1 to 12), $day,
# $hour (0 to 23) and $hour12 (1 to 12) are numbers; $minute is two digits,
# such as "05"; $period is am or pm. Use the 24-hour clock with $hour and
# $minute, or the 12-hour one with $hour12, $minute and $period, as your
# language does. Name the month by the select on $month.
backup-date-time = { $month ->
    [1] January
    [2] February
    [3] March
    [4] April
    [5] May
    [6] June
    [7] July
    [8] August
    [9] September
    [10] October
    [11] November
   *[12] December
} { $day }, { $year }, { $hour12 }:{ $minute } { $period ->
    [am] AM
   *[pm] PM
}
# Instead of a row's when-text, for a backup whose date RABBIT can't tell.
backup-date-unknown = Date unknown
# Sizes, in steps of 1024. $count, $whole and $tenth are numbers. A size in
# GB is $whole and one decimal, $tenth: put your language's decimal mark
# between them.
size-under-kb = under 1 KB
size-kb = { $count } KB
size-mb = { $count } MB
size-gb = { $whole }.{ $tenth } GB
# Why a Backups row can't be ticked for the choice below the list: the short
# phrase that goes into wizard-package-row-unavailable-suffix at the end of
# the row. $package is the localized package name.
wizard-backups-unavailable-not-here = can't restore it here
wizard-backups-unavailable-installed-again = you have { $package } again
wizard-backups-unavailable-not-complete = part of it isn't in the backup
wizard-backups-unavailable-nothing-in-it = none of its files are in this backup
wizard-backups-unavailable-version-unknown = RABBIT doesn't know which version this was
wizard-backups-unavailable-other-language-pack = another language pack is installed
wizard-backups-unavailable-part-missing = part of it is missing
wizard-backups-unavailable-records-unreadable = RABBIT can't read its records
wizard-backups-unavailable-not-yours = belongs to another account
wizard-backups-unavailable-not-a-backup = RABBIT leaves it alone
wizard-backups-unavailable-receipt-in-set = it holds a copy of RABBIT's records
wizard-backups-unavailable-file-refused = RABBIT won't put back one of its files
wizard-backups-unavailable-cannot-restore = RABBIT can't restore it
wizard-backups-unavailable-tied = goes with a backup RABBIT can't delete
# The details text of a Backups row, a sentence or two per line, in this
# order: what the backup is; when it was made; what it holds and how big;
# whether it is the only copy; what restoring does, or why it can't; what
# deleting loses. $backup is what the row calls the backup, $package the
# localized package name, $version a version number, $date a date and time
# (backup-date-time), $size a size, such as "4 MB", and $count a number.
#
# What the backup is.
wizard-backups-details-uninstalled = { $backup }, as it was before you uninstalled it.
wizard-backups-details-earlier = { $backup }, an earlier version.
wizard-backups-details-keymap-own = Your key map, from before OSARA's key map replaced it.
wizard-backups-details-keymap-default = REAPER's default keys, from before OSARA's key map replaced them.
wizard-backups-details-keymap-osara = An earlier OSARA key map, from before a newer one replaced it.
wizard-backups-details-keymap-unknown = A key map from before OSARA's key map replaced it. It may be your own or an older OSARA key map: RABBIT didn't record which.
wizard-backups-details-keymap-restore = The key map a restore replaced. It may be your own or OSARA's.
wizard-backups-details-keymap-other = A key map RABBIT kept.
wizard-backups-details-records = Copies of RABBIT's list of what it installed, from earlier runs. Deleting them changes nothing that is installed.
wizard-backups-details-records-copy = A copy of RABBIT's list of what it installed, from an earlier run.
wizard-backups-details-deleting = A deletion of this backup started and didn't finish, so part of it may be gone already.
wizard-backups-details-not-a-backup = This isn't a backup RABBIT made: it is a link, or a folder whose name RABBIT never gives a backup.
wizard-backups-details-no-manifest = A backup with no record of what it holds, as an earlier RABBIT left one when it stopped early.
wizard-backups-details-damaged = A backup whose record of what it holds can't be read.
wizard-backups-details-before-restore = Files a restore replaced, kept so that the restore could be undone.
wizard-backups-details-update-files = Files an update replaced that no other row here brings back.
wizard-backups-details-uninstall-files = Files an uninstall moved that belong to no package it recorded.
wizard-backups-details-keymap-files = Files kept with a key map that no other row here brings back.
wizard-backups-details-unknown = A backup made by another version of RABBIT, for a reason this version doesn't know.
# When it was made.
wizard-backups-details-when-uninstalled = You uninstalled it on { $date }.
wizard-backups-details-when-updated-to = An update to { $version } replaced it on { $date }.
wizard-backups-details-when-updated = An update replaced it on { $date }.
wizard-backups-details-when-restored-to = Going back to { $version } replaced it on { $date }.
wizard-backups-details-when-restored = A restore replaced it on { $date }.
wizard-backups-details-when-osara = OSARA's key map replaced it on { $date }.
wizard-backups-details-when-keymap-restored = A restored key map replaced it on { $date }.
wizard-backups-details-when-made = RABBIT made this backup on { $date }.
wizard-backups-details-when-records = The newest copy is from { $date }.
wizard-backups-details-when-unknown = RABBIT can't tell when this backup was made.
# What it holds and how big it is. The settings are named only when the
# uninstall recorded that it moved them; backups from earlier versions of
# RABBIT don't say, so the -older form says "anything else".
wizard-backups-details-holds-with-settings = It holds its files and your { $package } settings, { $size }.
wizard-backups-details-holds-files = It holds its files, { $size }.
wizard-backups-details-holds-older = It holds its files and anything else that uninstall moved, { $size }.
wizard-backups-details-holds-replaced = It holds the files that were replaced, { $size }.
wizard-backups-details-holds-keymap = It holds the key map, { $size }.
wizard-backups-details-holds-size = It takes up { $size }.
wizard-backups-details-holds-records = { $count ->
    [one] There is one copy, { $size }.
   *[other] There are { $count } copies, { $size } in all.
}
wizard-backups-details-only-copy = It is the only copy.
# What restoring does, for a backup that can be restored. Name REAPER's
# Actions list as REAPER does in your language. The -uninstalled-files form
# is for a backup that names no version, which may hold no record of the
# package to put back, so it says nothing about RABBIT's records.
wizard-backups-details-restore-uninstalled = Restoring puts them back, and RABBIT counts { $package } as installed again.
wizard-backups-details-restore-uninstalled-files = Restoring puts them back.
wizard-backups-details-restore-stopped-partway = The uninstall stopped partway, so some of its files are only in this backup. Restoring puts them back.
wizard-backups-details-restore-already-back = { $package } is back already. Restoring puts its files back again.
wizard-backups-details-restore-earlier = Restoring goes back to { $version } and keeps the version you have now as a backup, so you can go forward again. Your settings stay as they are.
wizard-backups-details-restore-earlier-copy = Restoring puts this copy back and keeps the version you have now as a backup, so you can go forward again.
wizard-backups-details-restore-earlier-already-back = { $package } is at this version already. Restoring puts its files back again.
wizard-backups-details-restore-keymap = Restoring makes it REAPER's key map again and keeps the current one as a backup. Scripts you installed since stay in REAPER's Actions list.
wizard-backups-details-restore-default-keys = Restoring brings back REAPER's own keys, not a key map of yours, and keeps the current key map as a backup.
# Why it can't be restored, by what RABBIT found. Name the Uninstall tab as
# wizard-packages-tab-uninstall does.
wizard-backups-details-not-here = RABBIT can't restore it here.
wizard-backups-details-cannot-restore = RABBIT can't restore it.
wizard-backups-details-status-installed-again = You have { $package } again. Uninstall the one you have first, on the Uninstall tab, then restore.
wizard-backups-details-status-not-complete = Part of it isn't in the backup, so RABBIT can't go back to it.
wizard-backups-details-status-nothing-in-it = None of its files are in this backup, so there is nothing to restore.
wizard-backups-details-status-version-unknown = RABBIT doesn't know which version this was, so it can't go back to it.
wizard-backups-details-status-other-language-pack = Another language pack is installed, and installing one removes the other, so RABBIT doesn't bring this one back while it is.
wizard-backups-details-status-part-missing = Part of it is missing from the backup, so RABBIT can't restore it.
wizard-backups-details-status-records-unreadable = RABBIT can't read its records, so it can't tell whether this backup fits them.
wizard-backups-details-status-not-yours = It belongs to another account, so RABBIT neither restores nor deletes it.
wizard-backups-details-status-not-a-backup = RABBIT never looks inside it, and neither restores nor deletes it.
# macOS: how to remove another account's backup by hand. The backups are in
# the hidden Library folder, so the steps go through Finder's Go menu.
# $folder is the folder that holds the backup, a full macOS path, and $name
# the backup's own folder name; keep both as they are. Name Finder, its Go
# menu and the menu's Go to Folder item as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does. Never
# suggest sudo.
wizard-backups-details-not-yours-finder-macos = To remove it, in Finder choose Go > Go to Folder, enter { $folder }, then select { $name } and press Command-Delete. Finder asks for an administrator's name and password if it needs one.
# What deleting it loses.
wizard-backups-details-delete-uninstalled = If you delete this backup, RABBIT can't bring this copy back.
wizard-backups-details-delete-earlier = If you delete this backup, you can't go back to { $version }.
wizard-backups-details-delete-earlier-copy = If you delete this backup, you can't go back to this copy.
wizard-backups-details-delete-keymap = If you delete this backup, RABBIT can't bring this key map back.
wizard-backups-details-delete-already-back = Its files are on disk as well, so deleting it loses nothing.
wizard-backups-details-delete-other = If you delete it, it is gone for good.
wizard-backups-details-delete-deleting = Deleting it finishes that deletion.
# For rows that go together until a later version of RABBIT: one backup
# holds them all, and RABBIT deletes whole backups. $backups is a list of
# what the other rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-details-together = { $origin ->
    [uninstall] This uninstall's backup also holds { $backups }.
    [update] This update's backup also holds { $backups }.
   *[other] The same backup also holds { $backups }.
} Until a later version, RABBIT deletes them together.
# After the sentence above, when one of those rows can't be deleted.
wizard-backups-details-tied = One of them can't be deleted, so RABBIT can't delete this one either.
# The choice below the Backups list, Restore or Delete, keeps its own ticks.
# After a switch, VoiceOver on macOS says how many backups are ticked for the
# new choice. $choice is restore or delete, and $count a number.
wizard-backups-ticked-macos = { $choice ->
    [delete] { $count ->
        [0] No backups ticked to delete.
        [one] One backup ticked to delete.
       *[other] { $count } backups ticked to delete.
    }
   *[restore] { $count ->
        [0] No backups ticked to restore.
        [one] One backup ticked to restore.
       *[other] { $count } backups ticked to restore.
    }
}
# Said by VoiceOver on macOS after "Checked" or "Unchecked", when ticking a
# row to delete ticks or unticks the rows from the same backup too, such as
# "SWS 2.14.0.7 from the same uninstall is ticked too." $backups is a list
# of what those rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-also-ticked-macos = { $backups } from the same { $origin ->
    [uninstall] uninstall
    [update] update
   *[other] backup
} { $count ->
    [one] is
   *[other] are
} ticked too.
wizard-backups-also-unticked-macos = { $backups } from the same { $origin ->
    [uninstall] uninstall
    [update] update
   *[other] backup
} { $count ->
    [one] is
   *[other] are
} unticked too.
# Why a restore of a backup is refused, a sentence or two each, about the
# backup, which they call "it". Review puts one after "Can't restore <backup>
# yet.", and a row's details after what the backup is. $package is a
# localized package name, $packages a list of them, $path a file or folder
# as the computer writes it, keep it as it is, $detail an error message in
# English, $reason one of the wizard-restore-target-* phrases, and $programs
# a comma-separated list of program names, never translated. Name the
# Uninstall tab as wizard-packages-tab-uninstall does.
wizard-restore-refusal-invalid-id = Its name isn't one RABBIT gives a backup.
wizard-restore-refusal-not-found = It was deleted, or never existed. Start RABBIT again to see the backups as they are now.
wizard-restore-refusal-differs-in-case = RABBIT finds it only under a name in other upper and lower case.
wizard-restore-refusal-ends-in-dot-or-space = RABBIT finds it only under a name without the dot or space at its end.
wizard-restore-refusal-being-deleted = A deletion of it started and didn't finish, so part of it may be gone.
wizard-restore-refusal-is-link = RABBIT doesn't use { $path }: it is a symbolic link or junction, and RABBIT never follows one in its backups.
wizard-restore-refusal-no-manifest = It has no record of which of its files belong to { $package }.
wizard-restore-refusal-no-package-records = It records no packages, so RABBIT can't restore { $package } from it on its own.
wizard-restore-refusal-package-not-in-set = It holds no record of { $package }.
wizard-restore-refusal-no-keymap-in-set = It holds no key map that RABBIT can restore.
wizard-restore-refusal-damaged-manifest = RABBIT can't read its record of what it holds ({ $detail }).
wizard-restore-refusal-records-only = It holds only a copy of RABBIT's records, so there is nothing in it to restore.
wizard-restore-refusal-receipt-in-set = It holds a copy of RABBIT's records. Copying that back would undo every change to them since, so RABBIT doesn't restore it.
wizard-restore-refusal-reinstalled-since = You have { $package } again, a copy installed after this backup was made. Uninstall the one you have first, on the Uninstall tab, then restore.
wizard-restore-refusal-nothing-of-it-in-set = None of the files of { $packages } are in it, so there is nothing to restore.
# Instead of the sentence above, when nothing in a backup of earlier
# versions belongs to a package.
wizard-restore-refusal-nothing-of-it-in-set-none = Nothing in it belongs to a package RABBIT can put back.
wizard-restore-refusal-part-missing = Part of it is missing: its copy of { $path } is gone, or isn't the file it recorded.
wizard-restore-refusal-not-complete = It isn't complete: { $path } is neither in the backup nor on disk as that version had it.
wizard-restore-refusal-version-unknown = RABBIT doesn't know which version of { $package } it holds.
wizard-restore-refusal-other-language-pack-installed = RABBIT doesn't bring { $package } back while this is installed: { $packages }. Installing one of them removes the other.
wizard-restore-refusal-target-refused = RABBIT doesn't put back { $path }: { $reason }.
wizard-restore-refusal-newer-file-refused = Going back would take { $path }, a file of the newer version of { $package }, into a backup it couldn't come back from: { $reason }.
wizard-restore-refusal-earlier-file-refused = Its record of that version of { $package } lists { $path }, a place RABBIT never installs to: { $reason }.
wizard-restore-refusal-receipt-unreadable = RABBIT can't read its records ({ $detail }), so it can't tell whether this backup fits them.
wizard-restore-refusal-reaper-running = REAPER is open. Close it, then try again.
# The same, for packages every REAPER on the computer can use, such as
# app2clap. $packages is a list of localized package names.
wizard-restore-refusal-reaper-running-every = A REAPER is open. Close every copy of REAPER on this computer, including portable ones, then try again. Every REAPER can use { $packages }.
wizard-restore-refusal-keymap-needs-reaper-closed = It puts back REAPER's key map, which RABBIT does only while REAPER is closed.
wizard-restore-refusal-program-running = Close these programs, then try again: { $programs }.
wizard-restore-refusal-not-writable = RABBIT can't write where the restore puts files back, in { $path } ({ $detail }).
wizard-restore-refusal-lock-held = Another copy of RABBIT is working on this REAPER. Wait until it has finished.
# sudo and root are never translated.
wizard-restore-refusal-as-root = RABBIT is running as root, as it does under sudo, and doesn't restore backups like that. Quit RABBIT and start it as yourself.
wizard-restore-refusal-not-yours = It belongs to another account, so RABBIT doesn't restore it.
wizard-restore-refusal-changed-while-read = It changed while RABBIT read it, as when another copy of RABBIT was still writing it.
wizard-restore-refusal-changed-since-listed = It changed since RABBIT listed it. Start RABBIT again to see the backups as they are now.
# Why a restore won't put a file back at a place: the phrase that ends
# wizard-restore-refusal-target-refused and the two after it. Start in lower
# case where your language allows. $path is a file or folder, keep it as it
# is; $name a file or folder name; $detail an error message in English.
wizard-restore-target-rabbit-folder = RABBIT never puts a file back into its own folder
wizard-restore-target-leads-into-rabbit-folder = the way to it leads into RABBIT's own folder
wizard-restore-target-name-not-as-written = the name { $name } on the way to it may open another file or folder than it names
wizard-restore-target-place-is-link = it is a symbolic link or junction, which RABBIT never replaces
wizard-restore-target-through-link = the way to it leads through { $path }, a symbolic link or junction, which RABBIT never follows
wizard-restore-target-leads-outside-resource-folder = the way to it leads outside the resource folder, { $path }
wizard-restore-target-leads-outside = the way to it leads outside { $path }
wizard-restore-target-outside-allowed-folders = it is outside REAPER's resource folder and every other folder RABBIT installs into
wizard-restore-target-not-plain-path = it isn't a plain path inside the resource folder
wizard-restore-target-guard-folder-gone = the folder it goes back into, { $path }, is gone
wizard-restore-target-could-not-check = RABBIT couldn't check { $path } ({ $detail })
wizard-restore-target-two-files-for-one-place = the backup holds more than one file for this place
wizard-restore-target-outside-without-manifest = the backup has no record of where this file from outside the resource folder belongs
wizard-restore-target-copy-outside-set = the backup's record names its copy outside the backup
wizard-restore-target-keymap-among-version-files = it is REAPER's key map, which RABBIT puts back only as a key map of its own

# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = Unavailable: requires { $package } to be installed.
wizard-configuration-row-already-applied = Already applied on this REAPER target.
# Short status tag appended in parentheses to a configuration row's tree label
# when the row isn't actionable. Kept terse so the tree label stays readable;
# the longer sentence in `wizard-configuration-row-unavailable` /
# `wizard-configuration-row-already-applied` is still surfaced in the details
# pane and as the row's accessible reason.
# $reason is one of the "wizard-configuration-row-status-*" strings below.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package is the localized name of the dependency package.
wizard-configuration-row-status-requires = requires { $package }
wizard-configuration-row-status-already-applied = already applied
# Names the dependency of a configuration step that any one of several
# packages satisfies, so the step doesn't advertise one arbitrary package.
config-dependency-language-pack = a language pack
config-reapack-reaper-accessibility-name = Add Toni's REAPER Accessibility ReaPack repository
config-reapack-reaper-accessibility-description = Adds Ttoni Barth's REAPER Accessibility ReaPack repository (https://github.com/Timtam/reapack/raw/master/index.xml). Once added, go to the Extensions menu, ReaPack, Browse Packages to get extra accessible scripts and plug-ins.
config-reapack-reaper-accessible-fr-name = Add the REAPER Accessible (FR) ReaPack repository
config-reapack-reaper-accessible-fr-description = Adds the REAPER Accessible French-language ReaPack repository (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Once added, go to the Extensions menu, ReaPack, Browse Packages to get REAPER Accessible's French resources.
config-reapack-reaper-accessible-en-name = Add the REAPER Accessible (EN) ReaPack repository
config-reapack-reaper-accessible-en-description = Adds the REAPER Accessible English-language ReaPack repository (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Once added, go to the Extensions menu, ReaPack, Browse Packages to get REAPER Accessible's English resources.
config-set-reaper-language-name = Set REAPER's language
config-set-reaper-language-description = Tells REAPER which of the installed language packs to use, by writing it into reaper.ini. Without this you would have to select the language yourself under Options, Preferences, General. You can install several language packs and switch between them in REAPER later; this only chooses which one is active now. Untick if you only want the files installed.

wizard-reapack-ack-heading = ReaPack donation notice
wizard-reapack-ack-body = ReaPack is free software released under the LGPL. Its author Christian Fillion accepts optional donations to support continued development. Christian also maintains the SWS extensions and has landed code specifically to improve compatibility with OSARA in the past. Any support you can send has been well earned.
wizard-reapack-ack-link-label = Open the ReaPack donation page
wizard-reapack-ack-confirm-label = Skip donation this time, just install or update ReaPack
cli-reapack-ack-prompt-summary = ReaPack is free software (LGPL). Donations to its author Christian Fillion accepts optional donations at https://reapack.com/donate to support ongoing development.
cli-reapack-ack-flag-required = ReaPack is in this run's plan but the donation acknowledgement is missing. Re-run with --accept-reapack-donation-notice to confirm you have read https://reapack.com/donate and want RABBIT to install or update ReaPack.

wizard-version-check-heading = Checking latest versions
wizard-version-check-status-pending = Preparing latest-version check…
# $package is the localized package display name.
wizard-version-check-status-checking = Checking { $package }…
wizard-version-check-status-progress = Checking versions… ({ $done } of { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } version check(s) failed. Use Back to try a different target, or close RABBIT.
wizard-version-check-progress-label = Progress
wizard-version-check-error-heading = Failed checks
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Package details
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = What's new in { $package }:
wizard-packages-osara-keymap-heading = OSARA key map
wizard-packages-osara-keymap-replace-label = Replace your current key map with latest OSARA key map
packages-reaper-language-label = REAPER language after installation
packages-spanish-variant-label = Spanish OSARA translation
wizard-packages-osara-keymap-unavailable-note = Select OSARA to configure its key map behavior.
wizard-packages-osara-keymap-preserve-note = Your current key map stays as it is: RABBIT won't touch reaper-kb.ini. To add the new OSARA key bindings while keeping your own, run OSARA's "Merge OSARA key map" action afterwards (Extensions menu, OSARA).
wizard-packages-osara-keymap-replace-note = RABBIT will back up your current reaper-kb.ini, if there is one, and replace it with the latest OSARA key map. Key bindings you changed yourself are not carried over; the backup keeps them.
wizard-packages-osara-keymap-unchanged-note = Your key map is the OSARA key map with no changes of your own, so RABBIT will back it up and replace it with the latest OSARA key map. Your installed scripts stay in the action list.
wizard-packages-osara-keymap-default-keys-note = You're using REAPER's default key bindings rather than the OSARA key map, so RABBIT leaves them as they are. To switch to the OSARA key map, tick the box above.
wizard-package-details-handling-prefix = Handling
wizard-package-handling-automatic = RABBIT can install this package directly.
wizard-package-handling-unattended = RABBIT can install this package unattended, including launching its installer when required.
wizard-package-handling-planned = RABBIT is designed to run this package's installer or setup routine itself and finish the installation unattended, but this build still reports the steps instead of executing them.
wizard-package-handling-manual = RABBIT will download this package and report the manual steps after the run.
wizard-package-handling-unavailable = This package is not available for the selected platform or architecture.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package }: { $action }. You have { $installed }. Latest is { $available }

wizard-review-heading = Review what you've asked RABBIT to do
wizard-review-target-prefix = Target
wizard-review-package-heading = Selected packages
wizard-review-osara-keymap-heading = OSARA key map
wizard-review-osara-keymap-preserve = Preserve your current key map.
wizard-review-osara-keymap-replace = Backup your current key map then replace with the latest from OSARA.
wizard-review-osara-keymap-replace-unchanged = Back up your unchanged OSARA key map, then replace it with the latest from OSARA.
wizard-review-osara-keymap-keep-default-keys = Keep REAPER's default key bindings.
wizard-review-notes-heading = Notes
wizard-review-preflight-prefix = Cannot install yet

# $path is the selected REAPER resource path.
wizard-review-target = Target: { $path }
wizard-review-no-target = No target selected.
wizard-review-no-package = No package selected.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Ready to uninstall
wizard-review-uninstall-packages = Packages to uninstall
wizard-review-uninstall-no-package = Nothing is ticked to uninstall.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package }: { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = Its settings go too, into the backup: { $settings }.
wizard-review-uninstall-close-programs-note = Close other music programs first.
# Review's permission note, when a ticked package needs administrator rights.
# Then one line per ticked package its own uninstaller removes; $package is
# the localized package name.
wizard-review-uninstall-elevation-note-windows = Windows asks for your permission once, before anything is removed. If you don't allow it, nothing is uninstalled.
wizard-review-uninstall-vendor-window-note = If a window from { $package }'s own uninstaller appears, answer it.
# Review's permission note on macOS, when a ticked package needs an
# administrator. $packages is a list of package names. osascript is a program
# name, never translated: macOS names it in the password window. The window
# may offer Touch ID, so don't say a password is the only way. Use the same
# words for 'administrator command' as in the Done and progress lines that
# name it.
wizard-review-uninstall-elevation-note-macos = Before anything is removed, macOS asks once for an administrator's name and password. It may offer Touch ID instead. The window may name osascript, the tool RABBIT asks through. With that password, RABBIT runs one administrator command that uninstalls { $packages }. If you cancel the window, nothing is uninstalled.
# One line per ticked package that goes to the Trash; the second when your
# account may not move it, so an administrator's password moves it there.
# $package is the localized package name. Name the Trash and Put Back as
# Finder does in your language.
wizard-review-uninstall-trash-note = { $package } goes to the Trash. Until you empty the Trash, you can drag it back from there.
wizard-review-uninstall-trash-elevated-note-macos = { $package } goes into your Trash with the administrator's password. It keeps the owner it had, so Finder offers no Put Back for it, and emptying the Trash may ask for an administrator's password.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. RABBIT leaves out what it ticked by itself, such as updates, but the
# list can still hold a setting RABBIT ticked along with one of the user's
# ticks, so the note doesn't say who ticked it. $packages is a comma-separated
# list of names: packages, and for the Install or update tab also settings,
# such as "Set REAPER's language".
wizard-review-inactive-tab-note-install = What is ticked on the Install or update tab isn't part of this run: { $packages }.
wizard-review-inactive-tab-note-uninstall = What is ticked on the Uninstall tab isn't part of this run: { $packages }.
wizard-review-uninstall-reinstall-hint = You can install these again later from the Install or update tab.
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Can't uninstall yet: { $reason }
wizard-review-uninstall-not-writable = RABBIT can't write to this REAPER's folder.

# Review before a restore from the Backups tab. The heading names what the
# run does, so it is the first thing read.
wizard-review-restore-heading = Ready to restore
# One line per ticked backup. $backup is what its row calls it, $package the
# localized package name, $date a date and time (backup-date-time), and
# $settings yes or no: whether the backup holds the package's settings too.
wizard-review-restore-uninstalled = { $settings ->
    [yes] { $backup } comes back as it was on { $date }, with your { $package } settings.
   *[no] { $backup } comes back as it was on { $date }.
}
wizard-review-restore-uninstalled-undated = { $settings ->
    [yes] { $backup } comes back as it was before you uninstalled it, with your { $package } settings.
   *[no] { $backup } comes back as it was before you uninstalled it.
}
# An earlier version. $version is the version it goes back to, and $current
# the one it has now, both version numbers.
wizard-review-restore-earlier = { $package } goes back from { $current } to { $version }. RABBIT keeps { $current } as a backup.
wizard-review-restore-earlier-to = { $package } goes back to { $version }. RABBIT keeps the version you have now as a backup.
wizard-review-restore-earlier-copy = { $package } goes back to the copy an update replaced. RABBIT keeps the version you have now as a backup.
# A key map. $origin is own for your own key map, default-keys for REAPER's
# default keys, and other for any other; $date as above.
wizard-review-restore-keymap = { $origin ->
    [own] Your key map from { $date } replaces the current one, which RABBIT keeps as a backup.
    [default-keys] REAPER's default keys from { $date } replace the current key map, which RABBIT keeps as a backup.
   *[other] The key map from { $date } replaces the current one, which RABBIT keeps as a backup.
}
wizard-review-restore-keymap-undated = { $origin ->
    [own] Your key map replaces the current one, which RABBIT keeps as a backup.
    [default-keys] REAPER's default keys replace the current key map, which RABBIT keeps as a backup.
   *[other] This key map replaces the current one, which RABBIT keeps as a backup.
}
# Notes on what the restore changes besides the files. $file is a language
# pack's file name, such as de_DE.ReaperLangPack, never translated; $package
# is the localized package name.
wizard-review-restore-language-back = REAPER's language goes back to { $file }.
wizard-review-restore-language-stays = REAPER's language stays as it is, because another one was chosen since.
wizard-review-restore-not-counted = RABBIT no longer counts { $package } as installed by it.
wizard-review-restore-needs-reinstall = To uninstall { $package } with RABBIT later, install it again first.
# When a REAPER that has to be closed is running; the second when a ticked
# package serves every REAPER on the computer.
wizard-review-restore-close-reaper = Close REAPER first.
wizard-review-restore-close-every-reaper = Close every copy of REAPER first.
# The backups ticked for the other choice below the list stay ticked, but
# aren't part of this run. $count is a number.
wizard-review-restore-other-choice-note = { $count ->
    [one] One backup ticked to delete isn't part of this run.
   *[other] { $count } backups ticked to delete aren't part of this run.
}
# Why the Restore button is disabled. $reason is a sentence, such as
# wizard-review-restore-nothing-ticked. The -backup form names a backup that
# can't come back: $backup as in wizard-backups-row-reference, and $reason
# one of the wizard-restore-refusal-* sentences.
wizard-review-restore-blocked = Can't restore yet: { $reason }
wizard-review-restore-blocked-backup = Can't restore { $backup } yet. { $reason }
wizard-review-restore-nothing-ticked = Nothing is ticked to restore.
wizard-review-restore-not-writable = RABBIT can't write to this REAPER's folder.
# $first and $second name two ticked backups, as in
# wizard-backups-row-reference; $package is the localized package name.
wizard-review-restore-same-package = { $first } and { $second } are both backups of { $package }. Tick only one of them.
wizard-review-restore-same-file = { $first } and { $second } both put back the same file. Tick only one of them.
# When RABBIT couldn't check the restore before it starts: one backup, or
# all of them. $error is the error message, in English.
wizard-review-restore-plan-failed = RABBIT couldn't check it ({ $error }).
wizard-review-restore-check-failed = RABBIT couldn't check the restore ({ $error }).
# When RABBIT hasn't checked the backups ticked now, so the restore can't
# start yet.
wizard-review-restore-not-checked = RABBIT hasn't checked what is ticked.
# Review before a delete from the Backups tab.
wizard-review-delete-heading = Ready to delete backups
# $count is a number, the ticked backups, and $size a size, such as "520 MB".
wizard-review-delete-intro = { $count ->
    [one] This backup goes for good and frees { $size }:
   *[other] These backups go for good and free { $size }:
}
# One line per ticked backup, saying what is lost with it. $backup is what
# its row calls it, and $date a date and time (backup-date-time).
wizard-review-delete-uninstalled = { $backup }, uninstalled { $date }: the only copy.
wizard-review-delete-keymap = { $backup } from { $date }: the only copy.
wizard-review-delete-only-copy = { $backup }: the only copy.
wizard-review-delete-earlier = { $backup }: you can't go back to it after this.
wizard-review-delete-already-back = { $backup }: its files are on disk as well, so nothing is lost.
wizard-review-delete-records = { $backup }: nothing that is installed changes.
wizard-review-delete-deleting = { $backup }: this finishes a deletion that started earlier.
wizard-review-delete-other = { $backup }: it is gone for good.
# Until a later version, backups from one set go together. $backups is a
# list of what their rows call them, $count how many there are, a number, 2
# or more, $origin uninstall, update or other, for what made the backup, and
# $date a date and time (backup-date-time). The -undated form is for a
# backup whose date RABBIT can't tell.
wizard-review-delete-together = { $origin ->
    [uninstall] { $backups }, uninstalled { $date }, go together
    [update] { $backups }, replaced { $date }, go together
   *[other] { $backups }, from { $date }, go together
}: { $count ->
    [2] one backup holds both.
   *[other] one backup holds all { $count }.
}
wizard-review-delete-together-undated = { $count ->
    [2] { $backups } go together: one backup holds both.
   *[other] { $backups } go together: one backup holds all { $count }.
}
# $count is a number.
wizard-review-delete-other-choice-note = { $count ->
    [one] One backup ticked to restore isn't part of this run.
   *[other] { $count } backups ticked to restore aren't part of this run.
}
# Why the Delete button is disabled. $reason is a sentence.
wizard-review-delete-blocked = Can't delete yet: { $reason }
wizard-review-delete-nothing-ticked = Nothing is ticked to delete.

wizard-progress-heading = Installation progress
wizard-progress-status-idle = Ready to install.
wizard-progress-status-running = Installing selected packages. This might take a few minutes.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Stopping. RABBIT is finishing the step it started, then it closes.
wizard-progress-details-label = Progress details
wizard-progress-details-idle = No installation is running.
wizard-progress-details-starting = Starting setup operation.
wizard-progress-details-cache-prefix = Cache
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops once the step it is on has finished: one package,
# or on macOS the administrator command, which removes several at once.
wizard-progress-uninstall-heading = Uninstalling
wizard-progress-status-uninstall-running = Uninstalling the ticked packages.
wizard-progress-details-uninstall-starting = Starting to uninstall.
wizard-progress-status-uninstall-cancelling = Stopping. RABBIT is finishing the step it started, then it closes.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Downloading { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Downloading { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Downloading { $count } packages… { $downloaded } / { $total }
wizard-progress-status-installing = Installing { $package }…
wizard-progress-status-removing = Uninstalling { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Applying configuration step: { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = Downloading { $package }…
wizard-progress-log-download-completed = Downloaded { $package }.
wizard-progress-log-install-started = Installing { $package }…
wizard-progress-log-install-completed = Installed { $package }.
wizard-progress-log-remove-started = Uninstalling { $package }…
wizard-progress-log-remove-completed = Uninstalled { $package }.
wizard-progress-log-remove-failed = Couldn't uninstall { $package }.
# For a package its own uninstaller removes: the status and log line while
# it runs, the line said once when it runs for a long time, and the line
# said once when its files are still there long after it ended. $package
# is the localized package name. Name Alt+Tab as Windows does.
wizard-progress-status-vendor-running = Running { $package }'s own uninstaller…
wizard-progress-status-vendor-still-waiting = Still waiting for { $package }'s own uninstaller. It may be showing a question in another window; press Alt+Tab to find it.
wizard-progress-status-vendor-waiting-for-files = { $package }'s own uninstaller has finished, and RABBIT is waiting for its files to go. If another music program has { $package } open, close it.
# On macOS, said right before the administrator password window appears, and
# again if it has been open a while without an answer. RABBIT puts each on the
# status line and in the log too, in case VoiceOver cuts it off. $packages is
# a list of package names. Use the same words for 'administrator command' as
# in the Done and progress lines that name it.
wizard-progress-status-admin-prompt-macos = macOS now asks for an administrator's name and password, once. With it, RABBIT runs one administrator command that uninstalls { $packages }. If you cancel, nothing is uninstalled.
wizard-progress-status-admin-still-waiting-macos = Still waiting for the administrator password window. If you don't hear it, check whether it is behind RABBIT.
# While a package goes to the Trash, and in the log once the administrator
# command has worked on a package. $package is the localized package name.
# Name the Trash as Finder does.
wizard-progress-status-trashing = Moving { $package } to the Trash…
wizard-progress-log-admin-done = The administrator command has finished with { $package }.
wizard-progress-log-configuration-started = Applying { $step }…
wizard-progress-log-configuration-completed = Applied { $step }.

wizard-done-heading = Done
wizard-done-status-idle = No installation has been run from this window yet.
wizard-done-status-success = RABBIT finished working its magic! Review the details below.
wizard-done-status-error = Installation failed. Review the error below.
wizard-reaper-running-title = REAPER is still open
wizard-reaper-running-body = Close REAPER, then press Install again. RABBIT can't update REAPER or its extensions while REAPER is running.
wizard-error-reaper-running = RABBIT stopped because REAPER was open. Close REAPER, then run RABBIT again.
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Another copy of RABBIT is installing or uninstalling in this REAPER. Wait until it has finished, then run RABBIT again.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Close REAPER, then press Uninstall again. RABBIT can't uninstall anything from REAPER while it is running.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Close every copy of REAPER on this computer, including portable ones, then press Uninstall again. Every REAPER on this computer can use { $packages }.
# The same when REAPER itself is the only such package: its own uninstaller
# needs every REAPER closed.
wizard-reaper-running-uninstall-reaper-body = Close every copy of REAPER on this computer, including portable ones, then press Uninstall again. REAPER's own uninstaller doesn't run while any REAPER is open.
# The same on macOS, when RABBIT's administrator command is to move REAPER's
# app: it can't tell one REAPER from another.
wizard-reaper-running-uninstall-reaper-body-macos = Close every copy of REAPER on this computer, including portable ones, then press Uninstall again. The administrator command that moves REAPER can't tell one REAPER from another, so all of them have to be closed.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Close these programs first
wizard-programs-running-uninstall-body = Close { $programs }, then press Uninstall again.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT stopped because a REAPER was open. Close every copy of REAPER on this computer, including portable ones, then run RABBIT again.
wizard-error-uninstall-not-enough-space = RABBIT stopped because there isn't enough free space for the backup. Free some space on the drive that holds REAPER's resource folder, then run RABBIT again.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } wasn't uninstalled, because another program is using { $path }. Close that program, then run RABBIT again.
# The same, for a file a package's installer added outside REAPER's folder.
# Only the JAWS scripts' installer adds such files, and JAWS itself may have
# one open; the program to close would be the user's screen reader, so this
# says to wait and try again instead. $package is the localized package
# name, and $path the file's full path as Windows holds it. Keep JAWS as it
# is.
wizard-error-uninstall-vendor-file-in-use = { $package } wasn't uninstalled, because JAWS or another program is using { $path }. Wait a moment, then run RABBIT again. If that doesn't help, restart JAWS first.
# Why an uninstall didn't start on macOS: RABBIT itself runs as root, as under
# sudo. sudo and root are never translated.
wizard-error-uninstall-as-root = RABBIT is running as root, as it does under sudo, so it uninstalls nothing. Quit RABBIT and start it as yourself. It asks for an administrator's password by itself when it needs one.
wizard-done-status-completed-with-errors = Installation completed with errors. Review the details below.
wizard-done-status-cancelled = You stopped the installation. The list below shows what RABBIT finished before it stopped.
wizard-done-status-no-packages = No package was selected for installation or update.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = RABBIT uninstalled { $packages }.
wizard-done-uninstall-status-completed-with-errors = RABBIT uninstalled { $packages }, but not everything went as planned.
wizard-done-uninstall-status-error = Uninstalling failed.
wizard-done-uninstall-status-cancelled = You stopped the uninstall. Packages RABBIT hadn't reached yet are still installed.
wizard-done-uninstall-status-no-packages = Nothing was uninstalled.
# The Done page's first line when a removal finishes only after a restart,
# and when the Windows permission prompt was declined.
wizard-done-uninstall-status-pending-restart = RABBIT uninstalled { $packages }. Restart your computer to finish.
wizard-done-uninstall-status-declined = Nothing was uninstalled, because the Windows permission prompt was declined.
# The same on macOS, where the administrator password window was cancelled.
wizard-done-uninstall-status-declined-macos = Nothing was uninstalled, because the administrator password window was cancelled.
# The Done page's first line when the administrator command removed only part
# of a package and nothing else was uninstalled. $packages is a list of
# package names.
wizard-done-uninstall-status-partly-removed = RABBIT removed only part of { $packages }.
wizard-done-uninstall-removed-before-stop = Before you stopped it, RABBIT uninstalled { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = To get one back, run RABBIT again and tick it on the Install or update tab.
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } wasn't installed any more, so there was nothing to remove.
wizard-done-uninstall-left-in-backup = Some files of { $package } couldn't be put back and are still in the backup. Show details lists them.
wizard-done-uninstall-leftovers = RABBIT still finds some of { $package } and leaves it alone. Show details lists what.
wizard-done-uninstall-language-reset-failed = RABBIT couldn't set REAPER back to English after removing { $package }. Choose a language in REAPER's preferences.
wizard-done-uninstall-registry-failed = RABBIT uninstalled { $package }, but Windows' list of installed programs may still show it. Show details names the entry.
# After a package's own uninstaller ran, or was to run. $package is the
# localized package name, $code a number, $error an error message in
# English, $path a full path, and $paths a comma-separated list of full
# paths. Name "Show details" as wizard-done-show-details does, and Alt+Tab
# as Windows does.
wizard-done-uninstall-pending-restart = Restart your computer to finish removing { $package }.
wizard-done-uninstall-vendor-cancelled = { $package }'s own uninstaller was cancelled before it finished.
wizard-done-uninstall-vendor-failed = { $package }'s own uninstaller ended with error code { $code }. Show details lists what is still there.
wizard-done-uninstall-vendor-failed-unsure = { $package }'s own uninstaller ended with error code { $code }, so { $package } may not be fully removed.
wizard-done-uninstall-vendor-still-there = { $package }'s own uninstaller finished, but { $package } is still installed. You can uninstall it in Windows Settings, under Apps, Installed apps.
wizard-done-uninstall-vendor-still-open = { $package }'s own uninstaller is still open, so RABBIT stopped and left the packages after it alone. Press Alt+Tab to find its window, and answer it.
wizard-done-uninstall-vendor-run-failed = RABBIT couldn't finish running { $package }'s own uninstaller ({ $error }), so it stopped there.
wizard-done-uninstall-elevation-failed = RABBIT couldn't get administrator rights ({ $error }), so nothing was uninstalled.
wizard-done-uninstall-resource-damaged = Important: after { $package }'s own uninstaller ran, these files were missing from REAPER's resource folder: { $paths }. RABBIT stopped and wrote its records again. It can't bring REAPER's settings back; if you have a copy of reaper.ini, put it back before you start REAPER.
wizard-done-uninstall-receipt-not-saved = RABBIT uninstalled { $package }, but couldn't update its records. The next time RABBIT runs, it finds { $package } gone all the same.
wizard-done-uninstall-vendor-lost = RABBIT lost track of { $package }'s own uninstaller after it started. It may still be running, so RABBIT can't tell what it removed, and stopped there.
wizard-done-uninstall-vendor-lost-reaper = Once the uninstaller has finished, check that reaper.ini is still in REAPER's resource folder: it holds your REAPER settings.
wizard-done-uninstall-vendor-files-left = { $package }'s own uninstaller finished, but some of its files are still there. Show details lists them.
wizard-done-uninstall-vendor-not-confirmed = { $package }'s own uninstaller finished, but RABBIT can't confirm that { $package } is gone.
wizard-done-uninstall-vendor-not-listed = Windows no longer lists { $package } among the installed programs, so it isn't in Windows Settings either.
wizard-done-uninstall-working-folder-left = RABBIT couldn't remove { $path }, the folder it ran { $package }'s uninstaller from. Only an administrator can delete it.
wizard-done-uninstall-restart-list-unreadable = RABBIT couldn't read which files Windows removes at the next restart, so it can't tell whether the files of { $package } that are left go then.
wizard-done-uninstall-installed-list-unreadable = RABBIT couldn't read Windows' list of installed programs after { $package }'s uninstaller, so it couldn't check whether Windows still lists { $package }.
# After the elevated helper was to remove the uninstaller folder and
# registry entry a package's installer left. $package is the localized
# package name, and $error Windows' own error message or RABBIT's in
# English. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does, and "Show
# details" as wizard-done-show-details does.
wizard-done-uninstall-cleanup-refused = RABBIT checked { $package }'s uninstaller folder in Program Files and its registry entry again and didn't trust what it found, so it put back what it had moved.
wizard-done-uninstall-cleanup-failed = RABBIT couldn't remove all of { $package }'s uninstaller folder and registry entry ({ $error }), so it put back what it had moved. Show details lists what is already gone.
wizard-done-uninstall-cleanup-lost = RABBIT lost contact with its helper while it removed { $package }'s uninstaller folder and registry entry ({ $error }), so it put back what it had moved. Part of that folder or its registry entry may be gone.
# Instead of wizard-done-uninstall-cleanup-failed, when RABBIT removed
# nothing of the folder or the registry entry before it failed, so "Show
# details" lists nothing as gone. $package and $error as above.
wizard-done-uninstall-cleanup-failed-nothing-gone = RABBIT couldn't remove { $package }'s uninstaller folder and registry entry ({ $error }), so it put back what it had moved. Nothing of that folder or its registry entry was removed.
# After a macOS removal: to the Trash, or by the administrator command.
# $package is the localized package name, $path a full macOS path, keep it as
# it is, $error macOS's own error message, and $programs a list of program
# names, never translated. Name the Trash, Put Back and Finder as Finder does,
# "Show details" as wizard-done-show-details does, and System Settings >
# Privacy & Security > App Management as macOS names it in your language.
wizard-done-uninstall-moved-to-trash = RABBIT moved { $package } to the Trash. It is there as { $path }. Until you empty the Trash, you can drag it back from there.
wizard-done-uninstall-moved-to-trash-unknown = RABBIT moved { $package } to the Trash. Until you empty the Trash, you can drag it back from there.
wizard-done-uninstall-trash-owner = It keeps the owner it had, so Finder offers no Put Back for it. Emptying the Trash may ask for an administrator's password, and so may dragging it back.
wizard-done-uninstall-trash-failed = RABBIT couldn't move { $package } to the Trash, so nothing of it changed ({ $error }).
wizard-done-uninstall-trash-unclear = RABBIT asked macOS to move { $path } to the Trash, but can't confirm that it is there now. Look for { $package } in the Trash and in its folder in Finder. RABBIT stopped there.
wizard-done-uninstall-admin-step-failed = The administrator command stopped at { $path }, so nothing of { $package } was removed. RABBIT stopped there.
wizard-done-uninstall-admin-delete-failed = The administrator command stopped at { $path }, which may now be only partly there. Nothing else of { $package } was removed. RABBIT stopped there.
wizard-done-uninstall-admin-stopped = The administrator command ended without saying where it stopped, and nothing of { $package } was removed. RABBIT stopped there.
wizard-done-uninstall-partly-removed = The administrator command stopped at { $path }, so { $package } is only partly removed. Show details lists what is gone and what is still there. RABBIT stopped there.
wizard-done-uninstall-partly-removed-unclear = The administrator command ended without saying where it stopped, and { $package } is only partly removed. Show details lists what is gone and what is still there. RABBIT stopped there.
wizard-done-uninstall-admin-left = The administrator command ran, but { $path } is still there, or RABBIT can't see that it is gone, so { $package } isn't fully removed. Show details lists what is gone and what is still there. RABBIT stopped there.
wizard-done-uninstall-app-management = If macOS refused it: in System Settings > Privacy & Security > App Management, turn on RABBIT. Then quit RABBIT, start it again and uninstall again.
wizard-done-uninstall-admin-unclear = The administrator command ended without saying how, but everything of { $package } is gone.
wizard-done-uninstall-installer-records-remain = macOS's installer still lists receipts of { $package }, though its files are gone. That does no harm; Show details names them.
wizard-done-uninstall-installer-records-unchecked = RABBIT couldn't check that macOS's installer forgot { $package } ({ $error }). That does no harm: its files are gone.
wizard-done-uninstall-program-started = The administrator command found one of these programs running, or couldn't check for them, so it changed nothing: { $programs }. Close any that is open, then run RABBIT again.
wizard-done-uninstall-admin-steps-refused = RABBIT couldn't put the administrator command for { $package } together ({ $error }), so it asked for no password and uninstalled nothing.
wizard-done-show-details = Show details
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = Open REAPER and close RABBIT
wizard-done-launch-reaper-mnemonic = O
wizard-done-open-resource = Open resource folder (only for advanced manual maintenance)
wizard-done-open-resource-mnemonic = R
wizard-done-no-reaper-app = No launchable REAPER application is known for this target.
wizard-done-launch-reaper-error-prefix = REAPER could not be launched
wizard-done-open-resource-error-prefix = Resource folder could not be opened
wizard-done-self-update-apply-running = Applying RABBIT update…
wizard-done-self-update-error-prefix = RABBIT self-update failed
wizard-done-self-update-relaunch-prefix = Relaunched RABBIT
wizard-self-update-status-checking = Checking for RABBIT updates…

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = Stop the installation?
wizard-close-during-install-body = RABBIT is installing. If you stop now, RABBIT finishes the step it started, keeps what is already installed, cleans up what it started, and then closes. Packages it has not reached yet stay uninstalled. Stop the installation?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = Stop uninstalling?
wizard-close-during-uninstall-body = RABBIT is uninstalling. If you stop now, RABBIT finishes the step it started, then stops and closes. Packages it hasn't reached stay installed. Stop uninstalling?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Uninstall the ticked packages?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = A backup can't bring { $package } back. To have it again, you install it again.
# Instead of the sentence above, for a package whose files go into a backup
# while RABBIT removes its uninstaller folder in Program Files and its
# registry entry with administrator rights. Review shows this sentence too,
# or the one above when none of the package's files are left to move.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-confirm-uninstall-partly-unbacked = A backup can bring back { $package }'s files, but not its uninstaller folder in Program Files and its registry entry, which RABBIT removes with administrator rights.
# Instead of wizard-confirm-uninstall-unbacked, for a package that goes to
# the Trash; the second when your account may not move it, so an
# administrator's password moves it there. $package is the localized
# package name. Name the Trash as Finder does.
wizard-confirm-uninstall-trash = { $package } goes to the Trash. You can drag it back from there until you empty the Trash.
wizard-confirm-uninstall-trash-elevated-macos = { $package } goes to the Trash. You can drag it back from there until you empty the Trash, but macOS may ask for an administrator's password for that.
wizard-confirm-uninstall-question = Uninstall now?

# Asked before backups are deleted, every time. The No button is the focused
# one, so Enter never deletes anything. The text is the body, then the
# only-copy sentence when one applies, then the question, as one paragraph.
# $count and $only are numbers: the backups that go, and how many of them
# are the only copy of what they hold. $size is a size, such as "520 MB".
wizard-confirm-delete-backups-title = Delete backups for good?
wizard-confirm-delete-backups-body = RABBIT deletes { $count ->
    [one] one backup
   *[other] { $count } backups
} and frees { $size }.
wizard-confirm-delete-backups-only-copy = { $only ->
    [one] One of them is the only copy of what it holds.
   *[other] { $only } of them are the only copy of what they hold.
}
# Instead of the sentence above, when only one backup goes.
wizard-confirm-delete-backups-only-copy-single = It is the only copy of what it holds.
wizard-confirm-delete-backups-question = A deleted backup can't be brought back. Delete now?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = RABBIT is updating itself
wizard-close-during-self-update-body = RABBIT is replacing its own program files. This step cannot be stopped without damaging the installation. Wait for it to finish, then close RABBIT.

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = RABBIT update available
wizard-self-update-prompt-body = RABBIT { $latest } is available. You currently have { $current }. Update now? RABBIT will relaunch when the update finishes.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = What's new since RABBIT { $current }:
wizard-self-update-prompt-update-button = Update now
wizard-self-update-prompt-later-button = Not now

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = Updating RABBIT

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = RABBIT update available: { $current } → { $latest } (channel { $channel }). Relaunch RABBIT to be re-prompted.
self-update-status-up-to-date = RABBIT is up to date (current { $current }, channel { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = Self-update did not replace any files (target version { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = Replaced { $count } file(s) under { $root }; relaunch RABBIT to use { $version }.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Signature verification: { $signed } signed.
self-update-apply-signature-summary-unsigned-only = Signature verification: { $unsigned } unsigned.
self-update-apply-signature-summary-mixed = Signature verification: { $signed } signed, { $unsigned } unsigned.

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Another RABBIT install is in progress (PID { $pid }). Apply is paused until it finishes.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Target: { $path }
wizard-summary-portable = Portable target: { $value }
wizard-summary-dry-run = Dry run: { $value }
wizard-summary-packages-selected = Packages selected: { $packages }
wizard-summary-cache = Cache: { $path }
wizard-summary-planned-app = Planned app path: { $path }
wizard-summary-error = Error: { $message }
wizard-summary-error-antivirus = Windows security software blocked this download. This is usually a false positive: the installer is code-signed and RABBIT had already verified it against the publisher's checksum. Defender sometimes flags a freshly built installer it hasn't seen widely yet — OSARA development snapshots are the common case. To continue: open Windows Security, go to Virus & threat protection, then Protection history, select the blocked item and choose Allow, then run RABBIT again. You can also add RABBIT's download folder to the exclusions, or install this one package by hand from the publisher's website. RABBIT never switches your virus protection off.
wizard-summary-resource-items-created = Resource items created: { $count }
wizard-summary-packages-installed-or-checked = Packages installed or checked: { $count }
wizard-summary-packages-current = Packages already current: { $count }
wizard-summary-packages-manual = Packages requiring manual attention: { $count }
wizard-summary-backup-files-created = Backup files created: { $count }
wizard-summary-backup-file = Backup file: { $path }
wizard-summary-receipt-backup = Receipt backup: { $path }
wizard-summary-backup-manifest = Backup manifest: { $path }
wizard-summary-osara-keymap-merge-hint = Your key map was kept. To add the new OSARA key bindings to it, start REAPER and run OSARA's "Merge OSARA key map" action (Extensions menu, OSARA).
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Plan action: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Status: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Installed version: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architecture: { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Also remove settings: { $value }
# One line per file, below its package's line. $path is a full path.
# wizard-summary-left-over can also name a folder, such as a package's
# uninstaller folder in Program Files, or a registry key as regedit shows
# it; keep either as it is, never translated.
wizard-summary-removed-file = Removed: { $path }
wizard-summary-kept-file = Kept, because it changed since RABBIT installed it: { $path }
wizard-summary-kept-link = Kept, because it holds a link RABBIT never follows: { $path }
wizard-summary-kept-reapack = Kept, because ReaPack installed it and keeps track of it: { $path }
wizard-summary-left-over = Still there: { $path }
wizard-summary-left-in-backup = Couldn't be put back, still in the backup: { $path }
# A file a package's installer added outside REAPER's folder that stays:
# it was there before RABBIT ran that installer, or RABBIT recorded it more
# than once with contents that disagree. "the installer" is the package's
# own installer. $path is a full path as Windows holds it, never
# translated.
wizard-summary-kept-there-before = Kept, because it was already there when RABBIT ran the installer: { $path }
wizard-summary-kept-record-unclear = Kept, because RABBIT's records of it disagree: { $path }
# Below a package its own uninstaller removed: a file Windows removes at the
# next restart, and the uninstaller itself. $path is a full path, never
# translated.
wizard-summary-pending-restart = Goes when you restart the computer: { $path }
wizard-summary-vendor-uninstaller = Its own uninstaller: { $path }
# Below a package removed on macOS, one line per path: what went to the Trash
# and where it is there now, and what the administrator command removed; then
# the installer receipts macOS still lists, and macOS's own words when a step
# failed. On Windows, the line for what was removed with administrator rights
# also names a package's uninstaller folder in Program Files, and each file
# in it, that RABBIT removed that way. $path is a full Windows or macOS path,
# keep it as it is; $ids a comma-separated list of receipt names, never
# translated; $error macOS's own message. Name the Trash as Finder does.
wizard-summary-moved-to-trash = Moved to the Trash: { $path }
wizard-summary-in-trash-as = Now in the Trash: { $path }
wizard-summary-removed-as-admin = Removed with administrator rights: { $path }
wizard-summary-installer-records-remain = Still listed by macOS's installer: { $ids }
wizard-summary-macos-said = macOS said: { $error }
# What became of a package's entry in Windows' list of installed programs,
# below the package's line. $key is the entry's registry key, such as
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA, and is
# never translated; $error is Windows' own error message.
wizard-summary-registry-removed = Taken off Windows' list of installed programs: { $key }
wizard-summary-registry-kept = Still in Windows' list of installed programs, because it doesn't point into this REAPER: { $key }
wizard-summary-registry-failed = Still in Windows' list of installed programs, because RABBIT couldn't read or remove it ({ $error }): { $key }
# Below a package whose uninstaller folder and registry entry RABBIT removed
# with administrator rights: the registry entry. $key is the entry's
# registry key as regedit shows it, such as
# HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts,
# and is never translated. The folder and its files are listed with
# wizard-summary-removed-as-admin.
wizard-summary-registry-key-removed = Removed from the registry with administrator rights: { $key }
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Backup set: { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = To put { $package } back, close REAPER and run: { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Report: { $path }
status-installed-or-checked = Installed or checked
status-planned-unattended = Planned unattended
status-deferred-unattended = Deferred unattended
status-skipped-current = Skipped (already current)
status-failed = Failed
status-skipped-dependency-failed = Skipped (dependency failed)
status-cancelled = Not installed (you stopped)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Uninstalled
status-removed-with-leftovers = Uninstalled, but some of it is still there
status-removed-pending-restart = Uninstalled; the rest goes when you restart the computer
status-would-remove = Would be uninstalled
status-remove-not-installed = Wasn't installed
status-remove-blocked = Not uninstalled
status-remove-failed = Couldn't be uninstalled
status-remove-cancelled = Still installed (you stopped)
status-remove-not-reached = Still installed (the run stopped earlier)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Single extension binary handled by RABBIT installer.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = Installed version { $installed } is current or newer than available version { $available }.
package-status-skipped-content-unchanged = The installed copy is identical to the one published upstream.
# $error is the failure text. $dependency is the package (e.g. REAPER) that failed first.
package-status-install-failed = Installation failed: { $error }
package-status-skipped-dependency-failed = Skipped because { $dependency }, which it needs, did not install successfully.
package-status-cancelled = Not installed: you stopped the setup before RABBIT reached this package.
# Shown after the package's name when its vendor installer ended with an
# exit code other than 0. $code is the number. One of the
# package-status-vendor-exit-* sentences follows, then
# package-status-vendor-installer-log when the installer left a log.
package-status-vendor-installer-failed = The installer ended with exit code { $code }.
# Inno Setup's exit code 2: cancelled before it began installing. RABBIT runs
# it unattended, so in practice the user declined the Windows permission
# prompt (User Account Control) the installer raised. "Yes" is the prompt's
# button.
package-status-vendor-exit-cancelled-before-installing = That usually means the Windows permission prompt was declined, so nothing changed. Run RABBIT again and choose Yes when Windows asks for permission.
# Inno Setup's exit code 5: it stopped at a question, such as whether to close
# programs that use its files, that nobody can answer while RABBIT runs it
# unattended. $package is the package's name.
package-status-vendor-exit-stopped-at-question = That means it stopped at a question it can't ask while RABBIT runs it unattended, usually because another program has one of its files open. Close other programs, especially other music programs and { $package }, then run RABBIT again.
# Inno Setup's exit code 7: its preparation found it can't continue.
package-status-vendor-exit-preparation-failed = That means it found, while getting ready, that it can't install right now. Restart the computer, then run RABBIT again.
# Inno Setup's exit code 8: its preparation found Windows must restart first.
package-status-vendor-exit-restart-needed = That means it found, while getting ready, that Windows has to restart first. Restart the computer, then run RABBIT again.
# Any other exit code, or an installer whose codes RABBIT doesn't know.
# $package is the package's name.
package-status-vendor-exit-unknown = Close other programs and run RABBIT again. If it stops again, install { $package } from its publisher's website.
# $path is the log file the installer wrote.
package-status-vendor-installer-log = The installer's log is at { $path }.
# Shown after the package's name when RABBIT didn't start its installer
# because the package's own programs were open. $package is the package's
# name; $programs lists the open programs' file names, such as "Surge XT.exe".
package-status-blocking-programs-running = RABBIT didn't start the installer because { $package } is open ({ $programs }), and the installer can't replace a program that is running. Close it, then run RABBIT again.
# $automation is one of the "package-automation-*" labels (vendor installer / archive extraction / ...).
package-status-dry-run-would-run-unattended = Dry run: RABBIT would download and run this { $automation } unattended.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-staged = This build has not implemented the planned unattended { $automation } execution path yet. RABBIT staged the artifact in the cache but did not run it.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-not-staged = This build has not implemented the planned unattended { $automation } execution path yet. RABBIT did not download or run the artifact.
package-status-unattended-installed = RABBIT ran the upstream installer unattended, verified the expected target paths, and updated the RABBIT receipt.
package-status-osara-unattended-keymap-backed-up = RABBIT ran the upstream installer unattended, backed up reaper-kb.ini, applied the OSARA key map replacement, and updated the RABBIT receipt.
package-status-osara-unattended-keymap-replaced = RABBIT ran the upstream installer unattended, applied the OSARA key map replacement, and updated the RABBIT receipt.

# Short automation-kind labels interpolated into the per-package status
# messages above.
package-automation-installer = vendor installer
package-automation-archive = archive extraction
package-automation-disk-image = disk image install
package-automation-extension-binary = direct file install

# Per-configuration-step status messages surfaced on the wizard's Done page.
# `wizard-summary-configuration-message = { $step }: { $message }` is the
# wrapper template — the `*-message` keys below are the message body only.
# $name is the human-readable remote name; $url is the index XML URL.
config-message-reapack-remote-already-present = ReaPack remote { $name } ({ $url }) is already configured in reapack.ini.
config-message-reapack-remote-added = Added ReaPack remote { $name } ({ $url }) to reapack.ini.
config-message-reapack-remote-created-file = Created reapack.ini with ReaPack remote { $name } ({ $url }). ReaPack will add its default repositories on the next REAPER launch.
config-message-reapack-remote-dry-run = Would add ReaPack remote { $name } ({ $url }) to reapack.ini.
config-message-reaper-language-already-selected = REAPER was already set to use { $file }.
config-message-reaper-language-selected = Set REAPER's language to { $file }.
config-message-reaper-language-dry-run = Would set REAPER's language to { $file }.
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = Configuration step { $step } was not selected.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = Configuration step { $step } skipped because its dependency package { $dependency } was not installed and is not part of this plan.
config-message-applied-no-op = Configuration step applied without changes.
config-message-cancelled = This configuration step did not run: you stopped the setup first.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Status: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Applied
config-status-skipped = Skipped
config-status-skipped-dependency-missing = Skipped (dependency missing)
config-status-dry-run = Dry run
config-status-cancelled = Did not run (you stopped)
wizard-summary-planned-execution-title = Planned unattended execution:
wizard-summary-planned-execution-runner =   Runner: { $runner }
wizard-summary-planned-execution-artifact =   Artifact: { $artifact }
wizard-summary-planned-execution-program =   Program: { $program }
wizard-summary-planned-execution-program-zipped =   Program: the installer inside the .zip, once it is downloaded
wizard-summary-planned-execution-arguments =   Arguments: { $arguments }
wizard-summary-planned-execution-working-directory =   Working directory: { $path }
wizard-summary-planned-execution-verify =   Verify: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Note: { $note }
wizard-summary-status-finished = Finished. { $installed } package item(s) installed or checked; { $manual } require manual attention.
wizard-summary-status-finished-with-errors = Finished with errors. { $installed } package item(s) installed or checked; { $failed } failed.

wizard-planned-runner-launch-installer = Launch installer executable
wizard-planned-runner-extract-archive = Extract archive and run contained installer
wizard-planned-runner-extract-archive-copy-osara = Extract archive and copy OSARA installer assets
wizard-planned-runner-mount-disk-image = Mount disk image and run contained installer
wizard-planned-runner-mount-disk-image-copy-app = Mount disk image and copy contained app bundle
wizard-planned-runner-mount-disk-image-run-pkg = Mount disk image and run contained pkg installer

# Expert mode and release channels. Expert mode is a hidden, documented
# unlock (Ctrl+Shift+E / Cmd+Shift+E, or RABBIT_EXPERT=1) that offers pre-release
# builds of REAPER and OSARA.
package-channel-reaper-dev = Development builds from landoleet.org
package-channel-osara-pr = Pull request test builds
wizard-window-title-expert = { $title } – Expert mode
wizard-expert-enable-title = Turn on expert mode?
wizard-expert-enable-body = Expert mode lets you install pre-release builds: REAPER's development builds from landoleet.org, and test builds of OSARA pull requests. They are unsupported, can break REAPER or its accessibility, and are meant for testing only. Two build choices appear on this page, below the REAPER installation. Turn on expert mode?
wizard-expert-disabled-title = Expert mode is off
wizard-expert-disabled-body = Expert mode is off. Any package installed from a pre-release build goes back to its regular release the next time you install.
wizard-expert-first-page-title = Expert mode
wizard-expert-first-page-body = Expert mode can only be turned on or off on the first page, where it adds the build choices. Go back to the first page to change it.
wizard-expert-busy-body = Expert mode can't be changed while RABBIT is installing or after it has finished.
wizard-expert-reaper-builds-label = REAPER builds
wizard-expert-reaper-builds-stable = Regular releases
wizard-expert-osara-builds-label = OSARA builds
wizard-expert-osara-builds-snapshot = Regular snapshots
wizard-expert-osara-builds-loading = Looking for pull request builds…
wizard-expert-osara-builds-unavailable = Regular snapshots (pull request builds could not be listed)
wizard-expert-osara-builds-pr = Pull request { $number }: { $title }
wizard-expert-osara-builds-pr-untitled = Pull request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = development build
wizard-package-channel-dev-details = { $package } will be installed from its development builds: unsupported pre-release versions that can break at any time.
wizard-package-channel-pr = test build of pull request { $number }
wizard-package-channel-pr-details = { $package } will be installed from the test build of pull request { $number }: unreviewed, unsigned code that GitHub deletes after 90 days.
wizard-package-channel-back = back to the regular release
wizard-package-channel-back-details = { $package } is currently a pre-release build. This installation puts the regular release back.
