app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = sì
common-no = no

action-install = Da installare
action-update = Da aggiornare
action-keep = Non modificare

package-reaper = REAPER
package-osara = OSARA
package-sws = Estensione SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Script JAWS di Snowman per REAPER
package-ffmpeg = FFmpeg (supporto video migliorato)
package-surge-xt = Surge XT
package-app2clap = app2clap

package-reaper-description = REAPER è la workstation audio digitale su cui si basa tutto il resto. RABBIT può installarlo o aggiornarlo per te.
package-osara-description = OSARA è l'estensione di accessibilità open source che rende REAPER utilizzabile con uno screen reader. NVDA, JAWS e Assistente vocale su Windows, VoiceOver su macOS sono tutti ampiamente diffusi; anche altri screen reader per Windows potrebbero funzionare. Installa OSARA se ti affidi a uno screen reader per usare REAPER.
package-sws-description = L'estensione SWS è un insieme di lunga data, creato dalla community, di azioni, script e strumenti aggiuntivi che ampliano le funzionalità di REAPER. Per la configurazione di REAPER più accessibile possibile, sia su Windows che su Mac, dovresti installare SWS insieme a OSARA.
package-reapack-description = ReaPack è un gestore di pacchetti open source. Può essere usato per cercare, installare, monitorare e aggiornare script ed estensioni di terze parti direttamente da REAPER. Installalo se vuoi usare gli script condivisi dalla community di REAPER.
package-reakontrol-description = ReaKontrol fornisce un'integrazione open source per le tastiere Komplete Kontrol di Native Instruments. Installalo se possiedi una tastiera serie S MK2, serie A, M-32 o Kontrol MK3.
package-jaws-scripts-description = Gli script di Snowman migliorano il modo in cui JAWS gestisce le varie finestre di REAPER, oltre a offrire un supporto Braille esteso e molte altre funzionalità. Tieni presente che questi script sono concepiti per essere usati insieme a OSARA: non ne sono un'alternativa. Per un'accessibilità ottimale con JAWS, dovresti installarli entrambi.
package-ffmpeg-description = Le librerie di runtime condivise di FFmpeg consentono al decoder video di REAPER di importare e riprodurre i formati audio e video più comuni. RABBIT installa la cartella bin della build «GPL-shared» di BtbN in UserPlugins; il livello di patch non può essere dedotto dai soli nomi dei file DLL, perciò le installazioni esterne di FFmpeg vengono segnalate con un segnaposto «<major>.0.0».
package-surge-xt-description = Surge XT è un sintetizzatore ibrido gratuito e open source. RABBIT esegue il programma di installazione del fornitore al posto tuo — installa a livello di sistema i formati VST3, CLAP, AU (solo macOS) e standalone, così REAPER e gli altri DAW possono caricare Surge XT. Segue il canale nightly continuo perché l'ultima versione stabile (1.3.4) risale all'agosto 2024 e il progetto ormai viene distribuito di fatto tramite le nightly. Solo installazioni standard di REAPER: i dati di fabbrica risiedono al di fuori di qualsiasi cartella REAPER portatile.
package-app2clap-description = app2clap è un plug-in CLAP per Windows che cattura l'audio di altre applicazioni e lo porta in REAPER (o in qualsiasi host CLAP) come plug-in da inserire su una traccia — utile per registrare o elaborare il suono di un browser, un lettore multimediale o un altro programma. RABBIT scarica la versione più recente e installa app2clap.clap nella tua cartella CLAP personale, senza bisogno dei diritti di amministratore. Solo Windows. Solo installazioni standard di REAPER: viene installato al di fuori di qualsiasi cartella REAPER portatile.

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
wizard-button-close = Chiudi
wizard-button-close-mnemonic = C

wizard-target-heading = Scegli un'operazione
wizard-target-language-label = Lingua
wizard-target-language-restart-note = Cambiare la lingua riavvia RABBIT affinché la nuova lingua abbia effetto.
wizard-locale-name-en-US = Inglese (Stati Uniti)
wizard-locale-name-de-DE = Tedesco (Germania)
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

wizard-packages-heading = Scegli i pacchetti
wizard-packages-list-label = Pacchetti da installare o aggiornare
wizard-packages-tree-group-label = Pacchetti
wizard-additional-software-tree-group-label = Software aggiuntivo
wizard-configuration-tree-group-label = Configurazione
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
config-reapack-reaper-accessibility-name = Aggiungi il repository ReaPack «REAPER Accessibility» di Toni
config-reapack-reaper-accessibility-description = Aggiunge il repository ReaPack «REAPER Accessibility» di Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per ottenere ulteriori script ed estensioni accessibili.
config-reapack-reaper-accessible-fr-name = Aggiungi il repository ReaPack «REAPER Accessible (FR)»
config-reapack-reaper-accessible-fr-description = Aggiunge il repository ReaPack in lingua francese di REAPER Accessible (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per accedere alle risorse in francese di REAPER Accessible.
config-reapack-reaper-accessible-en-name = Aggiungi il repository ReaPack «REAPER Accessible (EN)»
config-reapack-reaper-accessible-en-description = Aggiunge il repository ReaPack in lingua inglese di REAPER Accessible (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Una volta aggiunto, apri il menu Estensioni, ReaPack, Sfoglia pacchetti per accedere alle risorse in inglese di REAPER Accessible.

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
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } controllo/i di versione non riuscito/i. Usa Indietro per provare un'altra destinazione, oppure chiudi RABBIT.
wizard-version-check-progress-label = Avanzamento
wizard-version-check-error-heading = Controlli non riusciti
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Dettagli del pacchetto
wizard-packages-osara-keymap-heading = Mappa dei tasti OSARA
wizard-packages-osara-keymap-replace-label = Sostituisci la tua mappa dei tasti attuale con l'ultima mappa dei tasti OSARA
wizard-packages-osara-keymap-unavailable-note = Seleziona OSARA per configurare il comportamento della sua mappa dei tasti.
wizard-packages-osara-keymap-preserve-note = Per utenti avanzati: la tua mappa dei tasti attuale verrà conservata. RABBIT non modificherà reaper-kb.ini; dovrai gestire manualmente l'aggiornamento alle ultime aggiunte alla mappa dei tasti OSARA.
wizard-packages-osara-keymap-replace-note = Consigliato per utenti da principianti a intermedi: RABBIT eseguirà il backup di una copia del tuo file reaper-kb.ini attuale, quindi lo sostituirà con l'ultima versione della mappa dei tasti OSARA.
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
wizard-review-notes-heading = Note
wizard-review-preflight-prefix = Impossibile installare per ora

# $path is the selected REAPER resource path.
wizard-review-target = Destinazione: { $path }
wizard-review-no-target = Nessuna destinazione selezionata.
wizard-review-no-package = Nessun pacchetto selezionato.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

wizard-progress-heading = Avanzamento dell'installazione
wizard-progress-status-idle = Pronto per l'installazione.
wizard-progress-status-running = Installazione dei pacchetti selezionati. Potrebbe richiedere alcuni minuti.
wizard-progress-details-label = Dettagli dell'avanzamento
wizard-progress-details-idle = Nessuna installazione in corso.
wizard-progress-details-starting = Avvio dell'operazione di configurazione.
wizard-progress-details-cache-prefix = Cache

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Download di { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Download di { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Download di { $count } pacchetti… { $downloaded } / { $total }
wizard-progress-status-installing = Installazione di { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Applicazione del passaggio di configurazione: { $step }

# Running log lines appended to the progress details text control.
wizard-progress-log-download-started = Download di { $package }…
wizard-progress-log-download-completed = { $package } scaricato.
wizard-progress-log-install-started = Installazione di { $package }…
wizard-progress-log-install-completed = { $package } installato.
wizard-progress-log-configuration-started = Applicazione di { $step }…
wizard-progress-log-configuration-completed = { $step } applicato.

wizard-done-heading = Completato
wizard-done-status-idle = Nessuna installazione è ancora stata avviata da questa finestra.
wizard-done-status-success = RABBIT ha finito di fare la sua magia! Consulta i dettagli qui sotto.
wizard-done-status-error = Installazione non riuscita. Consulta l'errore qui sotto.
wizard-done-status-no-packages = Nessun pacchetto è stato selezionato per l'installazione o l'aggiornamento.
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

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Aggiornamento di RABBIT disponibile
wizard-self-update-prompt-body = RABBIT { $latest } è disponibile. Attualmente hai { $current }. Aggiornare ora? RABBIT si riavvierà al termine dell'aggiornamento.

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
wizard-summary-resource-items-created = Elementi di risorse creati: { $count }
wizard-summary-packages-installed-or-checked = Pacchetti installati o verificati: { $count }
wizard-summary-packages-current = Pacchetti già aggiornati: { $count }
wizard-summary-packages-manual = Pacchetti che richiedono un intervento manuale: { $count }
wizard-summary-backup-files-created = File di backup creati: { $count }
wizard-summary-backup-file = File di backup: { $path }
wizard-summary-receipt-backup = Backup della ricevuta: { $path }
wizard-summary-backup-manifest = Manifesto del backup: { $path }
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Azione prevista: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Stato: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Versione installata: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Architettura: { $architecture }
status-installed-or-checked = Installato o verificato
status-planned-unattended = Previsto in modo automatico
status-deferred-unattended = Rinviato in modo automatico
status-skipped-current = Ignorato (già aggiornato)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Singolo binario di estensione gestito dal programma di installazione di RABBIT.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La versione installata { $installed } è uguale o più recente della versione disponibile { $available }.
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
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = Il passaggio di configurazione { $step } non è stato selezionato.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = Il passaggio di configurazione { $step } è stato ignorato perché il suo pacchetto prerequisito { $dependency } non era installato e non fa parte di questo piano.
config-message-applied-no-op = Passaggio di configurazione applicato senza modifiche.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Stato: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Applicato
config-status-skipped = Ignorato
config-status-skipped-dependency-missing = Ignorato (prerequisito mancante)
config-status-dry-run = Simulazione
wizard-summary-planned-execution-title = Esecuzione automatica prevista:
wizard-summary-planned-execution-runner =   Esecutore: { $runner }
wizard-summary-planned-execution-artifact =   Artefatto: { $artifact }
wizard-summary-planned-execution-program =   Programma: { $program }
wizard-summary-planned-execution-arguments =   Argomenti: { $arguments }
wizard-summary-planned-execution-working-directory =   Directory di lavoro: { $path }
wizard-summary-planned-execution-verify =   Verifica: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Nota: { $note }
wizard-summary-status-finished = Completato. { $installed } elemento/i di pacchetto installato/i o verificato/i; { $manual } richiede(ono) un intervento manuale.

wizard-planned-runner-launch-installer = Avvia l'eseguibile di installazione
wizard-planned-runner-extract-archive = Estrai l'archivio ed esegui il programma di installazione contenuto
wizard-planned-runner-extract-archive-copy-osara = Estrai l'archivio e copia le risorse di installazione di OSARA
wizard-planned-runner-mount-disk-image = Monta l'immagine disco ed esegui il programma di installazione contenuto
wizard-planned-runner-mount-disk-image-copy-app = Monta l'immagine disco e copia il bundle dell'applicazione contenuto
wizard-planned-runner-mount-disk-image-run-pkg = Monta l'immagine disco ed esegui il programma di installazione pkg contenuto
