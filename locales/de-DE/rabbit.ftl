app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = ja
common-no = nein

# What happens to a package in this run, the first thing its row says. An
# unticked row on either tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal.
action-install = Installieren
action-update = Aktualisieren
action-keep = Beibehalten
action-uninstall = Deinstallieren

package-reaper = REAPER
package-osara = OSARA
package-sws = SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = JAWS-Skripte für REAPER
package-ffmpeg = FFmpeg
package-surge-xt = Surge XT
package-app2clap = app2clap
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
package-app2clap-description = app2clap ist ein CLAP-Plugin für Windows, das Audio aus anderen Anwendungen erfasst und es in REAPER (oder jeden CLAP-Host) als Plug-in einspeist, das du in eine Spur einfügst — praktisch, um Ton aus einem Browser, Mediaplayer oder anderen Programm aufzunehmen oder zu bearbeiten. RABBIT lädt die neueste Version herunter und installiert app2clap.clap in deinen benutzereigenen CLAP-Ordner, sodass keine Administratorrechte nötig sind. Nur für Windows. Nur Standard-REAPER-Installationen: Die Installation erfolgt außerhalb eines portablen REAPER-Ordners.
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
uninstall-summary-app2clap-removes = Entfernt app2clap aus Ihrem CLAP-Ordner. Alle Programme, in denen Sie CLAP-Plug-ins verwenden, verlieren es, nicht nur REAPER.
uninstall-summary-app2clap-keeps = Der CLAP-Ordner und die anderen Plug-ins darin bleiben erhalten.
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
uninstall-kept-app2clap-clap-folder = der CLAP-Ordner, den auch andere Plug-ins nutzen
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

wizard-packages-heading = Auswählen, was installiert, aktualisiert oder deinstalliert wird
wizard-packages-list-label = Installieren oder aktualisieren: Pakete und Einstellungen
wizard-packages-row-checked = Ausgewählt
wizard-packages-row-unchecked = Nicht ausgewählt
wizard-packages-choice-shown = Neue Auswahl unter der Liste: { $choice }.
wizard-packages-choice-hidden = Auswahl entfernt: { $choice }.
wizard-packages-tree-group-label = Pakete
wizard-additional-software-tree-group-label = Zusätzliche Software
wizard-language-tree-group-label = Sprachpakete
wizard-configuration-tree-group-label = Konfiguration

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Installieren oder aktualisieren
wizard-packages-tab-uninstall = Deinstallieren
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
wizard-review-uninstall-reinstall-hint = Sie können diese Pakete später auf der Registerkarte „Installieren oder aktualisieren“ wieder installieren.
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Deinstallieren derzeit nicht möglich: { $reason }
wizard-review-uninstall-not-writable = RABBIT kann nicht in den Ordner dieses REAPER schreiben.

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
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Ein anderes geöffnetes RABBIT-Fenster installiert oder deinstalliert gerade in diesem REAPER. Warten Sie, bis es fertig ist, und starten Sie RABBIT dann erneut.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Schließen Sie REAPER und drücken Sie dann erneut auf „Deinstallieren“. RABBIT kann nichts aus REAPER deinstallieren, solange REAPER läuft.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
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
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } war nicht mehr installiert, es gab also nichts zu entfernen.
wizard-done-uninstall-left-in-backup = Einige Dateien von { $package } konnten nicht zurückgelegt werden und liegen noch in der Sicherung. „Details anzeigen“ listet sie auf.
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
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Sicherung: { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = Um { $package } zurückzuholen, schließen Sie REAPER und führen Sie diesen Befehl aus: { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Bericht: { $path }
status-installed-or-checked = Installiert oder geprüft
status-planned-unattended = Unbeaufsichtigt geplant
status-deferred-unattended = Unbeaufsichtigt verschoben
status-skipped-current = Übersprungen (bereits aktuell)
status-failed = Fehlgeschlagen
status-skipped-dependency-failed = Übersprungen (Abhängigkeit fehlgeschlagen)
status-cancelled = Nicht installiert (von Ihnen angehalten)
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

# Paketstatus-Meldungen, die auf der Fertig-Seite des Assistenten neben dem
# Paketnamen erscheinen (z. B. „OSARA: <Meldung>"). Die Wrapper-Vorlage
# `wizard-summary-package-message` setzt den Paketnamen bereits voran, daher
# enthält jeder dieser Strings nur den eigentlichen Meldungstext.
package-status-extension-binary-installed = Einzelne Erweiterungsbinärdatei wurde von RABBIT installiert.
# $installed ist die installierte Version; $available ist die neueste verfügbare Version.
package-status-skipped-current = Installierte Version { $installed } ist aktuell oder neuer als die verfügbare Version { $available }.
package-status-skipped-content-unchanged = Die installierte Fassung ist identisch mit der beim Herausgeber veröffentlichten.
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
