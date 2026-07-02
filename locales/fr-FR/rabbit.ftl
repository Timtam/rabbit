app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = oui
common-no = non

action-install = À installer
action-update = À mettre à jour
action-keep = Ne pas modifier

package-reaper = REAPER
package-osara = OSARA
package-sws = Extension SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Scripts JAWS de Snowman pour REAPER
package-ffmpeg = FFmpeg (prise en charge vidéo améliorée)
package-surge-xt = Surge XT
package-app2clap = app2clap

package-reaper-description = REAPER est la station de travail audionumérique sur laquelle tout le reste repose. RABBIT peut l'installer ou le mettre à jour pour vous.
package-osara-description = OSARA est l'extension d'accessibilité open source qui rend REAPER utilisable avec un lecteur d'écran. NVDA, JAWS et Narrateur sous Windows, VoiceOver sous macOS sont tous largement répandus ; d'autres lecteurs d'écran Windows peuvent également fonctionner. Installez OSARA si vous dépendez d'un lecteur d'écran pour utiliser REAPER.
package-sws-description = L'extension SWS est un ensemble communautaire de longue date d'actions, de scripts et d'outils supplémentaires qui étendent les fonctionnalités de REAPER. Pour la configuration de REAPER la plus accessible possible, que vous soyez sous Windows ou sous Mac, installez SWS en complément d'OSARA.
package-reapack-description = ReaPack est un gestionnaire de paquets open source. Il permet de rechercher, d'installer, de suivre et de mettre à jour des scripts et des extensions tiers directement depuis REAPER. Installez-le si vous souhaitez utiliser des scripts partagés par la communauté REAPER.
package-reakontrol-description = ReaKontrol offre une intégration open source pour les claviers Komplete Kontrol de Native Instruments. Installez-le si vous possédez un clavier de série S MK2, de série A, M-32 ou Kontrol MK3.
package-jaws-scripts-description = Les scripts de Snowman améliorent la façon dont JAWS gère les différentes fenêtres de REAPER, et offrent une prise en charge étendue du braille ainsi que de nombreuses autres fonctionnalités. Notez que ces scripts sont conçus pour être utilisés en complément d'OSARA : ils n'en sont pas une alternative. Pour une accessibilité optimale avec JAWS, installez les deux.
package-ffmpeg-description = Les bibliothèques d'exécution partagées de FFmpeg permettent au décodeur vidéo de REAPER d'importer et de lire les formats audio et vidéo courants. RABBIT installe le dossier bin de la version « GPL-shared » de BtbN dans UserPlugins ; le niveau de correctif ne peut pas être déduit des seuls noms de fichiers DLL, c'est pourquoi les installations externes de FFmpeg sont signalées avec un substitut « <major>.0.0 ».
package-surge-xt-description = Surge XT est un synthétiseur hybride gratuit et open source. RABBIT exécute le programme d'installation de l'éditeur pour vous — il installe les formats VST3, CLAP, AU (macOS uniquement) et autonome à l'échelle du système, afin que REAPER et les autres DAW puissent charger Surge XT. RABBIT suit le canal « nightly » continu, car la dernière version stable (1.3.4) date d'août 2024 et le projet est désormais distribué quasi exclusivement via les nightlies. Installations standard de REAPER uniquement : les données d'usine se trouvent en dehors de tout dossier REAPER portable.
package-app2clap-description = app2clap est un plug-in CLAP pour Windows qui capture l'audio d'autres applications et l'amène dans REAPER (ou tout hôte CLAP) sous forme de plug-in à insérer sur une piste — pratique pour enregistrer ou traiter le son d'un navigateur, d'un lecteur multimédia ou d'un autre programme. RABBIT télécharge la dernière version et installe app2clap.clap dans votre dossier CLAP personnel, sans droits d'administrateur. Windows uniquement. Installations standard de REAPER uniquement : l'installation se fait en dehors de tout dossier REAPER portable.

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
wizard-button-close = Fermer
wizard-button-close-mnemonic = F

wizard-target-heading = Choisissez une tâche
wizard-target-language-label = Langue
wizard-target-language-restart-note = Changer de langue redémarre RABBIT afin que la nouvelle langue prenne effet.
wizard-locale-name-en-US = Anglais (États-Unis)
wizard-locale-name-de-DE = Allemand (Allemagne)
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

wizard-packages-heading = Choisissez les paquets
wizard-packages-list-label = Paquets à installer ou à mettre à jour
wizard-packages-tree-group-label = Paquets
wizard-additional-software-tree-group-label = Logiciels supplémentaires
wizard-configuration-tree-group-label = Configuration
# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = Indisponible : nécessite l'installation de { $package }.
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
config-reapack-reaper-accessibility-name = Ajouter le dépôt ReaPack « REAPER Accessibility » de Toni
config-reapack-reaper-accessibility-description = Ajoute le dépôt ReaPack « REAPER Accessibility » de Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour obtenir des scripts et des extensions accessibles supplémentaires.
config-reapack-reaper-accessible-fr-name = Ajouter le dépôt ReaPack « REAPER Accessible (FR) »
config-reapack-reaper-accessible-fr-description = Ajoute le dépôt ReaPack francophone de REAPER Accessible (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour accéder aux ressources francophones de REAPER Accessible.
config-reapack-reaper-accessible-en-name = Ajouter le dépôt ReaPack « REAPER Accessible (EN) »
config-reapack-reaper-accessible-en-description = Ajoute le dépôt ReaPack anglophone de REAPER Accessible (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Une fois ajouté, ouvrez le menu Extensions, ReaPack, Parcourir les paquets pour accéder aux ressources anglophones de REAPER Accessible.

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
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } vérification(s) de version ont échoué. Utilisez Précédent pour essayer une autre cible, ou fermez RABBIT.
wizard-version-check-progress-label = Progression
wizard-version-check-error-heading = Vérifications échouées
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package } : { $message }
wizard-package-details-label = Détails du paquet
wizard-packages-osara-keymap-heading = Raccourcis clavier OSARA
wizard-packages-osara-keymap-replace-label = Remplacer vos raccourcis clavier par les derniers d'OSARA
wizard-packages-osara-keymap-unavailable-note = Sélectionnez OSARA pour configurer le comportement de ses raccourcis clavier.
wizard-packages-osara-keymap-preserve-note = Pour les utilisateurs avancés : vos raccourcis clavier actuels seront conservés. RABBIT ne touchera pas à reaper-kb.ini ; vous devrez gérer manuellement la mise à jour avec les derniers ajouts de raccourcis OSARA.
wizard-packages-osara-keymap-replace-note = Recommandé pour les utilisateurs débutants à intermédiaires : RABBIT sauvegardera une copie de votre fichier reaper-kb.ini actuel, puis le remplacera par la dernière version des raccourcis clavier OSARA.
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
wizard-review-notes-heading = Remarques
wizard-review-preflight-prefix = Installation impossible pour le moment

# $path is the selected REAPER resource path.
wizard-review-target = Cible : { $path }
wizard-review-no-target = Aucune cible sélectionnée.
wizard-review-no-package = Aucun paquet sélectionné.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package } : { $action }

wizard-progress-heading = Progression de l'installation
wizard-progress-status-idle = Prêt à installer.
wizard-progress-status-running = Installation des paquets sélectionnés. Cela peut prendre quelques minutes.
wizard-progress-details-label = Détails de la progression
wizard-progress-details-idle = Aucune installation en cours.
wizard-progress-details-starting = Démarrage de l'opération de configuration.
wizard-progress-details-cache-prefix = Cache

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Téléchargement de { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Téléchargement de { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Téléchargement de { $count } paquets… { $downloaded } / { $total }
wizard-progress-status-installing = Installation de { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Application de l'étape de configuration : { $step }

# Running log lines appended to the progress details text control.
wizard-progress-log-download-started = Téléchargement de { $package }…
wizard-progress-log-download-completed = { $package } téléchargé.
wizard-progress-log-install-started = Installation de { $package }…
wizard-progress-log-install-completed = { $package } installé.
wizard-progress-log-configuration-started = Application de { $step }…
wizard-progress-log-configuration-completed = { $step } appliqué.

wizard-done-heading = Terminé
wizard-done-status-idle = Aucune installation n'a encore été lancée depuis cette fenêtre.
wizard-done-status-success = RABBIT a fini d'opérer sa magie ! Consultez les détails ci-dessous.
wizard-done-status-error = L'installation a échoué. Consultez l'erreur ci-dessous.
wizard-done-status-no-packages = Aucun paquet n'a été sélectionné pour l'installation ou la mise à jour.
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

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Mise à jour de RABBIT disponible
wizard-self-update-prompt-body = RABBIT { $latest } est disponible. Vous avez actuellement { $current }. Mettre à jour maintenant ? RABBIT redémarrera une fois la mise à jour terminée.

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
wizard-summary-resource-items-created = Éléments de ressources créés : { $count }
wizard-summary-packages-installed-or-checked = Paquets installés ou vérifiés : { $count }
wizard-summary-packages-current = Paquets déjà à jour : { $count }
wizard-summary-packages-manual = Paquets nécessitant une intervention manuelle : { $count }
wizard-summary-backup-files-created = Fichiers de sauvegarde créés : { $count }
wizard-summary-backup-file = Fichier de sauvegarde : { $path }
wizard-summary-receipt-backup = Sauvegarde du reçu : { $path }
wizard-summary-backup-manifest = Manifeste de sauvegarde : { $path }
wizard-summary-package-message = { $package } : { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Action prévue : { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Statut : { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Version installée : { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architecture : { $architecture }
status-installed-or-checked = Installé ou vérifié
status-planned-unattended = Prévu sans intervention
status-deferred-unattended = Différé sans intervention
status-skipped-current = Ignoré (déjà à jour)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Binaire d'extension unique pris en charge par le programme d'installation de RABBIT.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La version installée { $installed } est égale ou plus récente que la version disponible { $available }.
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
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = L'étape de configuration { $step } n'a pas été sélectionnée.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = L'étape de configuration { $step } a été ignorée car son paquet prérequis { $dependency } n'était pas installé et ne fait pas partie de ce plan.
config-message-applied-no-op = Étape de configuration appliquée sans modification.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step } : { $message }
wizard-summary-configuration-status =   Statut : { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Appliqué
config-status-skipped = Ignoré
config-status-skipped-dependency-missing = Ignoré (prérequis manquant)
config-status-dry-run = Simulation
wizard-summary-planned-execution-title = Exécution sans intervention prévue :
wizard-summary-planned-execution-runner =   Exécuteur : { $runner }
wizard-summary-planned-execution-artifact =   Artefact : { $artifact }
wizard-summary-planned-execution-program =   Programme : { $program }
wizard-summary-planned-execution-arguments =   Arguments : { $arguments }
wizard-summary-planned-execution-working-directory =   Répertoire de travail : { $path }
wizard-summary-planned-execution-verify =   Vérification : { $path }
wizard-summary-manual-title = { $title } :
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Remarque : { $note }
wizard-summary-status-finished = Terminé. { $installed } élément(s) de paquet installé(s) ou vérifié(s) ; { $manual } nécessite(nt) une intervention manuelle.

wizard-planned-runner-launch-installer = Lancer l'exécutable d'installation
wizard-planned-runner-extract-archive = Extraire l'archive et exécuter le programme d'installation qu'elle contient
wizard-planned-runner-extract-archive-copy-osara = Extraire l'archive et copier les ressources d'installation d'OSARA
wizard-planned-runner-mount-disk-image = Monter l'image disque et exécuter le programme d'installation qu'elle contient
wizard-planned-runner-mount-disk-image-copy-app = Monter l'image disque et copier le bundle d'application qu'elle contient
wizard-planned-runner-mount-disk-image-run-pkg = Monter l'image disque et exécuter le programme d'installation pkg qu'elle contient
