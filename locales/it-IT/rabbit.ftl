app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = sì
common-no = no

action-install = Da installare
action-update = Da aggiornare
action-keep = Non modificare
action-uninstall = Da disinstallare

package-reaper = REAPER
package-osara = OSARA
package-sws = Estensione SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Script JAWS di Snowman per REAPER
package-ffmpeg = FFmpeg (supporto video migliorato)
package-surge-xt = Surge XT
package-app2clap = app2clap
package-langpack-es = REAPER in spagnolo
package-langpack-de = REAPER in tedesco
package-langpack-fr = REAPER in francese
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Team PMA (es_MX)

package-reaper-description = REAPER è la workstation audio digitale su cui si basa tutto il resto. RABBIT può installarlo o aggiornarlo per te.
package-osara-description = OSARA è l'estensione di accessibilità open source che rende REAPER utilizzabile con uno screen reader. NVDA, JAWS e Assistente vocale su Windows, VoiceOver su macOS sono tutti ampiamente diffusi; anche altri screen reader per Windows potrebbero funzionare. Installa OSARA se ti affidi a uno screen reader per usare REAPER.
package-sws-description = L'estensione SWS è un insieme di lunga data, creato dalla community, di azioni, script e strumenti aggiuntivi che ampliano le funzionalità di REAPER. Per la configurazione di REAPER più accessibile possibile, sia su Windows che su Mac, dovresti installare SWS insieme a OSARA.
package-reapack-description = ReaPack è un gestore di pacchetti open source. Può essere usato per cercare, installare, monitorare e aggiornare script ed estensioni di terze parti direttamente da REAPER. Installalo se vuoi usare gli script condivisi dalla community di REAPER.
package-reakontrol-description = ReaKontrol fornisce un'integrazione open source per le tastiere Komplete Kontrol di Native Instruments. Installalo se possiedi una tastiera serie S MK2, serie A, M-32 o Kontrol MK3.
package-jaws-scripts-description = Gli script di Snowman migliorano il modo in cui JAWS gestisce le varie finestre di REAPER, oltre a offrire un supporto Braille esteso e molte altre funzionalità. Tieni presente che questi script sono concepiti per essere usati insieme a OSARA: non ne sono un'alternativa. Per un'accessibilità ottimale con JAWS, dovresti installarli entrambi.
package-ffmpeg-description = Le librerie di runtime condivise di FFmpeg consentono al decoder video di REAPER di importare e riprodurre i formati audio e video più comuni. RABBIT installa la cartella bin della build «GPL-shared» di BtbN in UserPlugins; il livello di patch non può essere dedotto dai soli nomi dei file DLL, perciò le installazioni esterne di FFmpeg vengono segnalate con un segnaposto «<major>.0.0».
package-surge-xt-description = Surge XT è un sintetizzatore ibrido gratuito e open source. RABBIT esegue il programma di installazione del fornitore al posto tuo — installa a livello di sistema i formati VST3, CLAP, AU (solo macOS) e standalone, così REAPER e gli altri DAW possono caricare Surge XT. Segue il canale nightly continuo perché l'ultima versione stabile (1.3.4) risale all'agosto 2024 e il progetto ormai viene distribuito di fatto tramite le nightly. Solo installazioni standard di REAPER: i dati di fabbrica risiedono al di fuori di qualsiasi cartella REAPER portatile.
package-app2clap-description = app2clap è un plug-in CLAP per Windows che cattura l'audio di altre applicazioni e lo porta in REAPER (o in qualsiasi host CLAP) come plug-in da inserire su una traccia — utile per registrare o elaborare il suono di un browser, un lettore multimediale o un altro programma. RABBIT scarica la versione più recente e installa app2clap.clap nella tua cartella CLAP personale, senza bisogno dei diritti di amministratore. Solo Windows. Solo installazioni standard di REAPER: viene installato al di fuori di qualsiasi cartella REAPER portatile.
package-langpack-es-description = Traduce l'interfaccia di REAPER in spagnolo, inclusa l'estensione SWS. Curato da Javier Robledo per la comunità ispanofona di REAPER e pubblicato su reaperespa.com: RABBIT scarica la versione attuale e la installa come es_ES.ReaperLangPack. OSARA usa quel nome di file per scegliere la propria traduzione spagnola. Dopo l'installazione, seleziona la lingua nelle preferenze di REAPER (oppure lascia che sia RABBIT a farlo).
package-langpack-de-description = Traduce l'interfaccia di REAPER in tedesco, inclusa l'estensione SWS. Curato da MrData e pubblicato nello Stash di REAPER: RABBIT scarica la versione attuale e la installa come de_DE.ReaperLangPack. Dopo l'installazione, seleziona la lingua nelle preferenze di REAPER (oppure lascia che sia RABBIT a farlo).
package-langpack-fr-description = Traduce l'interfaccia di REAPER in francese, inclusa l'estensione SWS. Curato da Lee Julien e Pierre-Marie Curt per ReaperAccessible, la comunità francese degli utenti di screen reader, e pubblicato nel loro repository ReaPack: RABBIT scarica la versione attuale e la installa come fr_CA.ReaperLangPack. OSARA legge quel nome di file per scegliere la propria traduzione francese: quella fr_CA è quasi completa, mentre quella fr_FR lascia ancora circa un terzo dei messaggi in inglese; il pacchetto in sé è lo stesso francese standard in entrambi i casi. Dopo l'installazione, seleziona la lingua nelle preferenze di REAPER (oppure lascia che sia RABBIT a farlo).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = RABBIT non può ancora disinstallare questo pacchetto.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Rimuove l'estensione SWS.
uninstall-summary-sws-keeps = I tuoi groove e le tue impostazioni di SWS, come le tue azioni cicliche, restano.
uninstall-summary-reapack-removes = Rimuove l'estensione ReaPack.
uninstall-summary-reapack-keeps = Le tue impostazioni e i tuoi repository di ReaPack restano, così come l'elenco di ciò che ReaPack ha installato.
uninstall-summary-reakontrol-removes = Rimuove l'estensione ReaKontrol.
uninstall-summary-reakontrol-keeps = Le tue mappe FX restano, e RABBIT non tocca il tuo software Native Instruments.
uninstall-summary-ffmpeg-removes = Rimuove i file di FFmpeg che RABBIT ha installato in UserPlugins. Dopo, REAPER non può più usare FFmpeg per aprire o renderizzare file multimediali.
uninstall-summary-ffmpeg-keeps = I file di FFmpeg di cui RABBIT non ha traccia restano, come quelli lasciati da versioni precedenti.
uninstall-summary-app2clap-removes = Rimuove app2clap dalla tua cartella CLAP. Lo perdono tutti i programmi in cui usi plug-in CLAP, non solo REAPER.
uninstall-summary-app2clap-keeps = La cartella CLAP e gli altri plug-in che contiene restano.
uninstall-summary-langpack-removes = Rimuove il pacchetto lingua che RABBIT ha installato. Se REAPER lo usa, RABBIT riporta REAPER in inglese.
uninstall-summary-langpack-keeps = I pacchetti lingua che hai aggiunto tu restano.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = i tuoi groove
uninstall-kept-sws-settings = impostazioni di SWS
uninstall-kept-sws-cycle-actions = le tue azioni cicliche
uninstall-kept-reapack-settings = impostazioni di ReaPack e i tuoi repository
uninstall-kept-reapack-data = l'elenco di ciò che ReaPack ha installato e la sua cache dei download
uninstall-kept-reakontrol-fx-maps = le tue mappe FX
uninstall-kept-app2clap-clap-folder = la cartella CLAP, condivisa con altri plug-in

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Se usi la mappa dei tasti di OSARA, alcuni dei suoi tasti eseguono azioni di SWS. Quei tasti smettono di funzionare finché non reinstalli SWS.
uninstall-warning-sws-reapack-scripts = Molti script di REAPER, tra cui script per l'accessibilità, richiedono SWS. Gli script che hai installato con ReaPack potrebbero smettere di funzionare.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Gli script e le estensioni che hai installato con ReaPack restano, ma niente li tiene più aggiornati.

# How to remove a package by hand where RABBIT doesn't, shown line by line.
# Keep file names, folder paths and %APPDATA% exactly as they are.
uninstall-manual-jaws-scripts = RABBIT non può ancora rimuovere gli script di JAWS. Per rimuoverli a mano:
    1. Chiudi REAPER.
    2. Elimina Reaper_ComAccess32.dll e Reaper_ComAccess64.dll da %APPDATA%\REAPER\UserPlugins.
    I file degli script restano nella cartella delle impostazioni di JAWS, %APPDATA%\Freedom Scientific\JAWS. Alcuni contengono le tue modifiche, come le tue assegnazioni dei tasti.
uninstall-manual-reaper-portable = RABBIT non rimuove un REAPER portable: i file di REAPER e i tuoi stanno nella stessa cartella. Per rimuoverlo a mano:
    1. Chiudi REAPER.
    2. Copia fuori dalla cartella del REAPER portable tutto quello che vuoi conservare, come i progetti che hai salvato lì, le tue mappe dei tasti e le tue impostazioni.
    3. Elimina la cartella del REAPER portable.
    Anche i backup che RABBIT ha creato per questo REAPER si trovano in quella cartella, in RABBIT\backups, e vengono eliminati con essa.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (non disponibile: { $reason })
wizard-package-row-unavailable-portable = destinazione REAPER portatile
wizard-package-row-unavailable-version-check = controllo della versione online non riuscito

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package }: il controllo dell'ultima versione non è riuscito ({ $message }). L'installazione o l'aggiornamento di questo pacchetto è disabilitato per questa esecuzione.

detect-installed = Installato
detect-not-installed = Non installato
detect-version-unknown = Versione sconosciuta
detect-source-receipt = Ricevuta RABBIT
detect-source-files = Presenza di file in UserPlugins
detect-source-reapack-registry = Registro ReaPack

# $package is the localized package display name.
status-package-installed = { $package } installato

wizard-step-target = Destinazione
wizard-step-version-check = Controllo versioni
wizard-step-packages = Pacchetti
wizard-step-reapack-acknowledgement = Donazione ReaPack
wizard-step-review = Riepilogo
wizard-step-progress = Avanzamento
wizard-step-done = Completato

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Indietro
wizard-button-back-mnemonic = B
wizard-button-next = Avanti
wizard-button-next-mnemonic = A
wizard-button-install = Installa
wizard-button-install-mnemonic = I
wizard-button-uninstall = Disinstalla
wizard-button-uninstall-mnemonic = D
wizard-button-close = Chiudi
wizard-button-close-mnemonic = C
wizard-button-cancel = Annulla
wizard-button-cancel-mnemonic = N

wizard-target-heading = Scegli un'operazione
wizard-target-language-label = Lingua
wizard-target-language-restart-note = Cambiare la lingua riavvia RABBIT affinché la nuova lingua abbia effetto.
wizard-locale-name-en-US = Inglese (Stati Uniti)
wizard-locale-name-de-DE = Tedesco (Germania)
wizard-locale-name-es-ES = Spagnolo (Spagna)
wizard-locale-name-fr-FR = Francese (Francia)
wizard-locale-name-it-IT = Italiano (Italia)
wizard-target-choice-label = Percorso di installazione
wizard-target-details-label = Dettagli della destinazione
wizard-target-empty = Nessuna destinazione di installazione di REAPER è selezionata.
wizard-target-portable-choice = Crea o aggiorna una versione portatile di REAPER
wizard-target-portable-folder-label = Cartella portatile
wizard-target-portable-folder-message = Scegli una cartella REAPER portatile se ne hai già una, oppure una cartella vuota se vuoi creare una nuova versione portatile.
wizard-target-portable-folder-browse-label = Sfoglia…
wizard-target-portable-pending-details = Usa il pulsante Sfoglia per indicare la posizione di una versione portatile esistente, se ne hai una, oppure per scegliere una cartella vuota se vuoi creare una nuova versione portatile di REAPER.
wizard-target-custom-portable-label = Cartella REAPER portatile
wizard-target-custom-portable-app-path-label = Percorso dell'applicazione REAPER
wizard-target-custom-portable-path-label = Percorso delle risorse portatili
wizard-target-custom-portable-version-label = Versione di REAPER
wizard-target-custom-portable-writable-label = Scrivibile
wizard-target-custom-portable-note = RABBIT creerà qui il percorso delle risorse di REAPER se mancante.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } in { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = Percorso di installazione di REAPER: { $app_path }
    Versione: { $version }
    Percorso delle risorse: { $path }
    Scrivibile: { $writable }

wizard-packages-heading = Scegli cosa installare, aggiornare o disinstallare
wizard-packages-list-label = Installa o aggiorna: pacchetti e impostazioni
wizard-packages-row-checked = Selezionato
wizard-packages-row-unchecked = Non selezionato
wizard-packages-choice-shown = Nuova scelta sotto l'elenco: { $choice }.
wizard-packages-choice-hidden = Scelta rimossa: { $choice }.
wizard-packages-tree-group-label = Pacchetti
wizard-additional-software-tree-group-label = Software aggiuntivo
wizard-language-tree-group-label = Pacchetti lingua
wizard-configuration-tree-group-label = Configurazione

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Installa o aggiorna
wizard-packages-tab-uninstall = Disinstalla
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Disinstalla: software che RABBIT può rimuovere
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package }: { $action }. Hai { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = RABBIT non può ancora disinstallarlo
wizard-uninstall-row-unavailable-not-supported = RABBIT non può disinstallarlo
wizard-uninstall-row-unavailable-manual = da rimuovere a mano
wizard-uninstall-row-unavailable-reapack = gestito da ReaPack
wizard-uninstall-row-unavailable-reapack-check-failed = non è stato possibile controllare i registri di ReaPack
wizard-uninstall-row-unavailable-no-receipt = RABBIT non ha installato questa copia
wizard-uninstall-row-unavailable-no-trusted-uninstaller = nessun programma di disinstallazione di cui RABBIT si fidi
wizard-uninstall-row-unavailable-unsafe-path = un file che RABBIT non rimuove mai
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack gestisce { $package }: ha installato { $path } come { $reapack_package }. Disinstallalo invece da ReaPack.
wizard-uninstall-details-reapack-check-failed = RABBIT non ha potuto controllare i registri di ReaPack per { $path } ({ $reason }), quindi non tocca { $package }.
wizard-uninstall-details-portable = { $package } serve a tutti i REAPER di questo computer, quindi RABBIT lo rimuove solo dal REAPER standard, non da uno portatile.
wizard-uninstall-details-no-receipt = RABBIT non ha installato questa copia di { $package }, quindi molto probabilmente è tua, e RABBIT non la tocca.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT non ha trovato nessun programma di disinstallazione di { $package } di cui fidarsi.
wizard-uninstall-details-unsafe-path = RABBIT non rimuoverà { $path }. Preferisce non toccare { $package } piuttosto che rimuoverlo solo in parte.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT sposta i file in un backup invece di eliminarli.
wizard-uninstall-details-dependents-stay = Questi pacchetti si installano sopra { $package } e restano: { $packages }. Senza di esso potrebbero non funzionare.
wizard-uninstall-details-machine-wide = { $package } serve a tutti i REAPER e a tutti gli altri programmi di questo computer, che lo perdono tutti.
wizard-uninstall-details-reapack-owned-extensions = Questi li ha installati ReaPack, e restano senza aggiornamenti una volta rimosso ReaPack: { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT non ha potuto leggere i registri di ReaPack per sapere cosa gestisce ancora ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT non ha installato questa copia. Rimuove i file che { $package } notoriamente installa.
wizard-uninstall-details-reapack-orphan = { $package } è stato installato a suo tempo da ReaPack. ReaPack non c'è più, quindi RABBIT può rimuoverlo; i registri di ReaPack che lo riguardano restano.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = RABBIT non ha trovato niente che possa disinstallare in questo REAPER.
wizard-uninstall-empty-details = In questo REAPER non è installato nessuno dei software che RABBIT sa disinstallare.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = RABBIT non ha potuto leggere i suoi registri per questo REAPER ({ $error }), quindi non può elencare cosa potrebbe disinstallare.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Impostazioni
wizard-uninstall-remove-settings = Rimuovi anche le impostazioni
wizard-uninstall-settings-note-none-ticked = Seleziona un pacchetto per vedere quali delle sue impostazioni RABBIT può rimuovere. Se non selezioni questa opzione, le impostazioni restano.
wizard-uninstall-settings-note-none-removable = Nessuno dei pacchetti selezionati ha impostazioni che RABBIT possa rimuovere.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = Con questa opzione selezionata, RABBIT rimuove anche queste impostazioni. Finiscono nel backup insieme al resto, quindi il backup può recuperarle.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package }: { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Ora disponibile sotto l'elenco: { $option }.
wizard-uninstall-settings-unavailable = Non più disponibile: { $option }.

# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = Non disponibile: richiede l'installazione di { $package }.
wizard-configuration-row-already-applied = Già applicato su questa destinazione REAPER.
# Short status tag appended in parentheses to a configuration row's tree label
# when the row isn't actionable. Kept terse so the tree label stays readable;
# the longer sentence in `wizard-configuration-row-unavailable` /
# `wizard-configuration-row-already-applied` is still surfaced in the details
# pane and as the row's accessible reason.
# $reason is one of the "wizard-configuration-row-status-*" strings below.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package is the localized name of the dependency package.
wizard-configuration-row-status-requires = richiede { $package }
wizard-configuration-row-status-already-applied = già applicato
# Nomina la dipendenza di un passo di configurazione soddisfatto da uno
# qualsiasi di piu pacchetti.
config-dependency-language-pack = un pacchetto lingua
config-reapack-reaper-accessibility-name = Aggiungi il repository ReaPack «REAPER Accessibility» di Toni
config-reapack-reaper-accessibility-description = Aggiunge il repository ReaPack «REAPER Accessibility» di Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per ottenere ulteriori script ed estensioni accessibili.
config-reapack-reaper-accessible-fr-name = Aggiungi il repository ReaPack «REAPER Accessible (FR)»
config-reapack-reaper-accessible-fr-description = Aggiunge il repository ReaPack in lingua francese di REAPER Accessible (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per accedere alle risorse in francese di REAPER Accessible.
config-reapack-reaper-accessible-en-name = Aggiungi il repository ReaPack «REAPER Accessible (EN)»
config-reapack-reaper-accessible-en-description = Aggiunge il repository ReaPack in lingua inglese di REAPER Accessible (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per accedere alle risorse in inglese di REAPER Accessible.
config-set-reaper-language-name = Imposta la lingua di REAPER
config-set-reaper-language-description = Indica a REAPER quale dei pacchetti lingua installati usare, scrivendolo in reaper.ini. Senza questo dovresti selezionare la lingua manualmente in Opzioni, Preferenze, Generale. Puoi installare più pacchetti lingua e passare dall'uno all'altro in REAPER in seguito; qui si sceglie solo quale è attivo ora. Deseleziona se vuoi solo installare i file.

wizard-reapack-ack-heading = Avviso di donazione ReaPack
wizard-reapack-ack-body = ReaPack è software libero rilasciato sotto licenza LGPL. Il suo autore, Christian Fillion, accetta donazioni facoltative per sostenere lo sviluppo continuo. Christian gestisce anche le estensioni SWS e in passato ha integrato codice specificamente pensato per migliorare la compatibilità con OSARA. Qualsiasi sostegno tu possa offrirgli è ampiamente meritato.
wizard-reapack-ack-link-label = Apri la pagina di donazione di ReaPack
wizard-reapack-ack-confirm-label = Salta la donazione per questa volta, installa o aggiorna solo ReaPack
cli-reapack-ack-prompt-summary = ReaPack è software libero (LGPL). Il suo autore, Christian Fillion, accetta donazioni facoltative su https://reapack.com/donate per sostenere lo sviluppo continuo.
cli-reapack-ack-flag-required = ReaPack è incluso nel piano di questa esecuzione, ma manca la conferma della donazione. Riesegui il comando con --accept-reapack-donation-notice per confermare di aver letto https://reapack.com/donate e di voler far installare o aggiornare ReaPack a RABBIT.

wizard-version-check-heading = Controllo delle ultime versioni
wizard-version-check-status-pending = Preparazione del controllo dell'ultima versione…
# $package is the localized package display name.
wizard-version-check-status-checking = Controllo di { $package }…
wizard-version-check-status-progress = Controllo delle versioni… ({ $done } di { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } controllo/i di versione non riuscito/i. Usa Indietro per provare un'altra destinazione, oppure chiudi RABBIT.
wizard-version-check-progress-label = Avanzamento
wizard-version-check-error-heading = Controlli non riusciti
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Dettagli del pacchetto
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = Novità di { $package }:
wizard-packages-osara-keymap-heading = Mappa dei tasti OSARA
wizard-packages-osara-keymap-replace-label = Sostituisci la tua mappa dei tasti attuale con l'ultima mappa dei tasti OSARA
packages-reaper-language-label = Lingua di REAPER dopo l'installazione
packages-spanish-variant-label = Traduzione spagnola di OSARA
wizard-packages-osara-keymap-unavailable-note = Seleziona OSARA per configurare il comportamento della sua mappa dei tasti.
wizard-packages-osara-keymap-preserve-note = La tua mappa dei tasti attuale resta com'è: RABBIT non tocca reaper-kb.ini. Per aggiungere le nuove scorciatoie di OSARA mantenendo le tue, esegui poi l'azione di OSARA «Merge OSARA key map» (menu Estensioni, OSARA).
wizard-packages-osara-keymap-replace-note = RABBIT eseguirà il backup del tuo file reaper-kb.ini attuale, se esiste, quindi lo sostituirà con l'ultima mappa dei tasti OSARA. Le scorciatoie che hai modificato tu non vengono mantenute; il backup le conserva.
wizard-packages-osara-keymap-unchanged-note = La tua mappa dei tasti è quella di OSARA senza modifiche tue, quindi RABBIT ne farà un backup e la sostituirà con l'ultima mappa dei tasti OSARA. Gli script che hai installato restano nell'elenco delle azioni.
wizard-packages-osara-keymap-default-keys-note = Stai usando le scorciatoie predefinite di REAPER invece della mappa dei tasti OSARA, quindi RABBIT le lascia come sono. Per passare alla mappa dei tasti OSARA, seleziona la casella qui sopra.
wizard-package-details-handling-prefix = Gestione
wizard-package-handling-automatic = RABBIT può installare questo pacchetto direttamente.
wizard-package-handling-unattended = RABBIT può installare questo pacchetto in modo automatico, incluso l'avvio del suo programma di installazione quando necessario.
wizard-package-handling-planned = RABBIT è progettato per eseguire autonomamente il programma di installazione o la procedura di configurazione di questo pacchetto e completare l'installazione in modo automatico, ma questa build si limita ancora a segnalare i passaggi anziché eseguirli.
wizard-package-handling-manual = RABBIT scaricherà questo pacchetto e indicherà i passaggi manuali al termine dell'esecuzione.
wizard-package-handling-unavailable = Questo pacchetto non è disponibile per la piattaforma o l'architettura selezionata.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package }: { $action }. Hai { $installed }. L'ultima è { $available }

wizard-review-heading = Controlla ciò che hai chiesto a RABBIT di fare
wizard-review-target-prefix = Destinazione
wizard-review-package-heading = Pacchetti selezionati
wizard-review-osara-keymap-heading = Mappa dei tasti OSARA
wizard-review-osara-keymap-preserve = Conserva la tua mappa dei tasti attuale.
wizard-review-osara-keymap-replace = Esegui il backup della tua mappa dei tasti attuale, quindi sostituiscila con l'ultima di OSARA.
wizard-review-osara-keymap-replace-unchanged = Esegui il backup della tua mappa dei tasti OSARA non modificata, quindi sostituiscila con l'ultima di OSARA.
wizard-review-osara-keymap-keep-default-keys = Mantieni le scorciatoie predefinite di REAPER.
wizard-review-notes-heading = Note
wizard-review-preflight-prefix = Impossibile installare per ora

# $path is the selected REAPER resource path.
wizard-review-target = Destinazione: { $path }
wizard-review-no-target = Nessuna destinazione selezionata.
wizard-review-no-package = Nessun pacchetto selezionato.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Pronto per disinstallare
wizard-review-uninstall-packages = Pacchetti da disinstallare
wizard-review-uninstall-no-package = Non è selezionato niente da disinstallare.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package }: { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = Anche le sue impostazioni vengono rimosse e finiscono nel backup: { $settings }.
wizard-review-uninstall-close-programs-note = Prima chiudi gli altri programmi musicali.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. $packages is a comma-separated list of package names.
wizard-review-inactive-tab-note-install = Ciò che hai selezionato nella scheda «Installa o aggiorna» non fa parte di questa operazione: { $packages }.
wizard-review-inactive-tab-note-uninstall = Ciò che hai selezionato nella scheda «Disinstalla» non fa parte di questa operazione: { $packages }.
wizard-review-uninstall-reinstall-hint = Potrai reinstallarli più avanti dalla scheda «Installa o aggiorna».
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Impossibile disinstallare per ora: { $reason }
wizard-review-uninstall-not-writable = RABBIT non può scrivere nella cartella di questo REAPER.

wizard-progress-heading = Avanzamento dell'installazione
wizard-progress-status-idle = Pronto per l'installazione.
wizard-progress-status-running = Installazione dei pacchetti selezionati. Potrebbe richiedere alcuni minuti.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Arresto in corso. RABBIT termina il passaggio iniziato, poi si chiude.
wizard-progress-details-label = Dettagli dell'avanzamento
wizard-progress-details-idle = Nessuna installazione in corso.
wizard-progress-details-starting = Avvio dell'operazione di configurazione.
wizard-progress-details-cache-prefix = Cache
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops after the package it is on, not after a step.
wizard-progress-uninstall-heading = Disinstallazione
wizard-progress-status-uninstall-running = Disinstallazione dei pacchetti selezionati.
wizard-progress-details-uninstall-starting = Avvio della disinstallazione.
wizard-progress-status-uninstall-cancelling = Arresto in corso. RABBIT completa il pacchetto iniziato, poi si chiude.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Download di { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Download di { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Download di { $count } pacchetti… { $downloaded } / { $total }
wizard-progress-status-installing = Installazione di { $package }…
wizard-progress-status-removing = Disinstallazione di { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Applicazione del passaggio di configurazione: { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = Download di { $package }…
wizard-progress-log-download-completed = { $package } scaricato.
wizard-progress-log-install-started = Installazione di { $package }…
wizard-progress-log-install-completed = { $package } installato.
wizard-progress-log-remove-started = Disinstallazione di { $package }…
wizard-progress-log-remove-completed = { $package } disinstallato.
wizard-progress-log-remove-failed = Impossibile disinstallare { $package }.
wizard-progress-log-configuration-started = Applicazione di { $step }…
wizard-progress-log-configuration-completed = { $step } applicato.

wizard-done-heading = Completato
wizard-done-status-idle = Nessuna installazione è ancora stata avviata da questa finestra.
wizard-done-status-success = RABBIT ha finito di fare la sua magia! Consulta i dettagli qui sotto.
wizard-done-status-error = Installazione non riuscita. Consulta l'errore qui sotto.
wizard-reaper-running-title = REAPER è ancora aperto
wizard-reaper-running-body = Chiudi REAPER, poi premi di nuovo Installa. RABBIT non può aggiornare REAPER né le sue estensioni mentre REAPER è aperto.
wizard-error-reaper-running = RABBIT si è fermato perché REAPER era aperto. Chiudi REAPER, poi avvia di nuovo RABBIT.
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Un altro RABBIT aperto sta installando o disinstallando in questo REAPER. Aspetta che finisca, poi avvia di nuovo RABBIT.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Chiudi REAPER, poi premi di nuovo Disinstalla. RABBIT non può disinstallare niente da REAPER mentre REAPER è aperto.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Chiudi tutte le copie di REAPER su questo computer, comprese quelle portatili, poi premi di nuovo Disinstalla. Qualsiasi REAPER su questo computer può usare { $packages }.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Chiudi prima questi programmi
wizard-programs-running-uninstall-body = Chiudi { $programs }, poi premi di nuovo Disinstalla.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT si è fermato perché un REAPER era aperto. Chiudi tutte le copie di REAPER su questo computer, comprese quelle portatili, poi avvia di nuovo RABBIT.
wizard-error-uninstall-not-enough-space = RABBIT si è fermato perché non c'è abbastanza spazio libero per il backup. Libera spazio sull'unità che contiene la cartella delle risorse di REAPER, poi avvia di nuovo RABBIT.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } non è stato disinstallato perché un altro programma sta usando { $path }. Chiudi quel programma, poi avvia di nuovo RABBIT.
wizard-done-status-completed-with-errors = Installazione completata con errori. Consulta i dettagli qui sotto.
wizard-done-status-cancelled = Hai interrotto l'installazione. L'elenco qui sotto mostra ciò che RABBIT ha completato prima di fermarsi.
wizard-done-status-no-packages = Nessun pacchetto è stato selezionato per l'installazione o l'aggiornamento.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = RABBIT ha disinstallato { $packages }.
wizard-done-uninstall-status-completed-with-errors = RABBIT ha disinstallato { $packages }, ma non tutto è andato come previsto.
wizard-done-uninstall-status-error = Disinstallazione non riuscita.
wizard-done-uninstall-status-cancelled = Hai interrotto la disinstallazione. I pacchetti che RABBIT non aveva ancora raggiunto sono ancora installati.
wizard-done-uninstall-status-no-packages = Non è stato disinstallato niente.
wizard-done-uninstall-removed-before-stop = Prima che lo interrompessi, RABBIT ha disinstallato { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Per riaverne uno, avvia di nuovo RABBIT e selezionalo nella scheda «Installa o aggiorna».
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } non era più installato, quindi non c'era niente da rimuovere.
wizard-done-uninstall-left-in-backup = Alcuni file di { $package } non sono stati rimessi al loro posto e sono ancora nel backup. «Mostra dettagli» li elenca.
wizard-done-uninstall-leftovers = RABBIT trova ancora parte di { $package } e la lascia com'è. «Mostra dettagli» indica quale.
wizard-done-uninstall-language-reset-failed = RABBIT non è riuscito a riportare REAPER in inglese dopo aver rimosso { $package }. Scegli una lingua nelle preferenze di REAPER.
wizard-done-show-details = Mostra dettagli
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = Apri REAPER e chiudi RABBIT
wizard-done-launch-reaper-mnemonic = A
wizard-done-open-resource = Apri la cartella delle risorse (solo per manutenzione manuale avanzata)
wizard-done-open-resource-mnemonic = R
wizard-done-no-reaper-app = Nessuna applicazione REAPER avviabile è nota per questa destinazione.
wizard-done-launch-reaper-error-prefix = Impossibile avviare REAPER
wizard-done-open-resource-error-prefix = Impossibile aprire la cartella delle risorse
wizard-done-self-update-apply-running = Applicazione dell'aggiornamento di RABBIT…
wizard-done-self-update-error-prefix = Aggiornamento automatico di RABBIT non riuscito
wizard-done-self-update-relaunch-prefix = RABBIT riavviato
wizard-self-update-status-checking = Ricerca di aggiornamenti di RABBIT…

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = Interrompere l'installazione?
wizard-close-during-install-body = RABBIT sta installando. Se interrompi adesso, RABBIT completa il passaggio iniziato, mantiene ciò che è già installato, ripulisce quello che ha iniziato e poi si chiude. I pacchetti che non ha ancora raggiunto restano non installati. Vuoi interrompere l'installazione?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = Interrompere la disinstallazione?
wizard-close-during-uninstall-body = RABBIT sta disinstallando. Se interrompi adesso, RABBIT completa il pacchetto iniziato, poi si ferma e si chiude. I pacchetti che non ha ancora raggiunto restano installati. Vuoi interrompere la disinstallazione?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Disinstallare i pacchetti selezionati?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Un backup non può recuperare { $package }. Per riaverlo, dovrai reinstallarlo.
wizard-confirm-uninstall-question = Disinstallare adesso?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = RABBIT si sta aggiornando
wizard-close-during-self-update-body = RABBIT sta sostituendo i propri file di programma. Questo passaggio non può essere interrotto senza danneggiare l'installazione. Attendi che finisca, poi chiudi RABBIT.

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Aggiornamento di RABBIT disponibile
wizard-self-update-prompt-body = RABBIT { $latest } è disponibile. Attualmente hai { $current }. Aggiornare ora? RABBIT si riavvierà al termine dell'aggiornamento.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = Novità da RABBIT { $current }:
wizard-self-update-prompt-update-button = Aggiorna ora
wizard-self-update-prompt-later-button = Non ora

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = Aggiornamento di RABBIT

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = Aggiornamento di RABBIT disponibile: { $current } → { $latest } (canale { $channel }). Riavvia RABBIT per ricevere di nuovo la richiesta.
self-update-status-up-to-date = RABBIT è aggiornato (versione attuale { $current }, canale { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = L'aggiornamento automatico non ha sostituito alcun file (versione di destinazione { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = { $count } file sostituito/i in { $root }; riavvia RABBIT per usare { $version }.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Verifica della firma: { $signed } firmato/i.
self-update-apply-signature-summary-unsigned-only = Verifica della firma: { $unsigned } non firmato/i.
self-update-apply-signature-summary-mixed = Verifica della firma: { $signed } firmato/i, { $unsigned } non firmato/i.

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Un'altra installazione di RABBIT è in corso (PID { $pid }). L'applicazione è sospesa fino al suo completamento.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Destinazione: { $path }
wizard-summary-portable = Destinazione portatile: { $value }
wizard-summary-dry-run = Simulazione: { $value }
wizard-summary-packages-selected = Pacchetti selezionati: { $packages }
wizard-summary-cache = Cache: { $path }
wizard-summary-planned-app = Percorso applicazione previsto: { $path }
wizard-summary-error = Errore: { $message }
wizard-summary-error-antivirus = Il software di sicurezza di Windows ha bloccato questo download. Di solito si tratta di un falso positivo: il programma di installazione è firmato e RABBIT aveva già verificato il file con il checksum dell'editore. Defender a volte segnala un programma di installazione appena creato che non ha ancora visto spesso – tipicamente le versioni di sviluppo di OSARA. Per continuare: apri Sicurezza di Windows, vai a «Protezione da virus e minacce» e poi a «Cronologia protezione», seleziona l'elemento bloccato e scegli «Consenti», quindi riavvia RABBIT. In alternativa puoi aggiungere la cartella di download di RABBIT alle esclusioni oppure installare manualmente questo pacchetto dal sito dell'editore. RABBIT non disattiva mai la protezione antivirus.
wizard-summary-resource-items-created = Elementi di risorse creati: { $count }
wizard-summary-packages-installed-or-checked = Pacchetti installati o verificati: { $count }
wizard-summary-packages-current = Pacchetti già aggiornati: { $count }
wizard-summary-packages-manual = Pacchetti che richiedono un intervento manuale: { $count }
wizard-summary-backup-files-created = File di backup creati: { $count }
wizard-summary-backup-file = File di backup: { $path }
wizard-summary-receipt-backup = Backup della ricevuta: { $path }
wizard-summary-backup-manifest = Manifesto del backup: { $path }
wizard-summary-osara-keymap-merge-hint = La tua mappa dei tasti è stata mantenuta. Per aggiungerle le nuove scorciatoie di OSARA, avvia REAPER ed esegui l'azione di OSARA «Merge OSARA key map» (menu Estensioni, OSARA).
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Azione prevista: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Stato: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Versione installata: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architettura: { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Rimuovi anche le impostazioni: { $value }
# One line per file, below its package's line. $path is a full path.
wizard-summary-removed-file = Rimosso: { $path }
wizard-summary-kept-file = Conservato perché è cambiato da quando RABBIT lo ha installato: { $path }
wizard-summary-kept-link = Conservato perché contiene un collegamento che RABBIT non segue mai: { $path }
wizard-summary-kept-reapack = Conservato perché l'ha installato ReaPack, che ne tiene traccia: { $path }
wizard-summary-left-over = Ancora presente: { $path }
wizard-summary-left-in-backup = Non rimesso al suo posto, ancora nel backup: { $path }
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Backup: { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = Per recuperare { $package }, chiudi REAPER ed esegui: { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Rapporto: { $path }
status-installed-or-checked = Installato o verificato
status-planned-unattended = Previsto in modo automatico
status-deferred-unattended = Rinviato in modo automatico
status-skipped-current = Ignorato (già aggiornato)
status-failed = Non riuscito
status-skipped-dependency-failed = Saltato (dipendenza non riuscita)
status-cancelled = Non installato (hai interrotto)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Disinstallato
status-removed-with-leftovers = Disinstallato, ma una parte è ancora presente
status-removed-pending-restart = Disinstallato; il resto verrà rimosso al riavvio del computer
status-would-remove = Verrebbe disinstallato
status-remove-not-installed = Non era installato
status-remove-blocked = Non disinstallato
status-remove-failed = Impossibile disinstallare
status-remove-cancelled = Ancora installato (hai interrotto)
status-remove-not-reached = Ancora installato (l'operazione si è fermata prima)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Singolo binario di estensione gestito dal programma di installazione di RABBIT.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La versione installata { $installed } è uguale o più recente della versione disponibile { $available }.
package-status-skipped-content-unchanged = La copia installata è identica a quella pubblicata dall'editore.
package-status-install-failed = Installazione non riuscita: { $error }
package-status-skipped-dependency-failed = Saltato perché { $dependency }, da cui dipende, non è stato installato correttamente.
package-status-cancelled = Non installato: hai interrotto la configurazione prima che RABBIT arrivasse a questo pacchetto.
# $automation is one of the "package-automation-*" labels (vendor installer / archive extraction / ...).
package-status-dry-run-would-run-unattended = Simulazione: RABBIT scaricherebbe ed eseguirebbe l'operazione «{ $automation }» in modo automatico.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-staged = Questa build non implementa ancora il percorso di esecuzione automatica previsto per l'operazione «{ $automation }». RABBIT ha preparato l'artefatto nella cache ma non lo ha eseguito.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-not-staged = Questa build non implementa ancora il percorso di esecuzione automatica previsto per l'operazione «{ $automation }». RABBIT non ha né scaricato né eseguito l'artefatto.
package-status-unattended-installed = RABBIT ha eseguito il programma di installazione ufficiale in modo automatico, verificato i percorsi di destinazione attesi e aggiornato la ricevuta RABBIT.
package-status-osara-unattended-keymap-backed-up = RABBIT ha eseguito il programma di installazione ufficiale in modo automatico, eseguito il backup di reaper-kb.ini, applicato la sostituzione della mappa dei tasti OSARA e aggiornato la ricevuta RABBIT.
package-status-osara-unattended-keymap-replaced = RABBIT ha eseguito il programma di installazione ufficiale in modo automatico, applicato la sostituzione della mappa dei tasti OSARA e aggiornato la ricevuta RABBIT.

# Short automation-kind labels interpolated into the per-package status
# messages above.
package-automation-installer = programma di installazione del fornitore
package-automation-archive = estrazione dell'archivio
package-automation-disk-image = installazione da immagine disco
package-automation-extension-binary = installazione diretta di file

# Per-configuration-step status messages surfaced on the wizard's Done page.
# `wizard-summary-configuration-message = { $step }: { $message }` is the
# wrapper template — the `*-message` keys below are the message body only.
# $name is the human-readable remote name; $url is the index XML URL.
config-message-reapack-remote-already-present = Il repository remoto ReaPack { $name } ({ $url }) è già configurato in reapack.ini.
config-message-reapack-remote-added = Repository remoto ReaPack { $name } ({ $url }) aggiunto a reapack.ini.
config-message-reapack-remote-created-file = reapack.ini creato con il repository remoto ReaPack { $name } ({ $url }). ReaPack aggiungerà i suoi repository predefiniti al prossimo avvio di REAPER.
config-message-reapack-remote-dry-run = Il repository remoto ReaPack { $name } ({ $url }) verrebbe aggiunto a reapack.ini.
config-message-reaper-language-already-selected = REAPER era già impostato su { $file }.
config-message-reaper-language-selected = Lingua di REAPER impostata su { $file }.
config-message-reaper-language-dry-run = Imposterebbe la lingua di REAPER su { $file }.
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = Il passaggio di configurazione { $step } non è stato selezionato.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = Il passaggio di configurazione { $step } è stato ignorato perché il suo pacchetto prerequisito { $dependency } non era installato e non fa parte di questo piano.
config-message-applied-no-op = Passaggio di configurazione applicato senza modifiche.
config-message-cancelled = Questo passaggio di configurazione non è stato eseguito: hai interrotto prima la configurazione.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Stato: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Applicato
config-status-skipped = Ignorato
config-status-skipped-dependency-missing = Ignorato (prerequisito mancante)
config-status-dry-run = Simulazione
config-status-cancelled = Non eseguito (hai interrotto)
wizard-summary-planned-execution-title = Esecuzione automatica prevista:
wizard-summary-planned-execution-runner =   Esecutore: { $runner }
wizard-summary-planned-execution-artifact =   Artefatto: { $artifact }
wizard-summary-planned-execution-program =   Programma: { $program }
wizard-summary-planned-execution-program-zipped =   Programma: il programma di installazione contenuto nel file .zip, una volta scaricato
wizard-summary-planned-execution-arguments =   Argomenti: { $arguments }
wizard-summary-planned-execution-working-directory =   Directory di lavoro: { $path }
wizard-summary-planned-execution-verify =   Verifica: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Nota: { $note }
wizard-summary-status-finished = Completato. { $installed } elemento/i di pacchetto installato/i o verificato/i; { $manual } richiede(ono) un intervento manuale.
wizard-summary-status-finished-with-errors = Completato con errori. { $installed } elemento/i del pacchetto installato/i o verificato/i; { $failed } non riuscito/i.

wizard-planned-runner-launch-installer = Avvia l'eseguibile di installazione
wizard-planned-runner-extract-archive = Estrai l'archivio ed esegui il programma di installazione contenuto
wizard-planned-runner-extract-archive-copy-osara = Estrai l'archivio e copia le risorse di installazione di OSARA
wizard-planned-runner-mount-disk-image = Monta l'immagine disco ed esegui il programma di installazione contenuto
wizard-planned-runner-mount-disk-image-copy-app = Monta l'immagine disco e copia il bundle dell'applicazione contenuto
wizard-planned-runner-mount-disk-image-run-pkg = Monta l'immagine disco ed esegui il programma di installazione pkg contenuto

# Modalità esperto e canali delle versioni.
package-channel-reaper-dev = Versioni di sviluppo da landoleet.org
package-channel-osara-pr = Versioni di prova delle pull request
wizard-window-title-expert = { $title } – Modalità esperto
wizard-expert-enable-title = Attivare la modalità esperto?
wizard-expert-enable-body = La modalità esperto ti permette di installare versioni preliminari: le versioni di sviluppo di REAPER da landoleet.org e le versioni di prova delle pull request di OSARA. Non sono supportate, possono danneggiare REAPER o la sua accessibilità e servono solo per i test. In questa pagina compaiono due scelte di versione, sotto l'installazione di REAPER. Attivare la modalità esperto?
wizard-expert-disabled-title = La modalità esperto è disattivata
wizard-expert-disabled-body = La modalità esperto è disattivata. I pacchetti installati da una versione preliminare tornano alla loro versione normale alla prossima installazione.
wizard-expert-first-page-title = Modalità esperto
wizard-expert-first-page-body = La modalità esperto si può attivare o disattivare solo nella prima pagina, dove aggiunge le scelte di versione. Torna alla prima pagina per cambiarla.
wizard-expert-busy-body = La modalità esperto non si può cambiare mentre RABBIT sta installando o dopo che ha finito.
wizard-expert-reaper-builds-label = Versioni di REAPER
wizard-expert-reaper-builds-stable = Versioni normali
wizard-expert-osara-builds-label = Versioni di OSARA
wizard-expert-osara-builds-snapshot = Snapshot normali
wizard-expert-osara-builds-loading = Ricerca delle versioni delle pull request…
wizard-expert-osara-builds-unavailable = Snapshot normali (impossibile ottenere le versioni delle pull request)
wizard-expert-osara-builds-pr = Pull request { $number }: { $title }
wizard-expert-osara-builds-pr-untitled = Pull request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = versione di sviluppo
wizard-package-channel-dev-details = { $package } verrà installato dalle sue versioni di sviluppo: versioni preliminari non supportate che possono smettere di funzionare in qualsiasi momento.
wizard-package-channel-pr = versione di prova della pull request { $number }
wizard-package-channel-pr-details = { $package } verrà installato dalla versione di prova della pull request { $number }: codice non revisionato e non firmato, che GitHub elimina dopo 90 giorni.
wizard-package-channel-back = ritorno alla versione normale
wizard-package-channel-back-details = { $package } è attualmente una versione preliminare. Questa installazione ripristina la versione normale.
