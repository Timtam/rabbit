app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = oui
common-no = non

# What happens to a package in this run, the first thing its row says. An
# unticked row on every tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal, and on
# the Backups tab that the backup isn't ticked to restore or delete.
action-install = À installer
action-update = À mettre à jour
action-keep = Ne pas modifier
action-uninstall = À désinstaller
# On the Backups tab: what happens to a backup ticked to restore, or to
# delete. The last is for a backup that is the only copy of what it holds,
# such as software you uninstalled.
action-restore = À restaurer
action-delete = À supprimer
action-delete-only-copy = À supprimer (seule copie)

package-reaper = REAPER
package-osara = OSARA
package-sws = Extension SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Scripts JAWS de Snowman pour REAPER
package-ffmpeg = FFmpeg (prise en charge vidéo améliorée)
package-surge-xt = Surge XT
package-ljuno-116 = LJuno-116
package-lb-303 = LB-303
package-lr-608 = LR-608
package-lsampler-24 = LSampler-24
package-app2clap = app2clap
package-lreaparams = LReaParams
package-langpack-es = REAPER en espagnol
package-langpack-de = REAPER en allemand
package-langpack-fr = REAPER en français
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Équipe PMA (es_MX)

package-reaper-description = REAPER est la station de travail audionumérique sur laquelle tout le reste repose. RABBIT peut l'installer ou le mettre à jour pour vous.
package-osara-description = OSARA est l'extension d'accessibilité open source qui rend REAPER utilisable avec un lecteur d'écran. NVDA, JAWS et Narrateur sous Windows, VoiceOver sous macOS sont tous largement répandus ; d'autres lecteurs d'écran Windows peuvent également fonctionner. Installez OSARA si vous dépendez d'un lecteur d'écran pour utiliser REAPER.
package-sws-description = L'extension SWS est un ensemble communautaire de longue date d'actions, de scripts et d'outils supplémentaires qui étendent les fonctionnalités de REAPER. Pour la configuration de REAPER la plus accessible possible, que vous soyez sous Windows ou sous Mac, installez SWS en complément d'OSARA.
package-reapack-description = ReaPack est un gestionnaire de paquets open source. Il permet de rechercher, d'installer, de suivre et de mettre à jour des scripts et des extensions tiers directement depuis REAPER. Installez-le si vous souhaitez utiliser des scripts partagés par la communauté REAPER.
package-reakontrol-description = ReaKontrol offre une intégration open source pour les claviers Komplete Kontrol de Native Instruments. Installez-le si vous possédez un clavier de série S MK2, de série A, M-32 ou Kontrol MK3.
package-jaws-scripts-description = Les scripts de Snowman améliorent la façon dont JAWS gère les différentes fenêtres de REAPER, et offrent une prise en charge étendue du braille ainsi que de nombreuses autres fonctionnalités. Notez que ces scripts sont conçus pour être utilisés en complément d'OSARA : ils n'en sont pas une alternative. Pour une accessibilité optimale avec JAWS, installez les deux.
package-ffmpeg-description = Les bibliothèques d'exécution partagées de FFmpeg permettent au décodeur vidéo de REAPER d'importer et de lire les formats audio et vidéo courants. RABBIT installe le dossier bin de la version « shared » 64 bits (x64) de Gyan.dev dans UserPlugins. REAPER sur un PC Windows à processeur ARM peut charger les bibliothèques 64 bits (x64) : il reçoit donc lui aussi cette version. Le niveau de correctif ne peut pas être déduit des seuls noms de fichiers DLL, c'est pourquoi les installations externes de FFmpeg sont signalées avec un substitut « <major>.0.0 ».
package-surge-xt-description = Surge XT est un synthétiseur hybride gratuit et open source. RABBIT exécute le programme d'installation de l'éditeur pour vous — il installe les formats VST3, CLAP, AU (macOS uniquement) et autonome à l'échelle du système, afin que REAPER et les autres DAW puissent charger Surge XT. RABBIT suit le canal « nightly » continu, car la dernière version stable (1.3.4) date d'août 2024 et le projet est désormais distribué quasi exclusivement via les nightlies. Installations standard de REAPER uniquement : les données d'usine se trouvent en dehors de tout dossier REAPER portable.
# Lo-lo's four instruments, VST3 plug-ins that RABBIT installs into REAPER's
# own plug-in folder. Keep UserPlugins\FX and
# C:\Program Files\Common Files\VST3 as they are: they are the folders'
# names, and the full path works in File Explorer's address bar in any
# language. Where Windows in your language shows Program Files and Common
# Files under other names, add the path as File Explorer shows it in
# parentheses.
package-ljuno-116-description = LJuno-116 est un synthétiseur polyphonique open source de Lo-lo, conçu pour être joué et réglé avec un lecteur d'écran. RABBIT l'installe dans le dossier de plug-ins de REAPER lui-même, UserPlugins\FX, sans droits d'administrateur, et REAPER le trouve à son prochain démarrage ou après une nouvelle analyse des plug-ins. Si vous l'avez copié vous-même dans C:\Program Files\Common Files\VST3 (dans l'Explorateur de fichiers : Programmes\Fichiers communs\VST3), supprimez cette copie pour que REAPER n'utilise que celle que RABBIT tient à jour. Windows uniquement.
package-lb-303-description = LB-303 est un synthétiseur de lignes de basse open source de Lo-lo avec un séquenceur pas à pas, conçu pour être joué et réglé avec un lecteur d'écran. RABBIT l'installe dans le dossier de plug-ins de REAPER lui-même, UserPlugins\FX, sans droits d'administrateur, et REAPER le trouve à son prochain démarrage ou après une nouvelle analyse des plug-ins. Si vous l'avez copié vous-même dans C:\Program Files\Common Files\VST3 (dans l'Explorateur de fichiers : Programmes\Fichiers communs\VST3), supprimez cette copie pour que REAPER n'utilise que celle que RABBIT tient à jour. Windows uniquement.
package-lr-608-description = LR-608 est une boîte à rythmes open source de Lo-lo, conçue pour être jouée au clavier avec un lecteur d'écran. Elle synthétise ses sons et n'a donc besoin d'aucune banque d'échantillons. RABBIT l'installe dans le dossier de plug-ins de REAPER lui-même, UserPlugins\FX, sans droits d'administrateur, et REAPER la trouve à son prochain démarrage ou après une nouvelle analyse des plug-ins. Si vous l'avez copiée vous-même dans C:\Program Files\Common Files\VST3 (dans l'Explorateur de fichiers : Programmes\Fichiers communs\VST3), supprimez cette copie pour que REAPER n'utilise que celle que RABBIT tient à jour. Windows uniquement.
package-lsampler-24-description = LSampler-24 est un échantillonneur de Lo-lo avec 24 emplacements pour vos propres échantillons, conçu pour être utilisé avec un lecteur d'écran. RABBIT l'installe dans le dossier de plug-ins de REAPER lui-même, UserPlugins\FX, sans droits d'administrateur, et REAPER le trouve à son prochain démarrage ou après une nouvelle analyse des plug-ins. Si vous l'avez copié vous-même dans C:\Program Files\Common Files\VST3 (dans l'Explorateur de fichiers : Programmes\Fichiers communs\VST3), supprimez cette copie pour que REAPER n'utilise que celle que RABBIT tient à jour. Windows uniquement.
package-app2clap-description = app2clap est un plug-in CLAP pour Windows qui capture l'audio d'autres applications et l'amène dans REAPER sous forme de plug-in à insérer sur une piste — pratique pour enregistrer ou traiter le son d'un navigateur, d'un lecteur multimédia ou d'un autre programme. RABBIT télécharge la dernière version et installe app2clap.clap dans le dossier de plug-ins de REAPER lui-même, UserPlugins\FX, de sorte qu'il ne sert qu'à ce REAPER. Aucun droit d'administrateur n'est nécessaire, et cela fonctionne aussi avec un REAPER portable. Windows uniquement.
package-lreaparams-description = LReaParams est une extension open source de Lo-lo qui permet de contrôler au clavier, de façon accessible, les paramètres des FX et des pistes dans REAPER, et elle fonctionne avec OSARA. Windows uniquement.
package-langpack-es-description = Traduit l'interface de REAPER en espagnol, y compris l'extension SWS. Maintenu par Javier Robledo pour la communauté hispanophone de REAPER et publié sur reaperespa.com — RABBIT télécharge la version actuelle et l'installe sous le nom es_ES.ReaperLangPack. OSARA se sert de ce nom de fichier pour choisir sa propre traduction espagnole. Après l'installation, sélectionnez la langue dans les préférences de REAPER (ou laissez RABBIT s'en charger).
package-langpack-de-description = Traduit l'interface de REAPER en allemand, y compris l'extension SWS. Maintenu par MrData et publié dans le Stash de REAPER — RABBIT télécharge la version actuelle et l'installe sous le nom de_DE.ReaperLangPack. Après l'installation, sélectionnez la langue dans les préférences de REAPER (ou laissez RABBIT s'en charger).
package-langpack-fr-description = Traduit l'interface de REAPER en français, y compris l'extension SWS. Maintenu par Lee Julien et Pierre-Marie Curt pour ReaperAccessible, la communauté francophone des utilisateurs de lecteurs d'écran, et publié dans leur dépôt ReaPack — RABBIT télécharge la version actuelle et l'installe sous le nom fr_CA.ReaperLangPack. OSARA lit ce nom de fichier pour choisir sa propre traduction française : sa traduction fr_CA est presque complète, là où sa traduction fr_FR laisse encore environ un tiers des messages en anglais ; le pack, lui, est le même français standard dans les deux cas. Après l'installation, sélectionnez la langue dans les préférences de REAPER (ou laissez RABBIT s'en charger).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = RABBIT ne peut pas encore désinstaller ce paquet.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Supprime l'extension SWS.
uninstall-summary-sws-keeps = Vos grooves et vos réglages SWS, comme vos actions cycliques, sont conservés.
uninstall-summary-reapack-removes = Supprime l'extension ReaPack.
uninstall-summary-reapack-keeps = Vos réglages et vos dépôts ReaPack sont conservés, de même que la liste de ce que ReaPack a installé.
uninstall-summary-reakontrol-removes = Supprime l'extension ReaKontrol.
uninstall-summary-reakontrol-keeps = Vos mappages FX sont conservés, et RABBIT ne touche pas à vos logiciels Native Instruments.
uninstall-summary-ffmpeg-removes = Supprime les fichiers FFmpeg que RABBIT a installés dans UserPlugins. REAPER ne peut alors plus utiliser FFmpeg pour ouvrir des fichiers multimédias ou en faire le rendu.
uninstall-summary-ffmpeg-keeps = Les fichiers FFmpeg dont RABBIT n'a aucune trace sont conservés, comme ceux laissés par des versions plus anciennes.
uninstall-summary-app2clap-removes = Supprime app2clap du dossier UserPlugins\FX de REAPER.
# The same on the standard REAPER, which also removes any app2clap in the
# per-user CLAP folder, whoever put it there: earlier versions of RABBIT
# installed it there. Only when there is such a copy, a sentence of its own
# says that every program using CLAP plug-ins loses it, so this one doesn't.
# Keep UserPlugins\FX as it is: it is the folder's name.
uninstall-summary-app2clap-removes-standard = Supprime app2clap du dossier UserPlugins\FX de REAPER, ainsi que tout app2clap présent dans votre dossier CLAP, par exemple celui qu'une version antérieure de RABBIT y a installé.
uninstall-summary-app2clap-keeps = Vos autres plug-ins sont conservés.
uninstall-summary-lreaparams-removes = Supprime l'extension LReaParams.
uninstall-summary-lreaparams-keeps = Vos réglages et vos profils LReaParams sont conservés, de même que son guide.
# Lo-lo's instruments. Keep UserPlugins\FX as it is: it is the folder's name.
uninstall-summary-ljuno-116-removes = Supprime LJuno-116 du dossier UserPlugins\FX de REAPER.
uninstall-summary-ljuno-116-keeps = Vos presets LJuno-116, dans le dossier LJuno-116 de vos Documents, sont conservés, de même que vos autres plug-ins.
uninstall-summary-lb-303-removes = Supprime LB-303 du dossier UserPlugins\FX de REAPER.
uninstall-summary-lb-303-keeps = Vos presets LB-303, dans le dossier LB-303 de vos Documents, sont conservés, de même que vos autres plug-ins.
uninstall-summary-lr-608-removes = Supprime LR-608 du dossier UserPlugins\FX de REAPER.
uninstall-summary-lr-608-keeps = Vos kits et presets LR-608, dans le dossier LR-608 de vos Documents, sont conservés, de même que vos autres plug-ins.
uninstall-summary-lsampler-24-removes = Supprime LSampler-24 du dossier UserPlugins\FX de REAPER.
uninstall-summary-lsampler-24-keeps = Votre bibliothèque LSampler-24 d'échantillons, d'emplacements et de banques, dans le dossier LSampler-24 de vos Documents, est conservée, de même que l'audio extrait par LSampler-24 et vos autres plug-ins.
uninstall-summary-langpack-removes = Supprime le pack de langue que RABBIT a installé. Si REAPER l'utilise, RABBIT remet REAPER en anglais.
uninstall-summary-langpack-keeps = Les packs de langue que vous avez ajoutés vous-même sont conservés.
uninstall-summary-osara-removes = Supprime l'extension OSARA, ses traductions et la copie de ses raccourcis clavier dans le dossier KeyMaps.
uninstall-summary-osara-removes-windows = Supprime l'extension OSARA, ses traductions, son programme de désinstallation et la copie de ses raccourcis clavier dans le dossier KeyMaps. Si Windows affiche cet OSARA dans la liste des programmes installés, RABBIT l'en retire.
uninstall-summary-osara-keeps = Vos raccourcis clavier restent tels quels, de même que les réglages d'OSARA et les sauvegardes de vos raccourcis clavier faites par OSARA.
uninstall-summary-surge-removes = Le programme de désinstallation de Surge XT le supprime pour tous les programmes de musique de cet ordinateur. Une sauvegarde ne peut pas le restaurer.
uninstall-summary-surge-keeps = Vos propres patches, dans le dossier Surge XT de vos Documents, sont conservés.
uninstall-summary-reaper-windows-removes = Le programme de désinstallation de REAPER supprime le programme. Il supprime aussi les associations de fichiers de REAPER et ReaRoute pour tous les REAPER de cet ordinateur.
uninstall-summary-reaper-keeps = Vos réglages, extensions et projets sont conservés.
# The JAWS scripts: two plug-in files in REAPER's UserPlugins folder, and
# script files their installer adds to JAWS's settings folder, which RABBIT
# removes only while they are as the installer left them. Keep
# %APPDATA%\Freedom Scientific\JAWS as it is.
uninstall-summary-jaws-scripts-removes = Supprime les deux fichiers de plug-in des scripts du dossier UserPlugins de REAPER, ainsi que les fichiers de scripts que RABBIT a vu leur programme d'installation ajouter au dossier de paramètres de JAWS.
uninstall-summary-jaws-scripts-keeps = Les fichiers de scripts dont RABBIT ne peut pas être sûr qu'ils viennent du programme d'installation, et ceux qui ont changé depuis, comme vos propres affectations de touches, restent dans le dossier de paramètres de JAWS, %APPDATA%\Freedom Scientific\JAWS.
# The same two sentences on macOS, where Surge XT has no uninstaller of its
# own and REAPER is an app. Name the Trash and the Library folder as Finder
# does in your language.
uninstall-summary-surge-macos-removes = Supprime les apps et les plug-ins de Surge XT, ainsi que ses données d'usine dans le dossier Bibliothèque. Une sauvegarde ne peut pas les restaurer.
uninstall-summary-reaper-macos-removes = Place l'app REAPER dans la Corbeille.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = vos grooves
uninstall-kept-sws-settings = réglages de SWS
uninstall-kept-sws-cycle-actions = vos actions cycliques
uninstall-kept-reapack-settings = réglages de ReaPack et vos dépôts
uninstall-kept-reapack-data = la liste de ce que ReaPack a installé, et son cache de téléchargement
uninstall-kept-reakontrol-fx-maps = vos mappages FX
uninstall-kept-lreaparams-data = vos réglages et vos profils LReaParams, et son guide
uninstall-kept-ljuno-116-presets = vos presets LJuno-116
uninstall-kept-lb-303-presets = vos presets LB-303
uninstall-kept-lr-608-presets = vos kits et presets LR-608
uninstall-kept-lsampler-24-library = votre bibliothèque LSampler-24 d'échantillons, d'emplacements et de banques
uninstall-kept-lsampler-24-extracted-audio = l'audio que LSampler-24 a extrait de vidéos et de fichiers multicanaux
uninstall-kept-osara-key-map = vos raccourcis clavier
uninstall-kept-osara-replaced-key-map = vos raccourcis clavier d'avant leur remplacement par OSARA
uninstall-kept-osara-merged-key-map = vos raccourcis clavier d'avant l'ajout de ceux d'OSARA
uninstall-kept-osara-settings = les réglages d'OSARA, dans un fichier que partagent toutes les extensions
uninstall-kept-surge-user-data = vos patches et tout ce que vous avez enregistré avec Surge XT
uninstall-kept-reaper-resource = le dossier de ressources de REAPER, avec vos réglages, extensions et raccourcis clavier
# On macOS, in the Library folder of your home folder.
uninstall-kept-surge-settings-macos = vos réglages de Surge XT
# On macOS, in the Library folder of your home folder: a copy of Surge XT's
# factory data put there by hand, which Surge XT reads instead of the one it
# installs for everyone. Say "factory data" as in
# uninstall-summary-surge-macos-removes.
uninstall-kept-surge-factory-copy-macos = votre propre copie des données d'usine de Surge XT

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Si vous utilisez les raccourcis clavier d'OSARA, certains d'entre eux lancent des actions SWS. Ces raccourcis cessent de fonctionner jusqu'à ce que vous réinstalliez SWS.
uninstall-warning-sws-reapack-scripts = De nombreux scripts REAPER, dont des scripts d'accessibilité, ont besoin de SWS. Les scripts que vous avez installés avec ReaPack risquent de ne plus fonctionner.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Les scripts et extensions que vous avez installés avec ReaPack sont conservés, mais plus rien ne les tient à jour.
uninstall-note-surge-fonts-stay = Les polices installées par Surge XT sont conservées : son programme d'installation indique qu'elles ne doivent jamais être supprimées.
# A note on the JAWS script files that stay after the JAWS scripts are
# uninstalled, and how to remove them by hand. Keep %APPDATA%, the folder
# and file names, and "reaper" (the start of the file names) exactly as they
# are. For the example language folder, use JAWS's folder for your language:
# enu for English, deu for German, esn for Spanish, fra for French, ita for
# Italian.
uninstall-note-jaws-scripts-by-hand = Pour supprimer les fichiers de scripts qui restent, supprimez les fichiers dont le nom commence par reaper des dossiers de langue de %APPDATA%\Freedom Scientific\JAWS, par exemple 2025\Settings\fra. Copiez d'abord reaper.jkm, reaper.jcf et reaper.ini ailleurs si vous voulez garder vos affectations de touches et vos réglages.

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
uninstall-by-hand-jaws-scripts = Pour supprimer les scripts JAWS à la main :
    1. Fermez REAPER.
    2. Supprimez Reaper_ComAccess32.dll et Reaper_ComAccess64.dll de %APPDATA%\REAPER\UserPlugins.
    3. Supprimez le dossier %ProgramFiles(x86)%\Reaper_JawsScripts. Windows demande l'autorisation d'un administrateur.
    4. Pour supprimer aussi les fichiers de scripts, supprimez les fichiers dont le nom commence par reaper des dossiers de langue de %APPDATA%\Freedom Scientific\JAWS, par exemple 2025\Settings\fra. Copiez d'abord reaper.jkm, reaper.jcf et reaper.ini ailleurs si vous voulez garder vos affectations de touches et vos réglages.
uninstall-manual-reaper-portable = RABBIT ne supprime pas un REAPER portable : les fichiers de REAPER et les vôtres partagent son dossier. Pour le supprimer à la main :
    1. Fermez REAPER.
    2. Copiez hors du dossier du REAPER portable tout ce que vous voulez garder, comme les projets que vous y avez enregistrés, vos raccourcis clavier et vos réglages.
    3. Supprimez le dossier du REAPER portable.
    Les sauvegardes que RABBIT a faites pour ce REAPER se trouvent aussi dans ce dossier, sous RABBIT\backups, et disparaissent avec lui.

# Why RABBIT didn't start a program's own uninstaller, or didn't remove a
# package's uninstaller folder and registry entry. RABBIT does both with
# administrator rights. Nothing has changed at that point. RABBIT shows one
# of these, then uninstall-vendor-refused-nothing-changed, or, for the folder
# and registry entry, the steps to remove the package by hand.
# { $package } is the package's name, such as Surge XT, REAPER or
# JAWS-for-REAPER scripts.
# { $path } is a file, a folder or a value as Windows holds it; keep it as it
# is.
uninstall-vendor-refused-not-listed = Windows ne compte pas { $package } parmi les programmes installés pour tous les utilisateurs, donc RABBIT ne trouve pas son programme de désinstallation.
uninstall-vendor-refused-unclear-command = RABBIT ne sait pas quel programme Windows indique comme programme de désinstallation de { $package } : { $path }
uninstall-vendor-refused-not-a-full-path = Windows indique le programme de désinstallation de { $package } par un chemin auquel RABBIT ne fait pas confiance : { $path }
uninstall-vendor-refused-unknown-folder = RABBIT ne trouve pas un dossier dont il a besoin pour vérifier le programme de désinstallation de { $package }.
uninstall-vendor-refused-wrong-folder = Le programme de désinstallation de { $package } n'est pas dans le dossier où il doit être : { $path }
uninstall-vendor-refused-foreign-file = À côté du programme de désinstallation de { $package } se trouve un fichier qui n'a rien à y faire : { $path }
uninstall-vendor-refused-folder-or-link = À côté du programme de désinstallation de { $package } se trouve un dossier ou un lien qui n'a rien à y faire : { $path }
uninstall-vendor-refused-untrusted-owner = À côté du programme de désinstallation de { $package } se trouve un fichier qu'aucun administrateur n'y a mis : { $path }
uninstall-vendor-refused-install-location-missing = Windows n'indique pas dans quel dossier { $package } est installé.
uninstall-vendor-refused-install-location-mismatch = D'après Windows, { $package } est installé dans { $path }, et non dans le REAPER que vous avez choisi.
uninstall-vendor-refused-not-in-install-folder = Le programme de désinstallation de { $package } n'est pas dans son dossier de programme : { $path }
uninstall-vendor-refused-not-below-program-files = { $package } n'est pas installé dans un dossier à lui sous Programmes (Program Files) : { $path }
uninstall-vendor-refused-copy-differs = La copie que RABBIT a faite du programme de désinstallation de { $package } n'est pas identique à l'original.
uninstall-vendor-refused-unreadable = RABBIT n'a pas pu lire { $path } pour vérifier le programme de désinstallation de { $package }.
uninstall-vendor-refused-not-a-vendor-plan = RABBIT n'a aucun programme de désinstallation de { $package } qu'il ait le droit de lancer.
uninstall-vendor-refused-unsupported = RABBIT ne peut lancer le programme de désinstallation de { $package } que sous Windows.
# { $path } is the name of a value in the package's registry entry.
uninstall-vendor-refused-unexpected-registry-value = L'entrée de registre de { $package } contient une valeur à laquelle RABBIT ne s'attend pas : { $path }
# { $path } is the text of a value in the package's registry entry.
uninstall-vendor-refused-registry-value-mismatch = L'entrée de registre de { $package } pointe vers un emplacement auquel RABBIT ne s'attend pas : { $path }
# The package's registry entry has subkeys, which its installer doesn't write.
uninstall-vendor-refused-registry-key-has-subkeys = L'entrée de registre de { $package } contient plus que ce à quoi RABBIT s'attend.
# { $path } is the package's uninstaller folder in Program Files.
uninstall-vendor-refused-untrusted-folder = Le dossier de { $package } n'a pas été créé par un administrateur : { $path }
# Follows each of the messages above. { $package } as above.
uninstall-vendor-refused-nothing-changed = Rien n'a été modifié. Vous pouvez désinstaller { $package } dans les Paramètres de Windows, sous Applications, Applications installées.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (non disponible : { $reason })
wizard-package-row-unavailable-portable = cible REAPER portable
wizard-package-row-unavailable-version-check = échec de la vérification de version en ligne
# Why an Install or update row can't be ticked, for a package RABBIT leaves
# to ReaPack: ReaPack's records say ReaPack installed it, or RABBIT couldn't
# read those records to tell. Word them as
# wizard-uninstall-row-unavailable-reapack and its -check-failed partner do.
wizard-package-row-unavailable-reapack = géré par ReaPack
wizard-package-row-unavailable-reapack-check-failed = les registres de ReaPack n'ont pas pu être vérifiés
# The sentence the details of such a row add. ReaPack's records can name a
# package whose file is gone, so it doesn't say a copy is there. $package is
# the localized package name, $reapack_package the name ReaPack knows the
# package by, and $error an error message.
wizard-package-details-reapack = D'après les registres de ReaPack, ReaPack a installé { $package } en tant que { $reapack_package }, RABBIT le laisse donc à ReaPack. Installez-le, mettez-le à jour ou supprimez-le dans ReaPack.
wizard-package-details-reapack-check-failed = RABBIT n'a pas pu lire les registres de ReaPack ({ $error }) et ne peut donc pas savoir si ReaPack a installé { $package } ; il ne l'installe pas, ne le met pas à jour et ne le supprime pas.
# A sentence the package details add on a portable REAPER when the per-user
# CLAP folder holds a copy of a package that now installs into each REAPER's
# own UserPlugins\FX folder, such as app2clap. Every REAPER on the computer
# loads the plug-ins in the CLAP folder, so this one would load two copies.
# Updating or reinstalling the package in the standard REAPER moves the copy
# out of the CLAP folder. $package is the localized package name.
wizard-package-details-clap-copy-portable = Ce REAPER charge déjà la copie de { $package } de votre dossier CLAP, comme tous les REAPER de cet ordinateur. Si vous l'installez aussi ici, ce REAPER en chargera deux copies. Pour donner à ce REAPER sa propre copie, mettez d'abord à jour ou réinstallez { $package } dans le REAPER standard : la copie du dossier CLAP est alors déplacée dans une sauvegarde.
# A sentence the package details add on a portable REAPER that has its own copy
# of a package already, while the per-user CLAP folder holds another one, such
# as app2clap: every REAPER on the computer loads the plug-ins in the CLAP
# folder, so this one loads two copies. Updating or reinstalling the package
# in the standard REAPER moves the copy out of the CLAP folder. $package is the
# localized package name.
wizard-package-details-clap-copy-portable-installed = Ce REAPER charge deux copies de { $package } : la sienne et celle de votre dossier CLAP, que chargent tous les REAPER de cet ordinateur. Pour retirer celle du dossier CLAP, mettez à jour ou réinstallez { $package } dans le REAPER standard : elle est alors déplacée dans une sauvegarde.
# The same on the standard REAPER: updating or reinstalling the package there
# moves the copy out of the CLAP folder into a backup, so portable REAPERs and
# every other program that loaded it from there lose it. $package is the
# localized package name.
wizard-package-details-clap-copy-standard = Votre dossier CLAP contient une copie de { $package }. Mettre à jour ou réinstaller { $package } ici déplace cette copie dans une sauvegarde, pour que REAPER ne charge pas deux copies. Les REAPER portables et les autres programmes qui utilisent des plug-ins CLAP ne la trouvent alors plus, et RABBIT peut aussi installer { $package } dans chaque REAPER portable.
# Why a package row can't be ticked: there is no version of the package for
# the REAPER at the chosen target. The first names a 32-bit REAPER; the
# second covers any other REAPER a package has no version for, by the type
# of processor it runs on.
wizard-package-row-unavailable-32-bit = aucune version pour REAPER 32 bits
wizard-package-row-unavailable-processor = aucune version pour le type de processeur de ce REAPER
# The same, as a sentence in the row's details. $package is the localized
# package name.
wizard-package-details-unavailable-32-bit = { $package } n'a aucune version compatible avec REAPER 32 bits, RABBIT ne peut donc pas installer ce paquet ici.
wizard-package-details-unavailable-processor = { $package } n'a aucune version compatible avec le type de processeur de ce REAPER, RABBIT ne peut donc pas installer ce paquet ici.

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package } : la vérification de la dernière version a échoué ({ $message }). L'installation ou la mise à jour de ce paquet est désactivée pour cette exécution.

detect-installed = Installé
detect-not-installed = Non installé
detect-version-unknown = Version inconnue
detect-source-receipt = Reçu RABBIT
detect-source-files = Présence de fichiers dans UserPlugins
detect-source-reapack-registry = Registre ReaPack

# $package is the localized package display name.
status-package-installed = { $package } installé

wizard-step-target = Cible
wizard-step-version-check = Vérification des versions
wizard-step-packages = Paquets
wizard-step-reapack-acknowledgement = Don ReaPack
wizard-step-review = Récapitulatif
wizard-step-progress = Progression
wizard-step-done = Terminé

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Précédent
wizard-button-back-mnemonic = P
wizard-button-next = Suivant
wizard-button-next-mnemonic = S
wizard-button-install = Installer
wizard-button-install-mnemonic = I
wizard-button-uninstall = Désinstaller
wizard-button-uninstall-mnemonic = D
# The run button before a restore and before a delete of backups, both
# started from the Backups tab. Their access keys must differ from those of
# Back, Next, Close and Cancel, which sit beside it.
wizard-button-restore = Restaurer
wizard-button-restore-mnemonic = R
wizard-button-delete = Supprimer
wizard-button-delete-mnemonic = U
wizard-button-close = Fermer
wizard-button-close-mnemonic = F
wizard-button-cancel = Annuler
wizard-button-cancel-mnemonic = A

wizard-target-heading = Choisissez une tâche
wizard-target-language-label = Langue
wizard-target-language-restart-note = Changer de langue redémarre RABBIT afin que la nouvelle langue prenne effet.
wizard-locale-name-en-US = Anglais (États-Unis)
wizard-locale-name-de-DE = Allemand (Allemagne)
wizard-locale-name-es-ES = Espagnol (Espagne)
wizard-locale-name-fr-FR = Français (France)
wizard-locale-name-it-IT = Italien (Italie)
wizard-locale-name-pt-BR = Portugais (Brésil)
wizard-target-choice-label = Chemin d'installation
wizard-target-details-label = Détails de la cible
wizard-target-empty = Aucune cible d'installation REAPER n'est sélectionnée.
wizard-target-portable-choice = Créer ou mettre à jour une version portable de REAPER
wizard-target-portable-folder-label = Dossier portable
wizard-target-portable-folder-message = Choisissez un dossier REAPER portable si vous en avez déjà un, ou un dossier vide pour créer une nouvelle version portable.
wizard-target-portable-folder-browse-label = Parcourir…
wizard-target-portable-pending-details = Utilisez le bouton Parcourir pour indiquer l'emplacement d'une version portable existante si vous en avez une, ou pour choisir un dossier vide si vous souhaitez créer une nouvelle version portable de REAPER.
wizard-target-custom-portable-label = Dossier REAPER portable
wizard-target-custom-portable-app-path-label = Chemin de l'application REAPER
wizard-target-custom-portable-path-label = Chemin des ressources portables
wizard-target-custom-portable-version-label = Version de REAPER
wizard-target-custom-portable-writable-label = Accessible en écriture
wizard-target-custom-portable-note = RABBIT créera ici le chemin des ressources REAPER s'il est absent.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } dans { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = Chemin d'installation de REAPER : { $app_path }
    Version : { $version }
    Chemin des ressources : { $path }
    Accessible en écriture : { $writable }

wizard-packages-heading = Choisissez ce que RABBIT doit faire
wizard-packages-list-label = Installer ou mettre à jour : paquets et réglages
wizard-packages-row-checked = Coché
wizard-packages-row-unchecked = Non coché
wizard-packages-choice-shown = Nouveau choix sous la liste : { $choice }.
wizard-packages-choice-hidden = Choix retiré : { $choice }.
wizard-packages-tree-group-label = Paquets
# Group headings in the Install or update and Uninstall lists, beside
# Packages and Language packs. Instruments holds software instruments such as
# Surge XT; Tools holds programs and plug-ins that help with other work, such
# as app2clap. Screen readers read the heading alone, so keep the two clearly
# different from each other.
wizard-instruments-tree-group-label = Instruments
wizard-tools-tree-group-label = Outils
wizard-language-tree-group-label = Packs de langue
wizard-configuration-tree-group-label = Configuration

# The first two of the Packages page's three tabs. Screen readers read the
# selected tab's name when the page opens, so keep them short.
wizard-packages-tab-install = Installer ou mettre à jour
wizard-packages-tab-uninstall = Désinstaller
# The third tab, where RABBIT's backups are restored or deleted. Keep it
# short too.
wizard-packages-tab-backups = Sauvegardes
# macOS only: VoiceOver says this once, the first time the Packages page opens.
# Name the Tab and Shift keys as Apple's page of Mac keyboard shortcuts does,
# and write Control out in full.
wizard-packages-tabs-switch-hint-macos = Appuyez sur Contrôle + Tab ou Contrôle + Maj + Tab pour passer d'un onglet à l'autre.
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Désinstaller : logiciels que RABBIT peut supprimer
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package } : { $action }. Vous avez { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = RABBIT ne peut pas encore le désinstaller
wizard-uninstall-row-unavailable-not-supported = RABBIT ne peut pas le désinstaller
wizard-uninstall-row-unavailable-manual = à supprimer à la main
wizard-uninstall-row-unavailable-reapack = géré par ReaPack
wizard-uninstall-row-unavailable-reapack-check-failed = les registres de ReaPack n'ont pas pu être vérifiés
wizard-uninstall-row-unavailable-no-receipt = RABBIT n'a pas installé cette copie
wizard-uninstall-row-unavailable-no-trusted-uninstaller = aucun désinstalleur auquel RABBIT peut se fier
wizard-uninstall-row-unavailable-unsafe-path = un fichier que RABBIT ne supprime jamais
# macOS: the app has to be moved to the Trash in Finder. Name the Trash and
# Finder as macOS does in your language.
wizard-uninstall-row-unavailable-by-hand-macos = à placer dans la Corbeille depuis le Finder
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack gère { $package } : il a installé { $path } en tant que { $reapack_package }. Désinstallez-le plutôt dans ReaPack.
wizard-uninstall-details-reapack-check-failed = RABBIT n'a pas pu vérifier les registres de ReaPack pour { $path } ({ $reason }), il ne touche donc pas à { $package }.
wizard-uninstall-details-portable = { $package } sert à tous les REAPER de cet ordinateur, RABBIT ne le supprime donc que du REAPER standard, pas d'un REAPER portable.
# A sentence the Uninstall details add on a portable REAPER while the per-user
# CLAP folder holds a copy of the package, such as app2clap, which every REAPER
# on the computer loads: the uninstall removes only this REAPER's own copy, so
# it still loads that one. Uninstalling, updating or reinstalling the package
# in the standard REAPER takes that copy away. $package is the localized
# package name.
wizard-uninstall-details-clap-copy-portable = Après la désinstallation, ce REAPER charge toujours la copie de { $package } de votre dossier CLAP, comme tous les REAPER de cet ordinateur. Cette copie ne disparaît que si vous désinstallez, mettez à jour ou réinstallez { $package } dans le REAPER standard.
wizard-uninstall-details-no-receipt = RABBIT n'a pas installé cette copie de { $package } : c'est très probablement la vôtre, et RABBIT n'y touche pas.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT n'a trouvé aucun désinstalleur de { $package } auquel il peut se fier.
wizard-uninstall-details-unsafe-path = RABBIT ne supprimera pas { $path }. Il préfère ne pas toucher à { $package } plutôt que de n'en supprimer qu'une partie.
# The details text of an Uninstall row whose package RABBIT leaves alone
# because of the uninstaller folder and registry entry its installer left:
# RABBIT doesn't trust what it found there, or finds none of the package's
# files for this account. The package's steps to remove it by hand follow.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-cleanup-refused = RABBIT ne se fie pas à ce qu'il a trouvé dans le dossier du programme de désinstallation de { $package } sous Programmes (Program Files) ou dans son entrée de registre, et ne touche donc pas à { $package }.
wizard-uninstall-details-no-files-for-this-account = RABBIT ne trouve aucun fichier de { $package } pour votre compte. Si un autre compte a installé { $package }, désinstallez-le depuis ce compte ; d'ici là, RABBIT ne touche ni au dossier de son programme de désinstallation sous Programmes (Program Files) ni à son entrée de registre.
# The details text of a macOS app RABBIT can't move to the Trash, by itself or
# with an administrator's password: first why, then how to move it there in
# Finder. $package is the localized package name, $path the app's full macOS
# path, keep it as it is, and $error macOS's own error message. Name Finder,
# the Trash, File > Get Info and Locked as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does: Finder's
# menus show it only as symbols.
wizard-uninstall-details-by-hand-not-movable-macos = Votre compte ne peut pas déplacer { $path } : il appartient à un autre compte ou à macOS, ou il se trouve dans un dossier que vous ne pouvez pas modifier.
wizard-uninstall-details-by-hand-locked-macos = { $path } est verrouillé.
wizard-uninstall-details-by-hand-not-the-app-macos = { $path } n'est pas l'app { $package } que RABBIT attend à cet endroit, RABBIT n'y touche donc pas.
wizard-uninstall-details-by-hand-unchecked-macos = RABBIT n'a pas pu vérifier qui peut déplacer { $path } ({ $error }).
wizard-uninstall-details-by-hand-no-trash-folder-macos = Seul un administrateur peut déplacer { $path }, et votre Corbeille n'est pas un dossier à vous que RABBIT peut vérifier.
wizard-uninstall-details-by-hand-other-disk-macos = Seul un administrateur peut déplacer { $path }, et il se trouve sur un autre disque que votre Corbeille.
wizard-uninstall-details-by-hand-finder-macos = Pour supprimer { $package }, sélectionnez-le dans le Finder et appuyez sur Commande + Supprimer. Le Finder demande le nom et le mot de passe d'un administrateur s'il en a besoin.
wizard-uninstall-details-by-hand-unlock-macos = Pour supprimer { $package }, sélectionnez-le dans le Finder et choisissez Fichier > Lire les informations. Décochez Verrouillé et fermez cette fenêtre, puis appuyez sur Commande + Supprimer. Le Finder demande le nom et le mot de passe d'un administrateur s'il en a besoin. Si Verrouillé n'est pas coché, ou si le Finder ne peut toujours pas le déplacer, il a un verrou que le Finder ne peut pas retirer.
wizard-uninstall-details-by-hand-not-the-app-finder-macos = S'il s'agit bien du { $package } que vous voulez supprimer, sélectionnez-le dans le Finder et appuyez sur Commande + Supprimer.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT déplace les fichiers dans une sauvegarde au lieu de les effacer.
wizard-uninstall-details-dependents-stay = Ces paquets s'installent par-dessus { $package } et restent : { $packages }. Ils fonctionnent à nouveau dès que vous réinstallez { $package }.
wizard-uninstall-details-machine-wide = { $package } sert à tous les REAPER et à tous les autres programmes de cet ordinateur, qui le perdent tous.
wizard-uninstall-details-reapack-owned-extensions = ReaPack a installé ceux-ci, et ils restent sans mises à jour une fois ReaPack supprimé : { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT n'a pas pu lire les registres de ReaPack pour savoir ce qu'il gère encore ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT n'a pas installé cette copie. Il supprime les fichiers que { $package } est connu pour installer.
wizard-uninstall-details-reapack-orphan = ReaPack a autrefois installé { $package }. ReaPack n'est plus là, RABBIT peut donc le supprimer ; les registres de ReaPack à son sujet restent.
# A note on a package whose installer added files outside REAPER's folder,
# when RABBIT has no record of which ones. $package is the localized
# package name.
wizard-uninstall-details-no-vendor-record = RABBIT n'a aucune trace des fichiers que le programme d'installation de { $package } a ajoutés hors du dossier REAPER, ceux-ci restent donc tous.
wizard-uninstall-details-vendor = Windows vous demande l'autorisation avant de supprimer quoi que ce soit.
wizard-uninstall-details-vendor-by-hand = Vous pouvez désinstaller { $package } vous-même dans les Paramètres de Windows, sous Applications, Applications installées.
# The details text of an Uninstall row whose package's installer left an
# uninstaller folder in Program Files and a registry entry, which RABBIT
# removes with administrator rights once the package's files are in the
# backup. It follows wizard-uninstall-details-files, or takes its place
# when none of the package's files are left to move. $package is the
# localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-machine-cleanup = Seul un administrateur peut supprimer le dossier du programme de désinstallation de { $package } sous Programmes (Program Files) et son entrée de registre, donc Windows vous demande l'autorisation avant de supprimer quoi que ce soit. Une sauvegarde ne peut pas les restaurer.
# The details text of an Uninstall row on macOS, by how the package goes:
# deleted for good with an administrator's password; to the Trash, moved as
# you; or into your Trash with an administrator's password, because your
# account may not move it. Name the Trash and Put Back as Finder does in your
# language. The password window may offer Touch ID, so don't say a password
# is the only way. The Trash line follows the package's own sentence, which
# already says that it goes to the Trash, so it only says how to get it back.
# The administrator line follows the package's own sentence, which already
# says a backup can't bring it back, so it doesn't say that again.
wizard-uninstall-details-elevated-macos = macOS demande une seule fois le nom et le mot de passe d'un administrateur, avant de supprimer quoi que ce soit. Pour l'avoir à nouveau, il faudra le réinstaller.
wizard-uninstall-details-trash = Tant que vous ne videz pas la Corbeille, vous pouvez l'en faire glisser pour le récupérer.
wizard-uninstall-details-trash-elevated-macos = Votre compte ne peut pas le déplacer : macOS demande donc une seule fois le nom et le mot de passe d'un administrateur, avant de supprimer quoi que ce soit, puis RABBIT le place dans votre Corbeille. Il garde le propriétaire qu'il avait, le Finder ne propose donc pas « Remettre » pour lui, et vider la Corbeille peut demander le mot de passe d'un administrateur.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = RABBIT n'a rien trouvé qu'il puisse désinstaller dans ce REAPER.
wizard-uninstall-empty-details = Aucun des logiciels que RABBIT sait désinstaller n'est installé dans ce REAPER.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = RABBIT n'a pas pu lire ses registres pour ce REAPER ({ $error }), il ne peut donc pas lister ce qu'il pourrait désinstaller.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Réglages
wizard-uninstall-remove-settings = Supprimer aussi les réglages
wizard-uninstall-settings-note-none-ticked = Cochez un paquet pour voir lesquels de ses réglages RABBIT peut supprimer. Si vous ne cochez pas cette option, les réglages sont conservés.
wizard-uninstall-settings-note-none-removable = Aucun des paquets cochés n'a de réglages que RABBIT peut supprimer.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = Si cette option est cochée, RABBIT supprime aussi ces réglages. Ils vont dans la sauvegarde avec le reste, et la sauvegarde peut donc les restaurer.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package } : { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Désormais disponible sous la liste : { $option }.
wizard-uninstall-settings-unavailable = Désormais indisponible : { $option }.

# The Backups tab. The label right before its list names the tab first, as
# the other tabs' labels do; screen readers read it when focus enters the
# list. $size is a size, one of the size-* messages, such as "742 MB". The
# -empty form is for a list with no backups.
wizard-backups-list-label = Sauvegardes : ce que RABBIT peut restaurer ou supprimer, { $size } au total
wizard-backups-list-label-empty = Sauvegardes : ce que RABBIT peut restaurer ou supprimer
# The one row, without a checkbox, of a Backups list with nothing in it, and
# its details text. On macOS, VoiceOver says the row's sentence after the
# tab's name instead.
wizard-backups-empty = RABBIT n'a aucune sauvegarde pour ce REAPER.
wizard-backups-empty-details = RABBIT fait une sauvegarde quand il désinstalle un paquet, quand une mise à jour remplace les fichiers d'un paquet ou n'en installe plus certains, et quand les raccourcis clavier d'OSARA remplacent les vôtres. Rien de cela n'est encore arrivé dans ce REAPER, ou ses sauvegardes ont été supprimées. Ce que le propre programme d'installation ou de désinstallation d'un paquet, la Corbeille ou le mot de passe d'un administrateur a modifié n'a jamais de sauvegarde.
# Stands in for the Backups list when RABBIT can't read its backups folder.
# $error is the error message, in English.
wizard-backups-unreadable = RABBIT n'a pas pu lire les sauvegardes de ce REAPER.
wizard-backups-unreadable-details = RABBIT n'a pas pu lire son dossier de sauvegardes ({ $error }), il ne peut donc pas lister ce qu'il pourrait restaurer ou supprimer.
# The group headings of the Backups list, rows without a checkbox, then the
# details text each shows. The details say what backups are for, and what
# they never hold.
wizard-backups-group-removed = Logiciels que vous avez désinstallés
wizard-backups-group-removed-details = Quand RABBIT désinstalle un paquet, il déplace les fichiers ici au lieu de les effacer. Chacune de ces sauvegardes est la seule copie. Ce que le propre programme de désinstallation d'un paquet, la Corbeille ou le mot de passe d'un administrateur a supprimé n'est pas ici.
wizard-backups-group-versions = Versions précédentes
wizard-backups-group-versions-details = Quand une mise à jour remplace les fichiers d'un paquet ou n'en installe plus certains, RABBIT garde ici les anciens. Les mises à jour faites par le propre programme d'installation d'un paquet, comme celui de REAPER, n'en gardent aucun.
wizard-backups-group-keymaps = Raccourcis clavier
wizard-backups-group-keymaps-details = Quand les raccourcis clavier d'OSARA remplacent les vôtres, RABBIT garde ici les anciens. En général, seuls les plus anciens sont les vôtres.
wizard-backups-group-other = Autres sauvegardes
wizard-backups-group-other-details = RABBIT ne peut pas les restaurer ici. Vous pouvez les supprimer.
# One row of the Backups list, read as a list row. $backup is what the row
# calls the backup (one of the wizard-backups-name-* messages, or a package's
# name and version), $action is action-restore, action-delete,
# action-delete-only-copy or action-keep, $when one of the
# wizard-backups-when-* messages or backup-date-unknown, and $size a size,
# such as "4 MB".
wizard-backups-row = { $backup } : { $action }. { $when }, { $size }
# The row that stands for every old copy of RABBIT's records, read as a list
# row. $action as above, $count a number, the copies, and $size a size.
wizard-backups-row-records = Anciennes copies des registres de RABBIT : { $action }. { $count ->
    [one] Une copie
   *[other] { $count } copies
}, { $size }
# A backup named in a sentence on Review, so that two backups with the same
# name can be told apart. $backup and $when as in wizard-backups-row.
wizard-backups-row-reference = { $backup } ({ $when })
# Names joined in a sentence: $rest is the names but the last, separated by
# commas, and $last the last one, as in "FFmpeg, ReaPack and SWS".
wizard-backups-list-and = { $rest } et { $last }
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
wizard-backups-name-keymap-own = Vos raccourcis clavier
wizard-backups-name-keymap = Raccourcis clavier
wizard-backups-name-keymap-default = Les touches par défaut de REAPER
wizard-backups-name-keymap-osara = D'anciens raccourcis clavier d'OSARA
# What a row calls the other backups. "RABBIT's records" is RABBIT's list of
# what it installed.
wizard-backups-name-records = Anciennes copies des registres de RABBIT
wizard-backups-name-records-copy = Une copie des registres de RABBIT
wizard-backups-name-not-a-backup = Pas une sauvegarde de RABBIT
wizard-backups-name-deleting = Suppression inachevée
wizard-backups-name-no-manifest = Sauvegarde sans liste de son contenu
wizard-backups-name-damaged = Sauvegarde endommagée
wizard-backups-name-before-restore = Fichiers remplacés par une restauration
wizard-backups-name-update-files = Autres fichiers remplacés par une mise à jour
wizard-backups-name-uninstall-files = Autres fichiers déplacés par une désinstallation
wizard-backups-name-keymap-files = Autres fichiers gardés avec des raccourcis clavier
wizard-backups-name-unknown = Sauvegarde d'une autre version de RABBIT
# When a backup was made, as its row says it after the action. $date is a
# date and time (backup-date-time), and $version a version number, or for
# a package that comes in variants as wizard-backups-version-variant says.
# "Replaced by OSARA's" means by OSARA's key map.
wizard-backups-when-uninstalled = Désinstallé le { $date }
wizard-backups-when-replaced-by = Remplacé par { $version } le { $date }
wizard-backups-when-replaced = Remplacé le { $date }
wizard-backups-when-keymap-osara = Remplacés par ceux d'OSARA le { $date }
wizard-backups-when-keymap-restore = Remplacés par une restauration le { $date }
wizard-backups-when-made = Créée le { $date }
# A date and time, as rows, details and Review say when a backup was made,
# such as "November 9, 2026, 2:05 PM". $year, $month (1 to 12), $day,
# $hour (0 to 23) and $hour12 (1 to 12) are numbers; $minute is two digits,
# such as "05"; $period is am or pm. Use the 24-hour clock with $hour and
# $minute, or the 12-hour one with $hour12, $minute and $period, as your
# language does. Name the month by the select on $month. Sentences put the
# date after a word such as "on" or "from": where that word would change
# with the day, as Italian "l'8" or "dell'11" do, word them so it doesn't.
backup-date-time = { $day ->
    [1] 1er
   *[other] { $day }
} { $month ->
    [1] janvier
    [2] février
    [3] mars
    [4] avril
    [5] mai
    [6] juin
    [7] juillet
    [8] août
    [9] septembre
    [10] octobre
    [11] novembre
   *[12] décembre
} { $year } à { $hour } h { $minute }
# Instead of a row's when-text, for a backup whose date RABBIT can't tell.
backup-date-unknown = Date inconnue
# Sizes, in steps of 1024. $count, $whole and $tenth are numbers. A size in
# GB is $whole and one decimal, $tenth: put your language's decimal mark
# between them.
size-under-kb = moins de 1 Ko
size-kb = { $count } Ko
size-mb = { $count } Mo
size-gb = { $whole },{ $tenth } Go
# Why a Backups row can't be ticked for the choice below the list: the short
# phrase that goes into wizard-package-row-unavailable-suffix at the end of
# the row. $package is the localized package name.
wizard-backups-unavailable-not-here = impossible de la restaurer ici
wizard-backups-unavailable-installed-again = { $package } est de nouveau installé
wizard-backups-unavailable-not-complete = une partie manque dans la sauvegarde
wizard-backups-unavailable-nothing-in-it = aucun de ses fichiers n'est dans cette sauvegarde
wizard-backups-unavailable-version-unknown = RABBIT ne sait pas de quelle version il s'agit
wizard-backups-unavailable-other-language-pack = un autre pack de langue est installé
wizard-backups-unavailable-part-missing = une partie manque
wizard-backups-unavailable-records-unreadable = RABBIT ne peut pas lire ses registres
wizard-backups-unavailable-not-yours = appartient à un autre compte
wizard-backups-unavailable-not-a-backup = RABBIT n'y touche pas
wizard-backups-unavailable-receipt-in-set = contient une copie des registres de RABBIT
wizard-backups-unavailable-file-refused = RABBIT ne remet pas l'un de ses fichiers en place
wizard-backups-unavailable-cannot-restore = RABBIT ne peut pas la restaurer
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
wizard-backups-details-uninstalled = { $backup }, tel qu'il était avant que vous le désinstalliez.
wizard-backups-details-earlier = { $backup }, une version précédente.
wizard-backups-details-keymap-own = Vos raccourcis clavier, d'avant leur remplacement par ceux d'OSARA.
wizard-backups-details-keymap-default = Les touches par défaut de REAPER, d'avant leur remplacement par les raccourcis clavier d'OSARA.
wizard-backups-details-keymap-osara = D'anciens raccourcis clavier d'OSARA, d'avant leur remplacement par des plus récents.
wizard-backups-details-keymap-unknown = Des raccourcis clavier d'avant leur remplacement par ceux d'OSARA. Ce peuvent être les vôtres ou d'anciens raccourcis d'OSARA : RABBIT n'a pas noté lesquels.
wizard-backups-details-keymap-restore = Les raccourcis clavier qu'une restauration a remplacés. Ce peuvent être les vôtres ou ceux d'OSARA.
wizard-backups-details-keymap-other = Des raccourcis clavier que RABBIT a gardés.
wizard-backups-details-records = Des copies de la liste de ce que RABBIT a installé, faites lors d'opérations précédentes. Les supprimer ne change rien à ce qui est installé.
wizard-backups-details-records-copy = Une copie de la liste de ce que RABBIT a installé, faite lors d'une opération précédente.
wizard-backups-details-deleting = La suppression de cette sauvegarde a commencé sans se terminer, une partie a donc peut-être déjà disparu.
wizard-backups-details-not-a-backup = Ce n'est pas une sauvegarde de RABBIT : c'est un lien, ou un dossier dont RABBIT ne donne jamais le nom à une sauvegarde.
wizard-backups-details-no-manifest = Une sauvegarde sans liste de son contenu, comme en laissait un ancien RABBIT arrêté trop tôt.
wizard-backups-details-damaged = Une sauvegarde dont la liste du contenu est illisible.
wizard-backups-details-before-restore = Des fichiers qu'une restauration a remplacés, gardés pour pouvoir l'annuler.
wizard-backups-details-update-files = Des fichiers qu'une mise à jour a remplacés et qu'aucune autre ligne ici ne remet en place.
wizard-backups-details-uninstall-files = Des fichiers qu'une désinstallation a déplacés et qui n'appartiennent à aucun paquet qu'elle a noté.
wizard-backups-details-keymap-files = Des fichiers gardés avec des raccourcis clavier, qu'aucune autre ligne ici ne remet en place.
wizard-backups-details-unknown = Une sauvegarde faite par une autre version de RABBIT, pour une raison que cette version ne connaît pas.
# When it was made.
wizard-backups-details-when-uninstalled = Vous l'avez désinstallé le { $date }.
wizard-backups-details-when-updated-to = Une mise à jour vers { $version } l'a remplacée le { $date }.
wizard-backups-details-when-updated = Une mise à jour l'a remplacée le { $date }.
wizard-backups-details-when-restored-to = Le retour à { $version } l'a remplacée le { $date }.
wizard-backups-details-when-restored = Une restauration l'a remplacée le { $date }.
wizard-backups-details-when-osara = Les raccourcis clavier d'OSARA les ont remplacés le { $date }.
wizard-backups-details-when-keymap-restored = Des raccourcis clavier restaurés les ont remplacés le { $date }.
wizard-backups-details-when-made = RABBIT a fait cette sauvegarde le { $date }.
wizard-backups-details-when-records = La copie la plus récente date du { $date }.
wizard-backups-details-when-unknown = RABBIT ne peut pas dire quand cette sauvegarde a été faite.
# What it holds and how big it is. The settings are named only when the
# uninstall recorded that it moved them; backups from earlier versions of
# RABBIT don't say, so the -older form says "anything else".
wizard-backups-details-holds-with-settings = Cette sauvegarde contient ses fichiers et vos réglages pour { $package }, { $size }.
wizard-backups-details-holds-files = Cette sauvegarde contient ses fichiers, { $size }.
wizard-backups-details-holds-older = Cette sauvegarde contient ses fichiers et tout ce que cette désinstallation a déplacé d'autre, { $size }.
wizard-backups-details-holds-replaced = Cette sauvegarde contient les fichiers de cette version qui ont été remplacés ou ne sont plus installés, { $size }.
wizard-backups-details-holds-keymap = Cette sauvegarde contient les raccourcis clavier, { $size }.
wizard-backups-details-holds-size = Cette sauvegarde occupe { $size }.
wizard-backups-details-holds-records = { $count ->
    [one] Il y a une copie, { $size }.
   *[other] Il y a { $count } copies, { $size } au total.
}
wizard-backups-details-only-copy = C'est la seule copie.
# What restoring does, for a backup that can be restored. Name REAPER's
# Actions list as REAPER does in your language. The -uninstalled-files form
# is for a backup that names no version, which may hold no record of the
# package to put back, so it says nothing about RABBIT's records.
wizard-backups-details-restore-uninstalled = La restauration remet ses fichiers en place, et RABBIT compte de nouveau { $package } comme installé.
wizard-backups-details-restore-uninstalled-files = La restauration remet ses fichiers en place.
wizard-backups-details-restore-stopped-partway = La désinstallation s'est arrêtée en cours de route, certains de ses fichiers ne sont donc que dans cette sauvegarde. La restauration les remet en place.
wizard-backups-details-restore-already-back = { $package } est déjà revenu. La restauration remet ses fichiers en place une nouvelle fois.
wizard-backups-details-restore-earlier = La restauration revient à { $version } et garde la version que vous avez maintenant comme sauvegarde, pour que vous puissiez y revenir. Vos réglages restent tels quels.
# The same for a language pack, which has no settings of its own:
# REAPER's language setting names the pack's file, so it follows the
# earlier version while it names the version you have now.
wizard-backups-details-restore-earlier-language-pack = La restauration revient à { $version } et garde la version que vous avez maintenant comme sauvegarde, pour que vous puissiez y revenir. Si REAPER utilise pour sa langue la version que vous avez maintenant, il utilise celle-ci à la place.
wizard-backups-details-restore-earlier-copy = La restauration remet cette copie en place et garde la version que vous avez maintenant comme sauvegarde, pour que vous puissiez y revenir.
# The same for an earlier version of a package that isn't installed now,
# as after you uninstalled it: restoring installs it again, and there is no
# version to keep as a backup. A language pack's restore leaves REAPER's
# language as it is, so that form says how to choose it; name REAPER's
# preferences as REAPER does in your language. The -copy form is for a
# copy RABBIT didn't install, whose version it doesn't know.
wizard-backups-details-restore-earlier-again = La restauration installe à nouveau { $version } depuis cette sauvegarde, et RABBIT compte { $package } comme installé. Vos réglages restent tels quels.
wizard-backups-details-restore-earlier-again-language-pack = La restauration installe à nouveau { $version } depuis cette sauvegarde, et RABBIT compte { $package } comme installé. La langue de REAPER reste telle quelle : pour utiliser ce pack, choisissez-le dans les préférences de REAPER.
wizard-backups-details-restore-earlier-copy-again = La restauration remet cette copie en place.
wizard-backups-details-restore-earlier-already-back = { $package } est déjà à cette version. La restauration remet ses fichiers en place une nouvelle fois.
wizard-backups-details-restore-keymap = La restauration en refait les raccourcis clavier de REAPER et garde les actuels comme sauvegarde. Les scripts installés depuis restent dans la liste d'actions de REAPER.
wizard-backups-details-restore-default-keys = La restauration ramène les touches propres à REAPER, pas des raccourcis à vous, et garde les raccourcis clavier actuels comme sauvegarde.
# Why it can't be restored, by what RABBIT found. Name the Uninstall tab as
# wizard-packages-tab-uninstall does.
wizard-backups-details-not-here = RABBIT ne peut pas la restaurer ici.
wizard-backups-details-cannot-restore = RABBIT ne peut pas la restaurer.
wizard-backups-details-status-installed-again = { $package } est de nouveau installé. Désinstallez d'abord celui que vous avez, dans l'onglet « Désinstaller », puis restaurez.
wizard-backups-details-status-not-complete = Une partie manque dans la sauvegarde, RABBIT ne peut donc pas revenir à cette version.
wizard-backups-details-status-nothing-in-it = Aucun de ses fichiers n'est dans cette sauvegarde, il n'y a donc rien à restaurer.
wizard-backups-details-status-version-unknown = RABBIT ne sait pas de quelle version il s'agit, il ne peut donc pas y revenir.
wizard-backups-details-status-other-language-pack = Un autre pack de langue est installé, et en installer un supprime l'autre : RABBIT ne ramène donc pas celui-ci tant que l'autre est installé.
wizard-backups-details-status-part-missing = Une partie manque dans la sauvegarde, RABBIT ne peut donc pas la restaurer.
wizard-backups-details-status-records-unreadable = RABBIT ne peut pas lire ses registres, il ne peut donc pas savoir si cette sauvegarde leur correspond.
wizard-backups-details-status-not-yours = Elle appartient à un autre compte, RABBIT ne la restaure donc pas et ne la supprime pas.
wizard-backups-details-status-not-a-backup = RABBIT n'y regarde jamais, ne le restaure pas et ne le supprime pas.
# macOS: how to remove another account's backup by hand. The backups are in
# the hidden Library folder, so the steps go through Finder's Go menu.
# $folder is the folder that holds the backup, a full macOS path, and $name
# the backup's own folder name; keep both as they are. Name Finder, its Go
# menu and the menu's Go to Folder item as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does. Never
# suggest sudo.
wizard-backups-details-not-yours-finder-macos = Pour la supprimer, dans le Finder, choisissez Aller > Aller au dossier, saisissez { $folder }, puis sélectionnez { $name }, appuyez sur Commande + Supprimer et videz la Corbeille. Le Finder demande le nom et le mot de passe d'un administrateur s'il en a besoin.
# What deleting it loses.
wizard-backups-details-delete-uninstalled = Si vous supprimez cette sauvegarde, RABBIT ne peut plus récupérer cette copie.
wizard-backups-details-delete-earlier = Si vous supprimez cette sauvegarde, vous ne pouvez plus revenir à { $version }.
wizard-backups-details-delete-earlier-copy = Si vous supprimez cette sauvegarde, vous ne pouvez plus revenir à cette copie.
wizard-backups-details-delete-keymap = Si vous supprimez cette sauvegarde, RABBIT ne peut plus récupérer ces raccourcis clavier.
wizard-backups-details-delete-already-back = Ses fichiers sont aussi sur le disque, la supprimer ne fait donc rien perdre.
wizard-backups-details-delete-other = Si vous la supprimez, elle disparaît définitivement.
wizard-backups-details-delete-deleting = La supprimer termine cette suppression.
# For a backup RABBIT can only delete together with the whole backup set it
# is in, such as the files of a set that no other row brings back, after
# the sentence above: the other rows of that set, which go with it. They
# come after a colon, so a name like "Your key map" keeps its capital.
# $backups is a list of what they call their backups, joined as
# wizard-backups-list-and joins them, and $count how many there are, a
# number. The -undeletable form names only those RABBIT can't delete, such
# as another account's, so this one can't be deleted either.
wizard-backups-details-delete-only-with = RABBIT ne peut la supprimer qu'avec { $count ->
    [one] une autre sauvegarde
   *[other] { $count } autres sauvegardes
} : { $backups }.
wizard-backups-details-delete-only-with-undeletable = RABBIT ne peut la supprimer qu'avec { $count ->
    [one] une autre sauvegarde
   *[other] { $count } autres sauvegardes
} que RABBIT ne peut pas supprimer : { $backups }.
# The choice below the Backups list: its label, which screen readers read
# with the choice, then the two choices, Restore and Delete. Restore is
# where the tab starts. The choices have no access keys.
wizard-backups-choice-label = Que faire des sauvegardes cochées
wizard-backups-choice-restore = Restaurer
wizard-backups-choice-delete = Supprimer
# The Backups list has one set of ticks for both choices below it, Restore
# and Delete. After a switch, VoiceOver on macOS says how many backups are
# ticked for the new choice. $choice is restore or delete, and $count a
# number.
wizard-backups-ticked-macos = { $choice ->
    [delete] { $count ->
        [0] Aucune sauvegarde cochée pour la suppression.
        [one] Une sauvegarde cochée pour la suppression.
       *[other] { $count } sauvegardes cochées pour la suppression.
    }
   *[restore] { $count ->
        [0] Aucune sauvegarde cochée pour la restauration.
        [one] Une sauvegarde cochée pour la restauration.
       *[other] { $count } sauvegardes cochées pour la restauration.
    }
}
# Said after the sentence above, on macOS, when the switch unticked backups
# the new choice can't do, such as a backup that can't be restored. Review
# also gives it as a note, on every system, until those backups are ticked
# again. $choice is the choice that can't do them, restore or delete, and
# $count a number, never 0.
wizard-backups-unticked = { $choice ->
    [delete] { $count ->
        [one] Une sauvegarde ne peut pas être supprimée, elle n'est donc plus cochée.
       *[other] { $count } sauvegardes ne peuvent pas être supprimées, elles ne sont donc plus cochées.
    }
   *[restore] { $count ->
        [one] Une sauvegarde ne peut pas être restaurée, elle n'est donc plus cochée.
       *[other] { $count } sauvegardes ne peuvent pas être restaurées, elles ne sont donc plus cochées.
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
wizard-restore-refusal-invalid-id = Son nom n'est pas un nom que RABBIT donne à une sauvegarde.
wizard-restore-refusal-not-found = Cette sauvegarde a été supprimée ou n'a jamais existé. Relancez RABBIT pour voir les sauvegardes telles qu'elles sont maintenant.
wizard-restore-refusal-differs-in-case = RABBIT ne trouve cette sauvegarde que sous un nom aux majuscules et minuscules différentes.
wizard-restore-refusal-ends-in-dot-or-space = RABBIT ne trouve cette sauvegarde que sous un nom sans le point ou l'espace final.
wizard-restore-refusal-being-deleted = Sa suppression a commencé sans se terminer, une partie a donc peut-être disparu.
wizard-restore-refusal-is-link = RABBIT n'utilise pas { $path } : c'est un lien, et RABBIT ne suit jamais de lien dans ses sauvegardes.
wizard-restore-refusal-no-manifest = Cette sauvegarde n'indique pas lesquels de ses fichiers appartiennent à { $package }.
wizard-restore-refusal-no-package-records = Cette sauvegarde n'enregistre aucun paquet, RABBIT ne peut donc pas en restaurer { $package } seul.
wizard-restore-refusal-package-not-in-set = Cette sauvegarde ne contient aucun enregistrement de { $package }.
wizard-restore-refusal-no-keymap-in-set = Cette sauvegarde ne contient pas de raccourcis clavier que RABBIT puisse restaurer.
wizard-restore-refusal-damaged-manifest = RABBIT ne peut pas lire la liste de son contenu ({ $detail }).
wizard-restore-refusal-records-only = Cette sauvegarde ne contient qu'une copie des registres de RABBIT, il n'y a donc rien à restaurer.
wizard-restore-refusal-receipt-in-set = Cette sauvegarde contient une copie des registres de RABBIT. Recopier celle-ci annulerait toutes les modifications faites depuis, RABBIT ne restaure donc pas cette sauvegarde.
wizard-restore-refusal-reinstalled-since = { $package } est de nouveau installé, une copie installée après cette sauvegarde. Désinstallez d'abord celle que vous avez, dans l'onglet « Désinstaller », puis restaurez.
wizard-restore-refusal-nothing-of-it-in-set = Aucun des fichiers de { $packages } n'y figure, il n'y a donc rien à restaurer.
# Instead of the sentence above, when nothing in a backup of earlier
# versions belongs to a package.
wizard-restore-refusal-nothing-of-it-in-set-none = Rien de ce que contient cette sauvegarde n'appartient à un paquet que RABBIT peut remettre en place.
wizard-restore-refusal-part-missing = Une partie manque : sa copie de { $path } a disparu, ou n'est pas le fichier enregistré.
# Not complete: a file of that version isn't in the backup, and RABBIT's
# records don't list it on disk with that version's content. A file that
# is on disk but that RABBIT no longer keeps track of, such as one an
# update couldn't move, doesn't count. $path is the file's path.
wizard-restore-refusal-not-complete = Cette sauvegarde n'est pas complète : { $path } n'est pas dans la sauvegarde, et les registres de RABBIT ne l'indiquent pas sur le disque tel que cette version l'avait.
wizard-restore-refusal-version-unknown = RABBIT ne sait pas quelle version de { $package } cette sauvegarde contient.
wizard-restore-refusal-other-language-pack-installed = RABBIT ne ramène pas { $package } tant que ceci est installé : { $packages }. En installer un supprime l'autre.
wizard-restore-refusal-target-refused = RABBIT ne remet pas { $path } en place : { $reason }.
wizard-restore-refusal-newer-file-refused = Revenir en arrière déplacerait { $path }, un fichier de la version plus récente de { $package }, dans une sauvegarde d'où il ne pourrait pas revenir : { $reason }.
wizard-restore-refusal-earlier-file-refused = Son enregistrement de cette version de { $package } mentionne { $path }, un emplacement où RABBIT n'installe jamais rien : { $reason }.
wizard-restore-refusal-receipt-unreadable = RABBIT ne peut pas lire ses registres ({ $detail }), il ne peut donc pas savoir si cette sauvegarde leur correspond.
wizard-restore-refusal-reaper-running = REAPER est ouvert. Fermez-le, puis réessayez.
# The same, for packages every REAPER on the computer can use, such as
# the JAWS scripts. $packages is a list of localized package names.
wizard-restore-refusal-reaper-running-every = Un REAPER est ouvert. Fermez toutes les copies de REAPER sur cet ordinateur, y compris les portables, puis réessayez. Tout REAPER peut utiliser { $packages }.
wizard-restore-refusal-keymap-needs-reaper-closed = Cette sauvegarde remet en place les raccourcis clavier de REAPER, ce que RABBIT ne fait que lorsque REAPER est fermé.
wizard-restore-refusal-program-running = Fermez ces programmes, puis réessayez : { $programs }.
wizard-restore-refusal-not-writable = RABBIT ne peut pas écrire là où la restauration remet les fichiers, dans { $path } ({ $detail }).
wizard-restore-refusal-lock-held = Un autre RABBIT travaille sur ce REAPER. Attendez qu'il ait terminé.
# sudo and root are never translated.
wizard-restore-refusal-as-root = RABBIT s'exécute en tant que root, comme sous sudo, et ne restaure pas de sauvegarde ainsi. Quittez RABBIT et relancez-le avec votre propre compte.
wizard-restore-refusal-not-yours = Cette sauvegarde appartient à un autre compte, RABBIT ne la restaure donc pas.
wizard-restore-refusal-changed-while-read = Cette sauvegarde a changé pendant que RABBIT la lisait, comme quand un autre RABBIT était encore en train de l'écrire.
wizard-restore-refusal-changed-since-listed = Cette sauvegarde a changé depuis que RABBIT l'a listée. Relancez RABBIT pour voir les sauvegardes telles qu'elles sont maintenant.
# Why a restore won't put a file back at a place: the phrase that ends
# wizard-restore-refusal-target-refused and the two after it. Start in lower
# case where your language allows. $path is a file or folder, keep it as it
# is; $name a file or folder name; $detail an error message in English.
wizard-restore-target-rabbit-folder = RABBIT ne remet jamais de fichier dans son propre dossier
wizard-restore-target-leads-into-rabbit-folder = le chemin qui y mène passe par le propre dossier de RABBIT
wizard-restore-target-name-not-as-written = le nom { $name } sur le chemin peut ouvrir un autre fichier ou dossier que celui qu'il désigne
wizard-restore-target-place-is-link = c'est un lien, et RABBIT ne remplace jamais un lien
wizard-restore-target-through-link = le chemin passe par { $path }, un lien, et RABBIT ne suit jamais de lien
wizard-restore-target-leads-outside-resource-folder = le chemin sort du dossier de ressources, { $path }
wizard-restore-target-leads-outside = le chemin sort de { $path }
wizard-restore-target-outside-allowed-folders = l'emplacement est hors du dossier de ressources de REAPER et de tout autre dossier où RABBIT installe
wizard-restore-target-not-plain-path = ce n'est pas un simple chemin dans le dossier de ressources
wizard-restore-target-guard-folder-gone = le dossier où il retourne, { $path }, n'existe plus
wizard-restore-target-could-not-check = RABBIT n'a pas pu vérifier { $path } ({ $detail })
wizard-restore-target-two-files-for-one-place = la sauvegarde contient plus d'un fichier pour cet emplacement
wizard-restore-target-outside-without-manifest = la sauvegarde n'indique pas où va ce fichier venu d'en dehors du dossier de ressources
wizard-restore-target-copy-outside-set = la liste de la sauvegarde place sa copie en dehors de la sauvegarde
wizard-restore-target-keymap-among-version-files = ce sont les raccourcis clavier de REAPER, que RABBIT ne remet en place que comme raccourcis clavier à part entière
# Why RABBIT doesn't delete a backup, or any of them, a sentence or two
# each, as the Done page and Show details say it. Those about one backup
# call it "it": the Done page puts them after "RABBIT can't delete
# <backup>.". $path is a folder as the computer writes it, keep it as it
# is, $package a localized package name, and $detail an error message in
# English. sudo and root are never translated.
wizard-delete-refusal-invalid-id = Son nom n'est pas un nom que RABBIT donne à une sauvegarde.
wizard-delete-refusal-differs-in-case = RABBIT ne la trouve que sous un nom aux majuscules et minuscules différentes.
wizard-delete-refusal-ends-in-dot-or-space = RABBIT ne la trouve que sous un nom sans le point ou l'espace final.
wizard-delete-refusal-is-link = RABBIT n'utilise pas { $path } : c'est un lien, et RABBIT ne suit jamais de lien dans ses sauvegardes.
wizard-delete-refusal-not-yours = Elle appartient à un autre compte, RABBIT ne la supprime donc pas.
wizard-delete-refusal-changed-since-listed = Elle a changé depuis que RABBIT l'a listée. Relancez RABBIT pour voir les sauvegardes telles qu'elles sont maintenant.
wizard-delete-refusal-lock-held = Un autre RABBIT travaille sur ce REAPER. Attendez qu'il ait terminé, puis relancez RABBIT.
wizard-delete-refusal-as-root = RABBIT s'exécute en tant que root, comme sous sudo, et ne supprime pas de sauvegarde ainsi. Quittez RABBIT et relancez-le avec votre propre compte.
# Why RABBIT doesn't delete one package or the key map out of the backup set
# it is kept in on its own, as above. They call the package or key map
# "it", as the Done page puts them after its name, and that set "the backup
# it is kept in", or "was kept in" where the set no longer holds it.
wizard-delete-refusal-being-deleted = La suppression de la sauvegarde où elle est conservée a commencé sans se terminer, RABBIT ne peut donc que terminer la suppression de la sauvegarde entière.
wizard-delete-refusal-no-manifest = La sauvegarde où elle est conservée n'indique pas lesquels de ses fichiers appartiennent à { $package }, RABBIT ne les supprime donc pas séparément.
wizard-delete-refusal-damaged-manifest = RABBIT ne peut pas lire la liste du contenu de la sauvegarde où elle est conservée ({ $detail }), il ne supprime donc rien de cette sauvegarde.
wizard-delete-refusal-no-keymap-in-set = La sauvegarde où elle est conservée ne contient pas de raccourcis clavier que RABBIT puisse supprimer séparément.
wizard-delete-refusal-no-package-records = La sauvegarde où elle est conservée n'enregistre aucun paquet, RABBIT ne peut donc pas savoir lesquels de ses fichiers appartiennent à { $package }.
wizard-delete-refusal-package-not-in-set = La sauvegarde où elle était conservée ne contient plus de copie de { $package } que RABBIT puisse supprimer.
# Instead of wizard-delete-refusal-not-yours, when what belongs to another
# account is an unfinished deletion of the backup set the package, key map
# or files are kept in, which has the same name as that set.
wizard-delete-refusal-not-yours-earlier-deletion = Une suppression inachevée de la sauvegarde où elle est conservée appartient à un autre compte, RABBIT ne modifie donc pas cette sauvegarde.

# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = Indisponible : { $package } doit être installé.
wizard-configuration-row-already-applied = Déjà appliqué sur cette cible REAPER.
# Short status tag appended in parentheses to a configuration row's tree label
# when the row isn't actionable. Kept terse so the tree label stays readable;
# the longer sentence in `wizard-configuration-row-unavailable` /
# `wizard-configuration-row-already-applied` is still surfaced in the details
# pane and as the row's accessible reason.
# $reason is one of the "wizard-configuration-row-status-*" strings below.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package is the localized name of the dependency package.
wizard-configuration-row-status-requires = nécessite { $package }
wizard-configuration-row-status-already-applied = déjà appliqué
# Nomme la dependance d'une etape de configuration que n'importe lequel de
# plusieurs paquets satisfait.
config-dependency-language-pack = un pack de langue
config-reapack-reaper-accessibility-name = Ajouter le dépôt ReaPack « REAPER Accessibility » de Toni
config-reapack-reaper-accessibility-description = Ajoute le dépôt ReaPack « REAPER Accessibility » de Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour obtenir des scripts et des extensions accessibles supplémentaires.
config-reapack-reaper-accessible-fr-name = Ajouter le dépôt ReaPack « REAPER Accessible (FR) »
config-reapack-reaper-accessible-fr-description = Ajoute le dépôt ReaPack francophone de REAPER Accessible (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour accéder aux ressources francophones de REAPER Accessible.
config-reapack-reaper-accessible-en-name = Ajouter le dépôt ReaPack « REAPER Accessible (EN) »
config-reapack-reaper-accessible-en-description = Ajoute le dépôt ReaPack anglophone de REAPER Accessible (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour accéder aux ressources anglophones de REAPER Accessible.
# ReaWarp est l'ensemble de scripts accessibles de Gabriel Schuck pour le
# warping audio. « ReaWarp » et « ReaPack » ne se traduisent pas ; le chemin de
# menu nomme les menus de REAPER, comme pour les autres étapes de dépôt.
config-reapack-reawarp-name = Ajouter le dépôt ReaPack « ReaWarp »
config-reapack-reawarp-description = ReaWarp, de Gabriel Schuck, est un ensemble de scripts accessibles pour le warping audio, les cartes de tempo, le calage sur un groove et la quantification. Ses messages sont en anglais, portugais ou espagnol. Cette étape ajoute son dépôt ReaPack (https://github.com/gabrielschuck/reawarp/raw/main/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour installer ReaWarp ; ReaPack le tiendra ensuite à jour.
config-set-reaper-language-name = Définir la langue de REAPER
config-set-reaper-language-description = Indique à REAPER lequel des packs de langue installés utiliser, en l'inscrivant dans reaper.ini. Sans cela, vous devriez sélectionner la langue vous-même dans Options, Préférences, Général. Vous pouvez installer plusieurs packs de langue et basculer entre eux dans REAPER par la suite ; ceci choisit seulement celui qui est actif maintenant. Décochez si vous voulez seulement installer les fichiers.

wizard-reapack-ack-heading = Avis de don ReaPack
wizard-reapack-ack-body = ReaPack est un logiciel libre publié sous licence LGPL. Son auteur, Christian Fillion, accepte des dons facultatifs pour soutenir la poursuite du développement. Christian maintient également les extensions SWS et a, par le passé, intégré du code spécifiquement destiné à améliorer la compatibilité avec OSARA. Tout soutien que vous pourrez lui apporter est amplement mérité.
wizard-reapack-ack-link-label = Ouvrir la page de don de ReaPack
wizard-reapack-ack-confirm-label = Ignorer le don cette fois, installer ou mettre à jour ReaPack uniquement
cli-reapack-ack-prompt-summary = ReaPack est un logiciel libre (LGPL). Son auteur, Christian Fillion, accepte des dons facultatifs sur https://reapack.com/donate pour soutenir le développement continu.
cli-reapack-ack-flag-required = ReaPack figure dans le plan de cette exécution, mais l'accusé de réception du don est manquant. Relancez la commande avec --accept-reapack-donation-notice pour confirmer que vous avez lu https://reapack.com/donate et que vous souhaitez que RABBIT installe ou mette à jour ReaPack.

wizard-version-check-heading = Vérification des dernières versions
wizard-version-check-status-pending = Préparation de la vérification des dernières versions…
# $package is the localized package display name.
wizard-version-check-status-checking = Vérification de { $package }…
wizard-version-check-status-progress = Vérification des versions… ({ $done } sur { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } vérification(s) de version ont échoué. Utilisez Précédent pour essayer une autre cible, ou fermez RABBIT.
wizard-version-check-progress-label = Progression
wizard-version-check-error-heading = Vérifications échouées
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package } : { $message }
wizard-package-details-label = Détails du paquet
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = Nouveautés dans { $package } :
wizard-packages-osara-keymap-heading = Raccourcis clavier OSARA
wizard-packages-osara-keymap-replace-label = Remplacer vos raccourcis clavier par les derniers d'OSARA
packages-reaper-language-label = Langue de REAPER après l'installation
packages-spanish-variant-label = Traduction espagnole d'OSARA
wizard-packages-osara-keymap-unavailable-note = Sélectionnez OSARA pour configurer le comportement de ses raccourcis clavier.
wizard-packages-osara-keymap-preserve-note = Vos raccourcis clavier actuels restent tels quels : RABBIT ne modifie pas reaper-kb.ini. Pour ajouter les nouveaux raccourcis d'OSARA tout en conservant les vôtres, lancez ensuite l'action d'OSARA « Merge OSARA key map » (menu Extensions, OSARA).
wizard-packages-osara-keymap-replace-note = RABBIT sauvegardera votre fichier reaper-kb.ini actuel, s'il existe, puis le remplacera par les derniers raccourcis clavier d'OSARA. Les raccourcis que vous avez modifiés vous-même ne sont pas repris ; la sauvegarde les conserve.
wizard-packages-osara-keymap-unchanged-note = Vos raccourcis clavier sont ceux d'OSARA, sans modification de votre part : RABBIT va donc les sauvegarder et les remplacer par les derniers raccourcis clavier d'OSARA. Vos scripts installés restent dans la liste des actions.
wizard-packages-osara-keymap-default-keys-note = Vous utilisez les raccourcis clavier par défaut de REAPER plutôt que ceux d'OSARA : RABBIT les laisse tels quels. Pour passer aux raccourcis clavier d'OSARA, cochez la case ci-dessus.
wizard-package-details-handling-prefix = Prise en charge
wizard-package-handling-automatic = RABBIT peut installer ce paquet directement.
wizard-package-handling-unattended = RABBIT peut installer ce paquet sans intervention, y compris en lançant son programme d'installation lorsque c'est nécessaire.
wizard-package-handling-planned = RABBIT est conçu pour exécuter lui-même le programme d'installation ou la procédure de configuration de ce paquet et terminer l'installation sans intervention, mais cette version se contente encore de signaler les étapes au lieu de les exécuter.
wizard-package-handling-manual = RABBIT téléchargera ce paquet et indiquera les étapes manuelles après l'exécution.
wizard-package-handling-unavailable = Ce paquet n'est pas disponible pour la plateforme ou l'architecture sélectionnée.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package } : { $action }. Vous avez { $installed }. La dernière version est { $available }

wizard-review-heading = Vérifiez ce que vous avez demandé à RABBIT
wizard-review-target-prefix = Cible
wizard-review-package-heading = Paquets sélectionnés
wizard-review-osara-keymap-heading = Raccourcis clavier OSARA
wizard-review-osara-keymap-preserve = Conserver vos raccourcis clavier actuels.
wizard-review-osara-keymap-replace = Sauvegarder vos raccourcis clavier actuels, puis les remplacer par les derniers d'OSARA.
wizard-review-osara-keymap-replace-unchanged = Sauvegarder vos raccourcis clavier OSARA non modifiés, puis les remplacer par les derniers d'OSARA.
wizard-review-osara-keymap-keep-default-keys = Conserver les raccourcis clavier par défaut de REAPER.
wizard-review-notes-heading = Remarques
wizard-review-preflight-prefix = Installation impossible pour le moment

# $path is the selected REAPER resource path.
wizard-review-target = Cible : { $path }
wizard-review-no-target = Aucune cible sélectionnée.
wizard-review-no-package = Aucun paquet sélectionné.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package } : { $action }
# Review, under its notes, for a package ticked on the standard REAPER while
# the per-user CLAP folder holds a copy of it, such as app2clap: the install
# moves that copy into the update's backup, so portable REAPERs and other
# programs that use CLAP plug-ins lose it. $package is the localized package
# name.
wizard-review-clap-copy-moves = { $package } : la copie de votre dossier CLAP est déplacée dans la sauvegarde, et les REAPER portables et les autres programmes qui utilisent des plug-ins CLAP ne la trouvent plus.
# Review, under its notes, for a package ticked to install on a portable REAPER
# while the per-user CLAP folder holds a copy of it, which every REAPER loads:
# this REAPER will load two copies. $package is the localized package name.
wizard-review-clap-copy-twice = { $package } : ce REAPER charge aussi la copie de votre dossier CLAP, il en chargera donc deux.

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Prêt à désinstaller
wizard-review-uninstall-packages = Paquets à désinstaller
wizard-review-uninstall-no-package = Rien n'est coché pour la désinstallation.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package } : { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = Ses réglages sont aussi supprimés et vont dans la sauvegarde : { $settings }.
wizard-review-uninstall-close-programs-note = Fermez d'abord les autres programmes de musique.
wizard-review-uninstall-elevation-note-windows = Windows vous demande l'autorisation une seule fois, avant de supprimer quoi que ce soit. Si vous refusez, rien n'est désinstallé.
wizard-review-uninstall-vendor-window-note = Si une fenêtre du programme de désinstallation de { $package } apparaît, répondez-y.
# Review's permission note on macOS, when a ticked package needs an
# administrator. $packages is a list of package names. osascript is a program
# name, never translated: macOS names it in the password window. The window
# may offer Touch ID, so don't say a password is the only way. Use the same
# words for 'administrator command' as in the Done and progress lines that
# name it.
wizard-review-uninstall-elevation-note-macos = Avant de supprimer quoi que ce soit, macOS demande une seule fois le nom et le mot de passe d'un administrateur. Il peut proposer Touch ID à la place. La fenêtre peut mentionner osascript, l'outil par lequel RABBIT fait la demande. Avec ce mot de passe, RABBIT exécute une commande administrateur qui désinstalle { $packages }. Si vous annulez la fenêtre, rien n'est désinstallé.
# One line per ticked package that goes to the Trash; the second when your
# account may not move it, so an administrator's password moves it there.
# $package is the localized package name. Name the Trash and Put Back as
# Finder does in your language.
wizard-review-uninstall-trash-note = { $package } va dans la Corbeille. Tant que vous ne videz pas la Corbeille, vous pouvez l'en faire glisser pour le récupérer.
wizard-review-uninstall-trash-elevated-note-macos = { $package } va dans votre Corbeille grâce au mot de passe de l'administrateur. Il garde le propriétaire qu'il avait, le Finder ne propose donc pas « Remettre » pour lui, et vider la Corbeille peut demander le mot de passe d'un administrateur.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. RABBIT leaves out what it ticked by itself, such as updates, but the
# list can still hold a setting RABBIT ticked along with one of the user's
# ticks, so the note doesn't say who ticked it. $packages is a comma-separated
# list of names: packages, and for the Install or update tab also settings,
# such as "Set REAPER's language".
wizard-review-inactive-tab-note-install = Ce qui est coché dans l'onglet « Installer ou mettre à jour » ne fait pas partie de cette opération : { $packages }.
wizard-review-inactive-tab-note-uninstall = Ce qui est coché dans l'onglet « Désinstaller » ne fait pas partie de cette opération : { $packages }.
# The same note for the Backups tab. $backups is one of the two messages
# below: the tab has one set of ticks, for Restore or Delete as chosen below
# its list.
wizard-review-inactive-tab-note-backups = Ce qui est coché dans l'onglet « Sauvegardes » ne fait pas partie de cette opération : { $backups }.
# How many backups are ticked on the Backups tab, with Restore or with
# Delete chosen below its list, for the note above. $count is a number,
# never 0.
wizard-review-inactive-tab-backups-restore = { $count ->
    [one] une sauvegarde à restaurer
   *[other] { $count } sauvegardes à restaurer
}
wizard-review-inactive-tab-backups-delete = { $count ->
    [one] une sauvegarde à supprimer
   *[other] { $count } sauvegardes à supprimer
}
wizard-review-uninstall-reinstall-hint = Vous pourrez les réinstaller plus tard depuis l'onglet « Installer ou mettre à jour ».
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Désinstallation impossible pour le moment : { $reason }
wizard-review-uninstall-not-writable = RABBIT ne peut pas écrire dans le dossier de ce REAPER.

# Review before a restore from the Backups tab. The heading names what the
# run does, so it is the first thing read.
wizard-review-restore-heading = Prêt à restaurer
# One line per ticked backup. $backup is what its row calls it, $package the
# localized package name, $date a date and time (backup-date-time), and
# $settings yes or no: whether the backup holds the package's settings too.
wizard-review-restore-uninstalled = { $settings ->
    [yes] { $backup } revient tel qu'il était le { $date }, avec vos réglages pour { $package }.
   *[no] { $backup } revient tel qu'il était le { $date }.
}
wizard-review-restore-uninstalled-undated = { $settings ->
    [yes] { $backup } revient tel qu'il était avant la désinstallation, avec vos réglages pour { $package }.
   *[no] { $backup } revient tel qu'il était avant la désinstallation.
}
# An earlier version. $version is the version it goes back to, and $current
# the one it has now, both version numbers, or for a package that comes in
# variants as wizard-backups-version-variant says.
wizard-review-restore-earlier = { $package } revient de { $current } à { $version }. RABBIT garde { $current } comme sauvegarde.
wizard-review-restore-earlier-to = { $package } revient à { $version }. RABBIT garde la version que vous avez maintenant comme sauvegarde.
wizard-review-restore-earlier-copy = { $package } revient à la copie qu'une mise à jour a remplacée. RABBIT garde la version que vous avez maintenant comme sauvegarde.
# An earlier version of a package that isn't installed now, as after you
# uninstalled it: it comes back, and no version goes into a backup. The
# -copy form is for a copy RABBIT didn't install. $version as above.
wizard-review-restore-earlier-again = { $package } revient en version { $version }.
wizard-review-restore-earlier-copy-again = { $package } revient comme la copie qu'une mise à jour a remplacée.
# A key map. $origin is own for your own key map, default-keys for REAPER's
# default keys, and other for any other; $date as above.
wizard-review-restore-keymap = { $origin ->
    [own] Vos raccourcis clavier du { $date } remplacent les actuels, que RABBIT garde comme sauvegarde.
    [default-keys] Les touches par défaut de REAPER du { $date } remplacent les raccourcis clavier actuels, que RABBIT garde comme sauvegarde.
   *[other] Les raccourcis clavier du { $date } remplacent les actuels, que RABBIT garde comme sauvegarde.
}
wizard-review-restore-keymap-undated = { $origin ->
    [own] Vos raccourcis clavier remplacent les actuels, que RABBIT garde comme sauvegarde.
    [default-keys] Les touches par défaut de REAPER remplacent les raccourcis clavier actuels, que RABBIT garde comme sauvegarde.
   *[other] Ces raccourcis clavier remplacent les actuels, que RABBIT garde comme sauvegarde.
}
# Notes on what the restore changes besides the files. $file is a language
# pack's file name, such as de_DE.ReaperLangPack, never translated; $package
# is the localized package name.
wizard-review-restore-language-back = La langue de REAPER redevient { $file }.
wizard-review-restore-language-stays = La langue de REAPER reste telle quelle, car une autre a été choisie depuis.
# Going back to an earlier version of a language pack while REAPER uses
# another pack, or none: the restore leaves REAPER's language alone.
wizard-review-restore-language-unchanged = La langue de REAPER reste telle quelle.
# Going back to an earlier version of a language pack when RABBIT
# couldn't read REAPER's settings for Review: the restore reads them
# again, and Done says what came of REAPER's language. If it still can't
# read them, REAPER's language keeps naming the newer version's file,
# which moves into a backup all the same. $file is the earlier version's
# language pack file, as above.
wizard-review-restore-language-unreadable = RABBIT ne peut pas lire les réglages de REAPER pour le moment, donc il ne pourra peut-être pas mettre { $file } comme langue de REAPER.
wizard-review-restore-not-counted = RABBIT ne compte plus { $package } comme installé par lui.
wizard-review-restore-needs-reinstall = Pour désinstaller { $package } avec RABBIT plus tard, réinstallez-le d'abord.
# When a REAPER that has to be closed is running; the second when a ticked
# package serves every REAPER on the computer.
wizard-review-restore-close-reaper = Fermez d'abord REAPER.
wizard-review-restore-close-every-reaper = Fermez d'abord toutes les copies de REAPER.
# Why the Restore button is disabled. $reason is a sentence or two, such as
# wizard-review-restore-nothing-ticked. The -backup form names a backup that
# can't come back: $backup as in wizard-backups-row-reference, or for a key
# map as wizard-done-restore-keymap-in-sentence says it, and $reason one of
# the wizard-restore-refusal-* sentences or wizard-review-restore-plan-failed.
wizard-review-restore-blocked = Restauration impossible pour le moment : { $reason }
wizard-review-restore-blocked-backup = Impossible de restaurer { $backup } pour le moment. { $reason }
# With nothing ticked to restore. The second sentence says how to tick:
# Space is the Space bar, named as keyboards and screen readers in your
# language name it.
wizard-review-restore-nothing-ticked = Rien n'est coché pour la restauration. Cochez des sauvegardes dans la liste avec la barre d'espace.
wizard-review-restore-not-writable = RABBIT ne peut pas écrire dans le dossier de ce REAPER.
# $first and $second name two ticked backups, as in
# wizard-backups-row-reference, or for a key map as
# wizard-done-restore-keymap-in-sentence says it, so the sentence may start
# in lower case; $package is the localized package name.
wizard-review-restore-same-package = { $first } et { $second } sont toutes deux des sauvegardes de { $package }. N'en cochez qu'une.
wizard-review-restore-same-file = { $first } et { $second } remettent toutes deux en place le même fichier. N'en cochez qu'une.
# When RABBIT couldn't check the restore before it starts: one backup, or
# all of them. $error is the error message, in English.
wizard-review-restore-plan-failed = RABBIT n'a pas pu la vérifier ({ $error }).
wizard-review-restore-check-failed = RABBIT n'a pas pu vérifier la restauration ({ $error }).
# When RABBIT hasn't checked the backups ticked now, so the restore can't
# start yet.
wizard-review-restore-not-checked = RABBIT n'a pas vérifié ce qui est coché.
# Review before a delete from the Backups tab.
wizard-review-delete-heading = Prêt à supprimer des sauvegardes
# $count is a number, the ticked backups, and $size a size, such as "520 MB".
wizard-review-delete-intro = { $count ->
    [one] Cette sauvegarde est supprimée définitivement et libère { $size } :
   *[other] Ces sauvegardes sont supprimées définitivement et libèrent { $size } :
}
# As above, when $size is about what the delete frees: a package or key map
# goes out of the backup set it is kept in, and RABBIT knows its size only
# from that set's record of what it holds.
wizard-review-delete-intro-estimate = { $count ->
    [one] Cette sauvegarde est supprimée définitivement et libère environ { $size } :
   *[other] Ces sauvegardes sont supprimées définitivement et libèrent environ { $size } :
}
# One line per ticked backup, saying what is lost with it. $backup is what
# its row calls it, and $date a date and time (backup-date-time).
wizard-review-delete-uninstalled = { $backup }, désinstallé le { $date } : la seule copie.
wizard-review-delete-keymap = { $backup } du { $date } : la seule copie.
wizard-review-delete-only-copy = { $backup } : la seule copie.
wizard-review-delete-earlier = { $backup } : vous ne pourrez plus y revenir ensuite.
wizard-review-delete-already-back = { $backup } : ses fichiers sont aussi sur le disque, rien n'est donc perdu.
wizard-review-delete-records = { $backup } : rien ne change dans ce qui est installé.
wizard-review-delete-deleting = { $backup } : cela termine une suppression commencée plus tôt.
wizard-review-delete-other = { $backup } : cette sauvegarde disparaît définitivement.
# Why the Delete button is disabled. $reason is a sentence or two.
wizard-review-delete-blocked = Suppression impossible pour le moment : { $reason }
# As wizard-review-restore-nothing-ticked, with nothing ticked to delete.
wizard-review-delete-nothing-ticked = Rien n'est coché pour la suppression. Cochez des sauvegardes dans la liste avec la barre d'espace.
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
wizard-review-delete-only-with = RABBIT ne peut pas supprimer cette sauvegarde séparément : { $backup }. Il ne peut la supprimer qu'avec { $count ->
    [one] une autre sauvegarde
   *[other] { $count } autres sauvegardes
} : { $backups }. Pour la supprimer, cochez aussi { $count ->
    [one] l'autre
   *[other] les autres
}.
wizard-review-delete-only-with-undeletable = RABBIT ne peut pas supprimer cette sauvegarde séparément : { $backup }. Il ne peut la supprimer qu'avec { $count ->
    [one] une autre sauvegarde
   *[other] { $count } autres sauvegardes
} que RABBIT ne peut pas supprimer : { $backups }. Décochez cette sauvegarde.

wizard-progress-heading = Progression de l'installation
wizard-progress-status-idle = Prêt à installer.
wizard-progress-status-running = Installation des paquets sélectionnés. Cela peut prendre quelques minutes.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Arrêt en cours. RABBIT termine l'étape commencée, puis se ferme.
wizard-progress-details-label = Détails de la progression
wizard-progress-details-idle = Aucune installation en cours.
wizard-progress-details-starting = Démarrage de l'opération de configuration.
wizard-progress-details-cache-prefix = Cache
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops once the step it is on has finished: one package,
# or on macOS the administrator command, which removes several at once.
wizard-progress-uninstall-heading = Désinstallation
wizard-progress-status-uninstall-running = Désinstallation des paquets cochés.
wizard-progress-details-uninstall-starting = Démarrage de la désinstallation.
wizard-progress-status-uninstall-cancelling = Arrêt en cours. RABBIT termine l'étape commencée, puis se ferme.
# The progress page while backups are deleted: its heading, the first status
# line and the first line of the log. Once the user asked to stop, the
# status line says wizard-progress-status-cancelling.
wizard-progress-delete-heading = Suppression des sauvegardes
wizard-progress-status-delete-running = Suppression des sauvegardes cochées.
wizard-progress-details-delete-starting = Démarrage de la suppression des sauvegardes.
# The status and log line when RABBIT starts deleting from a backup set,
# which is spoken, then the log line when it is done with it. $backups is
# what the rows call the ticked backups in that set, one name or several
# joined as wizard-backups-list-and joins them, each with when it was made,
# as wizard-backups-row-reference says it, when another ticked backup has
# the same name. $count is how many there are, a number, and $size a size,
# such as "4 MB".
wizard-progress-status-deleting = { $count ->
    [one] Suppression de la sauvegarde : { $backups }…
   *[other] Suppression des sauvegardes : { $backups }…
}
wizard-progress-log-delete-deleted = { $backups } : suppression terminée. Espace libéré : { $size }.
wizard-progress-log-delete-finished = { $backups } : une suppression commencée plus tôt est terminée. Espace libéré : { $size }.
wizard-progress-log-delete-already-gone = { $backups } : il n'y avait déjà plus rien à supprimer.
wizard-progress-log-delete-partly = { $backups } : suppression seulement partielle. Espace libéré : { $size }.
wizard-progress-log-delete-failed = { $backups } : la suppression a échoué.
# The progress page while backups are restored: its heading, the first
# status line and the first line of the log. Once the user asked to stop,
# the status line says wizard-progress-status-cancelling.
wizard-progress-restore-heading = Restauration des sauvegardes
wizard-progress-status-restore-running = Restauration des sauvegardes cochées.
wizard-progress-details-restore-starting = Démarrage de la restauration des sauvegardes.
# The status and log line when RABBIT starts restoring a backup, which is
# spoken. $backup is what its row calls it, such as "SWS Extension
# 2.14.0.7" or "ReaPack 1.2.4", with when it was made, as
# wizard-backups-row-reference says it, when another ticked backup has the
# same name. $kind is uninstalled for software an uninstall kept, earlier
# for an earlier version whose row names its version, and earlier-copy for
# one whose row names only the package.
wizard-progress-status-restoring = { $kind ->
    [earlier] Retour à { $backup }…
    [earlier-copy] Retour à la copie de { $backup } qu'une mise à jour a remplacée…
   *[uninstalled] Restauration de { $backup }…
}
# The same for a key map. $origin is own for your own key map, default-keys
# for REAPER's default keys, and other for any other; $date is a date and
# time (backup-date-time). The -undated form is for a key map whose date
# RABBIT can't tell.
wizard-progress-status-restoring-keymap = { $origin ->
    [own] Restauration de vos raccourcis clavier du { $date }…
    [default-keys] Restauration des touches par défaut de REAPER du { $date }…
   *[other] Restauration des raccourcis clavier du { $date }…
}
wizard-progress-status-restoring-keymap-undated = { $origin ->
    [own] Restauration de vos raccourcis clavier…
    [default-keys] Restauration des touches par défaut de REAPER…
   *[other] Restauration des raccourcis clavier…
}
# The log line when RABBIT is done with a backup. $backup as above.
wizard-progress-log-restore-restored = { $backup } : restauration terminée.
wizard-progress-log-restore-not-restored = { $backup } : pas de restauration, tout est resté tel quel.
wizard-progress-log-restore-files-left = { $backup } : la restauration a échoué, et certains fichiers déjà modifiés n'ont pas pu être remis comme avant.
wizard-progress-log-restore-partly = { $backup } : restauration partielle seulement.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Téléchargement de { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Téléchargement de { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Téléchargement de { $count } paquets… { $downloaded } / { $total }
wizard-progress-status-installing = Installation de { $package }…
wizard-progress-status-removing = Désinstallation de { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Application de l'étape de configuration : { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = Téléchargement de { $package }…
wizard-progress-log-download-completed = { $package } téléchargé.
wizard-progress-log-install-started = Installation de { $package }…
wizard-progress-log-install-completed = { $package } installé.
wizard-progress-log-remove-started = Désinstallation de { $package }…
wizard-progress-log-remove-completed = { $package } désinstallé.
wizard-progress-log-remove-failed = Impossible de désinstaller { $package }.
wizard-progress-status-vendor-running = Exécution du programme de désinstallation de { $package }…
wizard-progress-status-vendor-still-waiting = Toujours en attente du programme de désinstallation de { $package }. Il affiche peut-être une question dans une autre fenêtre ; appuyez sur Alt+Tab pour la trouver.
wizard-progress-status-vendor-waiting-for-files = Le programme de désinstallation de { $package } s'est terminé, et RABBIT attend que ses fichiers disparaissent. Si un autre programme de musique a ouvert { $package }, fermez-le.
# On macOS, said right before the administrator password window appears, and
# again if it has been open a while without an answer. RABBIT puts each on the
# status line and in the log too, in case VoiceOver cuts it off. $packages is
# a list of package names. Use the same words for 'administrator command' as
# in the Done and progress lines that name it.
wizard-progress-status-admin-prompt-macos = macOS demande maintenant, une seule fois, le nom et le mot de passe d'un administrateur. Avec eux, RABBIT exécute une commande administrateur qui désinstalle { $packages }. Si vous annulez, rien n'est désinstallé.
wizard-progress-status-admin-still-waiting-macos = Toujours en attente de la fenêtre du mot de passe administrateur. Si vous ne l'entendez pas, vérifiez si elle se trouve derrière RABBIT.
# While a package goes to the Trash, and in the log once the administrator
# command has worked on a package. $package is the localized package name.
# Name the Trash as Finder does.
wizard-progress-status-trashing = Placement de { $package } dans la Corbeille…
wizard-progress-log-admin-done = La commande administrateur a terminé avec { $package }.
wizard-progress-log-configuration-started = Application de { $step }…
wizard-progress-log-configuration-completed = { $step } appliqué.

wizard-done-heading = Terminé
wizard-done-status-idle = Aucune installation n'a encore été lancée depuis cette fenêtre.
wizard-done-status-success = RABBIT a fini d'opérer sa magie ! Consultez les détails ci-dessous.
wizard-done-status-error = L'installation a échoué. Consultez l'erreur ci-dessous.
wizard-reaper-running-title = REAPER est toujours ouvert
wizard-reaper-running-body = Fermez REAPER, puis appuyez à nouveau sur Installer. RABBIT ne peut pas mettre à jour REAPER ni ses extensions tant que REAPER est ouvert.
wizard-error-reaper-running = RABBIT s'est arrêté parce que REAPER était ouvert. Fermez REAPER, puis relancez RABBIT.
# Why a run stopped when another RABBIT is working on the same REAPER:
# installing, uninstalling, or restoring or deleting backups.
wizard-error-lock-held = Un autre RABBIT travaille sur ce REAPER. Attendez qu'il ait terminé, puis relancez RABBIT.
# Why an install or an uninstall stopped before it changed anything:
# RABBIT's own folder in the REAPER, or the backups folder in it, is a
# symbolic link or junction, so a backup kept there couldn't be restored.
wizard-error-backup-folder-link = RABBIT s'est arrêté parce que son propre dossier dans ce REAPER, ou le dossier de sauvegardes qu'il contient, est un lien. RABBIT ne restaure jamais une sauvegarde située derrière un lien, il ne change donc rien tant que c'est le cas. Refaites-en un vrai dossier, avec les sauvegardes dedans, puis relancez RABBIT.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Fermez REAPER, puis appuyez à nouveau sur Désinstaller. RABBIT ne peut rien désinstaller de REAPER tant que REAPER est ouvert.
# The same, for packages every REAPER on the computer can load, such as
# Surge XT. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis appuyez à nouveau sur Désinstaller. Tout REAPER de cet ordinateur peut utiliser { $packages }.
wizard-reaper-running-uninstall-reaper-body = Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis appuyez à nouveau sur Désinstaller. Le programme de désinstallation de REAPER ne s'exécute pas tant qu'un REAPER est ouvert.
# The same on macOS, when RABBIT's administrator command is to move REAPER's
# app: it can't tell one REAPER from another.
wizard-reaper-running-uninstall-reaper-body-macos = Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis appuyez à nouveau sur Désinstaller. La commande administrateur qui déplace REAPER ne distingue pas un REAPER d'un autre, ils doivent donc tous être fermés.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Fermez d'abord ces programmes
wizard-programs-running-uninstall-body = Fermez { $programs }, puis appuyez à nouveau sur Désinstaller.
# Shown instead of starting a restore while REAPER is open, under
# wizard-reaper-running-title. Name the button as wizard-button-restore
# does.
wizard-reaper-running-restore-body = Fermez REAPER, puis appuyez à nouveau sur Restaurer. RABBIT ne peut pas restaurer de sauvegardes dans REAPER tant que REAPER est ouvert.
# The same, when a ticked backup holds a package every REAPER on the
# computer can load. $packages is a list of localized package names, joined
# as wizard-backups-list-and joins names.
wizard-reaper-running-restore-every-body = Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis appuyez à nouveau sur Restaurer. Tout REAPER de cet ordinateur peut utiliser { $packages }.
# Shown instead of starting a restore while a program that has to be closed
# first is running. $programs is a list of program names, joined as
# wizard-backups-list-and joins names. A name, such as "Surge XT.exe", is
# never translated.
wizard-programs-running-restore-title = Fermez d'abord ces programmes
wizard-programs-running-restore-body = Fermez { $programs }, puis appuyez à nouveau sur Restaurer.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT s'est arrêté parce qu'un REAPER était ouvert. Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis relancez RABBIT.
wizard-error-uninstall-not-enough-space = RABBIT s'est arrêté parce qu'il n'y a pas assez d'espace libre pour la sauvegarde. Libérez de l'espace sur le disque qui contient le dossier de ressources de REAPER, puis relancez RABBIT.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } n'a pas été désinstallé, car un autre programme utilise { $path }. Fermez ce programme, puis relancez RABBIT.
# The same, for a file a package's installer added outside REAPER's folder.
# Only the JAWS scripts' installer adds such files, and JAWS itself may have
# one open; the program to close would be the user's screen reader, so this
# says to wait and try again instead. $package is the localized package
# name, and $path the file's full path as Windows holds it. Keep JAWS as it
# is.
wizard-error-uninstall-vendor-file-in-use = { $package } n'a pas été désinstallé, car JAWS ou un autre programme utilise { $path }. Patientez un instant, puis relancez RABBIT. Si cela ne suffit pas, redémarrez d'abord JAWS.
# Why an uninstall didn't start on macOS: RABBIT itself runs as root, as under
# sudo. sudo and root are never translated.
wizard-error-uninstall-as-root = RABBIT s'exécute en tant que root, comme sous sudo, il ne désinstalle donc rien. Quittez RABBIT et relancez-le avec votre propre compte. Il demande lui-même le mot de passe d'un administrateur quand il en a besoin.
wizard-done-status-completed-with-errors = Installation terminée avec des erreurs. Consultez les détails ci-dessous.
wizard-done-status-cancelled = Vous avez arrêté l'installation. La liste ci-dessous montre ce que RABBIT a terminé avant de s'arrêter.
wizard-done-status-no-packages = Aucun paquet n'a été sélectionné pour l'installation ou la mise à jour.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = RABBIT a désinstallé { $packages }.
wizard-done-uninstall-status-completed-with-errors = RABBIT a désinstallé { $packages }, mais tout ne s'est pas passé comme prévu.
wizard-done-uninstall-status-error = La désinstallation a échoué.
wizard-done-uninstall-status-cancelled = Vous avez arrêté la désinstallation. Les paquets que RABBIT n'avait pas encore atteints sont toujours installés.
wizard-done-uninstall-status-no-packages = Rien n'a été désinstallé.
wizard-done-uninstall-status-pending-restart = RABBIT a désinstallé { $packages }. Redémarrez l'ordinateur pour terminer.
wizard-done-uninstall-status-declined = Rien n'a été désinstallé, car la demande d'autorisation de Windows a été refusée.
# The same on macOS, where the administrator password window was cancelled.
wizard-done-uninstall-status-declined-macos = Rien n'a été désinstallé, car la fenêtre du mot de passe administrateur a été annulée.
# The Done page's first line when the administrator command removed only part
# of a package and nothing else was uninstalled. $packages is a list of
# package names.
wizard-done-uninstall-status-partly-removed = RABBIT n'a supprimé qu'une partie de { $packages }.
wizard-done-uninstall-removed-before-stop = Avant que vous l'arrêtiez, RABBIT a désinstallé { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Pour en récupérer un, relancez RABBIT et cochez-le dans l'onglet « Installer ou mettre à jour ».
# Used instead of wizard-done-uninstall-reinstall-hint when RABBIT kept a
# backup of a removed package: the same sentence, then where to restore
# it. $packages is a list of localized package names, joined as
# wizard-backups-list-and joins names, only those whose files went into
# the backup, which may be fewer than were uninstalled. Name the tabs as
# wizard-packages-tab-install and wizard-packages-tab-backups do.
wizard-done-uninstall-reinstall-or-restore-hint = Pour en récupérer un, relancez RABBIT et cochez-le dans l'onglet « Installer ou mettre à jour ». Vous pouvez aussi restaurer { $packages } dans l'onglet « Sauvegardes ».
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } n'était plus installé, il n'y avait donc rien à supprimer.
# The backup still holds the files, so the Backups tab lists it and can put
# them back. Done has no way back to the tab, so the sentence says to run
# RABBIT again, as wizard-done-delete-partly does. Name the tab as
# wizard-packages-tab-backups does.
wizard-done-uninstall-left-in-backup = Certains fichiers de { $package } n'ont pas pu être remis en place et sont toujours dans la sauvegarde. « Afficher les détails » les énumère. Pour les remettre en place, relancez RABBIT et restaurez { $package } dans l'onglet « Sauvegardes ».
wizard-done-uninstall-leftovers = RABBIT trouve encore une partie de { $package } et n'y touche pas. « Afficher les détails » indique laquelle.
wizard-done-uninstall-language-reset-failed = RABBIT n'a pas pu remettre REAPER en anglais après avoir supprimé { $package }. Choisissez une langue dans les préférences de REAPER.
wizard-done-uninstall-registry-failed = RABBIT a désinstallé { $package }, mais la liste des programmes installés de Windows l'affiche peut-être encore. « Afficher les détails » indique l'entrée.
wizard-done-uninstall-pending-restart = Redémarrez l'ordinateur pour finir de supprimer { $package }.
wizard-done-uninstall-vendor-cancelled = Le programme de désinstallation de { $package } a été annulé avant la fin.
wizard-done-uninstall-vendor-failed = Le programme de désinstallation de { $package } s'est terminé avec le code d'erreur { $code }. « Afficher les détails » indique ce qui est encore là.
wizard-done-uninstall-vendor-failed-unsure = Le programme de désinstallation de { $package } s'est terminé avec le code d'erreur { $code }, donc { $package } n'est peut-être pas entièrement supprimé.
wizard-done-uninstall-vendor-still-there = Le programme de désinstallation de { $package } s'est terminé, mais { $package } est toujours installé. Vous pouvez le désinstaller dans les Paramètres de Windows, sous Applications, Applications installées.
wizard-done-uninstall-vendor-still-open = Le programme de désinstallation de { $package } est toujours ouvert, donc RABBIT s'est arrêté et n'a pas touché aux paquets suivants. Appuyez sur Alt+Tab pour trouver sa fenêtre, puis répondez-y.
wizard-done-uninstall-vendor-run-failed = RABBIT n'a pas pu aller au bout du programme de désinstallation de { $package } ({ $error }), il s'est donc arrêté là.
wizard-done-uninstall-elevation-failed = RABBIT n'a pas obtenu les droits d'administrateur ({ $error }), donc rien n'a été désinstallé.
wizard-done-uninstall-resource-damaged = Important : après l'exécution du programme de désinstallation de { $package }, ces fichiers manquaient dans le dossier de ressources de REAPER : { $paths }. RABBIT s'est arrêté et a réécrit ses enregistrements. Il ne peut pas restaurer les réglages de REAPER ; si vous avez une copie de reaper.ini, remettez-la en place avant de lancer REAPER.
wizard-done-uninstall-receipt-not-saved = RABBIT a désinstallé { $package }, mais n'a pas pu mettre à jour ses enregistrements. La prochaine fois, RABBIT constatera quand même que { $package } n'est plus là.
wizard-done-uninstall-vendor-lost = RABBIT a perdu la trace du programme de désinstallation de { $package } après son démarrage. Il est peut-être encore en cours, donc RABBIT ne peut pas savoir ce qu'il a supprimé, et s'est arrêté là.
wizard-done-uninstall-vendor-lost-reaper = Une fois le programme de désinstallation terminé, vérifiez que reaper.ini est toujours dans le dossier de ressources de REAPER : il contient vos réglages de REAPER.
wizard-done-uninstall-vendor-files-left = Le programme de désinstallation de { $package } s'est terminé, mais certains de ses fichiers sont toujours là. « Afficher les détails » les énumère.
wizard-done-uninstall-vendor-not-confirmed = Le programme de désinstallation de { $package } s'est terminé, mais RABBIT ne peut pas confirmer que { $package } n'est plus là.
wizard-done-uninstall-vendor-not-listed = Windows n'affiche plus { $package } dans la liste des programmes installés, il n'est donc pas non plus dans les Paramètres de Windows.
wizard-done-uninstall-working-folder-left = RABBIT n'a pas pu supprimer { $path }, le dossier depuis lequel il a lancé le programme de désinstallation de { $package }. Seul un administrateur peut le supprimer.
wizard-done-uninstall-restart-list-unreadable = RABBIT n'a pas pu lire quels fichiers Windows supprime au prochain redémarrage, il ne sait donc pas si les fichiers restants de { $package } partiront à ce moment-là.
wizard-done-uninstall-installed-list-unreadable = RABBIT n'a pas pu lire la liste des programmes installés de Windows après le programme de désinstallation de { $package }, il n'a donc pas pu vérifier si Windows affiche encore { $package }.
# After the elevated helper was to remove the uninstaller folder and
# registry entry a package's installer left. $package is the localized
# package name, and $error Windows' own error message or RABBIT's in
# English. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does, and "Show
# details" as wizard-done-show-details does.
wizard-done-uninstall-cleanup-refused = RABBIT a vérifié à nouveau le dossier du programme de désinstallation de { $package } sous Programmes (Program Files) et son entrée de registre, et ne s'est pas fié à ce qu'il a trouvé : il a donc remis en place ce qu'il avait déplacé.
wizard-done-uninstall-cleanup-failed = RABBIT n'a pas pu supprimer entièrement le dossier du programme de désinstallation et l'entrée de registre de { $package } ({ $error }) : il a donc remis en place ce qu'il avait déplacé. « Afficher les détails » indique ce qui a déjà disparu.
wizard-done-uninstall-cleanup-lost = RABBIT a perdu le contact avec son programme d'assistance pendant qu'il supprimait le dossier du programme de désinstallation et l'entrée de registre de { $package } ({ $error }) : il a donc remis en place ce qu'il avait déplacé. Une partie de ce dossier ou son entrée de registre a peut-être déjà disparu.
# Instead of wizard-done-uninstall-cleanup-failed, when RABBIT removed
# nothing of the folder or the registry entry before it failed, so "Show
# details" lists nothing as gone. $package and $error as above.
wizard-done-uninstall-cleanup-failed-nothing-gone = RABBIT n'a pas pu supprimer le dossier du programme de désinstallation et l'entrée de registre de { $package } ({ $error }) : il a donc remis en place ce qu'il avait déplacé. Rien de ce dossier ni de son entrée de registre n'a été supprimé.
# After a macOS removal: to the Trash, or by the administrator command.
# $package is the localized package name, $path a full macOS path, keep it as
# it is, $error macOS's own error message, and $programs a list of program
# names, never translated. Name the Trash, Put Back and Finder as Finder does,
# "Show details" as wizard-done-show-details does, and System Settings >
# Privacy & Security > App Management as macOS names it in your language.
wizard-done-uninstall-moved-to-trash = RABBIT a placé { $package } dans la Corbeille. Il s'y trouve à cet emplacement : { $path }. Tant que vous ne videz pas la Corbeille, vous pouvez l'en faire glisser pour le récupérer.
wizard-done-uninstall-moved-to-trash-unknown = RABBIT a placé { $package } dans la Corbeille. Tant que vous ne videz pas la Corbeille, vous pouvez l'en faire glisser pour le récupérer.
wizard-done-uninstall-trash-owner = Il garde le propriétaire qu'il avait, le Finder ne propose donc pas « Remettre » pour lui. Vider la Corbeille peut demander le mot de passe d'un administrateur, tout comme l'en faire glisser.
wizard-done-uninstall-trash-failed = RABBIT n'a pas pu placer { $package } dans la Corbeille, rien n'a donc changé pour lui ({ $error }).
wizard-done-uninstall-trash-unclear = RABBIT a demandé à macOS de placer { $path } dans la Corbeille, mais ne peut pas confirmer qu'il s'y trouve maintenant. Cherchez { $package } dans la Corbeille et dans son dossier, dans le Finder. RABBIT s'est arrêté là.
wizard-done-uninstall-admin-step-failed = La commande administrateur s'est arrêtée à { $path }, rien de { $package } n'a donc été supprimé. RABBIT s'est arrêté là.
wizard-done-uninstall-admin-delete-failed = La commande administrateur s'est arrêtée à { $path }, qui n'est peut-être plus là qu'en partie. Rien d'autre de { $package } n'a été supprimé. RABBIT s'est arrêté là.
wizard-done-uninstall-admin-stopped = La commande administrateur s'est terminée sans indiquer où elle s'est arrêtée, et rien de { $package } n'a été supprimé. RABBIT s'est arrêté là.
wizard-done-uninstall-partly-removed = La commande administrateur s'est arrêtée à { $path }, { $package } n'est donc supprimé qu'en partie. « Afficher les détails » indique ce qui a été supprimé et ce qui reste. RABBIT s'est arrêté là.
wizard-done-uninstall-partly-removed-unclear = La commande administrateur s'est terminée sans indiquer où elle s'est arrêtée, et { $package } n'est supprimé qu'en partie. « Afficher les détails » indique ce qui a été supprimé et ce qui reste. RABBIT s'est arrêté là.
wizard-done-uninstall-admin-left = La commande administrateur s'est exécutée, mais { $path } est toujours là, ou RABBIT ne peut pas voir qu'il a disparu, { $package } n'est donc pas entièrement supprimé. « Afficher les détails » indique ce qui a été supprimé et ce qui reste. RABBIT s'est arrêté là.
wizard-done-uninstall-app-management = Si macOS l'a refusé : dans Réglages Système > Confidentialité et sécurité > Gestion des apps, activez RABBIT. Puis quittez RABBIT, relancez-le et désinstallez à nouveau.
wizard-done-uninstall-admin-unclear = La commande administrateur s'est terminée sans dire comment, mais il ne reste plus rien de { $package }.
wizard-done-uninstall-installer-records-remain = Le programme d'installation de macOS garde encore des reçus de { $package }, bien que ses fichiers aient disparu. Cela ne pose aucun problème ; « Afficher les détails » les indique.
wizard-done-uninstall-installer-records-unchecked = RABBIT n'a pas pu vérifier que le programme d'installation de macOS a oublié { $package } ({ $error }). Cela ne pose aucun problème : ses fichiers ont disparu.
wizard-done-uninstall-program-started = La commande administrateur a trouvé l'un de ces programmes ouvert, ou n'a pas pu le vérifier, elle n'a donc rien changé : { $programs }. Fermez ceux qui sont ouverts, puis relancez RABBIT.
wizard-done-uninstall-admin-steps-refused = RABBIT n'a pas pu préparer la commande administrateur pour { $package } ({ $error }), il n'a donc demandé aucun mot de passe et n'a rien désinstallé.
wizard-done-show-details = Afficher les détails
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = Ouvrir REAPER et fermer RABBIT
wizard-done-launch-reaper-mnemonic = O
wizard-done-open-resource = Ouvrir le dossier de ressources (réservé à la maintenance manuelle avancée)
wizard-done-open-resource-mnemonic = R
wizard-done-no-reaper-app = Aucune application REAPER lançable n'est connue pour cette cible.
wizard-done-launch-reaper-error-prefix = REAPER n'a pas pu être lancé
wizard-done-open-resource-error-prefix = Le dossier de ressources n'a pas pu être ouvert
wizard-done-self-update-apply-running = Application de la mise à jour de RABBIT…
wizard-done-self-update-error-prefix = La mise à jour automatique de RABBIT a échoué
wizard-done-self-update-relaunch-prefix = RABBIT relancé
wizard-self-update-status-checking = Recherche de mises à jour de RABBIT…

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = Arrêter l'installation ?
wizard-close-during-install-body = RABBIT est en train d'installer. Si vous arrêtez maintenant, RABBIT termine l'étape commencée, conserve ce qui est déjà installé, nettoie ce qu'il a commencé, puis se ferme. Les paquets qu'il n'a pas encore atteints restent non installés. Arrêter l'installation ?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = Arrêter la désinstallation ?
wizard-close-during-uninstall-body = RABBIT est en train de désinstaller. Si vous arrêtez maintenant, RABBIT termine l'étape commencée, puis s'arrête et se ferme. Les paquets qu'il n'a pas encore atteints restent installés. Arrêter la désinstallation ?

# The same question while a restore runs, and while a delete of backups
# runs. No is the focused button here too.
wizard-close-during-restore-title = Arrêter la restauration ?
wizard-close-during-restore-body = RABBIT est en train de restaurer. Si vous arrêtez maintenant, RABBIT termine la sauvegarde qu'il restaure, puis s'arrête et se ferme. Les sauvegardes qu'il n'a pas encore atteintes restent telles quelles. Arrêter la restauration ?
wizard-close-during-delete-title = Arrêter la suppression des sauvegardes ?
wizard-close-during-delete-body = RABBIT est en train de supprimer des sauvegardes. Si vous arrêtez maintenant, RABBIT termine la sauvegarde qu'il supprime, puis s'arrête et se ferme. Les sauvegardes qu'il n'a pas encore atteintes sont conservées. Arrêter la suppression des sauvegardes ?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Désinstaller les paquets cochés ?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Une sauvegarde ne peut pas restaurer { $package }. Pour l'avoir à nouveau, il faudra le réinstaller.
# Instead of the sentence above, for a package whose files go into a backup
# while RABBIT removes its uninstaller folder in Program Files and its
# registry entry with administrator rights. Review shows this sentence too,
# or the one above when none of the package's files are left to move.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-confirm-uninstall-partly-unbacked = Une sauvegarde peut restaurer les fichiers de { $package }, mais pas le dossier de son programme de désinstallation sous Programmes (Program Files) ni son entrée de registre, que RABBIT supprime avec les droits d'administrateur.
# Instead of wizard-confirm-uninstall-unbacked, for a package that goes to
# the Trash; the second when your account may not move it, so an
# administrator's password moves it there. $package is the localized
# package name. Name the Trash as Finder does.
wizard-confirm-uninstall-trash = { $package } va dans la Corbeille. Vous pouvez l'en faire glisser pour le récupérer tant que vous ne videz pas la Corbeille.
wizard-confirm-uninstall-trash-elevated-macos = { $package } va dans la Corbeille. Vous pouvez l'en faire glisser pour le récupérer tant que vous ne videz pas la Corbeille, mais macOS peut demander le mot de passe d'un administrateur pour cela.
wizard-confirm-uninstall-question = Désinstaller maintenant ?

# Asked before backups are deleted, every time. The No button is the focused
# one, so Enter never deletes anything. The text is the body, then the
# only-copy sentence when one applies, then the question, as one paragraph.
# $count and $only are numbers: the backups that go, and how many of them
# are the only copy of what they hold. $size is a size, such as "520 MB".
wizard-confirm-delete-backups-title = Supprimer les sauvegardes définitivement ?
wizard-confirm-delete-backups-body = RABBIT supprime { $count ->
    [one] une sauvegarde
   *[other] { $count } sauvegardes
} et libère { $size }.
# As above, when $size is about what the delete frees, as for
# wizard-review-delete-intro-estimate.
wizard-confirm-delete-backups-body-estimate = RABBIT supprime { $count ->
    [one] une sauvegarde
   *[other] { $count } sauvegardes
} et libère environ { $size }.
wizard-confirm-delete-backups-only-copy = { $only ->
    [one] L'une d'elles est la seule copie de son contenu.
   *[other] { $only } d'entre elles sont la seule copie de leur contenu.
}
# Instead of the sentence above, when only one backup goes.
wizard-confirm-delete-backups-only-copy-single = C'est la seule copie de son contenu.
wizard-confirm-delete-backups-question = Une sauvegarde supprimée ne peut pas être récupérée. Supprimer maintenant ?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = RABBIT se met à jour
wizard-close-during-self-update-body = RABBIT remplace ses propres fichiers de programme. Cette étape ne peut pas être arrêtée sans endommager l'installation. Attendez la fin, puis fermez RABBIT.

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Mise à jour de RABBIT disponible
wizard-self-update-prompt-body = RABBIT { $latest } est disponible. Vous avez actuellement { $current }. Mettre à jour maintenant ? RABBIT redémarrera une fois la mise à jour terminée.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = Nouveautés depuis RABBIT { $current } :
wizard-self-update-prompt-update-button = Mettre à jour maintenant
wizard-self-update-prompt-later-button = Plus tard

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = Mise à jour de RABBIT

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = Mise à jour de RABBIT disponible : { $current } → { $latest } (canal { $channel }). Relancez RABBIT pour être invité à nouveau.
self-update-status-up-to-date = RABBIT est à jour (version actuelle { $current }, canal { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = La mise à jour automatique n'a remplacé aucun fichier (version cible { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = { $count } fichier(s) remplacé(s) dans { $root } ; relancez RABBIT pour utiliser { $version }.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Vérification de signature : { $signed } signé(s).
self-update-apply-signature-summary-unsigned-only = Vérification de signature : { $unsigned } non signé(s).
self-update-apply-signature-summary-mixed = Vérification de signature : { $signed } signé(s), { $unsigned } non signé(s).

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Une autre installation de RABBIT est en cours (PID { $pid }). L'application de la mise à jour est suspendue jusqu'à ce qu'elle se termine.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Cible : { $path }
wizard-summary-portable = Cible portable : { $value }
wizard-summary-dry-run = Simulation : { $value }
wizard-summary-packages-selected = Paquets sélectionnés : { $packages }
wizard-summary-cache = Cache : { $path }
wizard-summary-planned-app = Chemin d'application prévu : { $path }
wizard-summary-error = Erreur : { $message }
wizard-summary-error-antivirus = Le logiciel de sécurité Windows a bloqué ce téléchargement. Il s'agit généralement d'un faux positif : le programme d'installation est signé et RABBIT avait déjà vérifié le fichier avec la somme de contrôle de l'éditeur. Defender signale parfois un programme d'installation récemment créé qu'il n'a pas encore beaucoup vu – le plus souvent les versions de développement d'OSARA. Pour continuer : ouvrez Sécurité Windows, allez dans « Protection contre les virus et menaces » puis « Historique de la protection », sélectionnez l'élément bloqué et choisissez « Autoriser », puis relancez RABBIT. Vous pouvez également ajouter le dossier de téléchargement de RABBIT aux exclusions, ou installer ce paquet manuellement depuis le site de l'éditeur. RABBIT ne désactive jamais votre protection antivirus.
wizard-summary-resource-items-created = Éléments de ressources créés : { $count }
wizard-summary-packages-installed-or-checked = Paquets installés ou vérifiés : { $count }
wizard-summary-packages-current = Paquets déjà à jour : { $count }
wizard-summary-packages-manual = Paquets nécessitant une intervention manuelle : { $count }
wizard-summary-backup-files-created = Fichiers de sauvegarde créés : { $count }
wizard-summary-backup-file = Fichier de sauvegarde : { $path }
wizard-summary-receipt-backup = Sauvegarde du reçu : { $path }
wizard-summary-backup-manifest = Manifeste de sauvegarde : { $path }
wizard-summary-osara-keymap-merge-hint = Vos raccourcis clavier ont été conservés. Pour y ajouter les nouveaux raccourcis d'OSARA, démarrez REAPER et lancez l'action d'OSARA « Merge OSARA key map » (menu Extensions, OSARA).
# Show details after an install, for a file of a package's earlier version
# that the new version no longer installs: it moved into the update's
# backup, or it stays where it is, for the reason given. RABBIT no longer
# keeps track of a file that stays, and can't restore that earlier version.
# $path is the file's path.
wizard-summary-moved-to-backup = Déplacé dans la sauvegarde, car la nouvelle version ne l'installe plus : { $path }
wizard-summary-left-in-place-in-use = Reste où il est, car un autre programme l'a ouvert : { $path }
wizard-summary-left-in-place-link = Reste où il est, car c'est un lien que RABBIT ne suit jamais : { $path }
wizard-summary-left-in-place-folder = Reste où il est, car c'est un dossier : { $path }
wizard-summary-left-in-place-keymap = Reste où il est, car c'est le fichier des raccourcis clavier de REAPER : { $path }
wizard-summary-left-in-place-refused = Reste où il est, car RABBIT ne pourrait pas l'y remettre depuis une sauvegarde : { $path }
wizard-summary-left-in-place-unreadable = Reste où il est, car RABBIT n'a pas pu le lire : { $path }
wizard-summary-left-in-place-other = Reste où il est, car RABBIT n'a pas pu le déplacer dans la sauvegarde : { $path }
# Show details, instead of wizard-summary-moved-to-backup, for a file an
# update moved into its backup from a folder outside REAPER's own folder that
# other programs read too, such as the per-user CLAP folder: the new version
# installs it into REAPER's own folder instead, and they no longer find it
# there. $path is the file's path.
wizard-summary-moved-from-shared-folder = Déplacé dans la sauvegarde, car la nouvelle version l'installe désormais dans le dossier de REAPER lui-même. Les REAPER portables et les autres programmes qui le chargeaient depuis cet endroit ne le trouvent plus : { $path }
# Show details after an update that moved the language pack file REAPER's
# language named into the backup, such as after a switch between the two
# Spanish variants: REAPER's language now names the new version's file;
# or it still names the old one, because REAPER may be open and would
# write its own setting back when it quits; or RABBIT couldn't set it.
# $file is the new version's language pack file and $before the old one's,
# file names such as es_MX.ReaperLangPack, never translated. Name REAPER's
# preferences as REAPER does in your language.
wizard-summary-language-switched = La langue de REAPER utilise maintenant { $file }, car { $before } a été déplacé dans la sauvegarde.
wizard-summary-language-reaper-may-run = La langue de REAPER indique encore { $before }, qui a été déplacé dans la sauvegarde, car REAPER est peut-être ouvert. Une fois REAPER fermé, choisissez { $file } dans les préférences de REAPER.
wizard-summary-language-failed = RABBIT n'a pas pu mettre { $file } comme langue de REAPER. Choisissez-la dans les préférences de REAPER.
# Said on Done after an install that left files of a package's earlier
# version where they were, which the new version no longer installs,
# instead of moving them into the update's backup. $count is how many, a
# number.
wizard-summary-left-in-place-hint = { $count ->
    [one] Un fichier d'une version précédente est resté où il était. RABBIT n'en garde plus la trace et ne peut pas restaurer cette version. « Afficher les détails » indique lequel et pourquoi.
   *[other] { $count } fichiers de versions précédentes sont restés où ils étaient. RABBIT n'en garde plus la trace et ne peut pas restaurer ces versions. « Afficher les détails » indique lesquels et pourquoi.
}
wizard-summary-package-message = { $package } : { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Action prévue : { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Statut : { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Version installée : { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architecture : { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Supprimer aussi les réglages : { $value }
# One line per file, below its package's line. $path is a full path.
# wizard-summary-left-over can also name a folder, such as a package's
# uninstaller folder in Program Files, or a registry key as regedit shows
# it; keep either as it is, never translated.
wizard-summary-removed-file = Supprimé : { $path }
wizard-summary-kept-file = Conservé, car il a changé depuis que RABBIT l'a installé : { $path }
wizard-summary-kept-link = Conservé, car il contient un lien que RABBIT ne suit jamais : { $path }
wizard-summary-kept-reapack = Conservé, car ReaPack l'a installé et en garde la trace : { $path }
wizard-summary-left-over = Toujours là : { $path }
wizard-summary-left-in-backup = N'a pas pu être remis en place, toujours dans la sauvegarde : { $path }
# A file a package's installer added outside REAPER's folder that stays:
# it was there before RABBIT ran that installer, or RABBIT recorded it more
# than once with contents that disagree. "the installer" is the package's
# own installer. $path is a full path as Windows holds it, never
# translated.
wizard-summary-kept-there-before = Conservé, car il était déjà là quand RABBIT a lancé le programme d'installation : { $path }
wizard-summary-kept-record-unclear = Conservé, car les enregistrements de RABBIT à son sujet ne concordent pas : { $path }
wizard-summary-pending-restart = Supprimé au redémarrage de l'ordinateur : { $path }
wizard-summary-vendor-uninstaller = Son propre programme de désinstallation : { $path }
# Below a package removed on macOS, one line per path: what went to the Trash
# and where it is there now, and what the administrator command removed; then
# the installer receipts macOS still lists, and macOS's own words when a step
# failed. On Windows, the line for what was removed with administrator rights
# also names a package's uninstaller folder in Program Files, and each file
# in it, that RABBIT removed that way. $path is a full Windows or macOS path,
# keep it as it is; $ids a comma-separated list of receipt names, never
# translated; $error macOS's own message. Name the Trash as Finder does.
wizard-summary-moved-to-trash = Placé dans la Corbeille : { $path }
wizard-summary-in-trash-as = Maintenant dans la Corbeille : { $path }
wizard-summary-removed-as-admin = Supprimé avec les droits d'administrateur : { $path }
wizard-summary-installer-records-remain = Toujours connu du programme d'installation de macOS : { $ids }
wizard-summary-macos-said = macOS a indiqué : { $error }
# What became of a package's entry in Windows' list of installed programs,
# below the package's line. $key is the entry's registry key, such as
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA, and is
# never translated; $error is Windows' own error message.
wizard-summary-registry-removed = Retirée de la liste des programmes installés de Windows : { $key }
wizard-summary-registry-kept = Toujours dans la liste des programmes installés de Windows, car l'entrée ne pointe pas vers ce REAPER : { $key }
wizard-summary-registry-failed = Toujours dans la liste des programmes installés de Windows, car RABBIT n'a pas pu la lire ni la retirer ({ $error }) : { $key }
# Below a package whose uninstaller folder and registry entry RABBIT removed
# with administrator rights: the registry entry. $key is the entry's
# registry key as regedit shows it, such as
# HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts,
# and is never translated. The folder and its files are listed with
# wizard-summary-removed-as-admin.
wizard-summary-registry-key-removed = Supprimée du registre avec les droits d'administrateur : { $key }
# One line per uninstalled package whose files went into the backup.
# $package is the localized package name. Name the tab as
# wizard-packages-tab-backups does.
wizard-summary-uninstall-restorable = Vous pouvez restaurer { $package } dans l'onglet « Sauvegardes ».
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Rapport : { $path }
# The Done page after a delete of backups. Its first line, one of the seven
# below, also ends the progress page. Backups are counted as Review counts
# them, one for each ticked row. $count and $deleted are numbers: the
# backups ticked, or in the first message those deleted, and of them those
# deleted. $size is a size, such as "520 MB".
wizard-done-delete-status-success = RABBIT a supprimé { $count ->
    [one] une sauvegarde
   *[other] { $count } sauvegardes
} et libéré { $size }.
wizard-done-delete-status-partial = RABBIT a supprimé { $deleted ->
    [one] une sauvegarde
   *[other] { $deleted } sauvegardes
} sur { $count }.
wizard-done-delete-status-none = { $count ->
    [one] RABBIT n'a pas pu supprimer la sauvegarde.
   *[other] RABBIT n'a pas pu supprimer les sauvegardes.
}
wizard-done-delete-status-already-gone = { $count ->
    [one] La sauvegarde n'existait déjà plus, il n'y avait donc rien à supprimer.
   *[other] Les sauvegardes n'existaient déjà plus, il n'y avait donc rien à supprimer.
}
wizard-done-delete-status-stopped = { $deleted ->
    [0] RABBIT s'est arrêté avant de supprimer une sauvegarde. Celles qu'il n'avait pas encore atteintes sont conservées.
    [one] RABBIT s'est arrêté après avoir supprimé une sauvegarde sur { $count }. Celles qu'il n'avait pas encore atteintes sont conservées.
   *[other] RABBIT s'est arrêté après avoir supprimé { $deleted } sauvegardes sur { $count }. Celles qu'il n'avait pas encore atteintes sont conservées.
}
# When the user stopped the run partway through a backup, before RABBIT had
# deleted a whole one: part of a backup is gone.
wizard-done-delete-status-stopped-partway = { $count ->
    [one] RABBIT s'est arrêté avant d'avoir supprimé toute la sauvegarde.
   *[other] RABBIT s'est arrêté avant d'avoir supprimé une sauvegarde entière. Celles qu'il n'avait pas encore atteintes sont conservées.
}
# When RABBIT deleted nothing because it refused the run, or something went
# wrong before it started.
wizard-done-delete-status-nothing = Rien n'a été supprimé.
# Why backups weren't deleted, after the first line, one sentence each.
# $total is how many backups were ticked, and $count how many of them the
# sentence is about; both are numbers. The [1] form is for a delete of a
# single backup: the first line calls it "the backup", so the sentence
# calls it "it". Name "Show details" as wizard-done-show-details does, and
# the Backups tab as wizard-packages-tab-backups does.
wizard-done-delete-in-use = { $total ->
    [1] Un de ses fichiers est ouvert dans un autre programme. Fermez ce programme, puis relancez RABBIT pour la supprimer.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée, car un de ses fichiers est ouvert dans un autre programme. Fermez ce programme, puis relancez RABBIT pour la supprimer.
       *[other] { $count } n'ont pas pu être supprimées, car un de leurs fichiers est ouvert dans un autre programme. Fermez ce programme, puis relancez RABBIT pour les supprimer.
    }
}
wizard-done-delete-permission = { $total ->
    [1] RABBIT n'a pas le droit de la déplacer. Elle reste telle quelle.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée, car RABBIT n'a pas le droit de la déplacer. Elle reste telle quelle.
       *[other] { $count } n'ont pas pu être supprimées, car RABBIT n'a pas le droit de les déplacer. Elles restent telles quelles.
    }
}
# For a package or key map RABBIT couldn't take out of the backup set it is
# in, because the set's own record of what it holds was open in another
# program, or RABBIT isn't allowed to change it. Nothing of that set
# changed. $total and $count as above.
wizard-done-delete-record-in-use = { $total ->
    [1] La liste de son contenu est ouverte dans un autre programme. Fermez ce programme, puis relancez RABBIT pour la supprimer.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée, car la liste de son contenu est ouverte dans un autre programme. Fermez ce programme, puis relancez RABBIT pour la supprimer.
       *[other] { $count } n'ont pas pu être supprimées, car la liste de leur contenu est ouverte dans un autre programme. Fermez ce programme, puis relancez RABBIT pour les supprimer.
    }
}
wizard-done-delete-record-permission = { $total ->
    [1] RABBIT n'a pas le droit de modifier la liste de son contenu. Elle reste telle quelle.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée, car RABBIT n'a pas le droit de modifier la liste de son contenu. Elle reste telle quelle.
       *[other] { $count } n'ont pas pu être supprimées, car RABBIT n'a pas le droit de modifier la liste de leur contenu. Elles restent telles quelles.
    }
}
wizard-done-delete-earlier-deletion-left = { $total ->
    [1] Une suppression précédente de cette sauvegarde ne s'est pas terminée, et RABBIT n'a toujours pas pu la terminer, la sauvegarde reste donc telle quelle. Relancez RABBIT pour réessayer.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée, car une suppression précédente ne s'est pas terminée et RABBIT n'a toujours pas pu la terminer. Elle reste telle quelle. Relancez RABBIT pour réessayer.
       *[other] { $count } n'ont pas pu être supprimées, car des suppressions précédentes ne se sont pas terminées et RABBIT n'a toujours pas pu les terminer. Elles restent telles quelles. Relancez RABBIT pour réessayer.
    }
}
wizard-done-delete-other = { $total ->
    [1] Quelque chose s'est mal passé, et elle reste telle quelle. « Afficher les détails » indique quoi.
   *[other] { $count ->
        [one] Une n'a pas pu être supprimée et reste telle quelle. « Afficher les détails » indique pourquoi.
       *[other] { $count } n'ont pas pu être supprimées et restent telles quelles. « Afficher les détails » indique pourquoi.
    }
}
wizard-done-delete-partly = { $total ->
    [1] RABBIT ne l'a supprimée qu'en partie. Le reste est une suppression inachevée, qu'aucune restauration n'utilise plus. Pour la terminer, relancez RABBIT et supprimez-la dans l'onglet « Sauvegardes ».
   *[other] { $count ->
        [one] Une n'a été supprimée qu'en partie. Son reste est une suppression inachevée, qu'aucune restauration n'utilise plus. Pour la terminer, relancez RABBIT et supprimez-la dans l'onglet « Sauvegardes ».
       *[other] { $count } n'ont été supprimées qu'en partie. Leurs restes sont des suppressions inachevées, qu'aucune restauration n'utilise plus. Pour les terminer, relancez RABBIT et supprimez-les dans l'onglet « Sauvegardes ».
    }
}
# The row for the old copies of RABBIT's records stands for several
# backups on disk, and only it can end like this: RABBIT deleted some of
# them, and couldn't delete the others, or the user stopped the run before
# it reached them.
wizard-done-delete-part-left = { $total ->
    [1] RABBIT en a supprimé une partie, mais n'a pas pu supprimer le reste, qui est conservé tel quel. « Afficher les détails » indique pourquoi.
   *[other] { $count ->
        [one] Une n'a été supprimée qu'en partie : RABBIT n'a pas pu en supprimer le reste, qui est conservé tel quel. « Afficher les détails » indique pourquoi.
       *[other] { $count } n'ont été supprimées qu'en partie : RABBIT n'a pas pu en supprimer le reste, qui est conservé tel quel. « Afficher les détails » indique pourquoi.
    }
}
wizard-done-delete-stopped-partway = { $total ->
    [1] RABBIT en a supprimé une partie avant de s'arrêter. Le reste est conservé.
   *[other] { $count ->
        [one] Une n'a été supprimée qu'en partie avant que RABBIT ne s'arrête. Le reste est conservé.
       *[other] { $count } n'ont été supprimées qu'en partie avant que RABBIT ne s'arrête. Le reste est conservé.
    }
}
# After the first line, for packages or key maps RABBIT deleted out of the
# backup set they were kept in, when some of their files couldn't be
# removed, for example because another program had one open. Those files
# stay in that set with nothing naming them, so no row lists them, and they
# still take up room until the rest of the set is deleted. $total and
# $count as above.
wizard-done-delete-files-left = { $total ->
    [1] Certains de ses fichiers n'ont pas pu être retirés. Ils restent dans la sauvegarde où elle était conservée et y occupent de la place jusqu'à ce que le reste de cette sauvegarde soit supprimé.
   *[other] { $count ->
        [one] Une a été supprimée, mais certains de ses fichiers n'ont pas pu être retirés. Ils restent dans la sauvegarde où elle était conservée et y occupent de la place jusqu'à ce que le reste de cette sauvegarde soit supprimé.
       *[other] { $count } ont été supprimées, mais certains de leurs fichiers n'ont pas pu être retirés. Ils restent dans les sauvegardes où elles étaient conservées et y occupent de la place jusqu'à ce que le reste de ces sauvegardes soit supprimé.
    }
}
# After the first line, when some of the ticked backups were gone before the
# run. $count is a number, fewer than the backups ticked.
wizard-done-delete-already-gone = { $count ->
    [one] Une n'existait déjà plus.
   *[other] { $count } n'existaient déjà plus.
}
# After "Nothing was deleted." when a ticked backup can't be deleted, which
# refuses the whole run; said only when more than one backup was ticked.
wizard-done-delete-refused-run = RABBIT ne supprime les sauvegardes cochées que s'il peut toutes les supprimer.
# Why a backup can't be deleted. $backup is what its row calls it with when
# it was made, as wizard-backups-row-reference says it. It comes after a
# colon, so a name like "Your key map" keeps its capital. $reason is one of
# the wizard-delete-refusal-* sentences, which calls that backup "it".
wizard-done-delete-refused-backup = RABBIT ne peut pas supprimer cette sauvegarde : { $backup }. { $reason }
# When RABBIT couldn't read REAPER's resource folder, or a folder of its own
# backups in it, so it deleted nothing. $path is that folder as the computer
# writes it, keep it as it is.
wizard-done-delete-folder-unreadable = RABBIT n'a pas pu lire le dossier { $path }. Vérifiez qu'il existe et que vous pouvez l'ouvrir, puis relancez RABBIT.
# When something else went wrong before RABBIT deleted anything. $error is
# the error message, in English.
wizard-done-delete-error = Quelque chose s'est mal passé avant que RABBIT ne supprime quoi que ce soit : { $error }
# Show details after a delete of backups: one line per ticked backup, then
# lines indented below it. $backup is what its row calls it, with when it
# was made, as wizard-backups-row-reference says it, when another ticked
# backup has the same name. $status is one of the wizard-delete-status-*
# labels, $size a size, such as "4 MB", and $path a folder as the computer
# writes it, keep it as it is. Name the Backups tab as
# wizard-packages-tab-backups does. The lines below say "it" of the backup.
wizard-summary-delete-backup = { $backup } : { $status }
wizard-summary-delete-freed = Espace libéré : { $size }
# Below a package or key map RABBIT deleted, when some of its files
# couldn't be removed: they stay in the backup set it was in, with nothing
# naming them, and go when the rest of that set is deleted. $count is how
# many, a number.
wizard-summary-delete-files-left = { $count ->
    [one] Un de ses fichiers n'a pas pu être retiré et reste dans la sauvegarde où il était conservé. Il disparaît quand le reste de cette sauvegarde est supprimé.
   *[other] { $count } de ses fichiers n'ont pas pu être retirés et restent dans la sauvegarde où ils étaient conservés. Ils disparaissent quand le reste de cette sauvegarde est supprimé.
}
# Below a backup RABBIT deleted, or tried to, when deleting the whole backup
# set it was kept in also finished an unfinished deletion of that set,
# which wasn't ticked: the Backups tab listed it as an unfinished deletion.
# Said once for each set.
wizard-summary-delete-finished-earlier-deletion-too = RABBIT a aussi terminé une suppression inachevée de la sauvegarde où elle était conservée.
# What happened to a backup that wasn't deleted, then why, in a line of
# its own. Part of the row for the old copies of RABBIT's records, which
# stands for several backups on disk, may have gone though the rest didn't.
wizard-summary-delete-nothing-changed = Rien n'a changé.
wizard-summary-delete-part-left = RABBIT en a supprimé une partie. Le reste n'a pas pu être supprimé et est conservé tel quel.
wizard-summary-delete-in-use = Un de ses fichiers est ouvert dans un autre programme, elle n'a donc pas pu être déplacée.
wizard-summary-delete-permission = RABBIT n'a pas le droit de la déplacer.
# Why a package or key map couldn't be taken out of the backup set it is
# in: the set's own record of what it holds was open in another program, or
# RABBIT isn't allowed to change it.
wizard-summary-delete-record-in-use = La liste de son contenu est ouverte dans un autre programme, elle n'a donc pas pu être modifiée.
wizard-summary-delete-record-permission = RABBIT n'a pas le droit de modifier la liste de son contenu.
wizard-summary-delete-earlier-deletion-left = Une suppression précédente ne s'est pas terminée, et RABBIT n'a toujours pas pu la terminer. Ce qui reste de cette suppression se trouve dans { $path }.
wizard-summary-delete-left-in = Ce que RABBIT n'a pas pu retirer reste dans { $path }. L'onglet « Sauvegardes » l'affiche comme une suppression inachevée, qu'aucune restauration n'utilise plus ; la supprimer là-bas la termine.
wizard-summary-delete-not-reached-stopped = RABBIT s'est arrêté avant de l'atteindre, elle est donc conservée.
wizard-summary-delete-stopped-partway = RABBIT en a supprimé une partie, puis s'est arrêté. Le reste est conservé.
wizard-summary-delete-not-reached-refused = RABBIT n'a rien supprimé lors de cette opération, elle est donc conservée.
# For a backup RABBIT didn't get to, because it couldn't delete, or check,
# another ticked backup kept in the same backup set: RABBIT then leaves
# every backup of that set as it was, and goes on with the next set.
wizard-summary-delete-not-reached-kept-with = RABBIT l'a laissée telle quelle, car il n'a pas pu supprimer une autre sauvegarde cochée conservée avec elle.
# Below a ticked backup kept in the same backup set as a ticked backup
# above it, when RABBIT couldn't delete that set, deleted it only in part,
# or couldn't change its record of what it holds: what happened, and why,
# is the same for both, and is said only below the first. $backup is what
# that one's line calls it, as wizard-summary-delete-backup names it. It
# comes after a colon, so its capital stays.
wizard-summary-delete-same-as-above = Ce qui lui est arrivé, et pourquoi, est indiqué plus haut, sous la sauvegarde conservée avec elle : { $backup }.
wizard-summary-delete-freed-total = Espace libéré au total : { $size }
# $path is the full path of the report RABBIT saved.
wizard-summary-delete-report = Rapport : { $path }
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
wizard-done-restore-status-one = RABBIT a restauré { $backup }.
wizard-done-restore-status-earlier = { $package } est revenu à la version { $version }.
wizard-done-restore-status-earlier-copy = { $package } est revenu à la copie qu'une mise à jour avait remplacée.
wizard-done-restore-status-keymap = { $origin ->
    [own] Vos raccourcis clavier du { $date } sont de retour.
    [default-keys] Les touches par défaut de REAPER du { $date } sont de retour.
   *[other] Les raccourcis clavier du { $date } sont de retour.
}
wizard-done-restore-status-keymap-undated = { $origin ->
    [own] Vos raccourcis clavier sont de retour.
    [default-keys] Les touches par défaut de REAPER sont de retour.
   *[other] Les raccourcis clavier sont de retour.
}
# What the Done page and Review call a key map inside a sentence, as
# $backup in wizard-done-restore-status-none, in the sentences on why a
# backup didn't come back, such as "RABBIT couldn't restore your key map
# from <date>", and in Review's on why one can't be restored yet. Start in
# lower case where your language allows. $origin and $date as in
# wizard-progress-status-restoring-keymap.
wizard-done-restore-keymap-in-sentence = { $origin ->
    [own] vos raccourcis clavier du { $date }
    [default-keys] les touches par défaut de REAPER du { $date }
   *[other] les raccourcis clavier du { $date }
}
wizard-done-restore-keymap-in-sentence-undated = { $origin ->
    [own] vos raccourcis clavier
    [default-keys] les touches par défaut de REAPER
   *[other] les raccourcis clavier
}
# Several backups, all of them back; some of them; none, with nothing
# changed; none, where files RABBIT had changed couldn't be put back as they
# were; and a run the user stopped.
wizard-done-restore-status-success = RABBIT a restauré { $count ->
    [one] une sauvegarde
   *[other] { $count } sauvegardes
}.
wizard-done-restore-status-partial = { $restored ->
    [one] RABBIT a restauré une sauvegarde sur { $count }.
   *[other] RABBIT a restauré { $restored } sauvegardes sur { $count }.
}
wizard-done-restore-status-none = { $count ->
    [one] RABBIT n'a pas pu restaurer { $backup } et a tout laissé tel quel.
   *[other] RABBIT n'a pas pu restaurer les sauvegardes et a tout laissé tel quel.
}
wizard-done-restore-status-none-changed = { $count ->
    [one] RABBIT n'a pas pu restaurer { $backup }.
   *[other] RABBIT n'a pas pu restaurer les sauvegardes.
}
wizard-done-restore-status-stopped = { $restored ->
    [0] RABBIT s'est arrêté avant de restaurer une sauvegarde. Celles qu'il n'avait pas encore atteintes restent telles quelles.
    [one] RABBIT s'est arrêté après avoir restauré une sauvegarde sur { $count }. Celles qu'il n'avait pas encore atteintes restent telles quelles.
   *[other] RABBIT s'est arrêté après avoir restauré { $restored } sauvegardes sur { $count }. Celles qu'il n'avait pas encore atteintes restent telles quelles.
}
# When RABBIT restored nothing because it refused the run, or something went
# wrong before it started.
wizard-done-restore-status-nothing = Rien n'a été restauré.
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
   *[other] RABBIT n'a pas restauré { $backup }. { $reason }
}
wizard-done-restore-failed = { $total ->
    [1] Quelque chose s'est mal passé. « Afficher les détails » indique quoi.
   *[other] Quelque chose s'est mal passé avec { $backup }, donc RABBIT n'y a rien changé. « Afficher les détails » indique quoi.
}
wizard-done-restore-files-left = { $total ->
    [1] Certains fichiers que RABBIT avait déjà modifiés n'ont pas pu être remis comme avant. Les fichiers qu'ils ont remplacés sont gardés comme sauvegarde dans l'onglet « Sauvegardes ». « Afficher les détails » indique lesquels.
   *[other] RABBIT n'a pas pu restaurer { $backup }, et certains fichiers qu'il avait déjà modifiés n'ont pas pu être remis comme avant. Les fichiers qu'ils ont remplacés sont gardés comme sauvegarde dans l'onglet « Sauvegardes ». « Afficher les détails » indique lesquels.
}
# The same, when the restore had replaced nothing, so no backup keeps what
# it replaced: the files it couldn't take out again had nothing before them.
wizard-done-restore-files-left-no-backup = { $total ->
    [1] Certains fichiers que RABBIT avait déjà modifiés n'ont pas pu être remis comme avant. « Afficher les détails » indique lesquels.
   *[other] RABBIT n'a pas pu restaurer { $backup }, et certains fichiers qu'il avait déjà modifiés n'ont pas pu être remis comme avant. « Afficher les détails » indique lesquels.
}
wizard-done-restore-partly = { $total ->
    [1] Une partie seulement est revenue. « Afficher les détails » indique laquelle.
   *[other] RABBIT n'a restauré { $backup } qu'en partie. « Afficher les détails » indique quelle partie.
}
# After a refusal that ends "then try again" or "wait until it has
# finished", such as REAPER being open: Done has no way back, so the user
# tries again by starting RABBIT again.
wizard-done-restore-start-again = Pour réessayer, fermez RABBIT et relancez-le.
# When RABBIT couldn't read REAPER's resource folder, or a folder of its own
# backups in it, so it restored nothing. $path is that folder as the computer
# writes it, keep it as it is.
wizard-done-restore-folder-unreadable = RABBIT n'a pas pu lire le dossier { $path }. Vérifiez qu'il existe et que vous pouvez l'ouvrir, puis relancez RABBIT.
# When something else went wrong before RABBIT restored anything. $error is
# the error message, in English.
wizard-done-restore-error = Quelque chose s'est mal passé avant que RABBIT ne restaure quoi que ce soit : { $error }
# What else the restore changed, after the lines above, and in Show
# details. $file is a language pack's file name, such as
# de_DE.ReaperLangPack, never translated; $package is the localized package
# name, $packages several joined as wizard-backups-list-and joins names, and
# $count how many, a number.
wizard-done-restore-language-back = La langue de REAPER est de nouveau { $file }.
wizard-done-restore-language-stays = La langue de REAPER reste telle quelle, car une autre a été choisie depuis.
wizard-done-restore-language-failed = RABBIT n'a pas pu remettre { $file } comme langue de REAPER. « Afficher les détails » indique pourquoi.
# Going back to an earlier version of a language pack while REAPER uses
# another pack, or none: the restore leaves REAPER's language alone.
wizard-done-restore-language-unchanged = La langue de REAPER reste telle quelle.
wizard-done-restore-not-counted = RABBIT ne compte plus { $package } comme installé par lui.
wizard-done-restore-needs-reinstall = Pour désinstaller { $package } avec RABBIT plus tard, réinstallez-le d'abord.
wizard-done-restore-left-out = { $count ->
    [one] RABBIT a laissé de côté { $packages }, car aucun de ses fichiers n'était dans la sauvegarde.
   *[other] RABBIT a laissé de côté { $packages }, car aucun de leurs fichiers n'était dans la sauvegarde.
}
# Show details after a restore of backups: one line per ticked backup, then
# lines indented below it, which say "it" of the backup. $backup is what the
# row calls it, with when it was made, as wizard-backups-row-reference says
# it, when another ticked backup has the same name; $status is one of the
# wizard-restore-status-* labels, and $path a file or folder as the computer
# writes it, keep it as it is.
wizard-summary-restore-backup = { $backup } : { $status }
# Each file the restore worked on, and what came of it. "The new backup" is
# the one this restore made of what it replaced.
wizard-summary-restore-file-restored = Restauré : { $path }
wizard-summary-restore-file-put-back = Remis comme avant : { $path }
wizard-summary-restore-file-left = N'a pas pu être remis comme avant, il est donc tel que la sauvegarde le contient : { $path }
wizard-summary-restore-file-missing = Disparu, et n'a pas pu être remis : { $path }
wizard-summary-restore-replaced-copy = Une copie de ce qui s'y trouvait avant est dans { $path }
wizard-summary-restore-file-unfinished = Non restauré, car sa copie dans la sauvegarde n'a jamais été terminée : { $path }
# Files of the version a package had, when it went back to an earlier one.
wizard-summary-restore-moved-out = Déplacé dans la nouvelle sauvegarde, car la version antérieure ne l'a pas : { $path }
wizard-summary-restore-still-in-backup = N'a pas pu retourner à sa place, et reste dans la nouvelle sauvegarde : { $path }
wizard-summary-restore-changed-left = Reste où il est, car il a changé depuis que RABBIT l'a installé : { $path }
wizard-summary-restore-left-in-place = Reste où il est, avec une copie dans la nouvelle sauvegarde : { $path }
# The scripts a key map restore kept in REAPER's Actions list, which were
# registered since that key map was replaced. Name the Actions list as
# REAPER does in your language. $count is a number.
wizard-summary-restore-scripts-kept = { $count ->
    [one] RABBIT a gardé dans la liste d'actions de REAPER un script enregistré depuis.
   *[other] RABBIT a gardé dans la liste d'actions de REAPER { $count } scripts enregistrés depuis.
}
# The backup this restore made of what it replaced. $backup is what the
# Backups tab's row calls it, such as "Key map" or "ReaPack 1.2.5", and
# $date a date and time (backup-date-time). The -undated form is for a
# backup whose date RABBIT can't tell. Name the Backups tab as
# wizard-packages-tab-backups does.
wizard-summary-restore-kept = Ce que la restauration a remplacé est gardé comme sauvegarde dans l'onglet « Sauvegardes » : { $backup }, du { $date }.
wizard-summary-restore-kept-undated = Ce que la restauration a remplacé est gardé comme sauvegarde dans l'onglet « Sauvegardes » : { $backup }.
wizard-summary-restore-not-reached = RABBIT s'est arrêté avant de l'atteindre, elle reste donc telle quelle.
# $path is the full path of the report RABBIT saved.
wizard-summary-restore-report = Rapport : { $path }
status-installed-or-checked = Installé ou vérifié
status-planned-unattended = Prévu sans intervention
status-deferred-unattended = Différé sans intervention
status-skipped-current = Ignoré (déjà à jour)
status-failed = Échec
status-skipped-dependency-failed = Ignoré (dépendance en échec)
status-cancelled = Non installé (vous avez arrêté)
status-skipped-left-to-reapack = Ignoré (laissé à ReaPack)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Désinstallé
status-removed-with-leftovers = Désinstallé, mais une partie est toujours présente
status-removed-pending-restart = Désinstallé ; le reste sera supprimé au redémarrage de l'ordinateur
status-would-remove = Serait désinstallé
status-remove-not-installed = N'était pas installé
status-remove-blocked = Non désinstallé
status-remove-failed = N'a pas pu être désinstallé
status-remove-cancelled = Toujours installé (vous avez arrêté)
status-remove-not-reached = Toujours installé (l'opération s'est arrêtée avant)
# What came of a backup in a delete, after its name in "Show details".
wizard-delete-status-deleted = Supprimée
wizard-delete-status-finished-earlier-deletion = Supprimée, ce qui termine une suppression commencée plus tôt
wizard-delete-status-already-gone = N'existait déjà plus
wizard-delete-status-would-delete = Serait supprimée
wizard-delete-status-would-finish-earlier-deletion = Terminerait une suppression commencée plus tôt
wizard-delete-status-not-deleted = Pas supprimée
wizard-delete-status-partly-deleted = Supprimée en partie
wizard-delete-status-refused = Ne peut pas être supprimée
wizard-delete-status-not-reached = Pas atteinte
# What came of a backup in a restore, after its name in "Show details".
wizard-restore-status-restored = Restaurée
wizard-restore-status-would-restore = Serait restaurée
wizard-restore-status-not-restored = Non restaurée
wizard-restore-status-files-left = Non restaurée ; certains fichiers n'ont pas pu être remis comme avant
wizard-restore-status-not-reached = Pas atteinte
wizard-restore-status-partly-restored = Restaurée en partie

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Binaire d'extension unique pris en charge par le programme d'installation de RABBIT.
# Show details after RABBIT put a plug-in into REAPER's own plug-in folder,
# such as one of Lo-lo's instruments or app2clap, after the package's name.
# Keep UserPlugins\FX as it is: it is the folder's name. Word it so it fits
# any package name.
package-status-plugin-installed = Installé dans le dossier UserPlugins\FX de REAPER. REAPER trouve le plug-in à son prochain démarrage ou après une nouvelle analyse des plug-ins.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La version installée { $installed } est égale ou plus récente que la version disponible { $available }.
package-status-skipped-content-unchanged = La copie installée est identique à celle publiée par l'éditeur.
# A package RABBIT left to ReaPack, even when asked for it by name: ReaPack's
# records say ReaPack installed it, or RABBIT couldn't read them.
# $reapack_package is the name ReaPack knows the package by, $error an error
# message.
package-status-skipped-managed-by-reapack = D'après les registres de ReaPack, ReaPack a installé ce paquet en tant que { $reapack_package }, RABBIT l'a donc laissé à ReaPack. Installez-le, mettez-le à jour ou supprimez-le dans ReaPack.
package-status-skipped-reapack-records-unreadable = RABBIT n'a pas pu lire les registres de ReaPack ({ $error }) et ne peut donc pas savoir si ReaPack a installé ce paquet ; il ne l'a ni installé ni mis à jour.
package-status-install-failed = Échec de l'installation : { $error }
package-status-skipped-dependency-failed = Ignoré car { $dependency }, dont il dépend, ne s'est pas installé correctement.
package-status-cancelled = Non installé : vous avez arrêté la configuration avant que RABBIT n'atteigne ce paquet.
# Shown after the package's name when its vendor installer ended with an
# exit code other than 0. $code is the number. One of the
# package-status-vendor-exit-* sentences follows, then
# package-status-vendor-installer-log when the installer left a log.
package-status-vendor-installer-failed = Le programme d'installation s'est terminé avec le code d'erreur { $code }.
# Inno Setup's exit code 2: cancelled before it began installing. RABBIT runs
# it unattended, so in practice the user declined the Windows permission
# prompt (User Account Control) the installer raised. "Oui" is the prompt's
# button.
package-status-vendor-exit-cancelled-before-installing = Cela signifie généralement que la demande d'autorisation de Windows a été refusée : rien n'a changé. Relancez RABBIT et choisissez Oui quand Windows demande l'autorisation.
# Inno Setup's exit code 5: it stopped at a question, such as whether to close
# programs that use its files, that nobody can answer while RABBIT runs it
# unattended. $package is the package's name.
package-status-vendor-exit-stopped-at-question = Cela signifie qu'il s'est arrêté sur une question qu'il ne peut pas poser pendant que RABBIT l'exécute sans intervention, généralement parce qu'un autre programme a ouvert l'un de ses fichiers. Fermez les autres programmes, en particulier les autres logiciels de musique et { $package }, puis relancez RABBIT.
# Inno Setup's exit code 7: its preparation found it can't continue.
package-status-vendor-exit-preparation-failed = Cela signifie qu'en se préparant, il a constaté qu'il ne peut pas installer pour l'instant. Redémarrez l'ordinateur, puis relancez RABBIT.
# Inno Setup's exit code 8: its preparation found Windows must restart first.
package-status-vendor-exit-restart-needed = Cela signifie qu'en se préparant, il a constaté que Windows doit d'abord redémarrer. Redémarrez l'ordinateur, puis relancez RABBIT.
# Any other exit code, or an installer whose codes RABBIT doesn't know.
# $package is the package's name.
package-status-vendor-exit-unknown = Fermez les autres programmes et relancez RABBIT. S'il s'arrête encore, installez { $package } depuis le site web de son éditeur.
# $path is the log file the installer wrote.
package-status-vendor-installer-log = Le journal du programme d'installation se trouve ici : { $path }.
# Shown after the package's name when RABBIT didn't start its installer
# because the package's own programs were open. $package is the package's
# name; $programs lists the open programs' file names, such as "Surge XT.exe".
package-status-blocking-programs-running = RABBIT n'a pas lancé le programme d'installation, car { $package } est ouvert ({ $programs }) et le programme d'installation ne peut pas remplacer un programme en cours d'exécution. Fermez-le, puis relancez RABBIT.
# $automation is one of the "package-automation-*" labels (vendor installer / archive extraction / ...).
package-status-dry-run-would-run-unattended = Simulation : RABBIT téléchargerait et exécuterait l'opération « { $automation } » sans intervention.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-staged = Cette version n'implémente pas encore le chemin d'exécution sans intervention prévu pour l'opération « { $automation } ». RABBIT a préparé l'artefact dans le cache mais ne l'a pas exécuté.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-not-staged = Cette version n'implémente pas encore le chemin d'exécution sans intervention prévu pour l'opération « { $automation } ». RABBIT n'a ni téléchargé ni exécuté l'artefact.
package-status-unattended-installed = RABBIT a exécuté le programme d'installation officiel sans intervention, vérifié les chemins cibles attendus et mis à jour le reçu RABBIT.
package-status-osara-unattended-keymap-backed-up = RABBIT a exécuté le programme d'installation officiel sans intervention, sauvegardé reaper-kb.ini, appliqué le remplacement des raccourcis clavier OSARA et mis à jour le reçu RABBIT.
package-status-osara-unattended-keymap-replaced = RABBIT a exécuté le programme d'installation officiel sans intervention, appliqué le remplacement des raccourcis clavier OSARA et mis à jour le reçu RABBIT.

# Short automation-kind labels interpolated into the per-package status
# messages above.
package-automation-installer = programme d'installation de l'éditeur
package-automation-archive = extraction d'archive
package-automation-disk-image = installation depuis une image disque
package-automation-extension-binary = installation directe de fichier

# Per-configuration-step status messages surfaced on the wizard's Done page.
# `wizard-summary-configuration-message = { $step }: { $message }` is the
# wrapper template — the `*-message` keys below are the message body only.
# $name is the human-readable remote name; $url is the index XML URL.
config-message-reapack-remote-already-present = Le dépôt ReaPack { $name } ({ $url }) est déjà configuré dans reapack.ini.
config-message-reapack-remote-added = Dépôt ReaPack { $name } ({ $url }) ajouté à reapack.ini.
config-message-reapack-remote-created-file = reapack.ini créé avec le dépôt ReaPack { $name } ({ $url }). ReaPack ajoutera ses dépôts par défaut au prochain démarrage de REAPER.
config-message-reapack-remote-dry-run = Le dépôt ReaPack { $name } ({ $url }) serait ajouté à reapack.ini.
config-message-reaper-language-already-selected = REAPER utilisait déjà { $file }.
config-message-reaper-language-selected = Langue de REAPER définie sur { $file }.
config-message-reaper-language-dry-run = Définirait la langue de REAPER sur { $file }.
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = L'étape de configuration { $step } n'a pas été sélectionnée.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = L'étape de configuration { $step } a été ignorée car son paquet prérequis { $dependency } n'était pas installé et ne fait pas partie de ce plan.
config-message-applied-no-op = Étape de configuration appliquée sans modification.
config-message-cancelled = Cette étape de configuration n'a pas été exécutée : vous avez arrêté la configuration avant.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step } : { $message }
wizard-summary-configuration-status =   Statut : { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Appliqué
config-status-skipped = Ignoré
config-status-skipped-dependency-missing = Ignoré (prérequis manquant)
config-status-dry-run = Simulation
config-status-cancelled = Non exécutée (vous avez arrêté)
wizard-summary-planned-execution-title = Exécution sans intervention prévue :
wizard-summary-planned-execution-runner =   Exécuteur : { $runner }
wizard-summary-planned-execution-artifact =   Artefact : { $artifact }
wizard-summary-planned-execution-program =   Programme : { $program }
wizard-summary-planned-execution-program-zipped =   Programme : le programme d'installation contenu dans le fichier .zip, une fois celui-ci téléchargé
wizard-summary-planned-execution-arguments =   Arguments : { $arguments }
wizard-summary-planned-execution-working-directory =   Répertoire de travail : { $path }
wizard-summary-planned-execution-verify =   Vérification : { $path }
wizard-summary-manual-title = { $title } :
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Remarque : { $note }
wizard-summary-status-finished = Terminé. { $installed } élément(s) de paquet installé(s) ou vérifié(s) ; { $manual } nécessite(nt) une intervention manuelle.
wizard-summary-status-finished-with-errors = Terminé avec des erreurs. { $installed } élément(s) de paquet installé(s) ou vérifié(s) ; { $failed } en échec.

wizard-planned-runner-launch-installer = Lancer l'exécutable d'installation
wizard-planned-runner-extract-archive = Extraire l'archive et exécuter le programme d'installation qu'elle contient
wizard-planned-runner-extract-archive-copy-osara = Extraire l'archive et copier les ressources d'installation d'OSARA
wizard-planned-runner-mount-disk-image = Monter l'image disque et exécuter le programme d'installation qu'elle contient
wizard-planned-runner-mount-disk-image-copy-app = Monter l'image disque et copier le bundle d'application qu'elle contient
wizard-planned-runner-mount-disk-image-run-pkg = Monter l'image disque et exécuter le programme d'installation pkg qu'elle contient

# Mode expert et canaux de versions.
package-channel-reaper-dev = Versions de développement de landoleet.org
package-channel-osara-pr = Versions de test des pull requests
wizard-window-title-expert = { $title } – Mode expert
wizard-expert-enable-title = Activer le mode expert ?
wizard-expert-enable-body = Le mode expert vous permet d'installer des préversions : les versions de développement de REAPER depuis landoleet.org et les versions de test des pull requests d'OSARA. Elles ne sont pas prises en charge, peuvent casser REAPER ou son accessibilité et ne servent qu'aux tests. Deux choix de version apparaissent sur cette page, sous l'installation de REAPER. Activer le mode expert ?
wizard-expert-disabled-title = Le mode expert est désactivé
wizard-expert-disabled-body = Le mode expert est désactivé. Tout paquet installé depuis une préversion revient à sa version normale lors de la prochaine installation.
wizard-expert-first-page-title = Mode expert
wizard-expert-first-page-body = Le mode expert ne s'active ou ne se désactive que sur la première page, où il ajoute les choix de version. Revenez à la première page pour le modifier.
wizard-expert-busy-body = Le mode expert ne peut pas être modifié pendant l'installation ni une fois celle-ci terminée.
wizard-expert-reaper-builds-label = Versions de REAPER
wizard-expert-reaper-builds-stable = Versions normales
wizard-expert-osara-builds-label = Versions d'OSARA
wizard-expert-osara-builds-snapshot = Snapshots normaux
wizard-expert-osara-builds-loading = Recherche des versions de pull requests…
wizard-expert-osara-builds-unavailable = Snapshots normaux (impossible d'obtenir les versions de pull requests)
wizard-expert-osara-builds-pr = Pull request { $number } : { $title }
wizard-expert-osara-builds-pr-untitled = Pull request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = version de développement
wizard-package-channel-dev-details = { $package } sera installé depuis ses versions de développement : des préversions non prises en charge qui peuvent casser à tout moment.
wizard-package-channel-pr = version de test de la pull request { $number }
wizard-package-channel-pr-details = { $package } sera installé depuis la version de test de la pull request { $number } : du code non relu et non signé, que GitHub supprime au bout de 90 jours.
wizard-package-channel-back = retour à la version normale
wizard-package-channel-back-details = { $package } est actuellement une préversion. Cette installation remet la version normale.
