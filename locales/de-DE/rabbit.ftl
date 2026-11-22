app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = ja
common-no = nein

# What happens to a package in this run, the first thing its row says. An
# unticked row on every tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal, and on
# the Backups tab that the backup isn't ticked to restore or delete.
action-install = Installieren
action-update = Aktualisieren
action-keep = Beibehalten
action-uninstall = Deinstallieren
# On the Backups tab: what happens to a backup ticked to restore, or to
# delete. The last is for a backup that is the only copy of what it holds,
# such as software you uninstalled.
action-restore = Wiederherstellen
action-delete = Löschen
action-delete-only-copy = Einzige Kopie löschen

package-reaper = REAPER
package-osara = OSARA
package-sws = SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = JAWS-Skripte für REAPER
package-ffmpeg = FFmpeg
package-surge-xt = Surge XT
package-ljuno-116 = LJuno-116
package-lb-303 = LB-303
package-lr-608 = LR-608
package-lsampler-24 = LSampler-24
package-app2clap = app2clap
package-lreaparams = LReaParams
package-langpack-es = REAPER auf Spanisch
package-langpack-de = REAPER auf Deutsch
package-langpack-fr = REAPER auf Französisch
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Team PMA (es_MX)

package-reaper-description = Reaper ist eine der vielseitigsten Digital Audio Workstations (DAWs) auf dem Markt und eignet sich sowohl für Einsteiger als auch für Profis. Besonders durch ihre Vielseitigkeit und Anpassungsfähigkeit hebt sich Reaper von anderen DAWs ab und wird von vielen professionellen Produzenten für anspruchsvolle Projekte genutzt.
package-osara-description = OSARA macht REAPER mit Screenreadern bedienbar — NVDA, JAWS und Narrator unter Windows sowie VoiceOver unter macOS. Installieren Sie OSARA, wenn Sie REAPER mit einem Screenreader nutzen.
package-sws-description = Die SWS-Erweiterung ist eine seit langem etablierte Community-Sammlung zusätzlicher Aktionen, Skripte und Hilfsmittel, die das Bearbeiten in REAPER abrunden. Die meisten barrierefreien REAPER-Setups setzen sie voraus.
package-reapack-description = ReaPack ist der Paketmanager für REAPER: Er installiert und aktualisiert Skripte und Erweiterungen Dritter direkt aus REAPER heraus. Installieren Sie ReaPack, wenn Sie von der REAPER-Community geteilte Skripte verwenden möchten.
package-reakontrol-description = ReaKontrol fügt REAPER Unterstützung für Native Instruments Komplete-Kontrol-Tastaturen hinzu. Installieren Sie es, wenn Sie eine Komplete-Kontrol-Tastatur besitzen und Hardware-Steuerung nutzen möchten.
package-jaws-scripts-description = Die JAWS-Skripte für REAPER ergänzen den Screenreader JAWS unter Windows um Skript-Unterstützung für REAPER. RABBIT bietet sie nur an, wenn JAWS auf diesem PC erkannt wird.
package-ffmpeg-description = Die gemeinsam genutzten Laufzeitbibliotheken von FFmpeg ermöglichen es dem Videodecoder von REAPER, gängige Video- und Audioformate zu importieren und abzuspielen. RABBIT installiert den Inhalt des `bin`-Ordners aus dem GPL-Shared-Build von BtbN in `UserPlugins`. Die genaue Patch-Version lässt sich aus den DLL-Dateinamen allein nicht ableiten — extern installierte FFmpeg-Versionen werden deshalb als `<major>.0.0` gemeldet.
package-surge-xt-description = Surge XT ist ein freier, quelloffener Hybrid-Synthesizer. RABBIT führt das Installationsprogramm des Herstellers für Sie aus — es installiert die Formate VST3, CLAP, AU (nur macOS) und Standalone systemweit, sodass REAPER und andere DAWs Surge XT laden können. RABBIT verfolgt den fortlaufenden Nightly-Kanal, weil die letzte stabile Veröffentlichung (1.3.4) aus dem August 2024 stammt und das Projekt inzwischen praktisch ausschließlich über Nightly-Builds ausgeliefert wird. Nur in Standard-REAPER-Installationen verfügbar: Die mitgelieferten Daten liegen außerhalb eines portablen REAPER-Ordners.
# Lo-lo's four instruments, VST3 plug-ins that RABBIT installs into REAPER's
# own plug-in folder. Keep UserPlugins\FX and Common Files\VST3 as they are:
# they are the folders' names.
package-ljuno-116-description = LJuno-116 ist ein polyphoner Open-Source-Synthesizer von Lo-lo, den Sie mit einem Screenreader spielen und bearbeiten können. RABBIT installiert ihn ohne Administratorrechte in REAPERs eigenen Plug-in-Ordner UserPlugins\FX, und REAPER findet ihn nach dem nächsten Start oder einer erneuten Suche nach Plug-ins. Wenn Sie ihn selbst nach Common Files\VST3 kopiert haben, entfernen Sie diese Kopie, damit REAPER ihn nicht doppelt findet. Nur für Windows.
package-lb-303-description = LB-303 ist ein Open-Source-Bassline-Synthesizer von Lo-lo mit Step-Sequencer. Sie spielen und bearbeiten ihn mit einem Screenreader. RABBIT installiert ihn ohne Administratorrechte in REAPERs eigenen Plug-in-Ordner UserPlugins\FX, und REAPER findet ihn nach dem nächsten Start oder einer erneuten Suche nach Plug-ins. Wenn Sie ihn selbst nach Common Files\VST3 kopiert haben, entfernen Sie diese Kopie, damit REAPER ihn nicht doppelt findet. Nur für Windows.
package-lr-608-description = LR-608 ist eine Open-Source-Drum-Machine von Lo-lo, die ihre Klänge synthetisiert und deshalb keine Sample-Bibliothek braucht. Sie spielen sie mit der Tastatur und einem Screenreader. RABBIT installiert sie ohne Administratorrechte in REAPERs eigenen Plug-in-Ordner UserPlugins\FX, und REAPER findet sie nach dem nächsten Start oder einer erneuten Suche nach Plug-ins. Wenn Sie sie selbst nach Common Files\VST3 kopiert haben, entfernen Sie diese Kopie, damit REAPER sie nicht doppelt findet. Nur für Windows.
package-lsampler-24-description = LSampler-24 ist ein Sampler von Lo-lo mit 24 Slots für Ihre eigenen Samples, den Sie mit einem Screenreader bedienen können. RABBIT installiert ihn ohne Administratorrechte in REAPERs eigenen Plug-in-Ordner UserPlugins\FX, und REAPER findet ihn nach dem nächsten Start oder einer erneuten Suche nach Plug-ins. Wenn Sie ihn selbst nach Common Files\VST3 kopiert haben, entfernen Sie diese Kopie, damit REAPER ihn nicht doppelt findet. Nur für Windows.
package-app2clap-description = app2clap ist ein CLAP-Plugin für Windows, das Audio aus anderen Anwendungen erfasst und es in REAPER als Plug-in einspeist, das Sie in eine Spur einfügen — praktisch, um Ton aus einem Browser, Mediaplayer oder anderen Programm aufzunehmen oder zu bearbeiten. RABBIT lädt die neueste Version herunter und installiert app2clap.clap in REAPERs eigenen Plug-in-Ordner UserPlugins\FX, sodass es nur diesem REAPER zur Verfügung steht. Es sind keine Administratorrechte nötig, und das funktioniert auch mit einem portablen REAPER. Nur für Windows.
package-lreaparams-description = LReaParams ist eine Open-Source-Erweiterung von Lo-lo, mit der Sie FX- und Spurparameter in REAPER barrierefrei über die Tastatur steuern. Die Erweiterung arbeitet mit OSARA zusammen. Nur für Windows.
package-langpack-es-description = Übersetzt die Oberfläche von REAPER ins Spanische, einschließlich der SWS-Erweiterung. Betreut von Javier Robledo für die spanische REAPER-Community und über reaperespa.com veröffentlicht — RABBIT lädt die aktuelle Fassung herunter und installiert sie als es_ES.ReaperLangPack. OSARA wählt anhand dieses Dateinamens seine eigene spanische Übersetzung. Stellen Sie die Sprache anschließend in den REAPER-Einstellungen ein (oder lassen Sie RABBIT das übernehmen).
package-langpack-de-description = Übersetzt die Oberfläche von REAPER ins Deutsche, einschließlich der SWS-Erweiterung. Betreut von MrData und im REAPER-Stash veröffentlicht — RABBIT lädt die aktuelle Fassung herunter und installiert sie als de_DE.ReaperLangPack. Stellen Sie die Sprache anschließend in den REAPER-Einstellungen ein (oder lassen Sie RABBIT das übernehmen).
package-langpack-fr-description = Übersetzt die Oberfläche von REAPER ins Französische, einschließlich der SWS-Erweiterung. Betreut von Lee Julien und Pierre-Marie Curt für ReaperAccessible, die französische Screenreader-Gemeinschaft, und in deren ReaPack-Repository veröffentlicht — RABBIT lädt die aktuelle Fassung herunter und installiert sie als fr_CA.ReaperLangPack. OSARA wählt anhand dieses Dateinamens seine eigene französische Übersetzung: die fr_CA-Fassung ist nahezu vollständig, während die fr_FR-Fassung noch etwa ein Drittel der Meldungen auf Englisch lässt; das Sprachpaket selbst ist in beiden Fällen dasselbe Standardfranzösisch. Stellen Sie die Sprache anschließend in den REAPER-Einstellungen ein (oder lassen Sie RABBIT das übernehmen).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = RABBIT kann dieses Paket noch nicht deinstallieren.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Entfernt die SWS-Erweiterung.
uninstall-summary-sws-keeps = Ihre Grooves und Ihre SWS-Einstellungen, etwa Ihre Cycle Actions, bleiben erhalten.
uninstall-summary-reapack-removes = Entfernt die ReaPack-Erweiterung.
uninstall-summary-reapack-keeps = Ihre ReaPack-Einstellungen und Repositorys bleiben erhalten, ebenso die Liste der Pakete, die ReaPack installiert hat.
uninstall-summary-reakontrol-removes = Entfernt die ReaKontrol-Erweiterung.
uninstall-summary-reakontrol-keeps = Ihre FX-Maps bleiben erhalten, und Ihre Software von Native Instruments rührt RABBIT nicht an.
uninstall-summary-ffmpeg-removes = Entfernt die FFmpeg-Dateien, die RABBIT in UserPlugins installiert hat. REAPER kann FFmpeg danach nicht mehr zum Öffnen oder Rendern von Medien verwenden.
uninstall-summary-ffmpeg-keeps = FFmpeg-Dateien, über die RABBIT nichts aufgezeichnet hat, bleiben erhalten, etwa Reste älterer Versionen.
uninstall-summary-app2clap-removes = Entfernt app2clap aus dem Ordner UserPlugins\FX von REAPER.
# The same on the standard REAPER, which also removes any app2clap in the
# per-user CLAP folder, whoever put it there: earlier versions of RABBIT
# installed it there. Keep UserPlugins\FX as it is: it is the folder's name.
uninstall-summary-app2clap-removes-standard = Entfernt app2clap aus dem Ordner UserPlugins\FX von REAPER und jedes app2clap in Ihrem CLAP-Ordner, etwa eines, das eine frühere Version von RABBIT dort installiert hat. Alle Programme, in denen Sie CLAP-Plug-ins verwenden, verlieren die Kopie im CLAP-Ordner, nicht nur REAPER.
uninstall-summary-app2clap-keeps = Ihre anderen Plug-ins bleiben erhalten.
uninstall-summary-lreaparams-removes = Entfernt die LReaParams-Erweiterung.
uninstall-summary-lreaparams-keeps = Ihre LReaParams-Einstellungen und -Profile bleiben erhalten, ebenso die Anleitung.
# Lo-lo's instruments. Keep UserPlugins\FX as it is: it is the folder's name.
uninstall-summary-ljuno-116-removes = Entfernt LJuno-116 aus dem Ordner UserPlugins\FX von REAPER.
uninstall-summary-ljuno-116-keeps = Ihre LJuno-116-Presets im Ordner LJuno-116 in Ihren Dokumenten bleiben erhalten, ebenso Ihre anderen Plug-ins.
uninstall-summary-lb-303-removes = Entfernt LB-303 aus dem Ordner UserPlugins\FX von REAPER.
uninstall-summary-lb-303-keeps = Ihre LB-303-Presets im Ordner LB-303 in Ihren Dokumenten bleiben erhalten, ebenso Ihre anderen Plug-ins.
uninstall-summary-lr-608-removes = Entfernt LR-608 aus dem Ordner UserPlugins\FX von REAPER.
uninstall-summary-lr-608-keeps = Ihre LR-608-Kits und -Presets im Ordner LR-608 in Ihren Dokumenten bleiben erhalten, ebenso Ihre anderen Plug-ins.
uninstall-summary-lsampler-24-removes = Entfernt LSampler-24 aus dem Ordner UserPlugins\FX von REAPER.
uninstall-summary-lsampler-24-keeps = Ihre LSampler-24-Bibliothek mit Samples, Slots und Bänken im Ordner LSampler-24 in Ihren Dokumenten bleibt erhalten, ebenso die Audiodateien, die LSampler-24 extrahiert hat, und Ihre anderen Plug-ins.
uninstall-summary-langpack-removes = Entfernt das Sprachpaket, das RABBIT installiert hat. Wenn REAPER es verwendet, stellt RABBIT REAPER wieder auf Englisch um.
uninstall-summary-langpack-keeps = Sprachpakete, die Sie selbst hinzugefügt haben, bleiben erhalten.
uninstall-summary-osara-removes = Entfernt die OSARA-Erweiterung, ihre Übersetzungen und die Kopie der OSARA-Tastenzuordnung im Ordner KeyMaps.
uninstall-summary-osara-removes-windows = Entfernt die OSARA-Erweiterung, ihre Übersetzungen, ihr Deinstallationsprogramm und die Kopie der OSARA-Tastenzuordnung im Ordner KeyMaps. Wenn Windows dieses OSARA in der Liste der installierten Programme führt, entfernt RABBIT es aus dieser Liste.
uninstall-summary-osara-keeps = Ihre Tastenzuordnung bleibt unverändert, ebenso die Einstellungen von OSARA und die Sicherungen Ihrer Tastenzuordnung, die OSARA angelegt hat.
uninstall-summary-surge-removes = Das eigene Deinstallationsprogramm von Surge XT entfernt es für alle Musikprogramme auf diesem Computer. Eine Sicherung kann es nicht zurückholen.
uninstall-summary-surge-keeps = Ihre eigenen Patches im Ordner Surge XT in Ihren Dokumenten bleiben erhalten.
uninstall-summary-reaper-windows-removes = Das eigene Deinstallationsprogramm von REAPER entfernt das Programm. Es entfernt außerdem die Dateizuordnungen von REAPER und ReaRoute für jedes REAPER auf diesem Computer.
uninstall-summary-reaper-keeps = Ihre Einstellungen, Erweiterungen und Projekte bleiben erhalten.
# The JAWS scripts: two plug-in files in REAPER's UserPlugins folder, and
# script files their installer adds to JAWS's settings folder, which RABBIT
# removes only while they are as the installer left them. Keep
# %APPDATA%\Freedom Scientific\JAWS as it is.
uninstall-summary-jaws-scripts-removes = Entfernt die beiden Plug-in-Dateien der Skripte aus dem UserPlugins-Ordner von REAPER und die Skriptdateien, die ihr Installationsprogramm laut RABBITs Aufzeichnung im Einstellungsordner von JAWS angelegt hat.
uninstall-summary-jaws-scripts-keeps = Skriptdateien, bei denen RABBIT nicht sicher ist, dass das Installationsprogramm sie angelegt hat, und solche, die sich seither geändert haben, etwa Ihre eigenen Tastenbelegungen, bleiben im Einstellungsordner von JAWS, %APPDATA%\Freedom Scientific\JAWS.
# The same two sentences on macOS, where Surge XT has no uninstaller of its
# own and REAPER is an app. Name the Trash and the Library folder as Finder
# does in your language.
uninstall-summary-surge-macos-removes = Entfernt die Apps und Plug-ins von Surge XT und seine Werksdaten im Ordner Library. Eine Sicherung kann sie nicht zurückholen.
uninstall-summary-reaper-macos-removes = Legt die App REAPER in den Papierkorb.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = Ihre Grooves
uninstall-kept-sws-settings = SWS-Einstellungen
uninstall-kept-sws-cycle-actions = Ihre Cycle Actions
uninstall-kept-reapack-settings = ReaPack-Einstellungen und Ihre Repositorys
uninstall-kept-reapack-data = die Liste der von ReaPack installierten Pakete und sein Download-Cache
uninstall-kept-reakontrol-fx-maps = Ihre FX-Maps
uninstall-kept-lreaparams-data = Ihre LReaParams-Einstellungen und -Profile sowie die Anleitung
uninstall-kept-ljuno-116-presets = Ihre LJuno-116-Presets
uninstall-kept-lb-303-presets = Ihre LB-303-Presets
uninstall-kept-lr-608-presets = Ihre LR-608-Kits und -Presets
uninstall-kept-lsampler-24-library = Ihre LSampler-24-Bibliothek mit Samples, Slots und Bänken
uninstall-kept-lsampler-24-extracted-audio = die Audiodateien, die LSampler-24 aus Videos und Mehrkanaldateien extrahiert hat
uninstall-kept-osara-key-map = Ihre Tastenzuordnung
uninstall-kept-osara-replaced-key-map = Ihre Tastenzuordnung, bevor OSARA sie ersetzt hat
uninstall-kept-osara-merged-key-map = Ihre Tastenzuordnung, bevor OSARA seine Tasten hinzugefügt hat
uninstall-kept-osara-settings = die Einstellungen von OSARA, in einer Datei, die alle Erweiterungen gemeinsam nutzen
uninstall-kept-surge-user-data = Ihre Patches und alles andere, was Sie mit Surge XT gespeichert haben
uninstall-kept-reaper-resource = der Ressourcenordner von REAPER mit Ihren Einstellungen, Erweiterungen und Tastenzuordnungen
# On macOS, in the Library folder of your home folder.
uninstall-kept-surge-settings-macos = Ihre Einstellungen für Surge XT
# On macOS, in the Library folder of your home folder: a copy of Surge XT's
# factory data put there by hand, which Surge XT reads instead of the one it
# installs for everyone. Say "factory data" as in
# uninstall-summary-surge-macos-removes.
uninstall-kept-surge-factory-copy-macos = Ihre eigene Kopie der Werksdaten von Surge XT

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Wenn Sie die OSARA-Tastenzuordnung verwenden, führen einige ihrer Tasten SWS-Aktionen aus. Diese Tasten funktionieren erst wieder, wenn Sie SWS erneut installieren.
uninstall-warning-sws-reapack-scripts = Viele REAPER-Skripte, darunter Skripte für die Barrierefreiheit, benötigen SWS. Skripte, die Sie mit ReaPack installiert haben, funktionieren dann möglicherweise nicht mehr.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Skripte und Erweiterungen, die Sie mit ReaPack installiert haben, bleiben erhalten, aber nichts hält sie mehr aktuell.
uninstall-note-surge-fonts-stay = Die Schriftarten, die Surge XT installiert hat, bleiben erhalten: Sein Installationsprogramm legt fest, dass sie nie entfernt werden.
# A note on the JAWS script files that stay after the JAWS scripts are
# uninstalled, and how to remove them by hand. Keep %APPDATA%, the folder
# and file names, and "reaper" (the start of the file names) exactly as they
# are. For the example language folder, use JAWS's folder for your language:
# enu for English, deu for German, esn for Spanish, fra for French, ita for
# Italian.
uninstall-note-jaws-scripts-by-hand = Um die verbleibenden Skriptdateien zu entfernen, löschen Sie die Dateien, deren Name mit reaper beginnt, aus den Sprachordnern in %APPDATA%\Freedom Scientific\JAWS, etwa 2025\Settings\deu. Kopieren Sie vorher reaper.jkm, reaper.jcf und reaper.ini an einen anderen Ort, wenn Sie Ihre Tastenbelegungen und Einstellungen behalten möchten.

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
uninstall-by-hand-jaws-scripts = So entfernen Sie die JAWS-Skripte von Hand:
    1. Schließen Sie REAPER.
    2. Löschen Sie Reaper_ComAccess32.dll und Reaper_ComAccess64.dll aus %APPDATA%\REAPER\UserPlugins.
    3. Löschen Sie den Ordner %ProgramFiles(x86)%\Reaper_JawsScripts. Windows fragt dabei nach der Erlaubnis eines Administrators.
    4. Um auch die Skriptdateien zu entfernen, löschen Sie die Dateien, deren Name mit reaper beginnt, aus den Sprachordnern in %APPDATA%\Freedom Scientific\JAWS, etwa 2025\Settings\deu. Kopieren Sie vorher reaper.jkm, reaper.jcf und reaper.ini an einen anderen Ort, wenn Sie Ihre Tastenbelegungen und Einstellungen behalten möchten.
uninstall-manual-reaper-portable = RABBIT entfernt kein portables REAPER: Die Dateien von REAPER und Ihre eigenen liegen im selben Ordner. So entfernen Sie es von Hand:
    1. Schließen Sie REAPER.
    2. Kopieren Sie alles, was Sie behalten möchten, aus dem portablen REAPER-Ordner heraus, etwa dort gespeicherte Projekte, Ihre Tastenzuordnungen und Ihre Einstellungen.
    3. Löschen Sie den portablen REAPER-Ordner.
    Die Sicherungen, die RABBIT für dieses REAPER angelegt hat, liegen ebenfalls in diesem Ordner, unter RABBIT\backups, und werden mit ihm gelöscht.

# Why RABBIT didn't start a program's own uninstaller, or didn't remove a
# package's uninstaller folder and registry entry. RABBIT does both with
# administrator rights. Nothing has changed at that point. RABBIT shows one
# of these, then uninstall-vendor-refused-nothing-changed, or, for the folder
# and registry entry, the steps to remove the package by hand.
# { $package } is the package's name, such as Surge XT, REAPER or
# JAWS-for-REAPER scripts.
# { $path } is a file, a folder or a value as Windows holds it; keep it as it
# is.
uninstall-vendor-refused-not-listed = Windows führt { $package } nicht unter den Programmen, die für alle Benutzer installiert sind. Deshalb findet RABBIT das Deinstallationsprogramm nicht.
uninstall-vendor-refused-unclear-command = RABBIT kann nicht erkennen, welches Programm Windows als Deinstallationsprogramm von { $package } angibt: { $path }
uninstall-vendor-refused-not-a-full-path = Windows gibt das Deinstallationsprogramm von { $package } mit einem Pfad an, dem RABBIT nicht vertraut: { $path }
uninstall-vendor-refused-unknown-folder = RABBIT findet einen Ordner nicht, den es braucht, um das Deinstallationsprogramm von { $package } zu prüfen.
uninstall-vendor-refused-wrong-folder = Das Deinstallationsprogramm von { $package } liegt nicht in dem Ordner, in den es gehört: { $path }
uninstall-vendor-refused-foreign-file = Neben dem Deinstallationsprogramm von { $package } liegt eine Datei, die nicht dorthin gehört: { $path }
uninstall-vendor-refused-folder-or-link = Neben dem Deinstallationsprogramm von { $package } liegt ein Ordner oder ein Link, der nicht dorthin gehört: { $path }
uninstall-vendor-refused-untrusted-owner = Neben dem Deinstallationsprogramm von { $package } liegt eine Datei, die kein Administrator dort abgelegt hat: { $path }
uninstall-vendor-refused-install-location-missing = Windows gibt nicht an, in welchem Ordner { $package } installiert ist.
uninstall-vendor-refused-install-location-mismatch = Laut Windows ist { $package } in { $path } installiert, nicht in dem REAPER, das Sie gewählt haben.
uninstall-vendor-refused-not-in-install-folder = Das Deinstallationsprogramm von { $package } liegt nicht in seinem Programmordner: { $path }
uninstall-vendor-refused-not-below-program-files = { $package } ist nicht in einem eigenen Ordner unter Programme (Program Files) installiert: { $path }
uninstall-vendor-refused-copy-differs = Die Kopie, die RABBIT vom Deinstallationsprogramm von { $package } angelegt hat, stimmt nicht mit dem Original überein.
uninstall-vendor-refused-unreadable = RABBIT konnte { $path } nicht lesen, um das Deinstallationsprogramm von { $package } zu prüfen.
uninstall-vendor-refused-not-a-vendor-plan = RABBIT hat kein Deinstallationsprogramm für { $package }, das es ausführen darf.
uninstall-vendor-refused-unsupported = Nur unter Windows kann RABBIT das Deinstallationsprogramm von { $package } ausführen.
# { $path } is the name of a value in the package's registry entry.
uninstall-vendor-refused-unexpected-registry-value = Der Registrierungseintrag von { $package } enthält einen Wert, den RABBIT nicht erwartet: { $path }
# { $path } is the text of a value in the package's registry entry.
uninstall-vendor-refused-registry-value-mismatch = Der Registrierungseintrag von { $package } verweist auf einen Ort, den RABBIT nicht erwartet: { $path }
# The package's registry entry has subkeys, which its installer doesn't write.
uninstall-vendor-refused-registry-key-has-subkeys = Der Registrierungseintrag von { $package } enthält mehr, als RABBIT erwartet.
# { $path } is the package's uninstaller folder in Program Files.
uninstall-vendor-refused-untrusted-folder = Der Ordner von { $package } wurde nicht von einem Administrator angelegt: { $path }
# Follows each of the messages above. { $package } as above.
uninstall-vendor-refused-nothing-changed = Es wurde nichts geändert. Sie können { $package } in den Windows-Einstellungen unter Apps, Installierte Apps deinstallieren.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (nicht verfügbar: { $reason })
wizard-package-row-unavailable-portable = portable REAPER-Installation
wizard-package-row-unavailable-version-check = Online-Versionsprüfung fehlgeschlagen
# Why an Install or update row can't be ticked, for a package RABBIT leaves
# to ReaPack: ReaPack's records say ReaPack installed it, or RABBIT couldn't
# read those records to tell. Word them as
# wizard-uninstall-row-unavailable-reapack and its -check-failed partner do.
wizard-package-row-unavailable-reapack = von ReaPack verwaltet
wizard-package-row-unavailable-reapack-check-failed = die Einträge von ReaPack ließen sich nicht prüfen
# The sentence the details of such a row add. ReaPack's records can name a
# package whose file is gone, so it doesn't say a copy is there. $package is
# the localized package name, $reapack_package the name ReaPack knows the
# package by, and $error an error message.
wizard-package-details-reapack = Laut den Einträgen von ReaPack hat ReaPack { $package } als { $reapack_package } installiert, deshalb überlässt RABBIT es ReaPack. Installieren, aktualisieren oder entfernen Sie es in ReaPack.
wizard-package-details-reapack-check-failed = RABBIT konnte die Einträge von ReaPack nicht lesen ({ $error }) und kann deshalb nicht sagen, ob ReaPack { $package } installiert hat. Daher installiert, aktualisiert oder entfernt RABBIT es nicht.
# A sentence the package details add on a portable REAPER when the per-user
# CLAP folder holds a copy of a package that now installs into each REAPER's
# own UserPlugins\FX folder, such as app2clap. Every REAPER on the computer
# loads the plug-ins in the CLAP folder, so this one would load two copies.
# Updating or reinstalling the package in the standard REAPER moves the copy
# out of the CLAP folder. $package is the localized package name.
wizard-package-details-clap-copy-portable = Dieses REAPER lädt bereits die Kopie von { $package } in Ihrem CLAP-Ordner, wie jedes REAPER auf diesem Computer. Wird es hier zusätzlich installiert, lädt dieses REAPER zwei Kopien. Damit es eine eigene Kopie bekommt, aktualisieren Sie { $package } zuerst in der Standardinstallation von REAPER oder installieren Sie es dort neu: Dadurch wird die Kopie im CLAP-Ordner in eine Sicherung verschoben.
# The same on the standard REAPER: updating or reinstalling the package there
# moves the copy out of the CLAP folder into a backup, so portable REAPERs and
# every other program that loaded it from there lose it. $package is the
# localized package name.
wizard-package-details-clap-copy-standard = Ihr CLAP-Ordner enthält eine Kopie von { $package }. Wenn Sie { $package } hier aktualisieren oder neu installieren, wird diese Kopie in eine Sicherung verschoben, damit REAPER nicht zwei Kopien lädt. Portable REAPER und andere Programme, die CLAP-Plug-ins verwenden, finden sie danach nicht mehr. RABBIT kann { $package } auch in jedes portable REAPER installieren.

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package }: Die Prüfung auf die neueste Version ist fehlgeschlagen ({ $message }). Installation und Aktualisierung dieses Pakets sind in diesem Durchlauf deaktiviert.

detect-installed = Installiert
detect-not-installed = Nicht installiert
detect-version-unknown = Version unbekannt
detect-source-receipt = RABBIT-Beleg
detect-source-files = Vorhandene Datei in UserPlugins
detect-source-reapack-registry = ReaPack-Registry

# $package is the localized package display name.
status-package-installed = { $package } installiert

wizard-step-target = Ziel
wizard-step-version-check = Versionsprüfung
wizard-step-packages = Pakete
wizard-step-reapack-acknowledgement = ReaPack-Spendenhinweis
wizard-step-review = Überprüfung
wizard-step-progress = Fortschritt
wizard-step-done = Fertig

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Zurück
wizard-button-back-mnemonic = Z
wizard-button-next = Weiter
wizard-button-next-mnemonic = W
wizard-button-install = Installieren
wizard-button-install-mnemonic = I
wizard-button-uninstall = Deinstallieren
wizard-button-uninstall-mnemonic = D
# The run button before a restore and before a delete of backups, both
# started from the Backups tab. Their access keys must differ from those of
# Back, Next, Close and Cancel, which sit beside it.
wizard-button-restore = Wiederherstellen
wizard-button-restore-mnemonic = H
wizard-button-delete = Löschen
wizard-button-delete-mnemonic = L
wizard-button-close = Schließen
wizard-button-close-mnemonic = S
wizard-button-cancel = Abbrechen
wizard-button-cancel-mnemonic = A

wizard-target-heading = REAPER-Installation auswählen
wizard-target-language-label = Sprache
wizard-target-language-restart-note = Beim Wechsel der Sprache wird RABBIT neu gestartet, damit die neue Sprache wirksam wird.
wizard-locale-name-en-US = Englisch (Vereinigte Staaten)
wizard-locale-name-de-DE = Deutsch (Deutschland)
wizard-locale-name-es-ES = Spanisch (Spanien)
wizard-locale-name-fr-FR = Französisch (Frankreich)
wizard-locale-name-it-IT = Italienisch (Italien)
wizard-locale-name-pt-BR = Portugiesisch (Brasilien)
wizard-target-choice-label = Installationsziel
wizard-target-details-label = Zieldetails
wizard-target-empty = Es ist kein REAPER-Installationsziel ausgewählt.
wizard-target-portable-choice = Portablen REAPER-Ordner installieren oder aktualisieren
wizard-target-portable-folder-label = Portabler Ordner
wizard-target-portable-folder-message = Wählen Sie einen portablen REAPER-Ordner oder einen leeren Ordner für eine neue portable Einrichtung.
wizard-target-portable-folder-browse-label = Durchsuchen…
wizard-target-portable-pending-details = Wählen Sie zunächst die Option für ein portables Ziel und anschließend einen portablen REAPER-Ordner oder einen leeren Ordner für eine neue portable Einrichtung.
wizard-target-custom-portable-label = Portabler REAPER-Ordner
wizard-target-custom-portable-app-path-label = Pfad der REAPER-Anwendung
wizard-target-custom-portable-path-label = Portabler Ressourcenpfad
wizard-target-custom-portable-version-label = REAPER-Version
wizard-target-custom-portable-writable-label = Beschreibbar
wizard-target-custom-portable-note = RABBIT legt das REAPER-Ressourcenlayout hier an, falls es fehlt.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } unter { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = REAPER-Anwendung: { $app_path }
    REAPER-Version: { $version }
    Ressourcenpfad: { $path }
    Beschreibbar: { $writable }

wizard-packages-heading = Auswählen, was RABBIT tun soll
wizard-packages-list-label = Installieren oder aktualisieren: Pakete und Einstellungen
wizard-packages-row-checked = Ausgewählt
wizard-packages-row-unchecked = Nicht ausgewählt
wizard-packages-choice-shown = Neue Auswahl unter der Liste: { $choice }.
wizard-packages-choice-hidden = Auswahl entfernt: { $choice }.
wizard-packages-tree-group-label = Pakete
# Group headings in the Install or update and Uninstall lists, beside
# Packages and Language packs. Instruments holds software instruments such as
# Surge XT; Tools holds programs and plug-ins that help with other work, such
# as app2clap. Screen readers read the heading alone, so keep the two clearly
# different from each other.
wizard-instruments-tree-group-label = Instrumente
wizard-tools-tree-group-label = Werkzeuge
wizard-language-tree-group-label = Sprachpakete
wizard-configuration-tree-group-label = Konfiguration

# The first two of the Packages page's three tabs. Screen readers read the
# selected tab's name when the page opens, so keep them short.
wizard-packages-tab-install = Installieren oder aktualisieren
wizard-packages-tab-uninstall = Deinstallieren
# The third tab, where RABBIT's backups are restored or deleted. Keep it
# short too.
wizard-packages-tab-backups = Sicherungen
# macOS only: VoiceOver says this once, the first time the Packages page opens.
# Name the Tab and Shift keys as Apple's page of Mac keyboard shortcuts does,
# and write Control out in full.
wizard-packages-tabs-switch-hint-macos = Drücken Sie Control-Tabulatortaste oder Control-Umschalttaste-Tabulatortaste, um zwischen den Registerkarten zu wechseln.
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Deinstallieren: Software, die RABBIT entfernen kann
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package }: { $action }. Installiert: { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = RABBIT kann es noch nicht deinstallieren
wizard-uninstall-row-unavailable-not-supported = RABBIT kann es nicht deinstallieren
wizard-uninstall-row-unavailable-manual = von Hand entfernen
wizard-uninstall-row-unavailable-reapack = von ReaPack verwaltet
wizard-uninstall-row-unavailable-reapack-check-failed = die Einträge von ReaPack ließen sich nicht prüfen
wizard-uninstall-row-unavailable-no-receipt = RABBIT hat diese Kopie nicht installiert
wizard-uninstall-row-unavailable-no-trusted-uninstaller = kein Deinstallationsprogramm, dem RABBIT vertraut
wizard-uninstall-row-unavailable-unsafe-path = eine Datei, die RABBIT nie entfernt
# macOS: the app has to be moved to the Trash in Finder. Name the Trash and
# Finder as macOS does in your language.
wizard-uninstall-row-unavailable-by-hand-macos = im Finder in den Papierkorb legen
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack verwaltet { $package }: Es hat { $path } als { $reapack_package } installiert. Deinstallieren Sie es stattdessen in ReaPack.
wizard-uninstall-details-reapack-check-failed = RABBIT konnte die Einträge von ReaPack zu { $path } nicht prüfen ({ $reason }) und lässt { $package } deshalb unverändert.
wizard-uninstall-details-portable = { $package } dient jedem REAPER auf diesem Computer. RABBIT entfernt es deshalb nur aus der Standardinstallation von REAPER, nicht aus einer portablen.
wizard-uninstall-details-no-receipt = RABBIT hat diese Kopie von { $package } nicht installiert. Sie ist also höchstwahrscheinlich Ihre eigene, und RABBIT lässt sie unverändert.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT hat für { $package } kein Deinstallationsprogramm gefunden, dem es vertrauen kann.
wizard-uninstall-details-unsafe-path = RABBIT entfernt { $path } nicht. Es lässt { $package } lieber ganz unverändert, als es nur zum Teil zu entfernen.
# The details text of an Uninstall row whose package RABBIT leaves alone
# because of the uninstaller folder and registry entry its installer left:
# RABBIT doesn't trust what it found there, or finds none of the package's
# files for this account. The package's steps to remove it by hand follow.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-cleanup-refused = RABBIT vertraut dem nicht, was es im Ordner mit dem Deinstallationsprogramm von { $package } unter Programme (Program Files) oder in seinem Registrierungseintrag gefunden hat, und lässt { $package } deshalb unverändert.
wizard-uninstall-details-no-files-for-this-account = RABBIT findet für Ihr Benutzerkonto keine Dateien von { $package }. Wenn ein anderes Konto { $package } installiert hat, deinstallieren Sie es von jenem Konto aus. Bis dahin lässt RABBIT den Ordner mit dem Deinstallationsprogramm unter Programme (Program Files) und den Registrierungseintrag unverändert.
# The details text of a macOS app RABBIT can't move to the Trash, by itself or
# with an administrator's password: first why, then how to move it there in
# Finder. $package is the localized package name, $path the app's full macOS
# path, keep it as it is, and $error macOS's own error message. Name Finder,
# the Trash, File > Get Info and Locked as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does: Finder's
# menus show it only as symbols.
wizard-uninstall-details-by-hand-not-movable-macos = Ihr Benutzerkonto darf { $path } nicht verschieben: Es gehört einem anderen Benutzerkonto oder macOS, oder es liegt in einem Ordner, den Sie nicht ändern können.
wizard-uninstall-details-by-hand-locked-macos = { $path } ist gesperrt.
wizard-uninstall-details-by-hand-not-the-app-macos = { $path } ist nicht die App { $package }, die RABBIT dort erwartet. RABBIT lässt sie deshalb unverändert.
wizard-uninstall-details-by-hand-unchecked-macos = RABBIT konnte nicht prüfen, wer { $path } verschieben darf ({ $error }).
wizard-uninstall-details-by-hand-no-trash-folder-macos = Nur ein Administrator kann { $path } verschieben, und Ihr Papierkorb ist kein Ordner, der Ihnen gehört und den RABBIT prüfen kann.
wizard-uninstall-details-by-hand-other-disk-macos = Nur ein Administrator kann { $path } verschieben, und es liegt auf einem anderen Laufwerk als Ihr Papierkorb.
wizard-uninstall-details-by-hand-finder-macos = Um { $package } zu entfernen, wählen Sie es im Finder aus und drücken Sie Befehlstaste-Rückschritttaste. Der Finder fragt nach Name und Passwort eines Administrators, wenn er sie braucht.
wizard-uninstall-details-by-hand-unlock-macos = Um { $package } zu entfernen, wählen Sie es im Finder aus und wählen Sie „Ablage“ > „Informationen“. Deaktivieren Sie „Gesperrt“ und schließen Sie das Fenster. Drücken Sie dann Befehlstaste-Rückschritttaste. Der Finder fragt nach Name und Passwort eines Administrators, wenn er sie braucht. Wenn „Gesperrt“ nicht aktiviert ist oder der Finder es trotzdem nicht verschieben kann, hat es eine Sperre, die der Finder nicht aufheben kann.
wizard-uninstall-details-by-hand-not-the-app-finder-macos = Falls es doch { $package } ist und Sie es entfernen möchten, wählen Sie es im Finder aus und drücken Sie Befehlstaste-Rückschritttaste.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT verschiebt die Dateien in eine Sicherung, statt sie zu löschen.
wizard-uninstall-details-dependents-stay = Diese Pakete bauen auf { $package } auf und bleiben installiert: { $packages }. Sie funktionieren wieder, sobald Sie { $package } erneut installieren.
wizard-uninstall-details-machine-wide = { $package } dient jedem REAPER und jedem anderen Programm auf diesem Computer. Sie alle verlieren es.
wizard-uninstall-details-reapack-owned-extensions = Diese wurden mit ReaPack installiert und bleiben ohne Aktualisierungen zurück, sobald ReaPack entfernt ist: { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT konnte die Einträge von ReaPack nicht lesen und kann deshalb nicht sagen, was ReaPack noch verwaltet ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT hat diese Kopie nicht installiert. Es entfernt die Dateien, die { $package } bekanntermaßen installiert.
wizard-uninstall-details-reapack-orphan = { $package } wurde einmal mit ReaPack installiert. ReaPack ist nicht mehr da, also kann RABBIT es entfernen; die Einträge von ReaPack dazu bleiben.
# A note on a package whose installer added files outside REAPER's folder,
# when RABBIT has no record of which ones. $package is the localized
# package name.
wizard-uninstall-details-no-vendor-record = RABBIT hat nicht aufgezeichnet, welche Dateien das Installationsprogramm von { $package } außerhalb des REAPER-Ordners angelegt hat. Sie bleiben deshalb alle erhalten.
wizard-uninstall-details-vendor = Windows fragt Sie um Erlaubnis, bevor etwas entfernt wird.
wizard-uninstall-details-vendor-by-hand = Sie können { $package } selbst in den Windows-Einstellungen unter Apps, Installierte Apps deinstallieren.
# The details text of an Uninstall row whose package's installer left an
# uninstaller folder in Program Files and a registry entry, which RABBIT
# removes with administrator rights once the package's files are in the
# backup. It follows wizard-uninstall-details-files, or takes its place
# when none of the package's files are left to move. $package is the
# localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-machine-cleanup = Den Ordner mit dem Deinstallationsprogramm von { $package } unter Programme (Program Files) und seinen Registrierungseintrag darf nur ein Administrator entfernen. Deshalb fragt Windows Sie um Erlaubnis, bevor etwas entfernt wird. Eine Sicherung kann diese beiden nicht zurückholen.
# The details text of an Uninstall row on macOS, by how the package goes:
# deleted for good with an administrator's password; to the Trash, moved as
# you; or into your Trash with an administrator's password, because your
# account may not move it. Name the Trash and Put Back as Finder does in your
# language. The password window may offer Touch ID, so don't say a password
# is the only way. The Trash line follows the package's own sentence, which
# already says that it goes to the Trash, so it only says how to get it back.
# The administrator line follows the package's own sentence, which already
# says a backup can't bring it back, so it doesn't say that again.
wizard-uninstall-details-elevated-macos = macOS fragt einmal nach Name und Passwort eines Administrators, bevor etwas entfernt wird. Um es wieder zu haben, installieren Sie es erneut.
wizard-uninstall-details-trash = Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen.
wizard-uninstall-details-trash-elevated-macos = Ihr Benutzerkonto darf es nicht verschieben. Deshalb fragt macOS einmal nach Name und Passwort eines Administrators, bevor etwas entfernt wird, und RABBIT legt es dann in Ihren Papierkorb. Es behält seinen bisherigen Eigentümer, deshalb bietet der Finder dafür kein „Zurücklegen“ an, und das Entleeren des Papierkorbs kann nach dem Passwort eines Administrators fragen.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = RABBIT hat in diesem REAPER nichts gefunden, das es deinstallieren kann.
wizard-uninstall-empty-details = Keines der Programme, die RABBIT deinstallieren kann, ist in diesem REAPER installiert.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = RABBIT konnte seine Einträge zu diesem REAPER nicht lesen ({ $error }) und kann deshalb nicht auflisten, was es deinstallieren könnte.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Einstellungen
wizard-uninstall-remove-settings = Auch die Einstellungen entfernen
wizard-uninstall-settings-note-none-ticked = Wählen Sie ein Paket aus, um zu sehen, welche seiner Einstellungen RABBIT entfernen kann. Ohne diese Option bleiben Einstellungen erhalten.
wizard-uninstall-settings-note-none-removable = Keines der ausgewählten Pakete hat Einstellungen, die RABBIT entfernen kann.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = Mit dieser Option entfernt RABBIT auch diese Einstellungen. Sie kommen mit dem Rest in die Sicherung, aus der sie sich also wiederherstellen lassen.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package }: { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Jetzt unter der Liste verfügbar: { $option }.
wizard-uninstall-settings-unavailable = Nicht mehr verfügbar: { $option }.

# The Backups tab. The label right before its list names the tab first, as
# the other tabs' labels do; screen readers read it when focus enters the
# list. $size is a size, one of the size-* messages, such as "742 MB". The
# -empty form is for a list with no backups.
wizard-backups-list-label = Sicherungen: was RABBIT wiederherstellen oder löschen kann, insgesamt { $size }
wizard-backups-list-label-empty = Sicherungen: was RABBIT wiederherstellen oder löschen kann
# The one row, without a checkbox, of a Backups list with nothing in it, and
# its details text. On macOS, VoiceOver says the row's sentence after the
# tab's name instead.
wizard-backups-empty = RABBIT hat keine Sicherungen für dieses REAPER.
wizard-backups-empty-details = RABBIT legt eine Sicherung an, wenn es ein Paket deinstalliert, wenn eine Aktualisierung die Dateien eines Pakets ersetzt oder einige davon nicht mehr installiert und wenn die OSARA-Tastenzuordnung Ihre eigene ersetzt. Nichts davon ist in diesem REAPER bisher geschehen, oder seine Sicherungen wurden gelöscht. Für Änderungen durch das eigene Installations- oder Deinstallationsprogramm eines Pakets, den Papierkorb oder das Passwort eines Administrators gibt es nie eine Sicherung.
# Stands in for the Backups list when RABBIT can't read its backups folder.
# $error is the error message, in English.
wizard-backups-unreadable = RABBIT konnte die Sicherungen dieses REAPER nicht lesen.
wizard-backups-unreadable-details = RABBIT konnte seinen Sicherungsordner nicht lesen ({ $error }) und kann deshalb nicht auflisten, was es wiederherstellen oder löschen könnte.
# The group headings of the Backups list, rows without a checkbox, then the
# details text each shows. The details say what backups are for, and what
# they never hold.
wizard-backups-group-removed = Deinstallierte Software
wizard-backups-group-removed-details = Wenn RABBIT ein Paket deinstalliert, verschiebt es die Dateien hierher, statt sie zu löschen. Jede dieser Sicherungen ist die einzige Kopie. Was das eigene Deinstallationsprogramm eines Pakets, der Papierkorb oder das Passwort eines Administrators entfernt hat, ist nicht hier.
wizard-backups-group-versions = Frühere Versionen
wizard-backups-group-versions-details = Wenn eine Aktualisierung die Dateien eines Pakets ersetzt oder einige davon nicht mehr installiert, bewahrt RABBIT die alten hier auf. Aktualisierungen durch das eigene Installationsprogramm eines Pakets, etwa das von REAPER, bewahren keine auf.
wizard-backups-group-keymaps = Tastenzuordnungen
wizard-backups-group-keymaps-details = Wenn die OSARA-Tastenzuordnung Ihre eigene ersetzt, bewahrt RABBIT die alte hier auf. Meist ist nur die älteste Ihre eigene.
wizard-backups-group-other = Andere Sicherungen
wizard-backups-group-other-details = Diese Sicherungen kann RABBIT hier nicht wiederherstellen. Sie können sie löschen.
# One row of the Backups list, read as a list row. $backup is what the row
# calls the backup (one of the wizard-backups-name-* messages, or a package's
# name and version), $action is action-restore, action-delete,
# action-delete-only-copy or action-keep, $when one of the
# wizard-backups-when-* messages or backup-date-unknown, and $size a size,
# such as "4 MB".
wizard-backups-row = { $backup }: { $action }. { $when }, { $size }
# The row that stands for every old copy of RABBIT's records, read as a list
# row. $action as above, $count a number, the copies, and $size a size.
wizard-backups-row-records = Alte Kopien der Einträge von RABBIT: { $action }. { $count ->
    [one] Eine Kopie
   *[other] { $count } Kopien
}, { $size }
# A backup named in a sentence on Review, so that two backups with the same
# name can be told apart. $backup and $when as in wizard-backups-row.
wizard-backups-row-reference = { $backup } ({ $when })
# Names joined in a sentence: $rest is the names but the last, separated by
# commas, and $last the last one, as in "FFmpeg, ReaPack and SWS".
wizard-backups-list-and = { $rest } und { $last }
# What a row calls a package's backup: its localized name, $package, and the
# version in the backup, $version, a version number, or for a package that
# comes in variants that number with its variant, as
# wizard-backups-version-variant says it.
wizard-backups-name-package = { $package } { $version }
# A version of a package that comes in variants, such as the Spanish
# language pack's two OSARA translations, wherever the Backups tab, its
# Review and Done name a version, so the same number in another variant
# reads apart: $version is the version number, $variant the variant's
# localized name, as package-langpack-es-variant-* gives it. Each $version
# of the Backups tab's sentences below may be in this form, or, where
# the other version a sentence names is the same number in another
# variant, the variant's localized name alone.
wizard-backups-version-variant = { $version }, { $variant }
# What a row calls a package's backup whose version RABBIT can't tell, or
# that differs from the version it is named beside only by its variant:
# $package and $variant as above.
wizard-backups-name-package-variant = { $package }, { $variant }
# What a row calls a key map: your own; one whose kind RABBIT didn't record,
# as in backups from earlier versions of RABBIT and those a restore made;
# REAPER's own default keys; an earlier OSARA key map you hadn't changed.
wizard-backups-name-keymap-own = Ihre Tastenzuordnung
wizard-backups-name-keymap = Tastenzuordnung
wizard-backups-name-keymap-default = Die Standardtasten von REAPER
wizard-backups-name-keymap-osara = Eine frühere OSARA-Tastenzuordnung
# What a row calls the other backups. "RABBIT's records" is RABBIT's list of
# what it installed.
wizard-backups-name-records = Alte Kopien der Einträge von RABBIT
wizard-backups-name-records-copy = Eine Kopie der Einträge von RABBIT
wizard-backups-name-not-a-backup = Keine Sicherung von RABBIT
wizard-backups-name-deleting = Nicht abgeschlossene Löschung
wizard-backups-name-no-manifest = Sicherung ohne Inhaltsverzeichnis
wizard-backups-name-damaged = Beschädigte Sicherung
wizard-backups-name-before-restore = Von einer Wiederherstellung ersetzte Dateien
wizard-backups-name-update-files = Weitere von einer Aktualisierung ersetzte Dateien
wizard-backups-name-uninstall-files = Weitere von einer Deinstallation verschobene Dateien
wizard-backups-name-keymap-files = Weitere Dateien zu einer Tastenzuordnung
wizard-backups-name-unknown = Sicherung aus einer anderen RABBIT-Version
# When a backup was made, as its row says it after the action. $date is a
# date and time (backup-date-time), and $version a version number, or for
# a package that comes in variants as wizard-backups-version-variant says.
# "Replaced by OSARA's" means by OSARA's key map.
wizard-backups-when-uninstalled = Deinstalliert am { $date }
wizard-backups-when-replaced-by = Ersetzt durch { $version } am { $date }
wizard-backups-when-replaced = Ersetzt am { $date }
wizard-backups-when-keymap-osara = Ersetzt durch die OSARA-Tastenzuordnung am { $date }
wizard-backups-when-keymap-restore = Ersetzt durch eine Wiederherstellung am { $date }
wizard-backups-when-made = Angelegt am { $date }
# A date and time, as rows, details and Review say when a backup was made,
# such as "November 9, 2026, 2:05 PM". $year, $month (1 to 12), $day,
# $hour (0 to 23) and $hour12 (1 to 12) are numbers; $minute is two digits,
# such as "05"; $period is am or pm. Use the 24-hour clock with $hour and
# $minute, or the 12-hour one with $hour12, $minute and $period, as your
# language does. Name the month by the select on $month. Sentences put the
# date after a word such as "on" or "from": where that word would change
# with the day, as Italian "l'8" or "dell'11" do, word them so it doesn't.
backup-date-time = { $day }. { $month ->
    [1] Januar
    [2] Februar
    [3] März
    [4] April
    [5] Mai
    [6] Juni
    [7] Juli
    [8] August
    [9] September
    [10] Oktober
    [11] November
   *[12] Dezember
} { $year }, { $hour }:{ $minute } Uhr
# Instead of a row's when-text, for a backup whose date RABBIT can't tell.
backup-date-unknown = Datum unbekannt
# Sizes, in steps of 1024. $count, $whole and $tenth are numbers. A size in
# GB is $whole and one decimal, $tenth: put your language's decimal mark
# between them.
size-under-kb = unter 1 KB
size-kb = { $count } KB
size-mb = { $count } MB
size-gb = { $whole },{ $tenth } GB
# Why a Backups row can't be ticked for the choice below the list: the short
# phrase that goes into wizard-package-row-unavailable-suffix at the end of
# the row. $package is the localized package name.
wizard-backups-unavailable-not-here = hier nicht wiederherstellbar
wizard-backups-unavailable-installed-again = { $package } ist wieder installiert
wizard-backups-unavailable-not-complete = ein Teil davon ist nicht in der Sicherung
wizard-backups-unavailable-nothing-in-it = keine der Dateien ist in dieser Sicherung
wizard-backups-unavailable-version-unknown = RABBIT weiß nicht, welche Version das war
wizard-backups-unavailable-other-language-pack = ein anderes Sprachpaket ist installiert
wizard-backups-unavailable-part-missing = ein Teil davon fehlt
wizard-backups-unavailable-records-unreadable = RABBIT kann seine Einträge nicht lesen
wizard-backups-unavailable-not-yours = gehört einem anderen Benutzerkonto
wizard-backups-unavailable-not-a-backup = RABBIT lässt das unverändert
wizard-backups-unavailable-receipt-in-set = enthält eine Kopie der Einträge von RABBIT
wizard-backups-unavailable-file-refused = RABBIT legt eine der Dateien nicht zurück
wizard-backups-unavailable-cannot-restore = RABBIT kann sie nicht wiederherstellen
# The details text of a Backups row, a sentence or two per line, in this
# order: what the backup is; when it was made; what it holds and how big;
# whether it is the only copy; what restoring does, or why it can't; what
# deleting loses. $backup is what the row calls the backup, $package the
# localized package name, $version a version number, or for a package
# that comes in variants as wizard-backups-version-variant says, $date a
# date and time (backup-date-time), $size a size, such as "4 MB", and
# $count a number.
#
# What the backup is.
wizard-backups-details-uninstalled = { $backup } im Zustand vor der Deinstallation.
wizard-backups-details-earlier = { $backup }, eine frühere Version.
wizard-backups-details-keymap-own = Ihre Tastenzuordnung, wie sie war, bevor die OSARA-Tastenzuordnung sie ersetzt hat.
wizard-backups-details-keymap-default = Die Standardtasten von REAPER, wie sie waren, bevor die OSARA-Tastenzuordnung sie ersetzt hat.
wizard-backups-details-keymap-osara = Eine frühere OSARA-Tastenzuordnung, wie sie war, bevor eine neuere sie ersetzt hat.
wizard-backups-details-keymap-unknown = Eine Tastenzuordnung, wie sie war, bevor die OSARA-Tastenzuordnung sie ersetzt hat. Es kann Ihre eigene oder eine ältere OSARA-Tastenzuordnung sein: RABBIT hat nicht festgehalten, welche.
wizard-backups-details-keymap-restore = Die Tastenzuordnung, die eine Wiederherstellung ersetzt hat. Es kann Ihre eigene oder die von OSARA sein.
wizard-backups-details-keymap-other = Eine Tastenzuordnung, die RABBIT aufbewahrt hat.
wizard-backups-details-records = Kopien der Liste, in der RABBIT festhält, was es installiert hat, aus früheren Durchläufen. Wenn Sie sie löschen, ändert das nichts an dem, was installiert ist.
wizard-backups-details-records-copy = Eine Kopie der Liste, in der RABBIT festhält, was es installiert hat, aus einem früheren Durchlauf.
wizard-backups-details-deleting = Das Löschen dieser Sicherung hat begonnen und wurde nicht abgeschlossen, daher fehlt ein Teil davon möglicherweise schon.
wizard-backups-details-not-a-backup = Das ist keine Sicherung von RABBIT: Es ist eine Verknüpfung oder ein Ordner mit einem Namen, den RABBIT nie einer Sicherung gibt.
wizard-backups-details-no-manifest = Eine Sicherung ohne Inhaltsverzeichnis, wie sie ein früheres RABBIT hinterlassen hat, das vorzeitig angehalten wurde.
wizard-backups-details-damaged = Eine Sicherung, deren Inhaltsverzeichnis sich nicht lesen lässt.
wizard-backups-details-before-restore = Dateien, die eine Wiederherstellung ersetzt hat. RABBIT hat sie aufbewahrt, damit sich die Wiederherstellung rückgängig machen lässt.
wizard-backups-details-update-files = Dateien, die eine Aktualisierung ersetzt hat und die keine andere Zeile hier zurückbringt.
wizard-backups-details-uninstall-files = Dateien, die eine Deinstallation verschoben hat und die zu keinem der dabei aufgezeichneten Pakete gehören.
wizard-backups-details-keymap-files = Dateien zu einer Tastenzuordnung, die keine andere Zeile hier zurückbringt.
wizard-backups-details-unknown = Eine Sicherung aus einer anderen RABBIT-Version, aus einem Grund, den diese Version nicht kennt.
# When it was made.
wizard-backups-details-when-uninstalled = Die Deinstallation war am { $date }.
wizard-backups-details-when-updated-to = Eine Aktualisierung auf { $version } hat sie am { $date } ersetzt.
wizard-backups-details-when-updated = Eine Aktualisierung hat sie am { $date } ersetzt.
wizard-backups-details-when-restored-to = Die Rückkehr zu { $version } hat sie am { $date } ersetzt.
wizard-backups-details-when-restored = Eine Wiederherstellung hat sie am { $date } ersetzt.
wizard-backups-details-when-osara = Die OSARA-Tastenzuordnung hat sie am { $date } ersetzt.
wizard-backups-details-when-keymap-restored = Eine wiederhergestellte Tastenzuordnung hat sie am { $date } ersetzt.
wizard-backups-details-when-made = RABBIT hat diese Sicherung am { $date } angelegt.
wizard-backups-details-when-records = Die neueste Kopie ist vom { $date }.
wizard-backups-details-when-unknown = RABBIT kann nicht feststellen, wann diese Sicherung angelegt wurde.
# What it holds and how big it is. The settings are named only when the
# uninstall recorded that it moved them; backups from earlier versions of
# RABBIT don't say, so the -older form says "anything else".
wizard-backups-details-holds-with-settings = Diese Sicherung enthält die Dateien und Ihre Einstellungen für { $package }, { $size }.
wizard-backups-details-holds-files = Diese Sicherung enthält die Dateien, { $size }.
wizard-backups-details-holds-older = Diese Sicherung enthält die Dateien und alles andere, was diese Deinstallation verschoben hat, { $size }.
wizard-backups-details-holds-replaced = Diese Sicherung enthält die Dateien dieser Version, die ersetzt oder nicht mehr installiert wurden, { $size }.
wizard-backups-details-holds-keymap = Diese Sicherung enthält die Tastenzuordnung, { $size }.
wizard-backups-details-holds-size = Diese Sicherung belegt { $size }.
wizard-backups-details-holds-records = { $count ->
    [one] Es gibt eine Kopie, { $size }.
   *[other] Es gibt { $count } Kopien, insgesamt { $size }.
}
wizard-backups-details-only-copy = Es ist die einzige Kopie.
# What restoring does, for a backup that can be restored. Name REAPER's
# Actions list as REAPER does in your language. The -uninstalled-files form
# is for a backup that names no version, which may hold no record of the
# package to put back, so it says nothing about RABBIT's records.
wizard-backups-details-restore-uninstalled = Die Wiederherstellung legt die Dateien zurück, und RABBIT zählt { $package } wieder als installiert.
wizard-backups-details-restore-uninstalled-files = Die Wiederherstellung legt die Dateien zurück.
wizard-backups-details-restore-stopped-partway = Die Deinstallation wurde mittendrin angehalten, daher sind einige Dateien nur in dieser Sicherung. Die Wiederherstellung legt sie zurück.
wizard-backups-details-restore-already-back = { $package } ist schon wieder da. Die Wiederherstellung legt die Dateien noch einmal zurück.
wizard-backups-details-restore-earlier = Die Wiederherstellung kehrt zu { $version } zurück und bewahrt die Version, die Sie jetzt haben, als Sicherung auf, sodass Sie wieder zu ihr wechseln können. Ihre Einstellungen bleiben, wie sie sind.
# The same for a language pack, which has no settings of its own:
# REAPER's language setting names the pack's file, so it follows the
# earlier version while it names the version you have now.
wizard-backups-details-restore-earlier-language-pack = Die Wiederherstellung kehrt zu { $version } zurück und bewahrt die Version, die Sie jetzt haben, als Sicherung auf, sodass Sie wieder zu ihr wechseln können. Wenn REAPER die Version, die Sie jetzt haben, für seine Sprache verwendet, verwendet es stattdessen diese.
wizard-backups-details-restore-earlier-copy = Die Wiederherstellung legt diese Kopie zurück und bewahrt die Version, die Sie jetzt haben, als Sicherung auf, sodass Sie wieder zu ihr wechseln können.
# The same for an earlier version of a package that isn't installed now,
# as after you uninstalled it: restoring installs it again, and there is no
# version to keep as a backup. A language pack's restore leaves REAPER's
# language as it is, so that form says how to choose it; name REAPER's
# preferences as REAPER does in your language. The -copy form is for a
# copy RABBIT didn't install, whose version it doesn't know.
wizard-backups-details-restore-earlier-again = Die Wiederherstellung installiert { $version } aus dieser Sicherung wieder, und RABBIT zählt { $package } als installiert. Ihre Einstellungen bleiben, wie sie sind.
wizard-backups-details-restore-earlier-again-language-pack = Die Wiederherstellung installiert { $version } aus dieser Sicherung wieder, und RABBIT zählt { $package } als installiert. Die Sprache von REAPER bleibt, wie sie ist: Um dieses Sprachpaket zu verwenden, wählen Sie es in den Einstellungen von REAPER.
wizard-backups-details-restore-earlier-copy-again = Die Wiederherstellung legt diese Kopie zurück.
wizard-backups-details-restore-earlier-already-back = { $package } hat diese Version schon. Die Wiederherstellung legt die Dateien noch einmal zurück.
wizard-backups-details-restore-keymap = Die Wiederherstellung macht sie wieder zur Tastenzuordnung von REAPER und bewahrt die aktuelle als Sicherung auf. Skripte, die Sie seitdem installiert haben, bleiben in der Aktionsliste von REAPER.
wizard-backups-details-restore-default-keys = Die Wiederherstellung bringt die Standardtasten von REAPER zurück, nicht Ihre eigene Tastenzuordnung, und bewahrt die aktuelle Tastenzuordnung als Sicherung auf.
# Why it can't be restored, by what RABBIT found. Name the Uninstall tab as
# wizard-packages-tab-uninstall does.
wizard-backups-details-not-here = RABBIT kann sie hier nicht wiederherstellen.
wizard-backups-details-cannot-restore = RABBIT kann sie nicht wiederherstellen.
wizard-backups-details-status-installed-again = { $package } ist wieder installiert. Deinstallieren Sie zuerst die vorhandene Kopie auf der Registerkarte „Deinstallieren“, und stellen Sie dann die Sicherung wieder her.
wizard-backups-details-status-not-complete = Ein Teil davon ist nicht in der Sicherung, daher kann RABBIT nicht zu dieser Version zurückkehren.
wizard-backups-details-status-nothing-in-it = Keine der Dateien ist in dieser Sicherung, es gibt also nichts wiederherzustellen.
wizard-backups-details-status-version-unknown = RABBIT weiß nicht, welche Version das war, und kann deshalb nicht dorthin zurückkehren.
wizard-backups-details-status-other-language-pack = Ein anderes Sprachpaket ist installiert, und die Installation des einen entfernt das andere. Deshalb bringt RABBIT dieses nicht zurück, solange das andere installiert ist.
wizard-backups-details-status-part-missing = Ein Teil davon fehlt in der Sicherung, daher kann RABBIT sie nicht wiederherstellen.
wizard-backups-details-status-records-unreadable = RABBIT kann seine Einträge nicht lesen und deshalb nicht feststellen, ob diese Sicherung dazu passt.
wizard-backups-details-status-not-yours = Sie gehört einem anderen Benutzerkonto, daher stellt RABBIT sie weder wieder her, noch löscht es sie.
wizard-backups-details-status-not-a-backup = RABBIT schaut nie hinein, stellt nichts daraus wieder her und löscht es nicht.
# macOS: how to remove another account's backup by hand. The backups are in
# the hidden Library folder, so the steps go through Finder's Go menu.
# $folder is the folder that holds the backup, a full macOS path, and $name
# the backup's own folder name; keep both as they are. Name Finder, its Go
# menu and the menu's Go to Folder item as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does. Never
# suggest sudo.
wizard-backups-details-not-yours-finder-macos = Um sie zu entfernen, wählen Sie im Finder „Gehe zu“ > „Gehe zum Ordner“, geben Sie { $folder } ein, wählen Sie dann { $name } aus, drücken Sie Befehlstaste-Rückschritttaste und entleeren Sie den Papierkorb. Der Finder fragt nach Name und Passwort eines Administrators, wenn er sie braucht.
# What deleting it loses.
wizard-backups-details-delete-uninstalled = Wenn Sie diese Sicherung löschen, kann RABBIT diese Kopie nicht zurückbringen.
wizard-backups-details-delete-earlier = Wenn Sie diese Sicherung löschen, können Sie nicht mehr zu { $version } zurückkehren.
wizard-backups-details-delete-earlier-copy = Wenn Sie diese Sicherung löschen, können Sie nicht mehr zu dieser Kopie zurückkehren.
wizard-backups-details-delete-keymap = Wenn Sie diese Sicherung löschen, kann RABBIT diese Tastenzuordnung nicht zurückbringen.
wizard-backups-details-delete-already-back = Die Dateien liegen auch auf der Festplatte, daher geht beim Löschen nichts verloren.
wizard-backups-details-delete-other = Wenn Sie sie löschen, ist sie endgültig weg.
wizard-backups-details-delete-deleting = Wenn Sie sie löschen, wird diese Löschung abgeschlossen.
# For a backup RABBIT can only delete together with the whole backup set it
# is in, such as the files of a set that no other row brings back, after
# the sentence above: the other rows of that set, which go with it. They
# come after a colon, so a name like "Your key map" keeps its capital.
# $backups is a list of what they call their backups, joined as
# wizard-backups-list-and joins them, and $count how many there are, a
# number. The -undeletable form names only those RABBIT can't delete, such
# as another account's, so this one can't be deleted either.
wizard-backups-details-delete-only-with = RABBIT kann diese Sicherung nur zusammen mit { $count ->
    [one] einer weiteren Sicherung
   *[other] { $count } weiteren Sicherungen
} löschen: { $backups }.
wizard-backups-details-delete-only-with-undeletable = RABBIT kann diese Sicherung nur zusammen mit { $count ->
    [one] einer weiteren Sicherung
   *[other] { $count } weiteren Sicherungen
} löschen, die RABBIT nicht löschen kann: { $backups }.
# The choice below the Backups list: its label, which screen readers read
# with the choice, then the two choices, Restore and Delete. Restore is
# where the tab starts. The choices have no access keys.
wizard-backups-choice-label = Was mit den ausgewählten Sicherungen geschehen soll
wizard-backups-choice-restore = Wiederherstellen
wizard-backups-choice-delete = Löschen
# The choice below the Backups list, Restore or Delete, keeps its own ticks.
# After a switch, VoiceOver on macOS says how many backups are ticked for the
# new choice. $choice is restore or delete, and $count a number.
wizard-backups-ticked-macos = { $choice ->
    [delete] { $count ->
        [0] Keine Sicherung zum Löschen ausgewählt.
        [one] Eine Sicherung zum Löschen ausgewählt.
       *[other] { $count } Sicherungen zum Löschen ausgewählt.
    }
   *[restore] { $count ->
        [0] Keine Sicherung zum Wiederherstellen ausgewählt.
        [one] Eine Sicherung zum Wiederherstellen ausgewählt.
       *[other] { $count } Sicherungen zum Wiederherstellen ausgewählt.
    }
}
# Why a restore of a backup is refused, a sentence or two each, about the
# backup, which they call "it". Review puts one after "Can't restore
# <backup> yet." or "Can't restore yet:", the Done page after "RABBIT
# didn't restore <backup>." or after its first line, and a row's details
# after what the backup is. The sentence before names the backup by what
# its row calls it, such as "SWS Extension 2.14.0.7" or "your key map",
# never by the word "backup": where your language's pronoun needs a
# matching noun, start with "this backup" instead. $package is a localized
# package name, $packages a list of them, $path a file or folder as the
# computer writes it, keep it as it is, $detail an error message in
# English, $reason one of the wizard-restore-target-* phrases, and
# $programs a comma-separated list of program names, never translated.
# Name the Uninstall tab as wizard-packages-tab-uninstall does.
wizard-restore-refusal-invalid-id = RABBIT gibt einer Sicherung nie einen solchen Namen.
wizard-restore-refusal-not-found = Diese Sicherung wurde gelöscht oder hat nie existiert. Starten Sie RABBIT erneut, um die Sicherungen so zu sehen, wie sie jetzt sind.
wizard-restore-refusal-differs-in-case = RABBIT findet diese Sicherung nur unter einem Namen mit anderer Groß- und Kleinschreibung.
wizard-restore-refusal-ends-in-dot-or-space = RABBIT findet diese Sicherung nur unter einem Namen ohne den Punkt oder das Leerzeichen am Ende.
wizard-restore-refusal-being-deleted = Das Löschen dieser Sicherung hat begonnen und wurde nicht abgeschlossen, daher fehlt ein Teil davon möglicherweise.
wizard-restore-refusal-is-link = RABBIT verwendet { $path } nicht: Es ist eine Verknüpfung, und Verknüpfungen in seinen Sicherungen folgt RABBIT nie.
wizard-restore-refusal-no-manifest = Diese Sicherung verzeichnet nicht, welche ihrer Dateien zu { $package } gehören.
wizard-restore-refusal-no-package-records = Diese Sicherung verzeichnet keine Pakete, daher kann RABBIT { $package } nicht allein daraus wiederherstellen.
wizard-restore-refusal-package-not-in-set = Diese Sicherung enthält keinen Eintrag zu { $package }.
wizard-restore-refusal-no-keymap-in-set = Diese Sicherung enthält keine Tastenzuordnung, die RABBIT wiederherstellen kann.
wizard-restore-refusal-damaged-manifest = RABBIT kann das Inhaltsverzeichnis dieser Sicherung nicht lesen ({ $detail }).
wizard-restore-refusal-records-only = Diese Sicherung enthält nur eine Kopie der Einträge von RABBIT, es gibt darin also nichts wiederherzustellen.
wizard-restore-refusal-receipt-in-set = Diese Sicherung enthält eine Kopie der Einträge von RABBIT. Diese Kopie zurückzulegen würde jede spätere Änderung an den Einträgen rückgängig machen, daher stellt RABBIT diese Sicherung nicht wieder her.
wizard-restore-refusal-reinstalled-since = { $package } ist wieder installiert, und zwar eine Kopie, die erst nach dieser Sicherung installiert wurde. Deinstallieren Sie diese zuerst auf der Registerkarte „Deinstallieren“, und stellen Sie dann die Sicherung wieder her.
wizard-restore-refusal-nothing-of-it-in-set = Keine der Dateien von { $packages } ist darin, es gibt also nichts wiederherzustellen.
# Instead of the sentence above, when nothing in a backup of earlier
# versions belongs to a package.
wizard-restore-refusal-nothing-of-it-in-set-none = Nichts darin gehört zu einem Paket, das RABBIT zurücklegen kann.
wizard-restore-refusal-part-missing = Ein Teil davon fehlt: Die Kopie von { $path } in dieser Sicherung ist weg oder ist nicht die Datei, die die Sicherung verzeichnet.
# Not complete: a file of that version isn't in the backup, and RABBIT's
# records don't list it on disk with that version's content. A file that
# is on disk but that RABBIT no longer keeps track of, such as one an
# update couldn't move, doesn't count. $path is the file's path.
wizard-restore-refusal-not-complete = Diese Sicherung ist nicht vollständig: { $path } ist nicht in der Sicherung, und die Einträge von RABBIT verzeichnen die Datei nicht so auf der Festplatte, wie diese Version sie hatte.
wizard-restore-refusal-version-unknown = RABBIT weiß nicht, welche Version von { $package } diese Sicherung enthält.
wizard-restore-refusal-other-language-pack-installed = RABBIT bringt { $package } nicht zurück, solange Folgendes installiert ist: { $packages }. Die Installation des einen Sprachpakets entfernt das andere.
wizard-restore-refusal-target-refused = RABBIT legt { $path } nicht zurück: { $reason }.
wizard-restore-refusal-newer-file-refused = Die Rückkehr würde { $path }, eine Datei der neueren Version von { $package }, in eine Sicherung verschieben, aus der sie nicht zurückkäme: { $reason }.
wizard-restore-refusal-earlier-file-refused = Der Eintrag dieser Sicherung zu jener Version von { $package } nennt { $path }, einen Ort, an dem RABBIT nie installiert: { $reason }.
wizard-restore-refusal-receipt-unreadable = RABBIT kann seine Einträge nicht lesen ({ $detail }) und deshalb nicht feststellen, ob diese Sicherung dazu passt.
wizard-restore-refusal-reaper-running = REAPER ist geöffnet. Schließen Sie es und versuchen Sie es dann erneut.
# The same, for packages every REAPER on the computer can use, such as
# the JAWS scripts. $packages is a list of localized package names.
wizard-restore-refusal-reaper-running-every = Ein REAPER ist geöffnet. Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und versuchen Sie es dann erneut. Jedes REAPER kann { $packages } verwenden.
wizard-restore-refusal-keymap-needs-reaper-closed = Diese Sicherung legt die Tastenzuordnung von REAPER zurück, und das tut RABBIT nur, solange REAPER geschlossen ist.
wizard-restore-refusal-program-running = Schließen Sie diese Programme und versuchen Sie es dann erneut: { $programs }.
wizard-restore-refusal-not-writable = RABBIT kann nicht dorthin schreiben, wohin die Wiederherstellung Dateien zurücklegt, in { $path } ({ $detail }).
wizard-restore-refusal-lock-held = Ein anderes RABBIT arbeitet gerade an diesem REAPER. Warten Sie, bis es fertig ist.
# sudo and root are never translated.
wizard-restore-refusal-as-root = RABBIT läuft als root, wie unter sudo, und stellt so keine Sicherungen wieder her. Beenden Sie RABBIT und starten Sie es mit Ihrem eigenen Benutzerkonto.
wizard-restore-refusal-not-yours = Diese Sicherung gehört einem anderen Benutzerkonto, daher stellt RABBIT sie nicht wieder her.
wizard-restore-refusal-changed-while-read = Diese Sicherung hat sich geändert, während RABBIT sie gelesen hat, etwa weil ein anderes RABBIT noch daran geschrieben hat.
wizard-restore-refusal-changed-since-listed = Diese Sicherung hat sich geändert, seit RABBIT sie aufgelistet hat. Starten Sie RABBIT erneut, um die Sicherungen so zu sehen, wie sie jetzt sind.
# Why a restore won't put a file back at a place: the phrase that ends
# wizard-restore-refusal-target-refused and the two after it. Start in lower
# case where your language allows. $path is a file or folder, keep it as it
# is; $name a file or folder name; $detail an error message in English.
wizard-restore-target-rabbit-folder = RABBIT legt nie eine Datei in seinen eigenen Ordner zurück
wizard-restore-target-leads-into-rabbit-folder = der Weg dorthin führt in den eigenen Ordner von RABBIT
wizard-restore-target-name-not-as-written = der Name { $name } auf dem Weg dorthin kann eine andere Datei oder einen anderen Ordner öffnen, als er nennt
wizard-restore-target-place-is-link = dort ist eine Verknüpfung, und Verknüpfungen ersetzt RABBIT nie
wizard-restore-target-through-link = der Weg dorthin führt über { $path }, eine Verknüpfung, und Verknüpfungen folgt RABBIT nie
wizard-restore-target-leads-outside-resource-folder = der Weg dorthin führt aus dem Ressourcenordner { $path } hinaus
wizard-restore-target-leads-outside = der Weg dorthin führt aus { $path } hinaus
wizard-restore-target-outside-allowed-folders = der Ort liegt außerhalb des Ressourcenordners von REAPER und jedes anderen Ordners, in den RABBIT installiert
wizard-restore-target-not-plain-path = es ist kein einfacher Pfad innerhalb des Ressourcenordners
wizard-restore-target-guard-folder-gone = der Ordner, in den die Datei zurückkommt, { $path }, ist nicht mehr da
wizard-restore-target-could-not-check = RABBIT konnte { $path } nicht prüfen ({ $detail })
wizard-restore-target-two-files-for-one-place = die Sicherung enthält mehr als eine Datei für diesen Ort
wizard-restore-target-outside-without-manifest = die Sicherung verzeichnet nicht, wohin diese Datei von außerhalb des Ressourcenordners gehört
wizard-restore-target-copy-outside-set = das Verzeichnis der Sicherung nennt ihre Kopie außerhalb der Sicherung
wizard-restore-target-keymap-among-version-files = es ist die Tastenzuordnung von REAPER, und die legt RABBIT nur als eigene Tastenzuordnung zurück
# Why RABBIT doesn't delete a backup, or any of them, a sentence or two
# each, as the Done page and Show details say it. Those about one backup
# call it "it": the Done page puts them after "RABBIT can't delete
# <backup>.". $path is a folder as the computer writes it, keep it as it
# is, $package a localized package name, and $detail an error message in
# English. sudo and root are never translated.
wizard-delete-refusal-invalid-id = RABBIT gibt einer Sicherung nie einen solchen Namen.
wizard-delete-refusal-differs-in-case = RABBIT findet diese Sicherung nur unter einem Namen mit anderer Groß- und Kleinschreibung.
wizard-delete-refusal-ends-in-dot-or-space = RABBIT findet diese Sicherung nur unter einem Namen ohne den Punkt oder das Leerzeichen am Ende.
wizard-delete-refusal-is-link = RABBIT verwendet { $path } nicht: Es ist eine Verknüpfung, und Verknüpfungen in seinen Sicherungen folgt RABBIT nie.
wizard-delete-refusal-not-yours = Diese Sicherung gehört einem anderen Benutzerkonto, daher löscht RABBIT sie nicht.
wizard-delete-refusal-changed-since-listed = Diese Sicherung hat sich geändert, seit RABBIT sie aufgelistet hat. Starten Sie RABBIT erneut, um die Sicherungen so zu sehen, wie sie jetzt sind.
wizard-delete-refusal-lock-held = Ein anderes RABBIT arbeitet gerade an diesem REAPER. Warten Sie, bis es fertig ist, und starten Sie RABBIT dann erneut.
wizard-delete-refusal-as-root = RABBIT läuft als root, wie unter sudo, und löscht so keine Sicherungen. Beenden Sie RABBIT und starten Sie es mit Ihrem eigenen Benutzerkonto.
# Why RABBIT doesn't delete one package or the key map out of the backup set
# it is kept in on its own, as above. They call the package or key map
# "it", as the Done page puts them after its name, and that set "the backup
# it is kept in", or "was kept in" where the set no longer holds it.
wizard-delete-refusal-being-deleted = Das Löschen der Sicherung, in der sie aufbewahrt wird, hat begonnen und wurde nicht abgeschlossen, daher kann RABBIT nur noch das Löschen der ganzen Sicherung abschließen.
wizard-delete-refusal-no-manifest = Die Sicherung, in der sie aufbewahrt wird, verzeichnet nicht, welche ihrer Dateien zu { $package } gehören, daher löscht RABBIT sie nicht einzeln.
wizard-delete-refusal-damaged-manifest = RABBIT kann das Inhaltsverzeichnis der Sicherung, in der sie aufbewahrt wird, nicht lesen ({ $detail }), daher löscht es nichts aus dieser Sicherung.
wizard-delete-refusal-no-keymap-in-set = Die Sicherung, in der sie aufbewahrt wird, enthält keine Tastenzuordnung, die RABBIT einzeln löschen kann.
wizard-delete-refusal-no-package-records = Die Sicherung, in der sie aufbewahrt wird, verzeichnet keine Pakete, daher kann RABBIT nicht erkennen, welche ihrer Dateien zu { $package } gehören.
wizard-delete-refusal-package-not-in-set = Die Sicherung, in der sie aufbewahrt wurde, enthält keine Kopie von { $package } mehr, die RABBIT löschen kann.
# Instead of wizard-delete-refusal-not-yours, when what belongs to another
# account is an unfinished deletion of the backup set the package, key map
# or files are kept in, which has the same name as that set.
wizard-delete-refusal-not-yours-earlier-deletion = Eine nicht abgeschlossene Löschung der Sicherung, in der sie aufbewahrt wird, gehört einem anderen Benutzerkonto, daher ändert RABBIT diese Sicherung nicht.

# $package ist der lokalisierte Paketname, von dem der Konfigurationsschritt abhängt.
wizard-configuration-row-unavailable = Nicht verfügbar: erfordert, dass { $package } installiert ist.
wizard-configuration-row-already-applied = Auf diesem REAPER-Ziel bereits angewendet.
# Kurzer Statushinweis, der in Klammern an die Baumzeile eines Konfigurationsschritts
# angehängt wird, wenn dieser nicht ausgewählt werden kann. Knapp gehalten, damit die
# Zeilenbeschriftung lesbar bleibt; die ausführlicheren Sätze aus
# `wizard-configuration-row-unavailable` / `wizard-configuration-row-already-applied`
# werden weiterhin im Detailbereich und als Barrierefreiheitsbegründung angezeigt.
# $reason ist einer der "wizard-configuration-row-status-*"-Strings unten.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package ist der lokalisierte Name des abhängigen Pakets.
wizard-configuration-row-status-requires = erfordert { $package }
wizard-configuration-row-status-already-applied = bereits angewendet
# Benennt die Abhaengigkeit eines Konfigurationsschritts, den ein beliebiges
# von mehreren Paketen erfuellt.
config-dependency-language-pack = ein Sprachpaket
config-reapack-reaper-accessibility-name = REAPER Accessibility ReaPack Repository
config-reapack-reaper-accessibility-description = Fügt das REAPER Accessibility Repository von Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml) zu ReaPack hinzu, sodass Pakete daraus direkt verfügbar sind.
config-reapack-reaper-accessible-fr-name = REAPER Accessible (FR) ReaPack Repository hinzufügen
config-reapack-reaper-accessible-fr-description = Fügt das französischsprachige REAPER Accessible Repository (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml) zu ReaPack hinzu. Öffne nach dem Hinzufügen das Menü Erweiterungen, ReaPack, Pakete durchsuchen, um die französischsprachigen Ressourcen von REAPER Accessible zu erhalten.
config-reapack-reaper-accessible-en-name = REAPER Accessible (EN) ReaPack Repository hinzufügen
config-reapack-reaper-accessible-en-description = Fügt das englischsprachige REAPER Accessible Repository (https://github.com/reaperaccessible/rap_en/raw/main/index.xml) zu ReaPack hinzu. Öffne nach dem Hinzufügen das Menü Erweiterungen, ReaPack, Pakete durchsuchen, um die englischsprachigen Ressourcen von REAPER Accessible zu erhalten.
config-set-reaper-language-name = REAPER-Sprache einstellen
config-set-reaper-language-description = Legt fest, welches der installierten Sprachpakete REAPER verwendet, indem es in die reaper.ini eingetragen wird. Andernfalls müssten Sie die Sprache selbst unter „Optionen“, „Einstellungen“, „Allgemein“ auswählen. Sie können mehrere Sprachpakete installieren und später in REAPER umschalten; hier wird nur ausgewählt, welches jetzt aktiv ist. Abwählen, wenn nur die Dateien installiert werden sollen.

wizard-reapack-ack-heading = ReaPack-Spendenhinweis
wizard-reapack-ack-body = ReaPack ist freie Software und steht unter der LGPL. Sein Autor Christian Fillion nimmt Spenden zur Unterstützung der Weiterentwicklung an. Spenden sind vollständig freiwillig und für die Nutzung von ReaPack oder RABBIT niemals erforderlich.
wizard-reapack-ack-link-label = ReaPack-Spendenseite öffnen
wizard-reapack-ack-confirm-label = Ich habe den obigen Hinweis gelesen und möchte mit der Installation oder Aktualisierung von ReaPack fortfahren
cli-reapack-ack-prompt-summary = ReaPack ist freie Software (LGPL). Spenden an seinen Autor Christian Fillion unter https://reapack.com/donate sind freiwillig und für die Nutzung von ReaPack oder RABBIT niemals erforderlich.
cli-reapack-ack-flag-required = ReaPack ist Teil dieses Plans, der Spendenhinweis wurde aber nicht bestätigt. Führen Sie den Befehl erneut mit --accept-reapack-donation-notice aus, um zu bestätigen, dass Sie https://reapack.com/donate gelesen haben und RABBIT ReaPack installieren oder aktualisieren soll.

wizard-version-check-heading = Prüfung auf neueste Versionen
wizard-version-check-status-pending = Versionsprüfung wird vorbereitet…
# $package is the localized package display name.
wizard-version-check-status-checking = { $package } wird geprüft…
wizard-version-check-status-progress = Versionen werden geprüft… ({ $done } von { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } Versionsprüfung(en) fehlgeschlagen. Wählen Sie „Zurück“, um ein anderes Ziel zu versuchen, oder schließen Sie RABBIT.
wizard-version-check-progress-label = Fortschritt
wizard-version-check-error-heading = Fehlgeschlagene Prüfungen
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Paketdetails
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = Neuerungen in { $package }:
wizard-packages-osara-keymap-heading = OSARA-Tastenzuordnung
wizard-packages-osara-keymap-replace-label = Aktuelle Tastenzuordnung durch OSARA-Tastenzuordnung ersetzen
packages-reaper-language-label = REAPER-Sprache nach der Installation
packages-spanish-variant-label = Spanische OSARA-Übersetzung
wizard-packages-osara-keymap-unavailable-note = Wählen Sie OSARA aus, um das Verhalten der Tastenzuordnung zu konfigurieren.
wizard-packages-osara-keymap-preserve-note = Ihre aktuelle Tastenzuordnung bleibt unverändert: RABBIT ändert die reaper-kb.ini nicht. Um die neuen OSARA-Tastenkürzel zu übernehmen und Ihre eigenen zu behalten, führen Sie anschließend die OSARA-Aktion „Merge OSARA key map“ aus (Menü „Erweiterungen“, OSARA).
wizard-packages-osara-keymap-replace-note = RABBIT sichert Ihre aktuelle reaper-kb.ini, falls vorhanden, und ersetzt sie durch die neueste OSARA-Tastenzuordnung. Selbst geänderte Tastenkürzel werden nicht übernommen; die Sicherung enthält sie.
wizard-packages-osara-keymap-unchanged-note = Ihre Tastenzuordnung ist die OSARA-Tastenzuordnung ohne eigene Änderungen. RABBIT sichert sie daher und ersetzt sie durch die neueste OSARA-Tastenzuordnung. Ihre installierten Skripte bleiben in der Aktionsliste.
wizard-packages-osara-keymap-default-keys-note = Sie verwenden die Standard-Tastenkürzel von REAPER statt der OSARA-Tastenzuordnung, daher lässt RABBIT sie unverändert. Um zur OSARA-Tastenzuordnung zu wechseln, aktivieren Sie das Kontrollkästchen oben.
wizard-package-details-handling-prefix = Behandlung
wizard-package-handling-automatic = RABBIT kann dieses Paket direkt installieren.
wizard-package-handling-unattended = RABBIT kann dieses Paket unbeaufsichtigt installieren und bei Bedarf das zugehörige Installationsprogramm starten.
wizard-package-handling-planned = RABBIT soll das Installationsprogramm bzw. die Einrichtungsroutine dieses Pakets selbst ausführen und die Installation unbeaufsichtigt abschließen, in dieser Version werden jedoch lediglich die erforderlichen Schritte gemeldet.
wizard-package-handling-manual = RABBIT lädt dieses Paket herunter und meldet die manuellen Schritte nach dem Durchlauf.
wizard-package-handling-unavailable = Dieses Paket ist für die ausgewählte Plattform oder Architektur nicht verfügbar.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package }: { $action }. Installiert: { $installed }. Verfügbar: { $available }

wizard-review-heading = Änderungen überprüfen
wizard-review-target-prefix = Ziel
wizard-review-package-heading = Ausgewählte Pakete
wizard-review-osara-keymap-heading = OSARA-Tastenzuordnung
wizard-review-osara-keymap-preserve = Aktuelle Tastenzuordnung beibehalten und die OSARA-Tastenzuordnung nicht anwenden.
wizard-review-osara-keymap-replace = Aktuelle Tastenzuordnung nach Sicherung von reaper-kb.ini ersetzen.
wizard-review-osara-keymap-replace-unchanged = Unveränderte OSARA-Tastenzuordnung sichern und durch die neueste von OSARA ersetzen.
wizard-review-osara-keymap-keep-default-keys = Standard-Tastenkürzel von REAPER beibehalten.
wizard-review-notes-heading = Hinweise
wizard-review-preflight-prefix = Installation derzeit nicht möglich

# $path is the selected REAPER resource path.
wizard-review-target = Ziel: { $path }
wizard-review-no-target = Kein Ziel ausgewählt.
wizard-review-no-package = Kein Paket ausgewählt.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Bereit zum Deinstallieren
wizard-review-uninstall-packages = Zu deinstallierende Pakete
wizard-review-uninstall-no-package = Zum Deinstallieren ist nichts ausgewählt.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package }: { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = Die Einstellungen werden ebenfalls entfernt und kommen in die Sicherung: { $settings }.
wizard-review-uninstall-close-programs-note = Schließen Sie vorher andere Musikprogramme.
wizard-review-uninstall-elevation-note-windows = Windows fragt Sie einmal um Erlaubnis, bevor etwas entfernt wird. Wenn Sie es nicht erlauben, wird nichts deinstalliert.
wizard-review-uninstall-vendor-window-note = Falls ein Fenster des Deinstallationsprogramms von { $package } erscheint, beantworten Sie es.
# Review's permission note on macOS, when a ticked package needs an
# administrator. $packages is a list of package names. osascript is a program
# name, never translated: macOS names it in the password window. The window
# may offer Touch ID, so don't say a password is the only way. Use the same
# words for 'administrator command' as in the Done and progress lines that
# name it.
wizard-review-uninstall-elevation-note-macos = Bevor etwas entfernt wird, fragt macOS einmal nach Name und Passwort eines Administrators. Es kann stattdessen Touch ID anbieten. Das Fenster nennt möglicherweise osascript, das Werkzeug, über das RABBIT fragt. Mit diesem Passwort führt RABBIT einen Administratorbefehl aus, der { $packages } deinstalliert. Wenn Sie das Fenster abbrechen, wird nichts deinstalliert.
# One line per ticked package that goes to the Trash; the second when your
# account may not move it, so an administrator's password moves it there.
# $package is the localized package name. Name the Trash and Put Back as
# Finder does in your language.
wizard-review-uninstall-trash-note = { $package } kommt in den Papierkorb. Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen.
wizard-review-uninstall-trash-elevated-note-macos = { $package } kommt mit dem Passwort des Administrators in Ihren Papierkorb. Es behält seinen bisherigen Eigentümer, deshalb bietet der Finder dafür kein „Zurücklegen“ an, und das Entleeren des Papierkorbs kann nach dem Passwort eines Administrators fragen.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. RABBIT leaves out what it ticked by itself, such as updates, but the
# list can still hold a setting RABBIT ticked along with one of the user's
# ticks, so the note doesn't say who ticked it. $packages is a comma-separated
# list of names: packages, and for the Install or update tab also settings,
# such as "Set REAPER's language".
wizard-review-inactive-tab-note-install = Was auf der Registerkarte „Installieren oder aktualisieren“ ausgewählt ist, gehört nicht zu diesem Durchlauf: { $packages }.
wizard-review-inactive-tab-note-uninstall = Was auf der Registerkarte „Deinstallieren“ ausgewählt ist, gehört nicht zu diesem Durchlauf: { $packages }.
# The same note for the Backups tab. $backups is one or both of the two
# messages below, joined as wizard-backups-list-and joins names.
wizard-review-inactive-tab-note-backups = Was auf der Registerkarte „Sicherungen“ ausgewählt ist, gehört nicht zu diesem Durchlauf: { $backups }.
# How many backups are ticked on the Backups tab to restore, and to delete,
# for the note above. $count is a number, never 0.
wizard-review-inactive-tab-backups-restore = { $count ->
    [one] eine Sicherung zum Wiederherstellen
   *[other] { $count } Sicherungen zum Wiederherstellen
}
wizard-review-inactive-tab-backups-delete = { $count ->
    [one] eine Sicherung zum Löschen
   *[other] { $count } Sicherungen zum Löschen
}
wizard-review-uninstall-reinstall-hint = Sie können diese Pakete später auf der Registerkarte „Installieren oder aktualisieren“ wieder installieren.
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Deinstallieren derzeit nicht möglich: { $reason }
wizard-review-uninstall-not-writable = RABBIT kann nicht in den Ordner dieses REAPER schreiben.

# Review before a restore from the Backups tab. The heading names what the
# run does, so it is the first thing read.
wizard-review-restore-heading = Bereit zum Wiederherstellen
# One line per ticked backup. $backup is what its row calls it, $package the
# localized package name, $date a date and time (backup-date-time), and
# $settings yes or no: whether the backup holds the package's settings too.
wizard-review-restore-uninstalled = { $settings ->
    [yes] { $backup } kommt im Zustand vom { $date } zurück, mit Ihren Einstellungen für { $package }.
   *[no] { $backup } kommt im Zustand vom { $date } zurück.
}
wizard-review-restore-uninstalled-undated = { $settings ->
    [yes] { $backup } kommt im Zustand vor der Deinstallation zurück, mit Ihren Einstellungen für { $package }.
   *[no] { $backup } kommt im Zustand vor der Deinstallation zurück.
}
# An earlier version. $version is the version it goes back to, and $current
# the one it has now, both version numbers, or for a package that comes in
# variants as wizard-backups-version-variant says.
wizard-review-restore-earlier = { $package } geht von { $current } auf { $version } zurück. RABBIT bewahrt { $current } als Sicherung auf.
wizard-review-restore-earlier-to = { $package } geht auf { $version } zurück. RABBIT bewahrt die Version, die Sie jetzt haben, als Sicherung auf.
wizard-review-restore-earlier-copy = { $package } geht auf die Kopie zurück, die eine Aktualisierung ersetzt hat. RABBIT bewahrt die Version, die Sie jetzt haben, als Sicherung auf.
# An earlier version of a package that isn't installed now, as after you
# uninstalled it: it comes back, and no version goes into a backup. The
# -copy form is for a copy RABBIT didn't install. $version as above.
wizard-review-restore-earlier-again = { $package } kommt in der Version { $version } zurück.
wizard-review-restore-earlier-copy-again = { $package } kommt als die Kopie zurück, die eine Aktualisierung ersetzt hat.
# A key map. $origin is own for your own key map, default-keys for REAPER's
# default keys, and other for any other; $date as above.
wizard-review-restore-keymap = { $origin ->
    [own] Ihre Tastenzuordnung vom { $date } ersetzt die aktuelle, die RABBIT als Sicherung aufbewahrt.
    [default-keys] Die Standardtasten von REAPER vom { $date } ersetzen die aktuelle Tastenzuordnung, die RABBIT als Sicherung aufbewahrt.
   *[other] Die Tastenzuordnung vom { $date } ersetzt die aktuelle, die RABBIT als Sicherung aufbewahrt.
}
wizard-review-restore-keymap-undated = { $origin ->
    [own] Ihre Tastenzuordnung ersetzt die aktuelle, die RABBIT als Sicherung aufbewahrt.
    [default-keys] Die Standardtasten von REAPER ersetzen die aktuelle Tastenzuordnung, die RABBIT als Sicherung aufbewahrt.
   *[other] Diese Tastenzuordnung ersetzt die aktuelle, die RABBIT als Sicherung aufbewahrt.
}
# Notes on what the restore changes besides the files. $file is a language
# pack's file name, such as de_DE.ReaperLangPack, never translated; $package
# is the localized package name.
wizard-review-restore-language-back = REAPER bekommt wieder die Sprache { $file }.
wizard-review-restore-language-stays = Die Sprache von REAPER bleibt, wie sie ist, weil seitdem eine andere gewählt wurde.
# Going back to an earlier version of a language pack while REAPER uses
# another pack, or none: the restore leaves REAPER's language alone.
wizard-review-restore-language-unchanged = Die Sprache von REAPER bleibt, wie sie ist.
# Going back to an earlier version of a language pack when RABBIT
# couldn't read REAPER's settings for Review: the restore reads them
# again, and Done says what came of REAPER's language. If it still can't
# read them, REAPER's language keeps naming the newer version's file,
# which moves into a backup all the same. $file is the earlier version's
# language pack file, as above.
wizard-review-restore-language-unreadable = RABBIT kann die Einstellungen von REAPER gerade nicht lesen und kann die Sprache von REAPER daher vielleicht nicht auf { $file } stellen.
wizard-review-restore-not-counted = RABBIT zählt { $package } nicht mehr als von ihm installiert.
wizard-review-restore-needs-reinstall = Um { $package } später mit RABBIT zu deinstallieren, installieren Sie es zuerst erneut.
# When a REAPER that has to be closed is running; the second when a ticked
# package serves every REAPER on the computer.
wizard-review-restore-close-reaper = Schließen Sie zuerst REAPER.
wizard-review-restore-close-every-reaper = Schließen Sie zuerst jedes REAPER auf diesem Computer.
# The backups ticked for the other choice below the list stay ticked, but
# aren't part of this run. $count is a number.
wizard-review-restore-other-choice-note = { $count ->
    [one] Eine zum Löschen ausgewählte Sicherung gehört nicht zu diesem Durchlauf.
   *[other] { $count } zum Löschen ausgewählte Sicherungen gehören nicht zu diesem Durchlauf.
}
# Why the Restore button is disabled. $reason is a sentence, such as
# wizard-review-restore-nothing-ticked. The -backup form names a backup that
# can't come back: $backup as in wizard-backups-row-reference, or for a key
# map as wizard-done-restore-keymap-in-sentence says it, and $reason one of
# the wizard-restore-refusal-* sentences or wizard-review-restore-plan-failed.
wizard-review-restore-blocked = Wiederherstellen derzeit nicht möglich: { $reason }
wizard-review-restore-blocked-backup = Derzeit lässt sich { $backup } nicht wiederherstellen. { $reason }
wizard-review-restore-nothing-ticked = Zum Wiederherstellen ist nichts ausgewählt.
wizard-review-restore-not-writable = RABBIT kann nicht in den Ordner dieses REAPER schreiben.
# $first and $second name two ticked backups, as in
# wizard-backups-row-reference, or for a key map as
# wizard-done-restore-keymap-in-sentence says it, so the sentence may start
# in lower case; $package is the localized package name.
wizard-review-restore-same-package = Zwei Sicherungen von { $package } sind ausgewählt: { $first } und { $second }. Wählen Sie nur eine der beiden aus.
wizard-review-restore-same-file = Zwei ausgewählte Sicherungen legen dieselbe Datei zurück: { $first } und { $second }. Wählen Sie nur eine der beiden aus.
# When RABBIT couldn't check the restore before it starts: one backup, or
# all of them. $error is the error message, in English.
wizard-review-restore-plan-failed = RABBIT konnte sie nicht prüfen ({ $error }).
wizard-review-restore-check-failed = RABBIT konnte die Wiederherstellung nicht prüfen ({ $error }).
# When RABBIT hasn't checked the backups ticked now, so the restore can't
# start yet.
wizard-review-restore-not-checked = RABBIT hat noch nicht geprüft, was ausgewählt ist.
# Review before a delete from the Backups tab.
wizard-review-delete-heading = Bereit zum Löschen von Sicherungen
# $count is a number, the ticked backups, and $size a size, such as "520 MB".
wizard-review-delete-intro = { $count ->
    [one] Diese Sicherung wird endgültig gelöscht und gibt { $size } frei:
   *[other] Diese Sicherungen werden endgültig gelöscht und geben { $size } frei:
}
# As above, when $size is about what the delete frees: a package or key map
# goes out of the backup set it is kept in, and RABBIT knows its size only
# from that set's record of what it holds.
wizard-review-delete-intro-estimate = { $count ->
    [one] Diese Sicherung wird endgültig gelöscht und gibt etwa { $size } frei:
   *[other] Diese Sicherungen werden endgültig gelöscht und geben etwa { $size } frei:
}
# One line per ticked backup, saying what is lost with it. $backup is what
# its row calls it, and $date a date and time (backup-date-time).
wizard-review-delete-uninstalled = { $backup }, deinstalliert am { $date }: die einzige Kopie.
wizard-review-delete-keymap = { $backup } vom { $date }: die einzige Kopie.
wizard-review-delete-only-copy = { $backup }: die einzige Kopie.
wizard-review-delete-earlier = { $backup }: Danach können Sie nicht mehr zu dieser Version zurückkehren.
wizard-review-delete-already-back = { $backup }: Die Dateien liegen auch auf der Festplatte, es geht also nichts verloren.
wizard-review-delete-records = { $backup }: An dem, was installiert ist, ändert sich nichts.
wizard-review-delete-deleting = { $backup }: Damit wird eine früher begonnene Löschung abgeschlossen.
wizard-review-delete-other = { $backup }: Diese Sicherung ist danach endgültig weg.
# $count is a number.
wizard-review-delete-other-choice-note = { $count ->
    [one] Eine zum Wiederherstellen ausgewählte Sicherung gehört nicht zu diesem Durchlauf.
   *[other] { $count } zum Wiederherstellen ausgewählte Sicherungen gehören nicht zu diesem Durchlauf.
}
# Why the Delete button is disabled. $reason is a sentence.
wizard-review-delete-blocked = Löschen derzeit nicht möglich: { $reason }
wizard-review-delete-nothing-ticked = Zum Löschen ist nichts ausgewählt.
# Why the Delete button is disabled, as $reason above: a ticked backup that
# RABBIT can only delete together with the whole backup set it is in, such
# as the set's own files, while other rows of that set aren't ticked. The
# -undeletable form is for rows of that set RABBIT can't delete, such as
# another account's, so the ticked backup can only be unticked. $backup is
# the ticked row, a set's own files or an unfinished deletion, named as
# wizard-backups-row-reference names it. It comes after a colon, so its
# capital stays. $backups names rows as wizard-backups-row-reference does,
# or a key map as wizard-done-restore-keymap-in-sentence says it, so it may
# start in lower case; $backups joins several as wizard-backups-list-and
# joins them, after a colon, and $count is how many it names, a number.
wizard-review-delete-only-with = RABBIT kann diese Sicherung nicht allein löschen: { $backup }. Sie lässt sich nur zusammen mit { $count ->
    [one] einer weiteren Sicherung
   *[other] { $count } weiteren Sicherungen
} löschen: { $backups }. Um sie zu löschen, wählen Sie { $count ->
    [one] die andere
   *[other] die anderen
} ebenfalls aus.
wizard-review-delete-only-with-undeletable = RABBIT kann diese Sicherung nicht allein löschen: { $backup }. Sie lässt sich nur zusammen mit { $count ->
    [one] einer weiteren Sicherung
   *[other] { $count } weiteren Sicherungen
} löschen, die RABBIT nicht löschen kann: { $backups }. Wählen Sie diese Sicherung daher ab.

wizard-progress-heading = Installationsfortschritt
wizard-progress-status-idle = Bereit zur Installation.
wizard-progress-status-running = Ausgewählte Pakete werden installiert. Dies kann mehrere Minuten dauern.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Wird angehalten. RABBIT beendet den begonnenen Schritt und schließt sich dann.
wizard-progress-details-label = Fortschrittsdetails
wizard-progress-details-idle = Es läuft keine Installation.
wizard-progress-details-starting = Einrichtungsvorgang wird gestartet.
wizard-progress-details-cache-prefix = Zwischenspeicher
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops once the step it is on has finished: one package,
# or on macOS the administrator command, which removes several at once.
wizard-progress-uninstall-heading = Deinstallation
wizard-progress-status-uninstall-running = Die ausgewählten Pakete werden deinstalliert.
wizard-progress-details-uninstall-starting = Deinstallation wird gestartet.
wizard-progress-status-uninstall-cancelling = Wird angehalten. RABBIT beendet den begonnenen Schritt und schließt sich dann.
# The progress page while backups are deleted: its heading, the first status
# line and the first line of the log. Once the user asked to stop, the
# status line says wizard-progress-status-cancelling.
wizard-progress-delete-heading = Sicherungen werden gelöscht
wizard-progress-status-delete-running = Die ausgewählten Sicherungen werden gelöscht.
wizard-progress-details-delete-starting = Das Löschen der Sicherungen wird gestartet.
# The status and log line when RABBIT starts deleting from a backup set,
# which is spoken, then the log line when it is done with it. $backups is
# what the rows call the ticked backups in that set, one name or several
# joined as wizard-backups-list-and joins them, each with when it was made,
# as wizard-backups-row-reference says it, when another ticked backup has
# the same name. $count is how many there are, a number, and $size a size,
# such as "4 MB".
wizard-progress-status-deleting = { $count ->
    [one] Die Sicherung wird gelöscht: { $backups }…
   *[other] Die Sicherungen werden gelöscht: { $backups }…
}
wizard-progress-log-delete-deleted = { $backups }: gelöscht, { $size } freigegeben.
wizard-progress-log-delete-finished = { $backups }: eine früher begonnene Löschung abgeschlossen, { $size } freigegeben.
wizard-progress-log-delete-already-gone = { $backups }: schon nicht mehr vorhanden.
wizard-progress-log-delete-partly = { $backups }: nur teilweise gelöscht, { $size } freigegeben.
wizard-progress-log-delete-failed = { $backups }: nicht gelöscht.
# The progress page while backups are restored: its heading, the first
# status line and the first line of the log. Once the user asked to stop,
# the status line says wizard-progress-status-cancelling.
wizard-progress-restore-heading = Sicherungen werden wiederhergestellt
wizard-progress-status-restore-running = Die ausgewählten Sicherungen werden wiederhergestellt.
wizard-progress-details-restore-starting = Das Wiederherstellen der Sicherungen wird gestartet.
# The status and log line when RABBIT starts restoring a backup, which is
# spoken. $backup is what its row calls it, such as "SWS Extension
# 2.14.0.7" or "ReaPack 1.2.4", with when it was made, as
# wizard-backups-row-reference says it, when another ticked backup has the
# same name. $kind is uninstalled for software an uninstall kept, earlier
# for an earlier version whose row names its version, and earlier-copy for
# one whose row names only the package.
wizard-progress-status-restoring = { $kind ->
    [earlier] Rückkehr zu { $backup }…
    [earlier-copy] Rückkehr zur Kopie von { $backup }, die eine Aktualisierung ersetzt hat…
   *[uninstalled] { $backup } wird wiederhergestellt…
}
# The same for a key map. $origin is own for your own key map, default-keys
# for REAPER's default keys, and other for any other; $date is a date and
# time (backup-date-time). The -undated form is for a key map whose date
# RABBIT can't tell.
wizard-progress-status-restoring-keymap = { $origin ->
    [own] Ihre Tastenzuordnung vom { $date } wird wiederhergestellt…
    [default-keys] Die Standardtasten von REAPER vom { $date } werden wiederhergestellt…
   *[other] Die Tastenzuordnung vom { $date } wird wiederhergestellt…
}
wizard-progress-status-restoring-keymap-undated = { $origin ->
    [own] Ihre Tastenzuordnung wird wiederhergestellt…
    [default-keys] Die Standardtasten von REAPER werden wiederhergestellt…
   *[other] Die Tastenzuordnung wird wiederhergestellt…
}
# The log line when RABBIT is done with a backup. $backup as above.
wizard-progress-log-restore-restored = { $backup }: wiederhergestellt.
wizard-progress-log-restore-not-restored = { $backup }: nicht wiederhergestellt, alles bleibt, wie es war.
wizard-progress-log-restore-files-left = { $backup }: nicht wiederhergestellt, und einige schon geänderte Dateien ließen sich nicht in den vorherigen Zustand zurückversetzen.
wizard-progress-log-restore-partly = { $backup }: nur teilweise wiederhergestellt.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = { $package } wird heruntergeladen…
# $downloaded and $total are human-readable byte counts (e.g. "12,4 MB", "30,0 MB").
wizard-progress-status-downloading-with-bytes = { $package } wird heruntergeladen… { $downloaded } / { $total }
wizard-progress-status-downloading-many = { $count } Pakete werden heruntergeladen… { $downloaded } / { $total }
wizard-progress-status-installing = { $package } wird installiert…
wizard-progress-status-removing = { $package } wird deinstalliert…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Konfigurationsschritt wird angewendet: { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = { $package } wird heruntergeladen…
wizard-progress-log-download-completed = { $package } heruntergeladen.
wizard-progress-log-install-started = { $package } wird installiert…
wizard-progress-log-install-completed = { $package } installiert.
wizard-progress-log-remove-started = { $package } wird deinstalliert…
wizard-progress-log-remove-completed = { $package } deinstalliert.
wizard-progress-log-remove-failed = { $package } konnte nicht deinstalliert werden.
wizard-progress-status-vendor-running = Das Deinstallationsprogramm von { $package } läuft…
wizard-progress-status-vendor-still-waiting = RABBIT wartet noch auf das Deinstallationsprogramm von { $package }. Vielleicht zeigt es eine Frage in einem anderen Fenster; drücken Sie Alt+Tab, um es zu finden.
wizard-progress-status-vendor-waiting-for-files = Das Deinstallationsprogramm von { $package } ist fertig, und RABBIT wartet darauf, dass seine Dateien verschwinden. Wenn ein anderes Musikprogramm { $package } geöffnet hat, schließen Sie es.
# On macOS, said right before the administrator password window appears, and
# again if it has been open a while without an answer. RABBIT puts each on the
# status line and in the log too, in case VoiceOver cuts it off. $packages is
# a list of package names. Use the same words for 'administrator command' as
# in the Done and progress lines that name it.
wizard-progress-status-admin-prompt-macos = macOS fragt jetzt einmal nach Name und Passwort eines Administrators. Damit führt RABBIT einen Administratorbefehl aus, der { $packages } deinstalliert. Wenn Sie abbrechen, wird nichts deinstalliert.
wizard-progress-status-admin-still-waiting-macos = Es wird noch auf das Passwortfenster des Administrators gewartet. Wenn Sie es nicht hören, prüfen Sie, ob es hinter RABBIT liegt.
# While a package goes to the Trash, and in the log once the administrator
# command has worked on a package. $package is the localized package name.
# Name the Trash as Finder does.
wizard-progress-status-trashing = { $package } wird in den Papierkorb gelegt…
wizard-progress-log-admin-done = Der Administratorbefehl ist mit { $package } fertig.
wizard-progress-log-configuration-started = { $step } wird angewendet…
wizard-progress-log-configuration-completed = { $step } angewendet.

wizard-done-heading = Fertig
wizard-done-status-idle = Aus diesem Fenster wurde noch keine Installation ausgeführt.
wizard-done-status-success = Installation abgeschlossen. Bitte prüfen Sie die Details unten.
wizard-done-status-error = Installation fehlgeschlagen. Bitte prüfen Sie den Fehler unten.
wizard-reaper-running-title = REAPER ist noch geöffnet
wizard-reaper-running-body = Schließen Sie REAPER und drücken Sie dann erneut auf „Installieren“. RABBIT kann REAPER und seine Erweiterungen nicht aktualisieren, solange REAPER läuft.
wizard-error-reaper-running = RABBIT hat angehalten, weil REAPER geöffnet war. Schließen Sie REAPER und starten Sie RABBIT dann erneut.
# Why a run stopped when another RABBIT is working on the same REAPER:
# installing, uninstalling, or restoring or deleting backups.
wizard-error-lock-held = Ein anderes RABBIT arbeitet gerade an diesem REAPER. Warten Sie, bis es fertig ist, und starten Sie RABBIT dann erneut.
# Why an install or an uninstall stopped before it changed anything:
# RABBIT's own folder in the REAPER, or the backups folder in it, is a
# symbolic link or junction, so a backup kept there couldn't be restored.
wizard-error-backup-folder-link = RABBIT hat angehalten, weil sein eigener Ordner in diesem REAPER oder der Sicherungsordner darin eine Verknüpfung ist. RABBIT stellt nie eine Sicherung aus einer Verknüpfung wieder her und ändert daher nichts, solange das so ist. Machen Sie daraus wieder einen echten Ordner mit den Sicherungen darin, und starten Sie RABBIT dann erneut.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Schließen Sie REAPER und drücken Sie dann erneut auf „Deinstallieren“. RABBIT kann nichts aus REAPER deinstallieren, solange REAPER läuft.
# The same, for packages every REAPER on the computer can load, such as
# Surge XT. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und drücken Sie dann erneut auf „Deinstallieren“. Jedes REAPER auf diesem Computer kann { $packages } verwenden.
wizard-reaper-running-uninstall-reaper-body = Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und drücken Sie dann erneut auf „Deinstallieren“. Das Deinstallationsprogramm von REAPER läuft nicht, solange irgendein REAPER geöffnet ist.
# The same on macOS, when RABBIT's administrator command is to move REAPER's
# app: it can't tell one REAPER from another.
wizard-reaper-running-uninstall-reaper-body-macos = Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und drücken Sie dann erneut auf „Deinstallieren“. Der Administratorbefehl, der REAPER verschiebt, kann ein REAPER nicht vom anderen unterscheiden. Deshalb müssen alle geschlossen sein.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Schließen Sie zuerst diese Programme
wizard-programs-running-uninstall-body = Schließen Sie { $programs } und drücken Sie dann erneut auf „Deinstallieren“.
# Shown instead of starting a restore while REAPER is open, under
# wizard-reaper-running-title. Name the button as wizard-button-restore
# does.
wizard-reaper-running-restore-body = Schließen Sie REAPER und drücken Sie dann erneut auf „Wiederherstellen“. RABBIT kann keine Sicherungen in REAPER wiederherstellen, solange REAPER läuft.
# The same, when a ticked backup holds a package every REAPER on the
# computer can load. $packages is a list of localized package names, joined
# as wizard-backups-list-and joins names.
wizard-reaper-running-restore-every-body = Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und drücken Sie dann erneut auf „Wiederherstellen“. Jedes REAPER auf diesem Computer kann { $packages } verwenden.
# Shown instead of starting a restore while a program that has to be closed
# first is running. $programs is a list of program names, joined as
# wizard-backups-list-and joins names. A name, such as "Surge XT.exe", is
# never translated.
wizard-programs-running-restore-title = Schließen Sie zuerst diese Programme
wizard-programs-running-restore-body = Schließen Sie { $programs } und drücken Sie dann erneut auf „Wiederherstellen“.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT hat angehalten, weil ein REAPER geöffnet war. Schließen Sie jedes REAPER auf diesem Computer, auch portable Versionen, und starten Sie RABBIT dann erneut.
wizard-error-uninstall-not-enough-space = RABBIT hat angehalten, weil nicht genug freier Speicherplatz für die Sicherung vorhanden ist. Schaffen Sie Platz auf dem Laufwerk mit dem Ressourcenordner von REAPER und starten Sie RABBIT dann erneut.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } wurde nicht deinstalliert, weil ein anderes Programm { $path } verwendet. Schließen Sie dieses Programm und starten Sie RABBIT dann erneut.
# The same, for a file a package's installer added outside REAPER's folder.
# Only the JAWS scripts' installer adds such files, and JAWS itself may have
# one open; the program to close would be the user's screen reader, so this
# says to wait and try again instead. $package is the localized package
# name, and $path the file's full path as Windows holds it. Keep JAWS as it
# is.
wizard-error-uninstall-vendor-file-in-use = { $package } wurde nicht deinstalliert, weil JAWS oder ein anderes Programm { $path } verwendet. Warten Sie einen Moment und starten Sie RABBIT dann erneut. Hilft das nicht, starten Sie zuerst JAWS neu.
# Why an uninstall didn't start on macOS: RABBIT itself runs as root, as under
# sudo. sudo and root are never translated.
wizard-error-uninstall-as-root = RABBIT läuft als root, wie unter sudo, und deinstalliert deshalb nichts. Beenden Sie RABBIT und starten Sie es mit Ihrem eigenen Benutzerkonto. Wenn es das Passwort eines Administrators braucht, fragt es selbst danach.
wizard-done-status-completed-with-errors = Installation mit Fehlern abgeschlossen. Bitte prüfen Sie die Details unten.
wizard-done-status-cancelled = Sie haben die Installation angehalten. Die Liste unten zeigt, was RABBIT vorher fertiggestellt hat.
wizard-done-status-no-packages = Es wurde kein Paket zur Installation oder Aktualisierung ausgewählt.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = RABBIT hat { $packages } deinstalliert.
wizard-done-uninstall-status-completed-with-errors = RABBIT hat { $packages } deinstalliert, aber nicht alles lief wie geplant.
wizard-done-uninstall-status-error = Die Deinstallation ist fehlgeschlagen.
wizard-done-uninstall-status-cancelled = Sie haben die Deinstallation angehalten. Pakete, die noch nicht an der Reihe waren, sind weiterhin installiert.
wizard-done-uninstall-status-no-packages = Es wurde nichts deinstalliert.
wizard-done-uninstall-status-pending-restart = RABBIT hat { $packages } deinstalliert. Starten Sie den Computer neu, um den Vorgang abzuschließen.
wizard-done-uninstall-status-declined = Es wurde nichts deinstalliert, weil die Windows-Abfrage nach Ihrer Erlaubnis abgelehnt wurde.
# The same on macOS, where the administrator password window was cancelled.
wizard-done-uninstall-status-declined-macos = Es wurde nichts deinstalliert, weil das Passwortfenster des Administrators abgebrochen wurde.
# The Done page's first line when the administrator command removed only part
# of a package and nothing else was uninstalled. $packages is a list of
# package names.
wizard-done-uninstall-status-partly-removed = RABBIT hat { $packages } nur zum Teil entfernt.
wizard-done-uninstall-removed-before-stop = Bevor Sie angehalten haben, hat RABBIT { $packages } deinstalliert.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Um ein Paket zurückzubekommen, starten Sie RABBIT erneut und wählen Sie es auf der Registerkarte „Installieren oder aktualisieren“ aus.
# Used instead of wizard-done-uninstall-reinstall-hint when RABBIT kept a
# backup of a removed package: the same sentence, then where to restore
# it. $packages is a list of localized package names, joined as
# wizard-backups-list-and joins names, only those whose files went into
# the backup, which may be fewer than were uninstalled. Name the tabs as
# wizard-packages-tab-install and wizard-packages-tab-backups do.
wizard-done-uninstall-reinstall-or-restore-hint = Um ein Paket zurückzubekommen, starten Sie RABBIT erneut und wählen Sie es auf der Registerkarte „Installieren oder aktualisieren“ aus. Sie können { $packages } auch auf der Registerkarte „Sicherungen“ wiederherstellen.
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } war nicht mehr installiert, es gab also nichts zu entfernen.
# The backup still holds the files, so the Backups tab lists it and can put
# them back. Done has no way back to the tab, so the sentence says to run
# RABBIT again, as wizard-done-delete-partly does. Name the tab as
# wizard-packages-tab-backups does.
wizard-done-uninstall-left-in-backup = Einige Dateien von { $package } konnten nicht zurückgelegt werden und liegen noch in der Sicherung. „Details anzeigen“ listet sie auf. Um sie zurückzulegen, starten Sie RABBIT erneut und stellen Sie { $package } auf der Registerkarte „Sicherungen“ wieder her.
wizard-done-uninstall-leftovers = RABBIT findet noch Teile von { $package } und lässt sie unangetastet. „Details anzeigen“ listet sie auf.
wizard-done-uninstall-language-reset-failed = RABBIT konnte REAPER nach dem Entfernen von { $package } nicht auf Englisch zurückstellen. Wählen Sie in den Einstellungen von REAPER eine Sprache.
wizard-done-uninstall-registry-failed = RABBIT hat { $package } deinstalliert, aber die Liste der installierten Programme von Windows zeigt es möglicherweise noch an. „Details anzeigen“ nennt den Eintrag.
wizard-done-uninstall-pending-restart = Starten Sie den Computer neu, damit { $package } vollständig entfernt wird.
wizard-done-uninstall-vendor-cancelled = Das Deinstallationsprogramm von { $package } wurde abgebrochen, bevor es fertig war.
wizard-done-uninstall-vendor-failed = Das Deinstallationsprogramm von { $package } wurde mit dem Fehlercode { $code } beendet. „Details anzeigen“ listet auf, was noch vorhanden ist.
wizard-done-uninstall-vendor-failed-unsure = Das Deinstallationsprogramm von { $package } wurde mit dem Fehlercode { $code } beendet. Deshalb ist { $package } möglicherweise nicht vollständig entfernt.
wizard-done-uninstall-vendor-still-there = Das Deinstallationsprogramm von { $package } ist fertig, aber { $package } ist weiterhin installiert. Sie können es in den Windows-Einstellungen unter Apps, Installierte Apps deinstallieren.
wizard-done-uninstall-vendor-still-open = Das Deinstallationsprogramm von { $package } ist noch geöffnet. Deshalb hat RABBIT angehalten und die folgenden Pakete nicht angerührt. Drücken Sie Alt+Tab, um sein Fenster zu finden, und beantworten Sie es.
wizard-done-uninstall-vendor-run-failed = RABBIT konnte das Deinstallationsprogramm von { $package } nicht zu Ende ausführen ({ $error }) und hat dort angehalten.
wizard-done-uninstall-elevation-failed = RABBIT hat keine Administratorrechte erhalten ({ $error }). Deshalb wurde nichts deinstalliert.
wizard-done-uninstall-resource-damaged = Wichtig: Nachdem das Deinstallationsprogramm von { $package } gelaufen war, fehlten diese Dateien im Ressourcenordner von REAPER: { $paths }. RABBIT hat angehalten und seine Aufzeichnungen neu geschrieben. Die Einstellungen von REAPER kann RABBIT nicht zurückholen; wenn Sie eine Kopie von reaper.ini haben, legen Sie sie zurück, bevor Sie REAPER starten.
wizard-done-uninstall-receipt-not-saved = RABBIT hat { $package } deinstalliert, konnte aber seine Aufzeichnungen nicht aktualisieren. Beim nächsten Start stellt RABBIT trotzdem fest, dass { $package } nicht mehr da ist.
wizard-done-uninstall-vendor-lost = RABBIT hat das Deinstallationsprogramm von { $package } aus den Augen verloren, nachdem es gestartet war. Vielleicht läuft es noch; deshalb kann RABBIT nicht sagen, was es entfernt hat, und hat dort angehalten.
wizard-done-uninstall-vendor-lost-reaper = Prüfen Sie, sobald das Deinstallationsprogramm fertig ist, ob reaper.ini noch im Ressourcenordner von REAPER liegt: Darin stehen Ihre Einstellungen für REAPER.
wizard-done-uninstall-vendor-files-left = Das Deinstallationsprogramm von { $package } ist fertig, aber einige seiner Dateien sind noch da. „Details anzeigen“ listet sie auf.
wizard-done-uninstall-vendor-not-confirmed = Das Deinstallationsprogramm von { $package } ist fertig, aber RABBIT kann nicht bestätigen, dass { $package } entfernt ist.
wizard-done-uninstall-vendor-not-listed = Windows führt { $package } nicht mehr in der Liste der installierten Programme, deshalb finden Sie es auch nicht in den Windows-Einstellungen.
wizard-done-uninstall-working-folder-left = RABBIT konnte { $path } nicht entfernen, den Ordner, aus dem es das Deinstallationsprogramm von { $package } gestartet hat. Nur ein Administrator kann ihn löschen.
wizard-done-uninstall-restart-list-unreadable = RABBIT konnte nicht lesen, welche Dateien Windows beim nächsten Neustart entfernt. Deshalb kann es nicht sagen, ob die verbliebenen Dateien von { $package } dann verschwinden.
wizard-done-uninstall-installed-list-unreadable = RABBIT konnte die Liste der installierten Programme von Windows nach dem Deinstallationsprogramm von { $package } nicht lesen. Deshalb konnte es nicht prüfen, ob Windows { $package } noch auflistet.
# After the elevated helper was to remove the uninstaller folder and
# registry entry a package's installer left. $package is the localized
# package name, and $error Windows' own error message or RABBIT's in
# English. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does, and "Show
# details" as wizard-done-show-details does.
wizard-done-uninstall-cleanup-refused = RABBIT hat den Ordner mit dem Deinstallationsprogramm von { $package } unter Programme (Program Files) und seinen Registrierungseintrag erneut geprüft und dem Gefundenen nicht vertraut. Es hat deshalb zurückgelegt, was es verschoben hatte.
wizard-done-uninstall-cleanup-failed = RABBIT konnte den Ordner mit dem Deinstallationsprogramm und den Registrierungseintrag von { $package } nicht vollständig entfernen ({ $error }). Es hat deshalb zurückgelegt, was es verschoben hatte. „Details anzeigen“ nennt, was schon entfernt ist.
wizard-done-uninstall-cleanup-lost = RABBIT hat beim Entfernen des Ordners mit dem Deinstallationsprogramm und des Registrierungseintrags von { $package } die Verbindung zu seinem Hilfsprogramm verloren ({ $error }). Es hat deshalb zurückgelegt, was es verschoben hatte. Ein Teil dieses Ordners oder der Registrierungseintrag ist möglicherweise schon entfernt.
# Instead of wizard-done-uninstall-cleanup-failed, when RABBIT removed
# nothing of the folder or the registry entry before it failed, so "Show
# details" lists nothing as gone. $package and $error as above.
wizard-done-uninstall-cleanup-failed-nothing-gone = RABBIT konnte den Ordner mit dem Deinstallationsprogramm und den Registrierungseintrag von { $package } nicht entfernen ({ $error }). Es hat deshalb zurückgelegt, was es verschoben hatte. Von diesem Ordner und dem Registrierungseintrag wurde nichts entfernt.
# After a macOS removal: to the Trash, or by the administrator command.
# $package is the localized package name, $path a full macOS path, keep it as
# it is, $error macOS's own error message, and $programs a list of program
# names, never translated. Name the Trash, Put Back and Finder as Finder does,
# "Show details" as wizard-done-show-details does, and System Settings >
# Privacy & Security > App Management as macOS names it in your language.
wizard-done-uninstall-moved-to-trash = RABBIT hat { $package } in den Papierkorb gelegt. Dort liegt es als { $path }. Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen.
wizard-done-uninstall-moved-to-trash-unknown = RABBIT hat { $package } in den Papierkorb gelegt. Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen.
wizard-done-uninstall-trash-owner = Es behält seinen bisherigen Eigentümer, deshalb bietet der Finder dafür kein „Zurücklegen“ an. Das Entleeren des Papierkorbs kann nach dem Passwort eines Administrators fragen, ebenso das Herausziehen.
wizard-done-uninstall-trash-failed = RABBIT konnte { $package } nicht in den Papierkorb legen. Deshalb hat sich daran nichts geändert ({ $error }).
wizard-done-uninstall-trash-unclear = RABBIT hat macOS gebeten, { $path } in den Papierkorb zu legen, kann aber nicht bestätigen, dass es jetzt dort liegt. Suchen Sie { $package } im Finder im Papierkorb und in seinem Ordner. RABBIT hat dort angehalten.
wizard-done-uninstall-admin-step-failed = Der Administratorbefehl hat bei { $path } angehalten. Von { $package } wurde deshalb nichts entfernt. RABBIT hat dort angehalten.
wizard-done-uninstall-admin-delete-failed = Der Administratorbefehl hat bei { $path } angehalten. Davon ist jetzt womöglich nur noch ein Teil vorhanden. Sonst wurde von { $package } nichts entfernt. RABBIT hat dort angehalten.
wizard-done-uninstall-admin-stopped = Der Administratorbefehl hat geendet, ohne zu sagen, wo er angehalten hat, und von { $package } wurde nichts entfernt. RABBIT hat dort angehalten.
wizard-done-uninstall-partly-removed = Der Administratorbefehl hat bei { $path } angehalten. { $package } ist deshalb nur zum Teil entfernt. „Details anzeigen“ listet auf, was entfernt und was noch vorhanden ist. RABBIT hat dort angehalten.
wizard-done-uninstall-partly-removed-unclear = Der Administratorbefehl hat geendet, ohne zu sagen, wo er angehalten hat, und { $package } ist nur zum Teil entfernt. „Details anzeigen“ listet auf, was entfernt und was noch vorhanden ist. RABBIT hat dort angehalten.
wizard-done-uninstall-admin-left = Der Administratorbefehl ist gelaufen, aber { $path } ist noch vorhanden, oder RABBIT kann nicht sehen, dass es entfernt ist. { $package } ist deshalb nicht vollständig entfernt. „Details anzeigen“ listet auf, was entfernt und was noch vorhanden ist. RABBIT hat dort angehalten.
wizard-done-uninstall-app-management = Falls macOS es verweigert hat: Aktivieren Sie RABBIT unter Systemeinstellungen > Datenschutz & Sicherheit > App-Verwaltung. Beenden Sie RABBIT dann, starten Sie es erneut und deinstallieren Sie noch einmal.
wizard-done-uninstall-admin-unclear = Der Administratorbefehl hat geendet, ohne zu sagen, wie, aber von { $package } ist alles entfernt.
wizard-done-uninstall-installer-records-remain = Das Installationsprogramm von macOS führt noch Installationsbelege von { $package }, obwohl seine Dateien entfernt sind. Das schadet nicht; „Details anzeigen“ nennt sie.
wizard-done-uninstall-installer-records-unchecked = RABBIT konnte nicht prüfen, ob das Installationsprogramm von macOS { $package } vergessen hat ({ $error }). Das schadet nicht: Seine Dateien sind entfernt.
wizard-done-uninstall-program-started = Der Administratorbefehl hat eines dieser Programme geöffnet vorgefunden oder konnte nicht danach suchen und hat deshalb nichts geändert: { $programs }. Schließen Sie alle, die geöffnet sind, und starten Sie RABBIT dann erneut.
wizard-done-uninstall-admin-steps-refused = RABBIT konnte den Administratorbefehl für { $package } nicht zusammenstellen ({ $error }). Es hat deshalb nach keinem Passwort gefragt und nichts deinstalliert.
wizard-done-show-details = Details anzeigen
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = REAPER öffnen und RABBIT schließen
wizard-done-launch-reaper-mnemonic = R
wizard-done-open-resource = Ressourcenordner öffnen
wizard-done-open-resource-mnemonic = O
wizard-done-no-reaper-app = Für dieses Ziel ist keine startbare REAPER-Anwendung bekannt.
wizard-done-launch-reaper-error-prefix = REAPER konnte nicht gestartet werden
wizard-done-open-resource-error-prefix = Ressourcenordner konnte nicht geöffnet werden
wizard-done-self-update-apply-running = RABBIT-Aktualisierung wird angewendet…
wizard-done-self-update-error-prefix = RABBIT-Selbstaktualisierung fehlgeschlagen
wizard-done-self-update-relaunch-prefix = RABBIT neu gestartet
wizard-self-update-status-checking = Suche nach RABBIT-Aktualisierungen…

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = Installation anhalten?
wizard-close-during-install-body = RABBIT installiert gerade. Wenn Sie jetzt anhalten, beendet RABBIT den begonnenen Schritt, behält das bereits Installierte, räumt Angefangenes auf und schließt sich dann. Pakete, die noch nicht an der Reihe waren, bleiben nicht installiert. Installation anhalten?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = Deinstallation anhalten?
wizard-close-during-uninstall-body = RABBIT deinstalliert gerade. Wenn Sie jetzt anhalten, beendet RABBIT den begonnenen Schritt, hält dann an und schließt sich. Pakete, die noch nicht an der Reihe waren, bleiben installiert. Deinstallation anhalten?

# The same question while a restore runs, and while a delete of backups
# runs. No is the focused button here too.
wizard-close-during-restore-title = Wiederherstellung anhalten?
wizard-close-during-restore-body = RABBIT stellt gerade Sicherungen wieder her. Wenn Sie jetzt anhalten, schließt RABBIT die Wiederherstellung der aktuellen Sicherung noch ab, hält dann an und schließt sich. Sicherungen, die noch nicht an der Reihe waren, bleiben, wie sie sind. Wiederherstellung anhalten?
wizard-close-during-delete-title = Löschen der Sicherungen anhalten?
wizard-close-during-delete-body = RABBIT löscht gerade Sicherungen. Wenn Sie jetzt anhalten, löscht RABBIT die aktuelle Sicherung noch zu Ende, hält dann an und schließt sich. Sicherungen, die noch nicht an der Reihe waren, bleiben erhalten. Löschen der Sicherungen anhalten?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Die ausgewählten Pakete deinstallieren?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Eine Sicherung kann { $package } nicht zurückholen. Um es wieder zu haben, installieren Sie es erneut.
# Instead of the sentence above, for a package whose files go into a backup
# while RABBIT removes its uninstaller folder in Program Files and its
# registry entry with administrator rights. Review shows this sentence too,
# or the one above when none of the package's files are left to move.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-confirm-uninstall-partly-unbacked = Eine Sicherung kann die Dateien von { $package } zurückholen, aber nicht den Ordner mit dem Deinstallationsprogramm unter Programme (Program Files) und den Registrierungseintrag, die RABBIT mit Administratorrechten entfernt.
# Instead of wizard-confirm-uninstall-unbacked, for a package that goes to
# the Trash; the second when your account may not move it, so an
# administrator's password moves it there. $package is the localized
# package name. Name the Trash as Finder does.
wizard-confirm-uninstall-trash = { $package } kommt in den Papierkorb. Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen.
wizard-confirm-uninstall-trash-elevated-macos = { $package } kommt in den Papierkorb. Bis Sie den Papierkorb entleeren, können Sie es von dort wieder herausziehen; macOS fragt dafür aber möglicherweise nach dem Passwort eines Administrators.
wizard-confirm-uninstall-question = Jetzt deinstallieren?

# Asked before backups are deleted, every time. The No button is the focused
# one, so Enter never deletes anything. The text is the body, then the
# only-copy sentence when one applies, then the question, as one paragraph.
# $count and $only are numbers: the backups that go, and how many of them
# are the only copy of what they hold. $size is a size, such as "520 MB".
wizard-confirm-delete-backups-title = Sicherungen endgültig löschen?
wizard-confirm-delete-backups-body = RABBIT löscht { $count ->
    [one] eine Sicherung
   *[other] { $count } Sicherungen
} und gibt { $size } frei.
# As above, when $size is about what the delete frees, as for
# wizard-review-delete-intro-estimate.
wizard-confirm-delete-backups-body-estimate = RABBIT löscht { $count ->
    [one] eine Sicherung
   *[other] { $count } Sicherungen
} und gibt etwa { $size } frei.
wizard-confirm-delete-backups-only-copy = { $only ->
    [one] Eine davon ist die einzige Kopie ihres Inhalts.
   *[other] { $only } davon sind die einzige Kopie ihres Inhalts.
}
# Instead of the sentence above, when only one backup goes.
wizard-confirm-delete-backups-only-copy-single = Sie ist die einzige Kopie ihres Inhalts.
wizard-confirm-delete-backups-question = Eine gelöschte Sicherung lässt sich nicht zurückholen. Jetzt löschen?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = RABBIT aktualisiert sich selbst
wizard-close-during-self-update-body = RABBIT ersetzt gerade seine eigenen Programmdateien. Dieser Schritt lässt sich nicht anhalten, ohne die Installation zu beschädigen. Warten Sie, bis er fertig ist, und schließen Sie RABBIT danach.

# Modaler Dialog, der einmal pro Sitzung erscheint, wenn die Startprüfung
# eine neuere Version findet. Titel ist kurz, der Text verwendet dieselben
# Platzhalter wie die Statuszeile unten.
wizard-self-update-prompt-title = RABBIT-Aktualisierung verfügbar
wizard-self-update-prompt-body = RABBIT { $latest } ist verfügbar. Sie haben aktuell { $current }. Jetzt aktualisieren? RABBIT wird nach Abschluss der Aktualisierung neu gestartet.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = Neuerungen seit RABBIT { $current }:
wizard-self-update-prompt-update-button = Jetzt aktualisieren
wizard-self-update-prompt-later-button = Jetzt nicht

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = RABBIT wird aktualisiert

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = RABBIT-Aktualisierung verfügbar: { $current } → { $latest } (Kanal { $channel }). Starten Sie RABBIT neu, um erneut gefragt zu werden.
self-update-status-up-to-date = RABBIT ist auf dem neuesten Stand (aktuell { $current }, Kanal { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = Selbstaktualisierung hat keine Dateien ersetzt (Zielversion { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = { $count } Datei(en) unter { $root } ersetzt; starten Sie RABBIT neu, um { $version } zu verwenden.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Signaturüberprüfung: { $signed } signiert.
self-update-apply-signature-summary-unsigned-only = Signaturüberprüfung: { $unsigned } unsigniert.
self-update-apply-signature-summary-mixed = Signaturüberprüfung: { $signed } signiert, { $unsigned } unsigniert.

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Eine andere RABBIT-Installation läuft bereits (PID { $pid }). Anwenden ist angehalten, bis sie abgeschlossen ist.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Ziel: { $path }
wizard-summary-portable = Portables Ziel: { $value }
wizard-summary-dry-run = Probelauf: { $value }
wizard-summary-packages-selected = Ausgewählte Pakete: { $packages }
wizard-summary-cache = Zwischenspeicher: { $path }
wizard-summary-planned-app = Geplanter Anwendungspfad: { $path }
wizard-summary-error = Fehler: { $message }
wizard-summary-error-antivirus = Die Windows-Sicherheitssoftware hat diesen Download blockiert. Das ist meist ein Fehlalarm: Das Installationsprogramm ist signiert, und RABBIT hatte die Datei zuvor bereits anhand der Prüfsumme des Herausgebers überprüft. Defender meldet mitunter ein frisch erstelltes Installationsprogramm, das es noch nicht oft gesehen hat – typischerweise bei OSARA-Entwicklungsversionen. So geht es weiter: Öffnen Sie die Windows-Sicherheit, gehen Sie zu „Viren- & Bedrohungsschutz“ und dann zum „Schutzverlauf“, wählen Sie das blockierte Element aus und wählen Sie „Zulassen“. Starten Sie RABBIT anschließend erneut. Alternativ können Sie den Download-Ordner von RABBIT zu den Ausschlüssen hinzufügen oder dieses eine Paket von Hand von der Website des Herausgebers installieren. RABBIT schaltet Ihren Virenschutz niemals ab.
wizard-summary-resource-items-created = Angelegte Ressourceneinträge: { $count }
wizard-summary-packages-installed-or-checked = Installierte oder geprüfte Pakete: { $count }
wizard-summary-packages-current = Bereits aktuelle Pakete: { $count }
wizard-summary-packages-manual = Pakete, die manuelle Aufmerksamkeit benötigen: { $count }
wizard-summary-backup-files-created = Angelegte Sicherungsdateien: { $count }
wizard-summary-backup-file = Sicherungsdatei: { $path }
wizard-summary-receipt-backup = Beleg-Sicherung: { $path }
wizard-summary-backup-manifest = Sicherungsmanifest: { $path }
wizard-summary-osara-keymap-merge-hint = Ihre Tastenzuordnung wurde beibehalten. Um die neuen OSARA-Tastenkürzel hinzuzufügen, starten Sie REAPER und führen Sie die OSARA-Aktion „Merge OSARA key map“ aus (Menü „Erweiterungen“, OSARA).
# Show details after an install, for a file of a package's earlier version
# that the new version no longer installs: it moved into the update's
# backup, or it stays where it is, for the reason given. RABBIT no longer
# keeps track of a file that stays, and can't restore that earlier version.
# $path is the file's path.
wizard-summary-moved-to-backup = In die Sicherung verschoben, weil die neue Version sie nicht mehr installiert: { $path }
wizard-summary-left-in-place-in-use = Bleibt, wo sie ist, weil ein anderes Programm sie geöffnet hat: { $path }
wizard-summary-left-in-place-link = Bleibt, wo sie ist, weil dort eine Verknüpfung liegt, der RABBIT nie folgt: { $path }
wizard-summary-left-in-place-folder = Bleibt, wo sie ist, weil dort ein Ordner liegt: { $path }
wizard-summary-left-in-place-keymap = Bleibt, wo sie ist, weil sie die Tastenzuordnung von REAPER ist: { $path }
wizard-summary-left-in-place-refused = Bleibt, wo sie ist, weil RABBIT sie aus einer Sicherung nicht dorthin zurücklegen könnte: { $path }
wizard-summary-left-in-place-unreadable = Bleibt, wo sie ist, weil RABBIT sie nicht lesen konnte: { $path }
wizard-summary-left-in-place-other = Bleibt, wo sie ist, weil RABBIT sie nicht in die Sicherung verschieben konnte: { $path }
# Show details' line right after wizard-summary-moved-to-backup, for a file
# that was outside REAPER's own folder, in a folder other programs read too,
# such as the per-user CLAP folder: they no longer find it there. $path is the
# file's path.
wizard-summary-moved-from-shared-folder = Portable REAPER und andere Programme, die sie von dort geladen haben, finden sie nicht mehr: { $path }
# Show details after an update that moved the language pack file REAPER's
# language named into the backup, such as after a switch between the two
# Spanish variants: REAPER's language now names the new version's file;
# or it still names the old one, because REAPER may be open and would
# write its own setting back when it quits; or RABBIT couldn't set it.
# $file is the new version's language pack file and $before the old one's,
# file names such as es_MX.ReaperLangPack, never translated. Name REAPER's
# preferences as REAPER does in your language.
wizard-summary-language-switched = Die Sprache von REAPER verwendet jetzt { $file }, weil { $before } in die Sicherung verschoben wurde.
wizard-summary-language-reaper-may-run = Die Sprache von REAPER nennt noch { $before }, das in die Sicherung verschoben wurde, weil REAPER vielleicht geöffnet ist. Sobald REAPER geschlossen ist, wählen Sie { $file } in den Einstellungen von REAPER.
wizard-summary-language-failed = RABBIT konnte die Sprache von REAPER nicht auf { $file } stellen. Wählen Sie sie in den Einstellungen von REAPER.
# Said on Done after an install that left files of a package's earlier
# version where they were, which the new version no longer installs,
# instead of moving them into the update's backup. $count is how many, a
# number.
wizard-summary-left-in-place-hint = { $count ->
    [one] Eine Datei einer früheren Version ist geblieben, wo sie war. RABBIT verwaltet sie nicht mehr und kann diese Version nicht wiederherstellen. „Details anzeigen“ sagt, welche und warum.
   *[other] { $count } Dateien früherer Versionen sind geblieben, wo sie waren. RABBIT verwaltet sie nicht mehr und kann diese Versionen nicht wiederherstellen. „Details anzeigen“ sagt, welche und warum.
}
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Installieren/Aktualisieren/Beibehalten).
wizard-summary-package-plan-action =   Plan-Aktion: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Status: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Installierte Version: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architektur: { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Auch die Einstellungen entfernen: { $value }
# One line per file, below its package's line. $path is a full path.
# wizard-summary-left-over can also name a folder, such as a package's
# uninstaller folder in Program Files, or a registry key as regedit shows
# it; keep either as it is, never translated.
wizard-summary-removed-file = Entfernt: { $path }
wizard-summary-kept-file = Behalten, weil sich die Datei geändert hat, seit RABBIT sie installiert hat: { $path }
wizard-summary-kept-link = Behalten, weil der Ordner eine Verknüpfung enthält, der RABBIT nie folgt: { $path }
wizard-summary-kept-reapack = Behalten, weil ReaPack die Datei installiert hat und verwaltet: { $path }
wizard-summary-left-over = Noch vorhanden: { $path }
wizard-summary-left-in-backup = Konnte nicht zurückgelegt werden, liegt noch in der Sicherung: { $path }
# A file a package's installer added outside REAPER's folder that stays:
# it was there before RABBIT ran that installer, or RABBIT recorded it more
# than once with contents that disagree. "the installer" is the package's
# own installer. $path is a full path as Windows holds it, never
# translated.
wizard-summary-kept-there-before = Behalten, weil die Datei schon da war, als RABBIT das Installationsprogramm ausgeführt hat: { $path }
wizard-summary-kept-record-unclear = Behalten, weil RABBITs Aufzeichnungen zu dieser Datei nicht übereinstimmen: { $path }
wizard-summary-pending-restart = Wird beim Neustart des Computers entfernt: { $path }
wizard-summary-vendor-uninstaller = Sein eigenes Deinstallationsprogramm: { $path }
# Below a package removed on macOS, one line per path: what went to the Trash
# and where it is there now, and what the administrator command removed; then
# the installer receipts macOS still lists, and macOS's own words when a step
# failed. On Windows, the line for what was removed with administrator rights
# also names a package's uninstaller folder in Program Files, and each file
# in it, that RABBIT removed that way. $path is a full Windows or macOS path,
# keep it as it is; $ids a comma-separated list of receipt names, never
# translated; $error macOS's own message. Name the Trash as Finder does.
wizard-summary-moved-to-trash = In den Papierkorb gelegt: { $path }
wizard-summary-in-trash-as = Jetzt im Papierkorb: { $path }
wizard-summary-removed-as-admin = Mit Administratorrechten entfernt: { $path }
wizard-summary-installer-records-remain = Noch im Installationsprogramm von macOS geführt: { $ids }
wizard-summary-macos-said = macOS meldete: { $error }
# What became of a package's entry in Windows' list of installed programs,
# below the package's line. $key is the entry's registry key, such as
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA, and is
# never translated; $error is Windows' own error message.
wizard-summary-registry-removed = Aus der Liste der installierten Programme von Windows entfernt: { $key }
wizard-summary-registry-kept = Weiterhin in der Liste der installierten Programme von Windows, weil der Eintrag nicht auf dieses REAPER verweist: { $key }
wizard-summary-registry-failed = Weiterhin in der Liste der installierten Programme von Windows, weil RABBIT den Eintrag nicht lesen oder entfernen konnte ({ $error }): { $key }
# Below a package whose uninstaller folder and registry entry RABBIT removed
# with administrator rights: the registry entry. $key is the entry's
# registry key as regedit shows it, such as
# HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts,
# and is never translated. The folder and its files are listed with
# wizard-summary-removed-as-admin.
wizard-summary-registry-key-removed = Mit Administratorrechten aus der Registrierung entfernt: { $key }
# One line per uninstalled package whose files went into the backup.
# $package is the localized package name. Name the tab as
# wizard-packages-tab-backups does.
wizard-summary-uninstall-restorable = Sie können { $package } auf der Registerkarte „Sicherungen“ wiederherstellen.
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Bericht: { $path }
# The Done page after a delete of backups. Its first line, one of the seven
# below, also ends the progress page. Backups are counted as Review counts
# them, one for each ticked row. $count and $deleted are numbers: the
# backups ticked, or in the first message those deleted, and of them those
# deleted. $size is a size, such as "520 MB".
wizard-done-delete-status-success = RABBIT hat { $count ->
    [one] eine Sicherung
   *[other] { $count } Sicherungen
} gelöscht und { $size } freigegeben.
wizard-done-delete-status-partial = RABBIT hat { $deleted } von { $count } Sicherungen gelöscht.
wizard-done-delete-status-none = { $count ->
    [one] RABBIT konnte die Sicherung nicht löschen.
   *[other] RABBIT konnte die Sicherungen nicht löschen.
}
wizard-done-delete-status-already-gone = { $count ->
    [one] Die Sicherung war schon nicht mehr vorhanden, es gab also nichts zu löschen.
   *[other] Die Sicherungen waren schon nicht mehr vorhanden, es gab also nichts zu löschen.
}
wizard-done-delete-status-stopped = { $deleted ->
    [0] RABBIT hat angehalten, bevor es eine Sicherung gelöscht hat. Die Sicherungen, die noch nicht an der Reihe waren, bleiben erhalten.
   *[other] RABBIT hat angehalten, nachdem es { $deleted } von { $count } Sicherungen gelöscht hat. Die Sicherungen, die noch nicht an der Reihe waren, bleiben erhalten.
}
# When the user stopped the run partway through a backup, before RABBIT had
# deleted a whole one: part of a backup is gone.
wizard-done-delete-status-stopped-partway = { $count ->
    [one] RABBIT hat angehalten, bevor es die Sicherung ganz gelöscht hatte.
   *[other] RABBIT hat angehalten, bevor es eine Sicherung ganz gelöscht hatte. Die Sicherungen, die noch nicht an der Reihe waren, bleiben erhalten.
}
# When RABBIT deleted nothing because it refused the run, or something went
# wrong before it started.
wizard-done-delete-status-nothing = Es wurde nichts gelöscht.
# Why backups weren't deleted, after the first line, one sentence each.
# $total is how many backups were ticked, and $count how many of them the
# sentence is about; both are numbers. The [1] form is for a delete of a
# single backup: the first line calls it "the backup", so the sentence
# calls it "it". Name "Show details" as wizard-done-show-details does, and
# the Backups tab as wizard-packages-tab-backups does.
wizard-done-delete-in-use = { $total ->
    [1] Eine Datei darin ist in einem anderen Programm geöffnet. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um die Sicherung zu löschen.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden, weil eine Datei darin in einem anderen Programm geöffnet ist. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um sie zu löschen.
       *[other] { $count } konnten nicht gelöscht werden, weil eine Datei darin in einem anderen Programm geöffnet ist. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um sie zu löschen.
    }
}
wizard-done-delete-permission = { $total ->
    [1] RABBIT darf die Sicherung nicht verschieben. Sie bleibt, wie sie war.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden, weil RABBIT sie nicht verschieben darf. Sie bleibt, wie sie war.
       *[other] { $count } konnten nicht gelöscht werden, weil RABBIT sie nicht verschieben darf. Sie bleiben, wie sie waren.
    }
}
# For a package or key map RABBIT couldn't take out of the backup set it is
# in, because the set's own record of what it holds was open in another
# program, or RABBIT isn't allowed to change it. Nothing of that set
# changed. $total and $count as above.
wizard-done-delete-record-in-use = { $total ->
    [1] Das Inhaltsverzeichnis der Sicherung ist in einem anderen Programm geöffnet. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um die Sicherung zu löschen.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden, weil ihr Inhaltsverzeichnis in einem anderen Programm geöffnet ist. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um sie zu löschen.
       *[other] { $count } konnten nicht gelöscht werden, weil ihr Inhaltsverzeichnis in einem anderen Programm geöffnet ist. Schließen Sie dieses Programm, und starten Sie RABBIT dann erneut, um sie zu löschen.
    }
}
wizard-done-delete-record-permission = { $total ->
    [1] RABBIT darf das Inhaltsverzeichnis der Sicherung nicht ändern. Sie bleibt, wie sie war.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden, weil RABBIT ihr Inhaltsverzeichnis nicht ändern darf. Sie bleibt, wie sie war.
       *[other] { $count } konnten nicht gelöscht werden, weil RABBIT ihr Inhaltsverzeichnis nicht ändern darf. Sie bleiben, wie sie waren.
    }
}
wizard-done-delete-earlier-deletion-left = { $total ->
    [1] Eine frühere Löschung dieser Sicherung wurde nicht abgeschlossen, und RABBIT konnte sie auch jetzt nicht abschließen, daher bleibt die Sicherung, wie sie war. Starten Sie RABBIT erneut, um es noch einmal zu versuchen.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden, weil eine frühere Löschung dieser Sicherung nicht abgeschlossen wurde und RABBIT sie auch jetzt nicht abschließen konnte. Sie bleibt, wie sie war. Starten Sie RABBIT erneut, um es noch einmal zu versuchen.
       *[other] { $count } konnten nicht gelöscht werden, weil frühere Löschungen dieser Sicherungen nicht abgeschlossen wurden und RABBIT sie auch jetzt nicht abschließen konnte. Sie bleiben, wie sie waren. Starten Sie RABBIT erneut, um es noch einmal zu versuchen.
    }
}
wizard-done-delete-other = { $total ->
    [1] Etwas ist schiefgegangen, und die Sicherung bleibt, wie sie war. „Details anzeigen“ sagt, was.
   *[other] { $count ->
        [one] Eine konnte nicht gelöscht werden und bleibt, wie sie war. „Details anzeigen“ sagt, warum.
       *[other] { $count } konnten nicht gelöscht werden und bleiben, wie sie waren. „Details anzeigen“ sagt, warum.
    }
}
wizard-done-delete-partly = { $total ->
    [1] RABBIT hat die Sicherung nur teilweise gelöscht. Der Rest ist eine nicht abgeschlossene Löschung, die keine Wiederherstellung mehr verwendet. Um sie abzuschließen, starten Sie RABBIT erneut und löschen Sie sie auf der Registerkarte „Sicherungen“.
   *[other] { $count ->
        [one] Eine wurde nur teilweise gelöscht. Was davon übrig ist, ist eine nicht abgeschlossene Löschung, die keine Wiederherstellung mehr verwendet. Um sie abzuschließen, starten Sie RABBIT erneut und löschen Sie sie auf der Registerkarte „Sicherungen“.
       *[other] { $count } wurden nur teilweise gelöscht. Was davon übrig ist, sind nicht abgeschlossene Löschungen, die keine Wiederherstellung mehr verwendet. Um sie abzuschließen, starten Sie RABBIT erneut und löschen Sie sie auf der Registerkarte „Sicherungen“.
    }
}
# The row for the old copies of RABBIT's records stands for several
# backups on disk, and only it can end like this: RABBIT deleted some of
# them, and couldn't delete the others, or the user stopped the run before
# it reached them.
wizard-done-delete-part-left = { $total ->
    [1] RABBIT hat einen Teil davon gelöscht, konnte den Rest aber nicht löschen; er bleibt, wie er war. „Details anzeigen“ sagt, warum.
   *[other] { $count ->
        [one] Eine wurde nur zum Teil gelöscht: Den Rest davon konnte RABBIT nicht löschen; er bleibt, wie er war. „Details anzeigen“ sagt, warum.
       *[other] { $count } wurden nur zum Teil gelöscht: Den Rest davon konnte RABBIT nicht löschen; er bleibt, wie er war. „Details anzeigen“ sagt, warum.
    }
}
wizard-done-delete-stopped-partway = { $total ->
    [1] RABBIT hat einen Teil davon gelöscht, bevor es angehalten hat. Der Rest bleibt erhalten.
   *[other] { $count ->
        [one] Eine wurde nur zum Teil gelöscht, bevor RABBIT angehalten hat. Der Rest davon bleibt erhalten.
       *[other] { $count } wurden nur zum Teil gelöscht, bevor RABBIT angehalten hat. Der Rest davon bleibt erhalten.
    }
}
# After the first line, for packages or key maps RABBIT deleted out of the
# backup set they were kept in, when some of their files couldn't be
# removed, for example because another program had one open. Those files
# stay in that set with nothing naming them, so no row lists them, and they
# still take up room until the rest of the set is deleted. $total and
# $count as above.
wizard-done-delete-files-left = { $total ->
    [1] Nicht alle ihrer Dateien ließen sich entfernen. Die nicht entfernten bleiben in der Sicherung, in der sie aufbewahrt wurde, und belegen dort Platz, bis der Rest dieser Sicherung gelöscht wird.
   *[other] { $count ->
        [one] Eine wurde gelöscht, aber nicht alle ihrer Dateien ließen sich entfernen. Die nicht entfernten bleiben in der Sicherung, in der sie aufbewahrt wurde, und belegen dort Platz, bis der Rest dieser Sicherung gelöscht wird.
       *[other] { $count } wurden gelöscht, aber nicht alle ihrer Dateien ließen sich entfernen. Die nicht entfernten bleiben in den Sicherungen, in denen sie aufbewahrt wurden, und belegen dort Platz, bis der Rest dieser Sicherungen gelöscht wird.
    }
}
# After the first line, when some of the ticked backups were gone before the
# run. $count is a number, fewer than the backups ticked.
wizard-done-delete-already-gone = { $count ->
    [one] Eine war schon nicht mehr vorhanden.
   *[other] { $count } waren schon nicht mehr vorhanden.
}
# After "Nothing was deleted." when a ticked backup can't be deleted, which
# refuses the whole run; said only when more than one backup was ticked.
wizard-done-delete-refused-run = RABBIT löscht die ausgewählten Sicherungen nur, wenn es jede davon löschen kann.
# Why a backup can't be deleted. $backup is what its row calls it with when
# it was made, as wizard-backups-row-reference says it. It comes after a
# colon, so a name like "Your key map" keeps its capital. $reason is one of
# the wizard-delete-refusal-* sentences, which calls that backup "it".
wizard-done-delete-refused-backup = RABBIT kann diese Sicherung nicht löschen: { $backup }. { $reason }
# When RABBIT couldn't read REAPER's resource folder, or a folder of its own
# backups in it, so it deleted nothing. $path is that folder as the computer
# writes it, keep it as it is.
wizard-done-delete-folder-unreadable = RABBIT konnte den Ordner { $path } nicht lesen. Prüfen Sie, ob er vorhanden ist und ob Sie ihn öffnen können, und starten Sie RABBIT dann erneut.
# When something else went wrong before RABBIT deleted anything. $error is
# the error message, in English.
wizard-done-delete-error = Etwas ist schiefgegangen, bevor RABBIT etwas gelöscht hat: { $error }
# Show details after a delete of backups: one line per ticked backup, then
# lines indented below it. $backup is what its row calls it, with when it
# was made, as wizard-backups-row-reference says it, when another ticked
# backup has the same name. $status is one of the wizard-delete-status-*
# labels, $size a size, such as "4 MB", and $path a folder as the computer
# writes it, keep it as it is. Name the Backups tab as
# wizard-packages-tab-backups does. The lines below say "it" of the backup.
wizard-summary-delete-backup = { $backup }: { $status }
wizard-summary-delete-freed = Freigegeben: { $size }
# Below a package or key map RABBIT deleted, when some of its files
# couldn't be removed: they stay in the backup set it was in, with nothing
# naming them, and go when the rest of that set is deleted. $count is how
# many, a number.
wizard-summary-delete-files-left = { $count ->
    [one] Eine ihrer Dateien ließ sich nicht entfernen und bleibt in der Sicherung, in der sie aufbewahrt wurde. Sie verschwindet, wenn der Rest dieser Sicherung gelöscht wird.
   *[other] { $count } ihrer Dateien ließen sich nicht entfernen und bleiben in der Sicherung, in der sie aufbewahrt wurden. Sie verschwinden, wenn der Rest dieser Sicherung gelöscht wird.
}
# Below a backup RABBIT deleted, or tried to, when deleting the whole backup
# set it was kept in also finished an unfinished deletion of that set,
# which wasn't ticked: the Backups tab listed it as an unfinished deletion.
# Said once for each set.
wizard-summary-delete-finished-earlier-deletion-too = RABBIT hat außerdem eine nicht abgeschlossene Löschung der Sicherung abgeschlossen, in der sie aufbewahrt wurde.
# What happened to a backup that wasn't deleted, then why, in a line of
# its own. Part of the row for the old copies of RABBIT's records, which
# stands for several backups on disk, may have gone though the rest didn't.
wizard-summary-delete-nothing-changed = Nichts daran wurde geändert.
wizard-summary-delete-part-left = RABBIT hat einen Teil davon gelöscht. Der Rest ließ sich nicht löschen und bleibt, wie er war.
wizard-summary-delete-in-use = Eine Datei darin ist in einem anderen Programm geöffnet, daher ließ sich die Sicherung nicht verschieben.
wizard-summary-delete-permission = RABBIT darf die Sicherung nicht verschieben.
# Why a package or key map couldn't be taken out of the backup set it is
# in: the set's own record of what it holds was open in another program, or
# RABBIT isn't allowed to change it.
wizard-summary-delete-record-in-use = Das Inhaltsverzeichnis der Sicherung ist in einem anderen Programm geöffnet, daher ließ es sich nicht ändern.
wizard-summary-delete-record-permission = RABBIT darf das Inhaltsverzeichnis der Sicherung nicht ändern.
wizard-summary-delete-earlier-deletion-left = Eine frühere Löschung dieser Sicherung wurde nicht abgeschlossen, und RABBIT konnte sie auch jetzt nicht abschließen. Der Rest dieser Löschung liegt in { $path }.
wizard-summary-delete-left-in = Was RABBIT nicht entfernen konnte, bleibt in { $path }. Die Registerkarte „Sicherungen“ führt es als nicht abgeschlossene Löschung, die keine Wiederherstellung mehr verwendet; wenn Sie es dort löschen, wird die Löschung abgeschlossen.
wizard-summary-delete-not-reached-stopped = RABBIT hat vorher angehalten, daher bleibt die Sicherung erhalten.
wizard-summary-delete-stopped-partway = RABBIT hat einen Teil davon gelöscht und dann angehalten. Der Rest bleibt erhalten.
wizard-summary-delete-not-reached-refused = RABBIT hat bei diesem Durchlauf nichts gelöscht, daher bleibt die Sicherung erhalten.
# For a backup RABBIT didn't get to, because it couldn't delete, or check,
# another ticked backup kept in the same backup set: RABBIT then leaves
# every backup of that set as it was, and goes on with the next set.
wizard-summary-delete-not-reached-kept-with = RABBIT hat sie unverändert gelassen, weil es eine andere ausgewählte Sicherung nicht löschen konnte, die mit ihr zusammen aufbewahrt wird.
# Below a ticked backup kept in the same backup set as a ticked backup
# above it, when RABBIT couldn't delete that set, deleted it only in part,
# or couldn't change its record of what it holds: what happened, and why,
# is the same for both, and is said only below the first. $backup is what
# that one's line calls it, as wizard-summary-delete-backup names it. It
# comes after a colon, so its capital stays.
wizard-summary-delete-same-as-above = Was mit ihr geschah und warum, steht weiter oben bei der Sicherung, mit der sie zusammen aufbewahrt wurde: { $backup }.
wizard-summary-delete-freed-total = Insgesamt freigegeben: { $size }
# $path is the full path of the report RABBIT saved.
wizard-summary-delete-report = Bericht: { $path }
# The Done page after a restore of backups. Its first line, one of those
# below, also ends the progress page. Backups are counted as Review counts
# them, one for each ticked row. $count and $restored are numbers: the
# backups ticked, and of them those restored. $backup is what the row of
# the one ticked backup calls it, such as "SWS Extension 2.14.0.7", or for
# a key map wizard-done-restore-keymap-in-sentence.
#
# When the one ticked backup came back: software an uninstall kept; an
# earlier version, where $package is the localized package name and
# $version the version it went back to, or for a package that comes in
# variants as wizard-backups-version-variant says; one whose version RABBIT
# doesn't know; a key map, with $origin and $date as in
# wizard-progress-status-restoring-keymap.
wizard-done-restore-status-one = RABBIT hat { $backup } wiederhergestellt.
wizard-done-restore-status-earlier = { $package } ist wieder auf Version { $version }.
wizard-done-restore-status-earlier-copy = { $package } ist wieder auf dem Stand der Kopie, die eine Aktualisierung ersetzt hatte.
wizard-done-restore-status-keymap = { $origin ->
    [own] Ihre Tastenzuordnung vom { $date } ist wieder da.
    [default-keys] Die Standardtasten von REAPER vom { $date } sind wieder da.
   *[other] Die Tastenzuordnung vom { $date } ist wieder da.
}
wizard-done-restore-status-keymap-undated = { $origin ->
    [own] Ihre Tastenzuordnung ist wieder da.
    [default-keys] Die Standardtasten von REAPER sind wieder da.
   *[other] Die Tastenzuordnung ist wieder da.
}
# What the Done page and Review call a key map inside a sentence, as
# $backup in wizard-done-restore-status-none, in the sentences on why a
# backup didn't come back, such as "RABBIT couldn't restore your key map
# from <date>", and in Review's on why one can't be restored yet. Start in
# lower case where your language allows. $origin and $date as in
# wizard-progress-status-restoring-keymap.
wizard-done-restore-keymap-in-sentence = { $origin ->
    [own] Ihre Tastenzuordnung vom { $date }
    [default-keys] die Standardtasten von REAPER vom { $date }
   *[other] die Tastenzuordnung vom { $date }
}
wizard-done-restore-keymap-in-sentence-undated = { $origin ->
    [own] Ihre Tastenzuordnung
    [default-keys] die Standardtasten von REAPER
   *[other] die Tastenzuordnung
}
# Several backups, all of them back; some of them; none, with nothing
# changed; none, where files RABBIT had changed couldn't be put back as they
# were; and a run the user stopped.
wizard-done-restore-status-success = RABBIT hat { $count ->
    [one] eine Sicherung
   *[other] { $count } Sicherungen
} wiederhergestellt.
wizard-done-restore-status-partial = RABBIT hat { $restored } von { $count } Sicherungen wiederhergestellt.
wizard-done-restore-status-none = { $count ->
    [one] RABBIT konnte { $backup } nicht wiederherstellen und hat alles gelassen, wie es war.
   *[other] RABBIT konnte die Sicherungen nicht wiederherstellen und hat alles gelassen, wie es war.
}
wizard-done-restore-status-none-changed = { $count ->
    [one] RABBIT konnte { $backup } nicht wiederherstellen.
   *[other] RABBIT konnte die Sicherungen nicht wiederherstellen.
}
wizard-done-restore-status-stopped = { $restored ->
    [0] RABBIT hat angehalten, bevor es eine Sicherung wiederhergestellt hat. Die Sicherungen, die noch nicht an der Reihe waren, bleiben, wie sie sind.
   *[other] RABBIT hat angehalten, nachdem es { $restored } von { $count } Sicherungen wiederhergestellt hat. Die Sicherungen, die noch nicht an der Reihe waren, bleiben, wie sie sind.
}
# When RABBIT restored nothing because it refused the run, or something went
# wrong before it started.
wizard-done-restore-status-nothing = Es wurde nichts wiederhergestellt.
# Why a backup didn't come back, after the first line, one sentence each.
# $total is how many backups were ticked, a number, and $backup what the row
# calls the one the sentence is about, or for a key map
# wizard-done-restore-keymap-in-sentence. The [1] form is for a restore of a
# single backup, which the first line names, so the sentence calls it "it".
# $reason is one of the wizard-restore-refusal-* sentences, which calls the
# backup "it". Name "Show details" as wizard-done-show-details does, and the
# Backups tab as wizard-packages-tab-backups does.
wizard-done-restore-refused = { $total ->
    [1] { $reason }
   *[other] RABBIT hat { $backup } nicht wiederhergestellt. { $reason }
}
wizard-done-restore-failed = { $total ->
    [1] Etwas ist schiefgegangen. „Details anzeigen“ sagt, was.
   *[other] Etwas ist schiefgegangen, daher hat RABBIT { $backup } unverändert gelassen. „Details anzeigen“ sagt, was.
}
wizard-done-restore-files-left = { $total ->
    [1] Einige Dateien, die RABBIT schon geändert hatte, ließen sich nicht in den vorherigen Zustand zurückversetzen. Die Dateien, die sie ersetzt haben, bleiben als Sicherung auf der Registerkarte „Sicherungen“ erhalten. „Details anzeigen“ sagt, welche.
   *[other] RABBIT konnte { $backup } nicht wiederherstellen, und einige Dateien, die es schon geändert hatte, ließen sich nicht in den vorherigen Zustand zurückversetzen. Die Dateien, die sie ersetzt haben, bleiben als Sicherung auf der Registerkarte „Sicherungen“ erhalten. „Details anzeigen“ sagt, welche.
}
# The same, when the restore had replaced nothing, so no backup keeps what
# it replaced: the files it couldn't take out again had nothing before them.
wizard-done-restore-files-left-no-backup = { $total ->
    [1] Einige Dateien, die RABBIT schon geändert hatte, ließen sich nicht in den vorherigen Zustand zurückversetzen. „Details anzeigen“ sagt, welche.
   *[other] RABBIT konnte { $backup } nicht wiederherstellen, und einige Dateien, die es schon geändert hatte, ließen sich nicht in den vorherigen Zustand zurückversetzen. „Details anzeigen“ sagt, welche.
}
wizard-done-restore-partly = { $total ->
    [1] Nur ein Teil davon ist wieder da. „Details anzeigen“ sagt, welcher.
   *[other] RABBIT hat { $backup } nur zum Teil wiederhergestellt. „Details anzeigen“ sagt, welcher Teil wieder da ist.
}
# After a refusal that ends "then try again" or "wait until it has
# finished", such as REAPER being open: Done has no way back, so the user
# tries again by starting RABBIT again.
wizard-done-restore-start-again = Um es erneut zu versuchen, schließen Sie RABBIT und starten Sie es neu.
# When RABBIT couldn't read REAPER's resource folder, or a folder of its own
# backups in it, so it restored nothing. $path is that folder as the computer
# writes it, keep it as it is.
wizard-done-restore-folder-unreadable = RABBIT konnte den Ordner { $path } nicht lesen. Prüfen Sie, ob er vorhanden ist und ob Sie ihn öffnen können, und starten Sie RABBIT dann erneut.
# When something else went wrong before RABBIT restored anything. $error is
# the error message, in English.
wizard-done-restore-error = Etwas ist schiefgegangen, bevor RABBIT etwas wiederhergestellt hat: { $error }
# What else the restore changed, after the lines above, and in Show
# details. $file is a language pack's file name, such as
# de_DE.ReaperLangPack, never translated; $package is the localized package
# name, $packages several joined as wizard-backups-list-and joins names, and
# $count how many, a number.
wizard-done-restore-language-back = REAPER hat wieder die Sprache { $file }.
wizard-done-restore-language-stays = Die Sprache von REAPER bleibt, wie sie ist, weil seitdem eine andere gewählt wurde.
wizard-done-restore-language-failed = RABBIT konnte die Sprache von REAPER nicht wieder auf { $file } stellen. „Details anzeigen“ sagt, warum.
# Going back to an earlier version of a language pack while REAPER uses
# another pack, or none: the restore leaves REAPER's language alone.
wizard-done-restore-language-unchanged = Die Sprache von REAPER bleibt, wie sie ist.
wizard-done-restore-not-counted = RABBIT zählt { $package } nicht mehr als von ihm installiert.
wizard-done-restore-needs-reinstall = Um { $package } später mit RABBIT zu deinstallieren, installieren Sie es zuerst erneut.
wizard-done-restore-left-out = { $count ->
    [one] RABBIT hat { $packages } ausgelassen, weil keine seiner Dateien in der Sicherung war.
   *[other] RABBIT hat { $packages } ausgelassen, weil keine ihrer Dateien in der Sicherung war.
}
# Show details after a restore of backups: one line per ticked backup, then
# lines indented below it, which say "it" of the backup. $backup is what the
# row calls it, with when it was made, as wizard-backups-row-reference says
# it, when another ticked backup has the same name; $status is one of the
# wizard-restore-status-* labels, and $path a file or folder as the computer
# writes it, keep it as it is.
wizard-summary-restore-backup = { $backup }: { $status }
# Each file the restore worked on, and what came of it. "The new backup" is
# the one this restore made of what it replaced.
wizard-summary-restore-file-restored = Wiederhergestellt: { $path }
wizard-summary-restore-file-put-back = In den vorherigen Zustand zurückversetzt: { $path }
wizard-summary-restore-file-left = Ließ sich nicht zurückversetzen und ist jetzt die Kopie aus der Sicherung: { $path }
wizard-summary-restore-file-missing = Fehlt und ließ sich nicht zurücklegen: { $path }
wizard-summary-restore-replaced-copy = Eine Kopie dessen, was vorher dort war, liegt in { $path }
wizard-summary-restore-file-unfinished = Nicht wiederhergestellt, weil ihre Kopie in der Sicherung nie fertig wurde: { $path }
# Files of the version a package had, when it went back to an earlier one.
wizard-summary-restore-moved-out = In die neue Sicherung verschoben, weil die frühere Version sie nicht hat: { $path }
wizard-summary-restore-still-in-backup = Ließ sich nicht an ihren Platz zurücklegen und liegt weiter in der neuen Sicherung: { $path }
wizard-summary-restore-changed-left = Bleibt, wo sie ist, weil sie sich geändert hat, seit RABBIT sie installiert hat: { $path }
wizard-summary-restore-left-in-place = Bleibt, wo sie ist; eine Kopie liegt in der neuen Sicherung: { $path }
# The scripts a key map restore kept in REAPER's Actions list, which were
# registered since that key map was replaced. Name the Actions list as
# REAPER does in your language. $count is a number.
wizard-summary-restore-scripts-kept = { $count ->
    [one] RABBIT hat ein seitdem registriertes Skript in der Aktionsliste von REAPER behalten.
   *[other] RABBIT hat { $count } seitdem registrierte Skripte in der Aktionsliste von REAPER behalten.
}
# The backup this restore made of what it replaced. $backup is what the
# Backups tab's row calls it, such as "Key map" or "ReaPack 1.2.5", and
# $date a date and time (backup-date-time). The -undated form is for a
# backup whose date RABBIT can't tell. Name the Backups tab as
# wizard-packages-tab-backups does.
wizard-summary-restore-kept = Was dabei ersetzt wurde, bleibt als Sicherung auf der Registerkarte „Sicherungen“ erhalten: { $backup }, vom { $date }.
wizard-summary-restore-kept-undated = Was dabei ersetzt wurde, bleibt als Sicherung auf der Registerkarte „Sicherungen“ erhalten: { $backup }.
wizard-summary-restore-not-reached = RABBIT hat vorher angehalten, daher bleibt die Sicherung, wie sie ist.
# $path is the full path of the report RABBIT saved.
wizard-summary-restore-report = Bericht: { $path }
status-installed-or-checked = Installiert oder geprüft
status-planned-unattended = Unbeaufsichtigt geplant
status-deferred-unattended = Unbeaufsichtigt verschoben
status-skipped-current = Übersprungen (bereits aktuell)
status-failed = Fehlgeschlagen
status-skipped-dependency-failed = Übersprungen (Abhängigkeit fehlgeschlagen)
status-cancelled = Nicht installiert (von Ihnen angehalten)
status-skipped-left-to-reapack = Übersprungen (ReaPack überlassen)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Deinstalliert
status-removed-with-leftovers = Deinstalliert, aber Teile davon sind noch vorhanden
status-removed-pending-restart = Deinstalliert; der Rest wird beim nächsten Neustart des Computers entfernt
status-would-remove = Würde deinstalliert
status-remove-not-installed = War nicht installiert
status-remove-blocked = Nicht deinstalliert
status-remove-failed = Konnte nicht deinstalliert werden
status-remove-cancelled = Weiterhin installiert (von Ihnen angehalten)
status-remove-not-reached = Weiterhin installiert (der Vorgang hat vorher angehalten)
# What came of a backup in a delete, after its name in "Show details".
wizard-delete-status-deleted = Gelöscht
wizard-delete-status-finished-earlier-deletion = Gelöscht; damit ist eine früher begonnene Löschung abgeschlossen
wizard-delete-status-already-gone = Schon nicht mehr vorhanden
wizard-delete-status-would-delete = Würde gelöscht
wizard-delete-status-would-finish-earlier-deletion = Würde eine früher begonnene Löschung abschließen
wizard-delete-status-not-deleted = Nicht gelöscht
wizard-delete-status-partly-deleted = Teilweise gelöscht
wizard-delete-status-refused = Kann nicht gelöscht werden
wizard-delete-status-not-reached = Nicht erreicht
# What came of a backup in a restore, after its name in "Show details".
wizard-restore-status-restored = Wiederhergestellt
wizard-restore-status-would-restore = Würde wiederhergestellt
wizard-restore-status-not-restored = Nicht wiederhergestellt
wizard-restore-status-files-left = Nicht wiederhergestellt; einige Dateien ließen sich nicht in den vorherigen Zustand zurückversetzen
wizard-restore-status-not-reached = Nicht erreicht
wizard-restore-status-partly-restored = Teilweise wiederhergestellt

# Paketstatus-Meldungen, die auf der Fertig-Seite des Assistenten neben dem
# Paketnamen erscheinen (z. B. „OSARA: <Meldung>"). Die Wrapper-Vorlage
# `wizard-summary-package-message` setzt den Paketnamen bereits voran, daher
# enthält jeder dieser Strings nur den eigentlichen Meldungstext.
package-status-extension-binary-installed = Einzelne Erweiterungsbinärdatei wurde von RABBIT installiert.
# $installed ist die installierte Version; $available ist die neueste verfügbare Version.
package-status-skipped-current = Installierte Version { $installed } ist aktuell oder neuer als die verfügbare Version { $available }.
package-status-skipped-content-unchanged = Die installierte Fassung ist identisch mit der beim Herausgeber veröffentlichten.
# A package RABBIT left to ReaPack, even when asked for it by name: ReaPack's
# records say ReaPack installed it, or RABBIT couldn't read them.
# $reapack_package is the name ReaPack knows the package by, $error an error
# message.
package-status-skipped-managed-by-reapack = Laut den Einträgen von ReaPack hat ReaPack dieses Paket als { $reapack_package } installiert, deshalb hat RABBIT es ReaPack überlassen. Installieren, aktualisieren oder entfernen Sie es in ReaPack.
package-status-skipped-reapack-records-unreadable = RABBIT konnte die Einträge von ReaPack nicht lesen ({ $error }) und kann deshalb nicht sagen, ob ReaPack dieses Paket installiert hat. Daher hat RABBIT es weder installiert noch aktualisiert.
package-status-install-failed = Installation fehlgeschlagen: { $error }
package-status-skipped-dependency-failed = Übersprungen, weil { $dependency }, was dafür benötigt wird, nicht erfolgreich installiert wurde.
package-status-cancelled = Nicht installiert: Sie haben die Einrichtung angehalten, bevor RABBIT dieses Paket erreicht hat.
# Erscheint nach dem Paketnamen, wenn das Installationsprogramm des Herstellers
# mit einem Code ungleich 0 beendet wurde. $code ist die Zahl. Danach folgt
# einer der Sätze package-status-vendor-exit-* und, wenn das
# Installationsprogramm ein Protokoll hinterlassen hat,
# package-status-vendor-installer-log.
package-status-vendor-installer-failed = Das Installationsprogramm wurde mit dem Fehlercode { $code } beendet.
# Exit-Code 2 von Inno Setup: abgebrochen, bevor die Installation begann. Da
# RABBIT es unbeaufsichtigt ausführt, wurde praktisch immer die
# Sicherheitsabfrage von Windows (Benutzerkontensteuerung) abgelehnt, die das
# Installationsprogramm angezeigt hat. „Ja“ ist die Schaltfläche der Abfrage.
package-status-vendor-exit-cancelled-before-installing = Das heißt meist, dass die Sicherheitsabfrage von Windows abgelehnt wurde; geändert wurde nichts. Starten Sie RABBIT erneut und wählen Sie „Ja“, wenn Windows um Erlaubnis fragt.
# Exit-Code 5 von Inno Setup: Es hat bei einer Rückfrage angehalten, etwa ob
# Programme geschlossen werden sollen, die seine Dateien verwenden, und die
# niemand beantworten kann, solange RABBIT es unbeaufsichtigt ausführt.
# $package ist der Paketname.
package-status-vendor-exit-stopped-at-question = Das heißt, es hat bei einer Rückfrage angehalten, die es nicht stellen kann, solange RABBIT es unbeaufsichtigt ausführt – meist, weil ein anderes Programm eine seiner Dateien geöffnet hat. Schließen Sie andere Programme, vor allem andere Musikprogramme und { $package }, und starten Sie RABBIT dann erneut.
# Exit-Code 7 von Inno Setup: Die Vorbereitung hat ergeben, dass es nicht weitergeht.
package-status-vendor-exit-preparation-failed = Das heißt, es hat bei der Vorbereitung festgestellt, dass es gerade nicht installieren kann. Starten Sie den Computer neu und danach RABBIT erneut.
# Exit-Code 8 von Inno Setup: Die Vorbereitung hat ergeben, dass Windows zuerst
# neu starten muss.
package-status-vendor-exit-restart-needed = Das heißt, es hat bei der Vorbereitung festgestellt, dass Windows zuerst neu gestartet werden muss. Starten Sie den Computer neu und danach RABBIT erneut.
# Jeder andere Code, oder ein Installationsprogramm, dessen Codes RABBIT nicht
# kennt. $package ist der Paketname.
package-status-vendor-exit-unknown = Schließen Sie andere Programme und starten Sie RABBIT erneut. Schlägt es wieder fehl, installieren Sie { $package } von Hand von der Website des Herausgebers.
# $path ist die Protokolldatei, die das Installationsprogramm geschrieben hat.
package-status-vendor-installer-log = Das Protokoll des Installationsprogramms liegt unter { $path }.
# Erscheint nach dem Paketnamen, wenn RABBIT das Installationsprogramm eines
# Pakets nicht gestartet hat, weil Programme des Pakets selbst geöffnet waren.
# $package ist der Paketname; $programs listet die Dateinamen der geöffneten
# Programme auf, etwa „Surge XT.exe“.
package-status-blocking-programs-running = RABBIT hat das Installationsprogramm nicht gestartet, weil { $package } geöffnet ist ({ $programs }) und das Installationsprogramm kein laufendes Programm ersetzen kann. Schließen Sie das Programm und starten Sie RABBIT dann erneut.
# $automation ist einer der „package-automation-*"-Texte (Vendor-Installationsprogramm / Archiv-Entpacken / …).
package-status-dry-run-would-run-unattended = Probelauf: RABBIT würde dieses { $automation } unbeaufsichtigt herunterladen und ausführen.
# $automation ist einer der „package-automation-*"-Texte.
package-status-deferred-unattended-staged = Diese RABBIT-Version unterstützt den geplanten unbeaufsichtigten Ablauf für { $automation } noch nicht. RABBIT hat das Artefakt im Zwischenspeicher abgelegt, aber nicht ausgeführt.
# $automation ist einer der „package-automation-*"-Texte.
package-status-deferred-unattended-not-staged = Diese RABBIT-Version unterstützt den geplanten unbeaufsichtigten Ablauf für { $automation } noch nicht. RABBIT hat das Artefakt weder heruntergeladen noch ausgeführt.
package-status-unattended-installed = RABBIT hat das Installationsprogramm unbeaufsichtigt ausgeführt, die erwarteten Zielpfade überprüft und den RABBIT-Beleg aktualisiert.
package-status-osara-unattended-keymap-backed-up = RABBIT hat das Installationsprogramm unbeaufsichtigt ausgeführt, reaper-kb.ini gesichert, die OSARA-Tastenzuordnung übernommen und den RABBIT-Beleg aktualisiert.
package-status-osara-unattended-keymap-replaced = RABBIT hat das Installationsprogramm unbeaufsichtigt ausgeführt, die OSARA-Tastenzuordnung übernommen und den RABBIT-Beleg aktualisiert.

# Kurze Beschreibungen der Automatisierungsart, die in die obigen
# Paketstatus-Meldungen eingesetzt werden.
package-automation-installer = Vendor-Installationsprogramm
package-automation-archive = Archiv-Entpacken
package-automation-disk-image = Image-Installation
package-automation-extension-binary = direkten Dateieinbau

# Statusmeldungen pro Konfigurationsschritt auf der Fertig-Seite des
# Assistenten. Die Wrapper-Vorlage `wizard-summary-configuration-message`
# setzt den Schrittnamen voran; die folgenden Strings enthalten nur den
# Meldungstext.
# $name ist der menschenlesbare Name des Remotes; $url ist die index-XML-URL.
config-message-reapack-remote-already-present = ReaPack-Remote { $name } ({ $url }) ist in reapack.ini bereits eingerichtet.
config-message-reapack-remote-added = ReaPack-Remote { $name } ({ $url }) wurde zu reapack.ini hinzugefügt.
config-message-reapack-remote-created-file = reapack.ini wurde mit dem ReaPack-Remote { $name } ({ $url }) angelegt. ReaPack ergänzt seine Standard-Repositorys beim nächsten REAPER-Start.
config-message-reapack-remote-dry-run = Würde den ReaPack-Remote { $name } ({ $url }) zu reapack.ini hinzufügen.
config-message-reaper-language-already-selected = REAPER war bereits auf { $file } eingestellt.
config-message-reaper-language-selected = REAPER-Sprache auf { $file } eingestellt.
config-message-reaper-language-dry-run = Würde die REAPER-Sprache auf { $file } einstellen.
# $step ist die Schritt-Kennung (z. B. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = Konfigurationsschritt { $step } wurde nicht ausgewählt.
# $step ist die Schritt-Kennung; $dependency ist die Kennung des abhängigen Pakets.
config-message-skipped-dependency-missing = Konfigurationsschritt { $step } übersprungen, weil das abhängige Paket { $dependency } weder installiert ist noch zu diesem Lauf gehört.
config-message-applied-no-op = Konfigurationsschritt ohne Änderungen angewendet.
config-message-cancelled = Dieser Konfigurationsschritt wurde nicht ausgeführt: Sie haben die Einrichtung vorher angehalten.

# Statuszeile pro Konfigurationsschritt auf der Fertig-Seite, analog zu
# `wizard-summary-package-status` für die Pakete.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Status: { $status }

# Statusbezeichner für Konfigurationsschritte in der „  Status: …"-Zeile.
config-status-applied = Angewendet
config-status-skipped = Übersprungen
config-status-skipped-dependency-missing = Übersprungen (Abhängigkeit fehlt)
config-status-dry-run = Probelauf
config-status-cancelled = Nicht ausgeführt (von Ihnen angehalten)
wizard-summary-planned-execution-title = Geplante unbeaufsichtigte Ausführung:
wizard-summary-planned-execution-runner =   Ausführer: { $runner }
wizard-summary-planned-execution-artifact =   Artefakt: { $artifact }
wizard-summary-planned-execution-program =   Programm: { $program }
wizard-summary-planned-execution-program-zipped =   Programm: das Installationsprogramm aus der ZIP-Datei, sobald sie heruntergeladen ist
wizard-summary-planned-execution-arguments =   Argumente: { $arguments }
wizard-summary-planned-execution-working-directory =   Arbeitsverzeichnis: { $path }
wizard-summary-planned-execution-verify =   Prüfen: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Hinweis: { $note }
wizard-summary-status-finished = Abgeschlossen. { $installed } Paketeintrag/Paketeinträge installiert oder geprüft; { $manual } benötigen manuelle Aufmerksamkeit.
wizard-summary-status-finished-with-errors = Mit Fehlern abgeschlossen. { $installed } Paketelement(e) installiert oder geprüft; { $failed } fehlgeschlagen.

wizard-planned-runner-launch-installer = Installationsprogramm ausführen
wizard-planned-runner-extract-archive = Archiv entpacken und enthaltenes Installationsprogramm ausführen
wizard-planned-runner-extract-archive-copy-osara = Archiv entpacken und OSARA-Installationsdateien kopieren
wizard-planned-runner-mount-disk-image = Image einhängen und enthaltenes Installationsprogramm ausführen
wizard-planned-runner-mount-disk-image-copy-app = Image einhängen und enthaltenes Anwendungspaket kopieren
wizard-planned-runner-mount-disk-image-run-pkg = Image einhängen und enthaltenes pkg-Installationsprogramm ausführen

# Expertenmodus und Versionskanäle.
package-channel-reaper-dev = Entwicklerversionen von landoleet.org
package-channel-osara-pr = Testversionen aus Pull-Requests
wizard-window-title-expert = { $title } – Expertenmodus
wizard-expert-enable-title = Expertenmodus einschalten?
wizard-expert-enable-body = Im Expertenmodus können Sie Vorabversionen installieren: Entwicklerversionen von REAPER von landoleet.org und Testversionen von OSARA-Pull-Requests. Sie werden nicht unterstützt, können REAPER oder seine Barrierefreiheit beeinträchtigen und sind nur zum Testen gedacht. Auf dieser Seite erscheinen dann unter der REAPER-Installation zwei Auswahlfelder für die Versionen. Expertenmodus einschalten?
wizard-expert-disabled-title = Expertenmodus ist aus
wizard-expert-disabled-body = Der Expertenmodus ist aus. Pakete, die aus einer Vorabversion installiert wurden, kehren bei der nächsten Installation zu ihrer regulären Version zurück.
wizard-expert-first-page-title = Expertenmodus
wizard-expert-first-page-body = Der Expertenmodus lässt sich nur auf der ersten Seite ein- oder ausschalten, weil er dort die Auswahl der Versionen hinzufügt. Gehen Sie zur ersten Seite zurück, um ihn zu ändern.
wizard-expert-busy-body = Der Expertenmodus lässt sich nicht ändern, während RABBIT installiert oder nachdem die Installation abgeschlossen ist.
wizard-expert-reaper-builds-label = REAPER-Versionen
wizard-expert-reaper-builds-stable = Reguläre Veröffentlichungen
wizard-expert-osara-builds-label = OSARA-Versionen
wizard-expert-osara-builds-snapshot = Reguläre Snapshots
wizard-expert-osara-builds-loading = Suche nach Pull-Request-Versionen…
wizard-expert-osara-builds-unavailable = Reguläre Snapshots (Pull-Request-Versionen konnten nicht abgerufen werden)
wizard-expert-osara-builds-pr = Pull-Request { $number }: { $title }
wizard-expert-osara-builds-pr-untitled = Pull-Request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = Entwicklerversion
wizard-package-channel-dev-details = { $package } wird aus den Entwicklerversionen installiert: nicht unterstützte Vorabversionen, die jederzeit fehlerhaft sein können.
wizard-package-channel-pr = Testversion aus Pull-Request { $number }
wizard-package-channel-pr-details = { $package } wird aus der Testversion von Pull-Request { $number } installiert: ungeprüfter, nicht signierter Code, den GitHub nach 90 Tagen löscht.
wizard-package-channel-back = zurück zur regulären Version
wizard-package-channel-back-details = { $package } ist derzeit eine Vorabversion. Diese Installation stellt die reguläre Version wieder her.
