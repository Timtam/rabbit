app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = oui
common-no = non

action-install = À installer
action-update = À mettre à jour
action-keep = Ne pas modifier
action-uninstall = À désinstaller

package-reaper = REAPER
package-osara = OSARA
package-sws = Extension SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Scripts JAWS de Snowman pour REAPER
package-ffmpeg = FFmpeg (prise en charge vidéo améliorée)
package-surge-xt = Surge XT
package-app2clap = app2clap
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
package-ffmpeg-description = Les bibliothèques d'exécution partagées de FFmpeg permettent au décodeur vidéo de REAPER d'importer et de lire les formats audio et vidéo courants. RABBIT installe le dossier bin de la version « GPL-shared » de BtbN dans UserPlugins ; le niveau de correctif ne peut pas être déduit des seuls noms de fichiers DLL, c'est pourquoi les installations externes de FFmpeg sont signalées avec un substitut « <major>.0.0 ».
package-surge-xt-description = Surge XT est un synthétiseur hybride gratuit et open source. RABBIT exécute le programme d'installation de l'éditeur pour vous — il installe les formats VST3, CLAP, AU (macOS uniquement) et autonome à l'échelle du système, afin que REAPER et les autres DAW puissent charger Surge XT. RABBIT suit le canal « nightly » continu, car la dernière version stable (1.3.4) date d'août 2024 et le projet est désormais distribué quasi exclusivement via les nightlies. Installations standard de REAPER uniquement : les données d'usine se trouvent en dehors de tout dossier REAPER portable.
package-app2clap-description = app2clap est un plug-in CLAP pour Windows qui capture l'audio d'autres applications et l'amène dans REAPER (ou tout hôte CLAP) sous forme de plug-in à insérer sur une piste — pratique pour enregistrer ou traiter le son d'un navigateur, d'un lecteur multimédia ou d'un autre programme. RABBIT télécharge la dernière version et installe app2clap.clap dans votre dossier CLAP personnel, sans droits d'administrateur. Windows uniquement. Installations standard de REAPER uniquement : l'installation se fait en dehors de tout dossier REAPER portable.
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
uninstall-summary-app2clap-removes = Supprime app2clap de votre dossier CLAP. Tous les programmes dans lesquels vous utilisez des plug-ins CLAP le perdent, pas seulement REAPER.
uninstall-summary-app2clap-keeps = Le dossier CLAP et les autres plug-ins qu'il contient sont conservés.
uninstall-summary-langpack-removes = Supprime le pack de langue que RABBIT a installé. Si REAPER l'utilise, RABBIT remet REAPER en anglais.
uninstall-summary-langpack-keeps = Les packs de langue que vous avez ajoutés vous-même sont conservés.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = vos grooves
uninstall-kept-sws-settings = réglages de SWS
uninstall-kept-sws-cycle-actions = vos actions cycliques
uninstall-kept-reapack-settings = réglages de ReaPack et vos dépôts
uninstall-kept-reapack-data = la liste de ce que ReaPack a installé, et son cache de téléchargement
uninstall-kept-reakontrol-fx-maps = vos mappages FX
uninstall-kept-app2clap-clap-folder = le dossier CLAP, partagé avec d'autres plug-ins

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Si vous utilisez les raccourcis clavier d'OSARA, certains d'entre eux lancent des actions SWS. Ces raccourcis cessent de fonctionner jusqu'à ce que vous réinstalliez SWS.
uninstall-warning-sws-reapack-scripts = De nombreux scripts REAPER, dont des scripts d'accessibilité, ont besoin de SWS. Les scripts que vous avez installés avec ReaPack risquent de ne plus fonctionner.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Les scripts et extensions que vous avez installés avec ReaPack sont conservés, mais plus rien ne les tient à jour.

# How to remove a package by hand where RABBIT doesn't, shown line by line.
# Keep file names, folder paths and %APPDATA% exactly as they are.
uninstall-manual-jaws-scripts = RABBIT ne peut pas encore supprimer les scripts JAWS. Pour les supprimer à la main :
    1. Fermez REAPER.
    2. Supprimez Reaper_ComAccess32.dll et Reaper_ComAccess64.dll de %APPDATA%\REAPER\UserPlugins.
    Les fichiers des scripts restent dans le dossier de paramètres de JAWS, %APPDATA%\Freedom Scientific\JAWS. Certains contiennent vos propres modifications, comme vos affectations de touches.
uninstall-manual-reaper-portable = RABBIT ne supprime pas un REAPER portable : les fichiers de REAPER et les vôtres partagent son dossier. Pour le supprimer à la main :
    1. Fermez REAPER.
    2. Copiez hors du dossier du REAPER portable tout ce que vous voulez garder, comme les projets que vous y avez enregistrés, vos raccourcis clavier et vos réglages.
    3. Supprimez le dossier du REAPER portable.
    Les sauvegardes que RABBIT a faites pour ce REAPER se trouvent aussi dans ce dossier, sous RABBIT\backups, et disparaissent avec lui.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (non disponible : { $reason })
wizard-package-row-unavailable-portable = cible REAPER portable
wizard-package-row-unavailable-version-check = échec de la vérification de version en ligne

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

wizard-packages-heading = Choisissez quoi installer, mettre à jour ou désinstaller
wizard-packages-list-label = Installer ou mettre à jour : paquets et réglages
wizard-packages-row-checked = Coché
wizard-packages-row-unchecked = Non coché
wizard-packages-choice-shown = Nouveau choix sous la liste : { $choice }.
wizard-packages-choice-hidden = Choix retiré : { $choice }.
wizard-packages-tree-group-label = Paquets
wizard-additional-software-tree-group-label = Logiciels supplémentaires
wizard-language-tree-group-label = Packs de langue
wizard-configuration-tree-group-label = Configuration

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Installer ou mettre à jour
wizard-packages-tab-uninstall = Désinstaller
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
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack gère { $package } : il a installé { $path } en tant que { $reapack_package }. Désinstallez-le plutôt dans ReaPack.
wizard-uninstall-details-reapack-check-failed = RABBIT n'a pas pu vérifier les registres de ReaPack pour { $path } ({ $reason }), il ne touche donc pas à { $package }.
wizard-uninstall-details-portable = { $package } sert à tous les REAPER de cet ordinateur, RABBIT ne le supprime donc que du REAPER standard, pas d'un REAPER portable.
wizard-uninstall-details-no-receipt = RABBIT n'a pas installé cette copie de { $package } : c'est très probablement la vôtre, et RABBIT n'y touche pas.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT n'a trouvé aucun désinstalleur de { $package } auquel il peut se fier.
wizard-uninstall-details-unsafe-path = RABBIT ne supprimera pas { $path }. Il préfère ne pas toucher à { $package } plutôt que de n'en supprimer qu'une partie.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT déplace les fichiers dans une sauvegarde au lieu de les effacer.
wizard-uninstall-details-dependents-stay = Ces paquets s'installent par-dessus { $package } et restent : { $packages }. Ils risquent de ne pas fonctionner sans lui.
wizard-uninstall-details-machine-wide = { $package } sert à tous les REAPER et à tous les autres programmes de cet ordinateur, qui le perdent tous.
wizard-uninstall-details-reapack-owned-extensions = ReaPack a installé ceux-ci, et ils restent sans mises à jour une fois ReaPack supprimé : { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT n'a pas pu lire les registres de ReaPack pour savoir ce qu'il gère encore ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT n'a pas installé cette copie. Il supprime les fichiers que { $package } est connu pour installer.
wizard-uninstall-details-reapack-orphan = ReaPack a autrefois installé { $package }. ReaPack n'est plus là, RABBIT peut donc le supprimer ; les registres de ReaPack à son sujet restent.
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
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. $packages is a comma-separated list of package names.
wizard-review-inactive-tab-note-install = Ce que vous avez coché dans l'onglet « Installer ou mettre à jour » ne fait pas partie de cette opération : { $packages }.
wizard-review-inactive-tab-note-uninstall = Ce que vous avez coché dans l'onglet « Désinstaller » ne fait pas partie de cette opération : { $packages }.
wizard-review-uninstall-reinstall-hint = Vous pourrez les réinstaller plus tard depuis l'onglet « Installer ou mettre à jour ».
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Désinstallation impossible pour le moment : { $reason }
wizard-review-uninstall-not-writable = RABBIT ne peut pas écrire dans le dossier de ce REAPER.

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
# stop. An uninstall stops after the package it is on, not after a step.
wizard-progress-uninstall-heading = Désinstallation
wizard-progress-status-uninstall-running = Désinstallation des paquets cochés.
wizard-progress-details-uninstall-starting = Démarrage de la désinstallation.
wizard-progress-status-uninstall-cancelling = Arrêt en cours. RABBIT termine le paquet commencé, puis se ferme.

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
wizard-progress-log-download-started = Téléchargement de { $package }…
wizard-progress-log-download-completed = { $package } téléchargé.
wizard-progress-log-install-started = Installation de { $package }…
wizard-progress-log-install-completed = { $package } installé.
wizard-progress-log-remove-started = Désinstallation de { $package }…
wizard-progress-log-remove-completed = { $package } désinstallé.
wizard-progress-log-remove-failed = Impossible de désinstaller { $package }.
wizard-progress-log-configuration-started = Application de { $step }…
wizard-progress-log-configuration-completed = { $step } appliqué.

wizard-done-heading = Terminé
wizard-done-status-idle = Aucune installation n'a encore été lancée depuis cette fenêtre.
wizard-done-status-success = RABBIT a fini d'opérer sa magie ! Consultez les détails ci-dessous.
wizard-done-status-error = L'installation a échoué. Consultez l'erreur ci-dessous.
wizard-reaper-running-title = REAPER est toujours ouvert
wizard-reaper-running-body = Fermez REAPER, puis appuyez à nouveau sur Installer. RABBIT ne peut pas mettre à jour REAPER ni ses extensions tant que REAPER est ouvert.
wizard-error-reaper-running = RABBIT s'est arrêté parce que REAPER était ouvert. Fermez REAPER, puis relancez RABBIT.
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Un autre RABBIT ouvert est en train d'installer ou de désinstaller dans ce REAPER. Attendez qu'il ait terminé, puis relancez RABBIT.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Fermez REAPER, puis appuyez à nouveau sur Désinstaller. RABBIT ne peut rien désinstaller de REAPER tant que REAPER est ouvert.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis appuyez à nouveau sur Désinstaller. Tout REAPER de cet ordinateur peut utiliser { $packages }.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Fermez d'abord ces programmes
wizard-programs-running-uninstall-body = Fermez { $programs }, puis appuyez à nouveau sur Désinstaller.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT s'est arrêté parce qu'un REAPER était ouvert. Fermez toutes les copies de REAPER sur cet ordinateur, y compris les versions portables, puis relancez RABBIT.
wizard-error-uninstall-not-enough-space = RABBIT s'est arrêté parce qu'il n'y a pas assez d'espace libre pour la sauvegarde. Libérez de l'espace sur le disque qui contient le dossier de ressources de REAPER, puis relancez RABBIT.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } n'a pas été désinstallé, car un autre programme utilise { $path }. Fermez ce programme, puis relancez RABBIT.
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
wizard-done-uninstall-removed-before-stop = Avant que vous l'arrêtiez, RABBIT a désinstallé { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Pour en récupérer un, relancez RABBIT et cochez-le dans l'onglet « Installer ou mettre à jour ».
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } n'était plus installé, il n'y avait donc rien à supprimer.
wizard-done-uninstall-left-in-backup = Certains fichiers de { $package } n'ont pas pu être remis en place et sont toujours dans la sauvegarde. « Afficher les détails » les énumère.
wizard-done-uninstall-leftovers = RABBIT trouve encore une partie de { $package } et n'y touche pas. « Afficher les détails » indique laquelle.
wizard-done-uninstall-language-reset-failed = RABBIT n'a pas pu remettre REAPER en anglais après avoir supprimé { $package }. Choisissez une langue dans les préférences de REAPER.
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
wizard-close-during-uninstall-body = RABBIT est en train de désinstaller. Si vous arrêtez maintenant, RABBIT termine le paquet commencé, puis s'arrête et se ferme. Les paquets qu'il n'a pas encore atteints restent installés. Arrêter la désinstallation ?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Désinstaller les paquets cochés ?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Une sauvegarde ne peut pas restaurer { $package }. Pour l'avoir à nouveau, il faudra le réinstaller.
wizard-confirm-uninstall-question = Désinstaller maintenant ?

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
wizard-summary-removed-file = Supprimé : { $path }
wizard-summary-kept-file = Conservé, car il a changé depuis que RABBIT l'a installé : { $path }
wizard-summary-kept-link = Conservé, car il contient un lien que RABBIT ne suit jamais : { $path }
wizard-summary-kept-reapack = Conservé, car ReaPack l'a installé et en garde la trace : { $path }
wizard-summary-left-over = Toujours présent : { $path }
wizard-summary-left-in-backup = N'a pas pu être remis en place, toujours dans la sauvegarde : { $path }
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Sauvegarde : { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = Pour récupérer { $package }, fermez REAPER et exécutez : { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Rapport : { $path }
status-installed-or-checked = Installé ou vérifié
status-planned-unattended = Prévu sans intervention
status-deferred-unattended = Différé sans intervention
status-skipped-current = Ignoré (déjà à jour)
status-failed = Échec
status-skipped-dependency-failed = Ignoré (dépendance en échec)
status-cancelled = Non installé (vous avez arrêté)
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

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Binaire d'extension unique pris en charge par le programme d'installation de RABBIT.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La version installée { $installed } est égale ou plus récente que la version disponible { $available }.
package-status-skipped-content-unchanged = La copie installée est identique à celle publiée par l'éditeur.
package-status-install-failed = Échec de l'installation : { $error }
package-status-skipped-dependency-failed = Ignoré car { $dependency }, dont il dépend, ne s'est pas installé correctement.
package-status-cancelled = Non installé : vous avez arrêté la configuration avant que RABBIT n'atteigne ce paquet.
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
