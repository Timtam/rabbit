app-title = Herramienta de instalación Bootstrap de Reaper y accesibilidad
app-short-name = RABBIT

common-yes = si
common-no = no

action-install = Instalar
action-update = Actualizar
action-keep = No hace falta tocarlo

package-reaper = REAPER
package-osara = OSARA
package-sws = Extensión SWS 
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Scripts de JAWS para Reaper desarrollados por SnowMan
package-ffmpeg = FFmpeg para soporte de video mejorado
package-surge-xt = Surge XT
package-app2clap = app2clap
package-langpack-es = REAPER en español
package-langpack-de = REAPER en alemán
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Equipo PMA (es_MX)

package-reaper-description = REAPER es la estación de trabajo de audio digital sobre la que se construye todo lo demás. RABBIT puede instalar o actualizar REAPER por ti.
package-osara-description = OSARA es la extensión de accesibilidad de código abierto que hace que REAPER sea utilizable con lectores de pantalla. NVDA, JAWS y Narrador en Windows, así como VoiceOver en macOS, están ampliamente adoptados; algunos otros lectores de pantalla para Windows también podrían funcionar. Instala OSARA si dependes de un lector de pantalla para usar REAPER.
package-sws-description = La extensión SWS es un paquete de acciones adicionales, scripts y utilidades creado por la comunidad que amplía las funciones de REAPER. Para la configuración más accesible de REAPER, ya sea en Windows o en Mac, deberías instalar SWS junto con OSARA.
package-reapack-description = ReaPack es un administrador de paquetes de código abierto. Se puede utilizar para buscar, instalar, seguir y actualizar scripts y paquetes de terceros desde el mismo Reaper. Instálalo si quieres utilizar paquetes compartidos por la comunidad de Reaper.
package-reakontrol-description = ReaKontrol ofrece integración de código abierto para los teclados Komplete Kontrol de Native Instruments. Instala esto si tienes un teclado de la serie S MK2, serie A, M‑32 o Kontrol MK3.
package-jaws-scripts-description = Los scripts de Snowman mejoran la forma en que JAWS maneja diversas ventanas dentro de REAPER, además de ofrecer soporte ampliado para Braille y muchas otras funciones. Ten en cuenta que estos scripts están pensados para usarse junto con OSARA; no son una alternativa a este. Para una accesibilidad óptima con JAWS, deberías instalar ambos.
package-ffmpeg-description = Las bibliotecas de tiempo de ejecución compartidas de FFmpeg permiten que el decodificador de video de REAPER importe y reproduzca formatos comunes de video y audio. RABBIT instala la carpeta bin de la compilación GPL‑shared de BtbN en UserPlugins; el nivel de parche no puede recuperarse únicamente a partir de los nombres de los archivos DLL, por lo que las instalaciones externas de FFmpeg se informan con un marcador de posición <major>.0.0.
package-surge-xt-description = Surge XT es un sintetizador híbrido gratuito y de código abierto. RABBIT ejecuta el instalador del proveedor por ti: instala los formatos VST3, CLAP, AU (solo en macOS) y la versión independiente a nivel del sistema, de modo que REAPER y otros DAWs puedan cargar Surge XT. Se sigue el canal nocturno continuo porque la versión estable más reciente (1.3.4) es de agosto de 2024 y el proyecto actualmente se distribuye principalmente mediante compilaciones tempranas. Solo en instalaciones estándar de REAPER: los datos de fábrica se almacenan fuera de cualquier carpeta portátil de REAPER.
package-app2clap-description = app2clap es un complemento CLAP para Windows que captura audio de otras aplicaciones y lo lleva a REAPER (o cualquier host CLAP) como un plug‑in que insertas en una pista — útil para grabar o procesar sonido desde un navegador, reproductor multimedia u otro programa. RABBIT descarga la última compilación e instala app2clap.clap en tu carpeta CLAP de usuario, por lo que no se requieren privilegios de administrador. Solo para Windows. Solo en instalaciones estándar de REAPER. se instala fuera de cualquier carpeta portátil de REAPER.
package-langpack-es-description = Traduce la interfaz de REAPER al español, incluida la extensión SWS. Mantenido por Javier Robledo para la comunidad hispanohablante de REAPER y publicado en reaperespa.com: RABBIT descarga la versión actual y la instala como es_ES.ReaperLangPack. OSARA usa ese nombre de archivo para elegir su propia traducción al español. Después de instalarlo, selecciona el idioma en las preferencias de REAPER (o deja que RABBIT lo haga por ti).
package-langpack-de-description = Traduce la interfaz de REAPER al alemán, incluida la extensión SWS. Mantenido por MrData y publicado en el Stash de REAPER: RABBIT descarga la versión actual y la instala como de_DE.ReaperLangPack. Después de instalarlo, selecciona el idioma en las preferencias de REAPER (o deja que RABBIT lo haga por ti).

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (No está disponible: { $reason })
wizard-package-row-unavailable-portable = Ruta de Reaper portable
wizard-package-row-unavailable-version-check = Falló la comprobación de la versión en línea

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package }: La última -comprobación de versión falló ({ $message }). La instalación o actualización de este paquete se desactiva para esta ejecución.

detect-installed = Instalado
detect-not-installed = No está instalado
detect-version-unknown = Versión desconocida
detect-source-receipt = Recibo de Rabbit
detect-source-files = Se encuentran los archivos en UserPlugins
detect-source-reapack-registry = Registro de ReaPack

# $package is the localized package display name.
status-package-installed = { $package } instalado

wizard-step-target = Destino
wizard-step-version-check = Comprobación de versión
wizard-step-packages = Paquetes
wizard-step-reapack-acknowledgement = Donación para ReaPack
wizard-step-review = Revisar
wizard-step-progress = Progreso
wizard-step-done = Hecho

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Atrás
wizard-button-back-mnemonic = A
wizard-button-next = Siguiente
wizard-button-next-mnemonic = S
wizard-button-install = Instalar
wizard-button-install-mnemonic = I
wizard-button-close = Cerrar
wizard-button-close-mnemonic = C

wizard-target-heading = Selecciona una tarea
wizard-target-language-label = Idioma
wizard-target-language-restart-note = Cambiar de idioma reinicia RABBIT para que el cambio de idioma tenga efecto
wizard-locale-name-en-US = inglés (Estados Unidos)
wizard-locale-name-de-DE = alemán (Alemania)
wizard-locale-name-es-ES = español (España)
wizard-locale-name-fr-FR = francés (Francia)
wizard-locale-name-it-IT = italiano (Italia)
wizard-target-choice-label = Carpeta de instalación
wizard-target-details-label = Detalles del destino
wizard-target-empty = No seleccionaste una ruta de instalación de Reaper.
wizard-target-portable-choice = Creación o actualización de una versión portable de Reaper
wizard-target-portable-folder-label = Carpeta de la copia portable
wizard-target-portable-folder-message = Selecciona una carpeta de una copia portable existente, o elige una carpeta vacía si estás creando una copia nueva.
wizard-target-portable-folder-browse-label = Examinar…
wizard-target-portable-pending-details = Usa el botón examinar para indicar dónde tienes una versión portable de Reaper, o selecciona una carpeta vacía si estás creando una nueva.
wizard-target-custom-portable-label = Carpeta de Reaper portable
wizard-target-custom-portable-app-path-label = Carpeta de la aplicación Reaper
wizard-target-custom-portable-path-label = Carpeta de recursos (resources) del portable 
wizard-target-custom-portable-version-label = Versión de Reaper
wizard-target-custom-portable-writable-label = Escritura disponible
wizard-target-custom-portable-note = RABBIT creará la carpeta de recursos del portable si no existe una.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } en { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = Carpeta de instalación de Reaper: { $app_path }
    Versión: { $version }
    Carpeta de recursos: { $path }
    Escritura disponible: { $writable }

wizard-packages-heading = Selecciona los paquetes
wizard-packages-list-label = Paquetes para instalar o actualizar
wizard-packages-tree-group-label = Paquetes
wizard-additional-software-tree-group-label = Software adicional
wizard-language-tree-group-label = Paquetes de idioma
wizard-configuration-tree-group-label = Configuración
# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = No está disponible: Se requiere instalar { $package } .
wizard-configuration-row-already-applied = Ya se aplicó en esta ruta de Reaper.
# Short status tag appended in parentheses to a configuration row's tree label
# when the row isn't actionable. Kept terse so the tree label stays readable;
# the longer sentence in `wizard-configuration-row-unavailable` /
# `wizard-configuration-row-already-applied` is still surfaced in the details
# pane and as the row's accessible reason.
# $reason is one of the "wizard-configuration-row-status-*" strings below.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package is the localized name of the dependency package.
wizard-configuration-row-status-requires = Se requiere { $package }
wizard-configuration-row-status-already-applied = Ya se aplicó
config-reapack-reaper-accessibility-name = Agregar el repositorio de scripts de Toni a ReaPack
config-reapack-reaper-accessibility-description = Agregar el repositorio de scripts de Ttoni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Después de agregarlo, abre el menú de extensiones de Reaper, ReaPack, Browse Packages para obtener scripts y plugings adicionales.
config-reapack-reaper-accessible-fr-name = Agregar el repositorio de Reaper Accessible (francés) a ReaPack
config-reapack-reaper-accessible-fr-description = Agrega el repositorio de ReaPack de Reaper Accessible en francés (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Después de agregarlo, abre el menú extensiones, ReaPack, Browse Packages para obtener los recursos de Reaper Accessible en francés.
config-reapack-reaper-accessible-en-name = Agregar el repositorio de Reaper Accessible (inglés) a ReaPack
config-reapack-reaper-accessible-en-description = Agrega el repositorio de Reaper Accessible en inglés a ReaPack (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Después de agregarlo, abre el menú extensiones, ReaPack, Browse Packages para obtener los recursos de Reaper Accessible en inglés.
config-set-reaper-language-es-name = Configurar REAPER en español
config-set-reaper-language-es-description = Indica a REAPER que use el paquete de idioma español, escribiéndolo en reaper.ini. Sin esto tendrías que seleccionar el idioma manualmente en Opciones, Preferencias, General. Desmarca esta casilla si solo quieres que se instale el archivo.
config-set-reaper-language-de-name = Configurar REAPER en alemán
config-set-reaper-language-de-description = Indica a REAPER que use el paquete de idioma alemán, escribiéndolo en reaper.ini. Sin esto tendrías que seleccionar el idioma manualmente en Opciones, Preferencias, General. Desmarca esta casilla si solo quieres que se instale el archivo.

wizard-reapack-ack-heading = Anuncio de donaciones para ReaPack
wizard-reapack-ack-body = ReaPack es software gratuito publicado bajo la licencia LGPL. Su autor, Christian Fillion, acepta donaciones opcionales para apoyar el desarrollo continuo. Christian también mantiene las extensiones SWS y en el pasado ha incorporado código específicamente para mejorar la compatibilidad con OSARA. Cualquier apoyo que puedas brindar está más que justificado.
wizard-reapack-ack-link-label = Abrir la página de donaciones para ReaPack
wizard-reapack-ack-confirm-label = Saltar donación esta vez, instalar o actualizar ReaPack
cli-reapack-ack-prompt-summary = ReaPack es software gratuito (LGPL). Su autor, Christian Fillion, acepta donaciones opcionales en https://reapack.com/donate, esto le llevará a reapack.com para apoyar el desarrollo continuo.
cli-reapack-ack-flag-required = ReaPack está en el plan de esta ejecución, pero falta el aviso de donación. Vuelve a ejecutar con --accept-reapack-donation-notice para confirmar que has leído https://reapack.com/donate, esto le llevará a reapack.com y que deseas que RABBIT instale o actualice ReaPack.

wizard-version-check-heading = Comprobando las últimas versiones…
wizard-version-check-status-pending = Preparando la comprobación de la última versión…
# $package is the localized package display name.
wizard-version-check-status-checking = revisando { $package }…
wizard-version-check-status-progress = Comprobando versiones… ({ $done } de { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } en la comprobación de versión (s) falló. Usa el botón atrás para intentar con una ruta diferente, o cierra RABBIT.
wizard-version-check-progress-label = progreso
wizard-version-check-error-heading = revisiones fallidas:
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Detalles del paquete
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = Novedades en { $package }:
wizard-packages-osara-keymap-heading = Mapa de atajos de teclado de Osara
wizard-packages-osara-keymap-replace-label = Reemplazar el mapa de teclado actual con el último mapa de teclado
packages-spanish-variant-label = Usar la traducción de OSARA al español del Equipo PMA (es_MX) en lugar de REAPER Accesible español (es_ES)
wizard-packages-osara-keymap-unavailable-note = Selecciona Osara para configurar el comportamiento de su mapa de teclado.
wizard-packages-osara-keymap-preserve-note = Para usuarios avanzados: Tu mapa de teclado se conservará. Rabbit no modifica tu archivo reaper-kb.ini, debes estar al día con las adiciones al mapa de forma manual.
wizard-packages-osara-keymap-replace-note = Recomendado para usuarios principiantes o intermedios: RABBIT ará una copia de tu archivo reaper-kb.ini, y reemplazará este archivo con una nueva copia del último mapa de teclado de Osara.
wizard-package-details-handling-prefix = procesamiento
wizard-package-handling-automatic = RABBIT puede instalar este paquete directamente.
wizard-package-handling-unattended = RABBIT puede instalar este paquete de forma desatendida, lo que incluye ejecutar el instalador cuando se requiere.
wizard-package-handling-planned = RABBIT está diseñado para descargar e instalar los paquetes de forma desatendida, pero informará de los pasos en lugar de ejecutarlos.
wizard-package-handling-manual = RABBIT ejecutará este paquete e informará de los pasos a realizar manualmente durante la ejecución.
wizard-package-handling-unavailable = Este paquete no está disponible para la plataforma o arquitectura actual.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package }: { $action }. Tienes  la versión { $installed }. La última es la  { $available }

wizard-review-heading = Revisa lo que le pediste a RABBIT que haga:
wizard-review-target-prefix = Destino
wizard-review-package-heading = Paquetes seleccionados
wizard-review-osara-keymap-heading = Mapa de teclado de Osara
wizard-review-osara-keymap-preserve = Conservar tu mapa de teclado actual de Osara.
wizard-review-osara-keymap-replace = Guardar tu mapa de teclado actual y reemplazarlo con el último mapa de Osara.
wizard-review-notes-heading = Notas
wizard-review-preflight-prefix = Todavía no se puede instalar

# $path is the selected REAPER resource path.
wizard-review-target = Destino: { $path }
wizard-review-no-target = No seleccionaste un destino.
wizard-review-no-package = No hay un paquete seleccionado.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

wizard-progress-heading = Progreso de la instalación
wizard-progress-status-idle = Listo para instalar.
wizard-progress-status-running = Instalando paquetes seleccionados. Esto puede tardar unos minutos.
wizard-progress-details-label = Detalles del progreso
wizard-progress-details-idle = Sin instalación en curso.
wizard-progress-details-starting = Iniciando operaciones de instalación.
wizard-progress-details-cache-prefix = Caché

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Descargando { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = descargando { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = descargando { $count } paquetes… { $downloaded } / { $total }
wizard-progress-status-installing = instalando { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Aplicando configuración: { $step }

# Running log lines appended to the progress details text control.
wizard-progress-log-download-started = Descargando { $package }…
wizard-progress-log-download-completed = se descargó { $package }.
wizard-progress-log-install-started = instalando { $package }…
wizard-progress-log-install-completed = se instaló { $package }.
wizard-progress-log-configuration-started = aplicando { $step }…
wizard-progress-log-configuration-completed = se aplicó { $step }.

wizard-done-heading = Hecho
wizard-done-status-idle = Todavía no se ejecutó una instalación desde esta ventana.
wizard-done-status-success = ¡RABBIT terminó de hacer su magia! Revisar los detalles acontinuación.
wizard-done-status-error = La instalación ha fallado. Revisa el error acontinuación.
wizard-done-status-completed-with-errors = La instalación se completó con errores. Revisa los detalles a continuación.
wizard-done-status-no-packages = No se ha seleccionado ningún paquete para instalar o actualizar.
wizard-done-show-details = Mostrar detalles
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = Abrir Reaper y cerrar RABBIT
wizard-done-launch-reaper-mnemonic = A
wizard-done-open-resource = Abrir carpeta de recursos de Reaper (solo para mantenimiento avanzado)
wizard-done-open-resource-mnemonic = R
wizard-done-no-reaper-app = No se encuentra una copia de Reaper que puedas abrir desde esta ruta.
wizard-done-launch-reaper-error-prefix = No se pudo abrir Reaper
wizard-done-open-resource-error-prefix = No se pudo abrir la carpeta de recursos de Reaper
wizard-done-self-update-apply-running = Actualizando RABBIT…
wizard-done-self-update-error-prefix = La actualización de RABBIT falló.
wizard-done-self-update-relaunch-prefix = RABBIT reiniciado
wizard-self-update-status-checking = Comprobando actualizaciones de RABBIT…

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Actualización de RABBIT disponible
wizard-self-update-prompt-body = RABBIT { $latest } está disponible. Ahora tienes { $current }. ¿Quieres actualizar? RABBIT se va a reiniciar cuando termine la actualización.

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = Actualización de RABBIT disponible: { $current } → { $latest } (Canal { $channel }). Reinicia RABBIT para volver a preguntar.
self-update-status-up-to-date = RABBIT está actualizado (current { $current }, canal { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = La actualización no reemplazó archivos (target version { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = Se reemplazaron los archivo (s) { $count } en { $root }; Reiniccia RABBIT para utilizar { $version }.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Verificación de firma: { $signed } firmado.
self-update-apply-signature-summary-unsigned-only = Comprobación de firma: { $unsigned } sin firmar.
self-update-apply-signature-summary-mixed = Verificación de firma: { $signed } firmado, { $unsigned } sin firmar.

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Otra instalación de RABBIT está en progreso (PID { $pid }). La aplicación estará suspendida hasta que finalice.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Destino: { $path }
wizard-summary-portable = Destino del portable: { $value }
wizard-summary-dry-run = Ejecutar Dry: { $value }
wizard-summary-packages-selected = Paquetes seleccionados: { $packages }
wizard-summary-cache = Caché: { $path }
wizard-summary-planned-app = Ruta de la planeación de la aplicación: { $path }
wizard-summary-error = Error: { $message }
wizard-summary-error-antivirus = El software de seguridad de Windows bloqueó esta descarga. Suele ser un falso positivo: el instalador está firmado digitalmente y RABBIT ya lo había verificado con la suma de comprobación del editor. A veces Defender marca un instalador recién compilado que todavía no ha visto con frecuencia; las versiones de desarrollo de OSARA son el caso más común. Para continuar: abre Seguridad de Windows, ve a «Protección contra virus y amenazas» y luego a «Historial de protección», selecciona el elemento bloqueado y elige «Permitir», y vuelve a ejecutar RABBIT. También puedes añadir la carpeta de descargas de RABBIT a las exclusiones, o instalar este paquete manualmente desde el sitio web del editor. RABBIT nunca desactiva tu protección antivirus.
wizard-summary-resource-items-created = Se crearon los items de recursos: { $count }
wizard-summary-packages-installed-or-checked = Paquetes instalados o revisados: { $count }
wizard-summary-packages-current = Paquetes actuales: { $count }
wizard-summary-packages-manual = Paquetes que necesitan atención manual: { $count }
wizard-summary-backup-files-created = Se crearon los archivos de copia de seguridad: { $count }
wizard-summary-backup-file = Archivo de copia de seguridad: { $path }
wizard-summary-receipt-backup = Copia del recibo: { $path }
wizard-summary-backup-manifest = Copia del manifiesto: { $path }
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Acción que se planea: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Estado: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Versión instalada: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Arquitectura: { $architecture }
status-installed-or-checked = Instalado o revisado
status-planned-unattended = Plan desatendido
status-deferred-unattended = Plan diferido
status-skipped-current = Omitido (ya existe)
status-failed = Fallido
status-skipped-dependency-failed = Omitido (falló una dependencia)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = RABBIT manejó el archivo binario simple de instalación.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = La versión instalada { $installed } esmás reciente que la versión disponible { $available }.
package-status-skipped-content-unchanged = La copia instalada es idéntica a la publicada por el editor.
# $error es el texto del error. $dependency es el paquete (p. ej. REAPER) que falló primero.
package-status-install-failed = La instalación falló: { $error }
package-status-skipped-dependency-failed = Omitido porque { $dependency }, que necesita, no se instaló correctamente.
# $automation is one of the "package-automation-*" labels (vendor installer / archive extraction / ...).
package-status-dry-run-would-run-unattended = Dry run: RABBIT Va a descargar y ejecutar esta { $automation } desatendida.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-staged = Esta compilación no tiene la automatización { $automation } de la carpeta de ejecución, no aún. RABBIT dejará los artefactos en la caché, pero no los ejecutará.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-not-staged = Esta compilación no tiene planeada la ejecución de la automatización { $automation } de la carpeta de ejecución, no aún. RABBIT no descargará ni ejecutará el artefacto.
package-status-unattended-installed = RABBIT ejecutó el instalador original en modo desatendido, verificó las rutas de destino esperadas y actualizó su recibo.
package-status-osara-unattended-keymap-backed-up = RABBIT Se ejecutó el instalador original en modo desatendido, se respaldó reaper-kb.ini, se aplicó el reemplazo del mapa de teclas de OSARA y se actualizó el recibo de RABBIT.
package-status-osara-unattended-keymap-replaced = RABBIT ejecutó el instalador original en modo desatendido, aplicó el reemplazo del mapa de teclas de OSARA y actualizó el recibo de RABBIT.

# Short automation-kind labels interpolated into the per-package status
# messages above.
package-automation-installer = Instalador del probeedor
package-automation-archive = Extracción de archivo
package-automation-disk-image = Imagen de disco de instalación
package-automation-extension-binary = Instalación directa desde el archivo

# Per-configuration-step status messages surfaced on the wizard's Done page.
# `wizard-summary-configuration-message = { $step }: { $message }` is the
# wrapper template — the `*-message` keys below are the message body only.
# $name is the human-readable remote name; $url is the index XML URL.
config-message-reapack-remote-already-present = ReaPack remote { $name } ({ $url }) ya se configuró en reapack.ini.
config-message-reapack-remote-added = Added ReaPack remote { $name } ({ $url }) a reapack.ini.
config-message-reapack-remote-created-file = Se creó reapack.ini with ReaPack remote { $name } ({ $url }). ReaPack agregará sus repositorios de fábrica en la siguiente ejecución.
config-message-reapack-remote-dry-run = Se agregó ReaPack remote { $name } ({ $url }) a reapack.ini.
config-message-reaper-language-already-selected = REAPER ya estaba configurado para usar { $file }.
config-message-reaper-language-selected = Se configuró el idioma de REAPER como { $file }.
config-message-reaper-language-dry-run = Se configuraría el idioma de REAPER como { $file }.
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = El paso de configuración { $step } No se ha seleccionado.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = El paso de configuración { $step } se saltó porque los paquetes dependientes  { $dependency } no se han instalado, y tampoco son parte de este plan.
config-message-applied-no-op = Se aplicaron los pasos de configuración sin cambios

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Estado: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Aplicado
config-status-skipped = Omitido
config-status-skipped-dependency-missing = Omitido  (falta dependencia)
config-status-dry-run = Dry ejecutado
wizard-summary-planned-execution-title = Ejecución desatendida planeada:
wizard-summary-planned-execution-runner =   Se ejecuta: { $runner }
wizard-summary-planned-execution-artifact =   Artefacto: { $artifact }
wizard-summary-planned-execution-program =   Programa: { $program }
wizard-summary-planned-execution-arguments =   Argumentos: { $arguments }
wizard-summary-planned-execution-working-directory =   Directorio de trabajo: { $path }
wizard-summary-planned-execution-verify =   Verificar: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Nota: { $note }
wizard-summary-status-finished = Terminó el proceso. { $installed } paquete (s) se han instalado o revisado; { $manual } requieren atención.
wizard-summary-status-finished-with-errors = Terminó con errores. { $installed } paquete (s) se han instalado o revisado; { $failed } fallaron.

wizard-planned-runner-launch-installer = Abrir ejecutable del isntalador
wizard-planned-runner-extract-archive = Extraer el archivo y abrir el instalador incluido
wizard-planned-runner-extract-archive-copy-osara = Extraer archivo y abrir los archivos internos de Osara
wizard-planned-runner-mount-disk-image = Abrir la imagen de disco y ejecutar el instalador incluido
wizard-planned-runner-mount-disk-image-copy-app = Montar esta imagen y copiar el contenido del paquete de la aplicación
wizard-planned-runner-mount-disk-image-run-pkg = Montar esta imagen y ejecutar el instalador pkg incluido
