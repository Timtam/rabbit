app-title = Herramienta de instalación Bootstrap de Reaper y accesibilidad
app-short-name = RABBIT

common-yes = si
common-no = no

# What happens to a package in this run, the first thing its row says. An
# unticked row on either tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal.
action-install = Instalar
action-update = Actualizar
action-keep = No modificar
action-uninstall = Desinstalar
# On the Backups tab: what happens to a backup ticked to restore, or to
# delete. The last is for a backup that is the only copy of what it holds,
# such as software you uninstalled.
action-restore = Restaurar
action-delete = Eliminar
action-delete-only-copy = Eliminar la única copia

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
package-langpack-fr = REAPER en francés
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
package-langpack-fr-description = Traduce la interfaz de REAPER al francés, incluida la extensión SWS. Mantenido por Lee Julien y Pierre-Marie Curt para ReaperAccessible, la comunidad francesa de usuarios de lectores de pantalla, y publicado en su repositorio de ReaPack: RABBIT descarga la versión actual y la instala como fr_CA.ReaperLangPack. OSARA lee ese nombre de archivo para elegir su propia traducción al francés: la fr_CA está casi completa, mientras que la fr_FR todavía deja alrededor de un tercio de los mensajes en inglés; el paquete en sí es el mismo francés estándar en ambos casos. Después de instalarlo, selecciona el idioma en las preferencias de REAPER (o deja que RABBIT lo haga por ti).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = RABBIT todavía no puede desinstalar este paquete.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Elimina la extensión SWS.
uninstall-summary-sws-keeps = Tus grooves y tu configuración de SWS, como tus acciones cíclicas, se conservan.
uninstall-summary-reapack-removes = Elimina la extensión ReaPack.
uninstall-summary-reapack-keeps = Tu configuración y tus repositorios de ReaPack se conservan, igual que la lista de lo que ReaPack instaló.
uninstall-summary-reakontrol-removes = Elimina la extensión ReaKontrol.
uninstall-summary-reakontrol-keeps = Tus mapas de FX se conservan, y RABBIT no toca tu software de Native Instruments.
uninstall-summary-ffmpeg-removes = Elimina los archivos de FFmpeg que RABBIT instaló en UserPlugins. Después, REAPER ya no puede usar FFmpeg para abrir o renderizar archivos multimedia.
uninstall-summary-ffmpeg-keeps = Los archivos de FFmpeg de los que RABBIT no tiene registro se conservan, como los que dejaron versiones anteriores.
uninstall-summary-app2clap-removes = Elimina app2clap de tu carpeta CLAP. Lo pierden todos los programas en los que usas plugins CLAP, no solo REAPER.
uninstall-summary-app2clap-keeps = La carpeta CLAP y los demás plugins que contiene se conservan.
uninstall-summary-langpack-removes = Elimina el paquete de idioma que instaló RABBIT. Si REAPER lo usa, RABBIT vuelve a poner REAPER en inglés.
uninstall-summary-langpack-keeps = Los paquetes de idioma que añadiste tú se conservan.
uninstall-summary-osara-removes = Elimina la extensión OSARA, sus traducciones y la copia de su mapa de teclado en la carpeta KeyMaps.
uninstall-summary-osara-removes-windows = Elimina la extensión OSARA, sus traducciones, su desinstalador y la copia de su mapa de teclado en la carpeta KeyMaps. Si Windows incluye este OSARA en su lista de programas instalados, RABBIT lo quita de esa lista.
uninstall-summary-osara-keeps = Tu mapa de teclado se queda como está, igual que la configuración de OSARA y las copias de seguridad de tu mapa de teclado que hizo OSARA.
uninstall-summary-surge-removes = El propio desinstalador de Surge XT lo quita para todos los programas de música de este ordenador. Una copia de seguridad no puede recuperarlo.
uninstall-summary-surge-keeps = Tus propios patches, en la carpeta Surge XT de tus Documentos, se conservan.
uninstall-summary-reaper-windows-removes = El propio desinstalador de REAPER quita el programa. También quita las asociaciones de archivos de REAPER y ReaRoute para todos los REAPER de este ordenador.
uninstall-summary-reaper-keeps = Tus ajustes, extensiones y proyectos se conservan.
# The JAWS scripts: two plug-in files in REAPER's UserPlugins folder, and
# script files their installer adds to JAWS's settings folder, which RABBIT
# removes only while they are as the installer left them. Keep
# %APPDATA%\Freedom Scientific\JAWS as it is.
uninstall-summary-jaws-scripts-removes = Elimina los dos archivos de plugin de los scripts de la carpeta UserPlugins de REAPER, y los archivos de scripts que RABBIT vio que su instalador añadía a la carpeta de configuración de JAWS.
uninstall-summary-jaws-scripts-keeps = Los archivos de scripts que RABBIT no sabe con certeza si añadió el instalador, y los que han cambiado desde entonces, como tus propias asignaciones de teclas, se conservan en la carpeta de configuración de JAWS, %APPDATA%\Freedom Scientific\JAWS.
# The same two sentences on macOS, where Surge XT has no uninstaller of its
# own and REAPER is an app. Name the Trash and the Library folder as Finder
# does in your language.
uninstall-summary-surge-macos-removes = Quita las apps y los plug-ins de Surge XT, y sus datos de fábrica de la carpeta Biblioteca. Una copia de seguridad no puede recuperarlos.
uninstall-summary-reaper-macos-removes = Mueve la app REAPER a la Papelera.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = tus grooves
uninstall-kept-sws-settings = configuración de SWS
uninstall-kept-sws-cycle-actions = tus acciones cíclicas
uninstall-kept-reapack-settings = configuración de ReaPack y tus repositorios
uninstall-kept-reapack-data = la lista de lo que ReaPack instaló y su caché de descargas
uninstall-kept-reakontrol-fx-maps = tus mapas de FX
uninstall-kept-app2clap-clap-folder = la carpeta CLAP, que comparten otros plugins
uninstall-kept-osara-key-map = tu mapa de teclado
uninstall-kept-osara-replaced-key-map = tu mapa de teclado de antes de que OSARA lo reemplazara
uninstall-kept-osara-merged-key-map = tu mapa de teclado de antes de que OSARA le añadiera sus atajos
uninstall-kept-osara-settings = la configuración de OSARA, en un archivo que comparten todas las extensiones
uninstall-kept-surge-user-data = tus patches y todo lo demás que hayas guardado con Surge XT
uninstall-kept-reaper-resource = la carpeta de recursos de REAPER, con tus ajustes, extensiones y mapas de teclado
# On macOS, in the Library folder of your home folder.
uninstall-kept-surge-settings-macos = tus ajustes de Surge XT
# On macOS, in the Library folder of your home folder: a copy of Surge XT's
# factory data put there by hand, which Surge XT reads instead of the one it
# installs for everyone. Say "factory data" as in
# uninstall-summary-surge-macos-removes.
uninstall-kept-surge-factory-copy-macos = tu propia copia de los datos de fábrica de Surge XT

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Si usas el mapa de teclado de OSARA, algunos de sus atajos ejecutan acciones de SWS. Esos atajos dejan de funcionar hasta que vuelvas a instalar SWS.
uninstall-warning-sws-reapack-scripts = Muchos scripts de REAPER, entre ellos scripts de accesibilidad, necesitan SWS. Los scripts que instalaste con ReaPack pueden dejar de funcionar.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Los scripts y extensiones que instalaste con ReaPack se conservan, pero ya nada los mantiene al día.
uninstall-note-surge-fonts-stay = Las fuentes que instaló Surge XT se conservan: su instalador las marca para que nunca se quiten.
# A note on the JAWS script files that stay after the JAWS scripts are
# uninstalled, and how to remove them by hand. Keep %APPDATA%, the folder
# and file names, and "reaper" (the start of the file names) exactly as they
# are. For the example language folder, use JAWS's folder for your language:
# enu for English, deu for German, esn for Spanish, fra for French, ita for
# Italian.
uninstall-note-jaws-scripts-by-hand = Para quitar los archivos de scripts que se conservan, borra los archivos cuyo nombre empieza por reaper de las carpetas de idioma de %APPDATA%\Freedom Scientific\JAWS, como 2025\Settings\esn. Antes, copia reaper.jkm, reaper.jcf y reaper.ini en otro lugar si quieres conservar tus asignaciones de teclas y tu configuración.

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
uninstall-by-hand-jaws-scripts = Para quitar los scripts de JAWS a mano:
    1. Cierra REAPER.
    2. Borra Reaper_ComAccess32.dll y Reaper_ComAccess64.dll de %APPDATA%\REAPER\UserPlugins.
    3. Borra la carpeta %ProgramFiles(x86)%\Reaper_JawsScripts. Windows pide el permiso de un administrador.
    4. Para quitar también los archivos de scripts, borra los archivos cuyo nombre empieza por reaper de las carpetas de idioma de %APPDATA%\Freedom Scientific\JAWS, como 2025\Settings\esn. Antes, copia reaper.jkm, reaper.jcf y reaper.ini en otro lugar si quieres conservar tus asignaciones de teclas y tu configuración.
uninstall-manual-reaper-portable = RABBIT no quita un REAPER portable: los archivos de REAPER y los tuyos comparten su carpeta. Para quitarlo a mano:
    1. Cierra REAPER.
    2. Copia fuera de la carpeta del REAPER portable todo lo que quieras conservar, como los proyectos que guardaste allí, tus mapas de teclado y tu configuración.
    3. Borra la carpeta del REAPER portable.
    Las copias de seguridad que RABBIT hizo para este REAPER también están en esa carpeta, en RABBIT\backups, y se borran con ella.

# Why RABBIT didn't start a program's own uninstaller, or didn't remove a
# package's uninstaller folder and registry entry. RABBIT does both with
# administrator rights. Nothing has changed at that point. RABBIT shows one
# of these, then uninstall-vendor-refused-nothing-changed, or, for the folder
# and registry entry, the steps to remove the package by hand.
# { $package } is the package's name, such as Surge XT, REAPER or
# JAWS-for-REAPER scripts.
# { $path } is a file, a folder or a value as Windows holds it; keep it as it
# is.
uninstall-vendor-refused-not-listed = Windows no incluye { $package } entre los programas instalados para todos los usuarios, así que RABBIT no encuentra su desinstalador.
uninstall-vendor-refused-unclear-command = RABBIT no sabe qué programa indica Windows como desinstalador de { $package }: { $path }
uninstall-vendor-refused-not-a-full-path = Windows indica el desinstalador de { $package } con una ruta en la que RABBIT no confía: { $path }
uninstall-vendor-refused-unknown-folder = RABBIT no encuentra una carpeta que necesita para comprobar el desinstalador de { $package }.
uninstall-vendor-refused-wrong-folder = El desinstalador de { $package } no está en la carpeta que le corresponde: { $path }
uninstall-vendor-refused-foreign-file = Junto al desinstalador de { $package } hay un archivo que no debería estar ahí: { $path }
uninstall-vendor-refused-folder-or-link = Junto al desinstalador de { $package } hay una carpeta o un enlace que no debería estar ahí: { $path }
uninstall-vendor-refused-untrusted-owner = Junto al desinstalador de { $package } hay un archivo que no puso ahí un administrador: { $path }
uninstall-vendor-refused-install-location-missing = Windows no indica en qué carpeta está instalado { $package }.
uninstall-vendor-refused-install-location-mismatch = Según Windows, { $package } está instalado en { $path }, no en el REAPER que elegiste.
uninstall-vendor-refused-not-in-install-folder = El desinstalador de { $package } no está en su carpeta de programa: { $path }
uninstall-vendor-refused-not-below-program-files = { $package } no está instalado en una carpeta propia dentro de Archivos de programa (Program Files): { $path }
uninstall-vendor-refused-copy-differs = La copia que RABBIT hizo del desinstalador de { $package } no es igual al original.
uninstall-vendor-refused-unreadable = RABBIT no pudo leer { $path } para comprobar el desinstalador de { $package }.
uninstall-vendor-refused-not-a-vendor-plan = RABBIT no tiene ningún desinstalador de { $package } que pueda ejecutar.
uninstall-vendor-refused-unsupported = RABBIT solo puede ejecutar el desinstalador de { $package } en Windows.
# { $path } is the name of a value in the package's registry entry.
uninstall-vendor-refused-unexpected-registry-value = La entrada del registro de { $package } contiene un valor que RABBIT no espera: { $path }
# { $path } is the text of a value in the package's registry entry.
uninstall-vendor-refused-registry-value-mismatch = La entrada del registro de { $package } apunta a un lugar que RABBIT no espera: { $path }
# The package's registry entry has subkeys, which its installer doesn't write.
uninstall-vendor-refused-registry-key-has-subkeys = La entrada del registro de { $package } contiene más de lo que RABBIT espera.
# { $path } is the package's uninstaller folder in Program Files.
uninstall-vendor-refused-untrusted-folder = La carpeta de { $package } no la creó un administrador: { $path }
# Follows each of the messages above. { $package } as above.
uninstall-vendor-refused-nothing-changed = No se ha cambiado nada. Puedes desinstalar { $package } en Configuración de Windows, en Aplicaciones, Aplicaciones instaladas.

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
wizard-button-uninstall = Desinstalar
wizard-button-uninstall-mnemonic = D
# The run button before a restore and before a delete of backups, both
# started from the Backups tab. Their access keys must differ from those of
# Back, Next, Close and Cancel, which sit beside it.
wizard-button-restore = Restaurar
wizard-button-restore-mnemonic = R
wizard-button-delete = Eliminar
wizard-button-delete-mnemonic = E
wizard-button-close = Cerrar
wizard-button-close-mnemonic = C
wizard-button-cancel = Cancelar
wizard-button-cancel-mnemonic = C

wizard-target-heading = Selecciona una tarea
wizard-target-language-label = Idioma
wizard-target-language-restart-note = Cambiar de idioma reinicia RABBIT para que el cambio de idioma tenga efecto
wizard-locale-name-en-US = inglés (Estados Unidos)
wizard-locale-name-de-DE = alemán (Alemania)
wizard-locale-name-es-ES = español (España)
wizard-locale-name-fr-FR = francés (Francia)
wizard-locale-name-it-IT = italiano (Italia)
wizard-locale-name-pt-BR = portugués (Brasil)
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

wizard-packages-heading = Elige qué instalar, actualizar o desinstalar
wizard-packages-list-label = Instalar o actualizar: paquetes y ajustes
wizard-packages-row-checked = Marcado
wizard-packages-row-unchecked = No marcado
wizard-packages-choice-shown = Nueva opción debajo de la lista: { $choice }.
wizard-packages-choice-hidden = Opción retirada: { $choice }.
wizard-packages-tree-group-label = Paquetes
wizard-additional-software-tree-group-label = Software adicional
wizard-language-tree-group-label = Paquetes de idioma
wizard-configuration-tree-group-label = Configuración

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Instalar o actualizar
wizard-packages-tab-uninstall = Desinstalar
# The third tab, where RABBIT's backups are restored or deleted. Keep it
# short too.
wizard-packages-tab-backups = Copias de seguridad
# macOS only: VoiceOver says this once, the first time the Packages page opens.
# Name the Tab and Shift keys as Apple's page of Mac keyboard shortcuts does,
# and write Control out in full.
wizard-packages-tabs-switch-hint-macos = Pulsa Control-Tabulador o Control-Mayúsculas-Tabulador para cambiar de pestaña.
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Desinstalar: software que RABBIT puede quitar
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package }: { $action }. Tienes la versión { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = RABBIT todavía no puede desinstalarlo
wizard-uninstall-row-unavailable-not-supported = RABBIT no puede desinstalarlo
wizard-uninstall-row-unavailable-manual = quítalo a mano
wizard-uninstall-row-unavailable-reapack = lo gestiona ReaPack
wizard-uninstall-row-unavailable-reapack-check-failed = no se pudieron comprobar los registros de ReaPack
wizard-uninstall-row-unavailable-no-receipt = RABBIT no instaló esta copia
wizard-uninstall-row-unavailable-no-trusted-uninstaller = no hay un desinstalador de confianza para RABBIT
wizard-uninstall-row-unavailable-unsafe-path = un archivo que RABBIT nunca quita
# macOS: the app has to be moved to the Trash in Finder. Name the Trash and
# Finder as macOS does in your language.
wizard-uninstall-row-unavailable-by-hand-macos = muévelo a la Papelera en el Finder
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = ReaPack gestiona { $package }: instaló { $path } como { $reapack_package }. Desinstálalo desde ReaPack.
wizard-uninstall-details-reapack-check-failed = RABBIT no pudo comprobar los registros de ReaPack para { $path } ({ $reason }), así que no toca { $package }.
wizard-uninstall-details-portable = { $package } sirve a todos los REAPER de este ordenador, así que RABBIT solo lo quita del REAPER estándar, no de uno portable.
wizard-uninstall-details-no-receipt = RABBIT no instaló esta copia de { $package }, así que lo más probable es que sea tuya, y RABBIT no la toca.
wizard-uninstall-details-no-trusted-uninstaller = RABBIT no encontró ningún desinstalador de { $package } en el que pueda confiar.
wizard-uninstall-details-unsafe-path = RABBIT no va a quitar { $path }. Prefiere no tocar { $package } antes que quitarlo solo en parte.
# The details text of an Uninstall row whose package RABBIT leaves alone
# because of the uninstaller folder and registry entry its installer left:
# RABBIT doesn't trust what it found there, or finds none of the package's
# files for this account. The package's steps to remove it by hand follow.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-cleanup-refused = RABBIT no confía en lo que ha encontrado en la carpeta del desinstalador de { $package } dentro de Archivos de programa (Program Files) ni en su entrada del registro, así que no toca { $package }.
wizard-uninstall-details-no-files-for-this-account = RABBIT no encuentra ningún archivo de { $package } en tu cuenta. Si otra cuenta instaló { $package }, desinstálalo desde esa cuenta; hasta entonces, RABBIT no toca la carpeta de su desinstalador dentro de Archivos de programa (Program Files) ni su entrada del registro.
# The details text of a macOS app RABBIT can't move to the Trash, by itself or
# with an administrator's password: first why, then how to move it there in
# Finder. $package is the localized package name, $path the app's full macOS
# path, keep it as it is, and $error macOS's own error message. Name Finder,
# the Trash, File > Get Info and Locked as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does: Finder's
# menus show it only as symbols.
wizard-uninstall-details-by-hand-not-movable-macos = Tu cuenta no puede mover { $path }: pertenece a otra cuenta o a macOS, o está en una carpeta que no puedes cambiar.
wizard-uninstall-details-by-hand-locked-macos = { $path } está bloqueado.
wizard-uninstall-details-by-hand-not-the-app-macos = { $path } no es la app { $package } que RABBIT espera ahí, así que RABBIT no la toca.
wizard-uninstall-details-by-hand-unchecked-macos = RABBIT no ha podido comprobar quién puede mover { $path } ({ $error }).
wizard-uninstall-details-by-hand-no-trash-folder-macos = Solo un administrador puede mover { $path }, y tu Papelera no es una carpeta tuya que RABBIT pueda comprobar.
wizard-uninstall-details-by-hand-other-disk-macos = Solo un administrador puede mover { $path }, y está en un disco distinto del de tu Papelera.
wizard-uninstall-details-by-hand-finder-macos = Para quitar { $package }, selecciónalo en el Finder y pulsa Comando-Suprimir. El Finder pide el nombre y la contraseña de un administrador si los necesita.
wizard-uninstall-details-by-hand-unlock-macos = Para quitar { $package }, selecciónalo en el Finder y elige Archivo > Obtener información. Desmarca Bloqueado y cierra esa ventana; luego pulsa Comando-Suprimir. El Finder pide el nombre y la contraseña de un administrador si los necesita. Si Bloqueado no está marcado, o el Finder sigue sin poder moverlo, tiene un bloqueo que el Finder no puede quitar.
wizard-uninstall-details-by-hand-not-the-app-finder-macos = Si es el { $package } que quieres quitar, selecciónalo en el Finder y pulsa Comando-Suprimir.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = RABBIT mueve los archivos a una copia de seguridad en lugar de borrarlos.
wizard-uninstall-details-dependents-stay = Estos paquetes se instalan sobre { $package } y se quedan: { $packages }. Vuelven a funcionar en cuanto instales { $package } de nuevo.
wizard-uninstall-details-machine-wide = { $package } sirve a todos los REAPER y a todos los demás programas de este ordenador, así que todos lo pierden.
wizard-uninstall-details-reapack-owned-extensions = ReaPack instaló estos, y se quedan sin actualizaciones cuando ReaPack ya no esté: { $packages }.
wizard-uninstall-details-reapack-records-unreadable = RABBIT no pudo leer los registros de ReaPack para saber qué sigue gestionando ({ $reason }).
wizard-uninstall-details-no-receipt-note = RABBIT no instaló esta copia. Quita los archivos que se sabe que instala { $package }.
wizard-uninstall-details-reapack-orphan = ReaPack instaló { $package } en su día. ReaPack ya no está, así que RABBIT puede quitarlo; los registros de ReaPack sobre él se quedan.
# A note on a package whose installer added files outside REAPER's folder,
# when RABBIT has no record of which ones. $package is the localized
# package name.
wizard-uninstall-details-no-vendor-record = RABBIT no tiene constancia de qué archivos añadió el instalador de { $package } fuera de la carpeta de REAPER, así que esos se quedan todos.
wizard-uninstall-details-vendor = Windows te pide permiso antes de quitar nada.
wizard-uninstall-details-vendor-by-hand = Puedes desinstalar { $package } tú mismo en Configuración de Windows, en Aplicaciones, Aplicaciones instaladas.
# The details text of an Uninstall row whose package's installer left an
# uninstaller folder in Program Files and a registry entry, which RABBIT
# removes with administrator rights once the package's files are in the
# backup. It follows wizard-uninstall-details-files, or takes its place
# when none of the package's files are left to move. $package is the
# localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-machine-cleanup = Solo un administrador puede quitar la carpeta del desinstalador de { $package } dentro de Archivos de programa (Program Files) y su entrada del registro, así que Windows te pide permiso antes de quitar nada. Una copia de seguridad no puede recuperarlas.
# The details text of an Uninstall row on macOS, by how the package goes:
# deleted for good with an administrator's password; to the Trash, moved as
# you; or into your Trash with an administrator's password, because your
# account may not move it. Name the Trash and Put Back as Finder does in your
# language. The password window may offer Touch ID, so don't say a password
# is the only way. The Trash line follows the package's own sentence, which
# already says that it goes to the Trash, so it only says how to get it back.
# The administrator line follows the package's own sentence, which already
# says a backup can't bring it back, so it doesn't say that again.
wizard-uninstall-details-elevated-macos = macOS pide una vez el nombre y la contraseña de un administrador antes de quitar nada. Para tenerlo de nuevo, tendrás que volver a instalarlo.
wizard-uninstall-details-trash = Hasta que vacíes la Papelera, puedes arrastrarlo de vuelta desde allí.
wizard-uninstall-details-trash-elevated-macos = Tu cuenta no puede moverlo, así que macOS pide una vez el nombre y la contraseña de un administrador antes de quitar nada, y RABBIT lo mueve luego a tu Papelera. Conserva el propietario que tenía, así que el Finder no ofrece «Sacar de la papelera» para él, y vaciar la Papelera puede pedir la contraseña de un administrador.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = RABBIT no encontró nada que pueda desinstalar en este REAPER.
wizard-uninstall-empty-details = No hay instalado en este REAPER nada del software que RABBIT sabe desinstalar.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = RABBIT no pudo leer sus registros de este REAPER ({ $error }), así que no puede mostrar qué podría desinstalar.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Configuración
wizard-uninstall-remove-settings = Quitar también la configuración
wizard-uninstall-settings-note-none-ticked = Marca un paquete para ver qué parte de su configuración puede quitar RABBIT. Si no marcas esta opción, la configuración se conserva.
wizard-uninstall-settings-note-none-removable = Ninguno de los paquetes marcados tiene configuración que RABBIT pueda quitar.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = Con esta opción marcada, RABBIT también quita esta configuración. Va a la copia de seguridad con todo lo demás, así que la copia puede recuperarla.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package }: { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Ahora disponible debajo de la lista: { $option }.
wizard-uninstall-settings-unavailable = Ya no está disponible: { $option }.

# The Backups tab. The label right before its list names the tab first, as
# the other tabs' labels do; screen readers read it when focus enters the
# list. $size is a size, one of the size-* messages, such as "742 MB". The
# -empty form is for a list with no backups.
wizard-backups-list-label = Copias de seguridad: lo que RABBIT puede restaurar o eliminar, { $size } en total
wizard-backups-list-label-empty = Copias de seguridad: lo que RABBIT puede restaurar o eliminar
# The one row, without a checkbox, of a Backups list with nothing in it, and
# its details text. On macOS, VoiceOver says the row's sentence after the
# tab's name instead.
wizard-backups-empty = RABBIT no tiene copias de seguridad de este REAPER.
wizard-backups-empty-details = RABBIT hace una copia de seguridad cuando desinstala un paquete, cuando una actualización reemplaza los archivos de un paquete y cuando el mapa de teclado de OSARA reemplaza el tuyo. Nada de eso ha pasado todavía en este REAPER, o se eliminaron sus copias de seguridad. Lo que cambió el propio instalador o desinstalador de un paquete, la Papelera o la contraseña de un administrador nunca tiene copia de seguridad.
# Stands in for the Backups list when RABBIT can't read its backups folder.
# $error is the error message, in English.
wizard-backups-unreadable = RABBIT no pudo leer las copias de seguridad de este REAPER.
wizard-backups-unreadable-details = RABBIT no pudo leer su carpeta de copias de seguridad ({ $error }), así que no puede mostrar lo que podría restaurar o eliminar.
# The group headings of the Backups list, rows without a checkbox, then the
# details text each shows. The details say what backups are for, and what
# they never hold.
wizard-backups-group-removed = Software que desinstalaste
wizard-backups-group-removed-details = Cuando RABBIT desinstala un paquete, mueve los archivos aquí en lugar de borrarlos. Cada una de estas copias es la única. Lo que quitó el propio desinstalador de un paquete, la Papelera o la contraseña de un administrador no está aquí.
wizard-backups-group-versions = Versiones anteriores
wizard-backups-group-versions-details = Cuando una actualización reemplaza los archivos de un paquete, RABBIT guarda aquí los antiguos. Las actualizaciones hechas por el propio instalador de un paquete, como el de REAPER, no guardan ninguno.
wizard-backups-group-keymaps = Mapas de teclado
wizard-backups-group-keymaps-details = Cuando el mapa de teclado de OSARA reemplaza el tuyo, RABBIT guarda aquí el antiguo. Normalmente solo el más antiguo es el tuyo.
wizard-backups-group-other = Otras copias de seguridad
wizard-backups-group-other-details = RABBIT no puede restaurarlas aquí. Puedes eliminarlas.
# One row of the Backups list, read as a list row. $backup is what the row
# calls the backup (one of the wizard-backups-name-* messages, or a package's
# name and version), $action is action-restore, action-delete,
# action-delete-only-copy or action-keep, $when one of the
# wizard-backups-when-* messages or backup-date-unknown, and $size a size,
# such as "4 MB".
wizard-backups-row = { $backup }: { $action }. { $when }, { $size }
# The row that stands for every old copy of RABBIT's records, read as a list
# row. $action as above, $count a number, the copies, and $size a size.
wizard-backups-row-records = Copias antiguas de los registros de RABBIT: { $action }. { $count ->
    [one] Una copia
   *[other] { $count } copias
}, { $size }
# A backup named in a sentence on Review, so that two backups with the same
# name can be told apart. $backup and $when as in wizard-backups-row.
wizard-backups-row-reference = { $backup } ({ $when })
# Names joined in a sentence: $rest is the names but the last, separated by
# commas, and $last the last one, as in "FFmpeg, ReaPack and SWS".
wizard-backups-list-and = { $rest } y { $last }
# What a row calls a package's backup: its localized name, $package, and the
# version in the backup, $version, a version number.
wizard-backups-name-package = { $package } { $version }
# What a row calls a key map: your own; one whose kind RABBIT didn't record,
# as in backups from earlier versions of RABBIT and those a restore made;
# REAPER's own default keys; an earlier OSARA key map you hadn't changed.
wizard-backups-name-keymap-own = Tu mapa de teclado
wizard-backups-name-keymap = Mapa de teclado
wizard-backups-name-keymap-default = Las teclas predeterminadas de REAPER
wizard-backups-name-keymap-osara = Un mapa de teclado anterior de OSARA
# What a row calls the other backups. "RABBIT's records" is RABBIT's list of
# what it installed.
wizard-backups-name-records = Copias antiguas de los registros de RABBIT
wizard-backups-name-records-copy = Una copia de los registros de RABBIT
wizard-backups-name-not-a-backup = No es una copia de seguridad de RABBIT
wizard-backups-name-deleting = Eliminación sin terminar
wizard-backups-name-no-manifest = Copia de seguridad sin registro de su contenido
wizard-backups-name-damaged = Copia de seguridad dañada
wizard-backups-name-before-restore = Archivos que reemplazó una restauración
wizard-backups-name-update-files = Otros archivos que reemplazó una actualización
wizard-backups-name-uninstall-files = Otros archivos que movió una desinstalación
wizard-backups-name-keymap-files = Otros archivos guardados con un mapa de teclado
wizard-backups-name-unknown = Copia de seguridad de otra versión de RABBIT
# When a backup was made, as its row says it after the action. $date is a
# date and time (backup-date-time), and $version a version number.
# "Replaced by OSARA's" means by OSARA's key map.
wizard-backups-when-uninstalled = Desinstalado el { $date }
wizard-backups-when-replaced-by = Reemplazado por { $version } el { $date }
wizard-backups-when-replaced = Reemplazado el { $date }
wizard-backups-when-keymap-osara = Reemplazado por el de OSARA el { $date }
wizard-backups-when-keymap-restore = Reemplazado por una restauración el { $date }
wizard-backups-when-made = Creada el { $date }
# A date and time, as rows, details and Review say when a backup was made,
# such as "November 9, 2026, 2:05 PM". $year, $month (1 to 12), $day,
# $hour (0 to 23) and $hour12 (1 to 12) are numbers; $minute is two digits,
# such as "05"; $period is am or pm. Use the 24-hour clock with $hour and
# $minute, or the 12-hour one with $hour12, $minute and $period, as your
# language does. Name the month by the select on $month.
backup-date-time = { $day } de { $month ->
    [1] enero
    [2] febrero
    [3] marzo
    [4] abril
    [5] mayo
    [6] junio
    [7] julio
    [8] agosto
    [9] septiembre
    [10] octubre
    [11] noviembre
   *[12] diciembre
} de { $year }, { $hour }:{ $minute }
# Instead of a row's when-text, for a backup whose date RABBIT can't tell.
backup-date-unknown = Fecha desconocida
# Sizes, in steps of 1024. $count, $whole and $tenth are numbers. A size in
# GB is $whole and one decimal, $tenth: put your language's decimal mark
# between them.
size-under-kb = menos de 1 KB
size-kb = { $count } KB
size-mb = { $count } MB
size-gb = { $whole },{ $tenth } GB
# Why a Backups row can't be ticked for the choice below the list: the short
# phrase that goes into wizard-package-row-unavailable-suffix at the end of
# the row. $package is the localized package name.
wizard-backups-unavailable-not-here = no se puede restaurar aquí
wizard-backups-unavailable-installed-again = vuelves a tener { $package }
wizard-backups-unavailable-not-complete = falta una parte en la copia de seguridad
wizard-backups-unavailable-nothing-in-it = ninguno de sus archivos está en esta copia de seguridad
wizard-backups-unavailable-version-unknown = RABBIT no sabe qué versión era
wizard-backups-unavailable-other-language-pack = hay otro paquete de idioma instalado
wizard-backups-unavailable-part-missing = falta una parte
wizard-backups-unavailable-records-unreadable = RABBIT no puede leer sus registros
wizard-backups-unavailable-not-yours = pertenece a otra cuenta
wizard-backups-unavailable-not-a-backup = RABBIT no lo toca
wizard-backups-unavailable-receipt-in-set = contiene una copia de los registros de RABBIT
wizard-backups-unavailable-file-refused = RABBIT no devuelve uno de sus archivos
wizard-backups-unavailable-cannot-restore = RABBIT no puede restaurarla
wizard-backups-unavailable-tied = va con una copia de seguridad que RABBIT no puede eliminar
# The details text of a Backups row, a sentence or two per line, in this
# order: what the backup is; when it was made; what it holds and how big;
# whether it is the only copy; what restoring does, or why it can't; what
# deleting loses. $backup is what the row calls the backup, $package the
# localized package name, $version a version number, $date a date and time
# (backup-date-time), $size a size, such as "4 MB", and $count a number.
#
# What the backup is.
wizard-backups-details-uninstalled = { $backup }, tal como estaba antes de desinstalarlo.
wizard-backups-details-earlier = { $backup }, una versión anterior.
wizard-backups-details-keymap-own = Tu mapa de teclado, de antes de que el mapa de teclado de OSARA lo reemplazara.
wizard-backups-details-keymap-default = Las teclas predeterminadas de REAPER, de antes de que el mapa de teclado de OSARA las reemplazara.
wizard-backups-details-keymap-osara = Un mapa de teclado anterior de OSARA, de antes de que uno más nuevo lo reemplazara.
wizard-backups-details-keymap-unknown = Un mapa de teclado de antes de que el de OSARA lo reemplazara. Puede ser el tuyo o un mapa de teclado de OSARA más antiguo: RABBIT no anotó cuál.
wizard-backups-details-keymap-restore = El mapa de teclado que reemplazó una restauración. Puede ser el tuyo o el de OSARA.
wizard-backups-details-keymap-other = Un mapa de teclado que RABBIT guardó.
wizard-backups-details-records = Copias de la lista de lo que RABBIT instaló, de operaciones anteriores. Eliminarlas no cambia nada de lo que está instalado.
wizard-backups-details-records-copy = Una copia de la lista de lo que RABBIT instaló, de una operación anterior.
wizard-backups-details-deleting = La eliminación de esta copia de seguridad empezó y no terminó, así que puede que ya falte una parte.
wizard-backups-details-not-a-backup = Esto no es una copia de seguridad de RABBIT: es un enlace, o una carpeta con un nombre que RABBIT nunca da a una copia de seguridad.
wizard-backups-details-no-manifest = Una copia de seguridad sin registro de su contenido, como la que dejaba un RABBIT anterior al detenerse antes de tiempo.
wizard-backups-details-damaged = Una copia de seguridad cuyo registro de contenido no se puede leer.
wizard-backups-details-before-restore = Archivos que reemplazó una restauración, guardados para poder deshacerla.
wizard-backups-details-update-files = Archivos que reemplazó una actualización y que ninguna otra fila de aquí devuelve.
wizard-backups-details-uninstall-files = Archivos que movió una desinstalación y que no pertenecen a ningún paquete que registrara.
wizard-backups-details-keymap-files = Archivos guardados con un mapa de teclado que ninguna otra fila de aquí devuelve.
wizard-backups-details-unknown = Una copia de seguridad hecha por otra versión de RABBIT, por un motivo que esta versión no conoce.
# When it was made.
wizard-backups-details-when-uninstalled = Lo desinstalaste el { $date }.
wizard-backups-details-when-updated-to = Una actualización a { $version } la reemplazó el { $date }.
wizard-backups-details-when-updated = Una actualización la reemplazó el { $date }.
wizard-backups-details-when-restored-to = La vuelta a { $version } la reemplazó el { $date }.
wizard-backups-details-when-restored = Una restauración la reemplazó el { $date }.
wizard-backups-details-when-osara = El mapa de teclado de OSARA lo reemplazó el { $date }.
wizard-backups-details-when-keymap-restored = Un mapa de teclado restaurado lo reemplazó el { $date }.
wizard-backups-details-when-made = RABBIT hizo esta copia de seguridad el { $date }.
wizard-backups-details-when-records = La copia más reciente es del { $date }.
wizard-backups-details-when-unknown = RABBIT no puede saber cuándo se hizo esta copia de seguridad.
# What it holds and how big it is. The settings are named only when the
# uninstall recorded that it moved them; backups from earlier versions of
# RABBIT don't say, so the -older form says "anything else".
wizard-backups-details-holds-with-settings = Contiene sus archivos y tu configuración de { $package }, { $size }.
wizard-backups-details-holds-files = Contiene sus archivos, { $size }.
wizard-backups-details-holds-older = Contiene sus archivos y cualquier otra cosa que movió esa desinstalación, { $size }.
wizard-backups-details-holds-replaced = Contiene los archivos que se reemplazaron, { $size }.
wizard-backups-details-holds-keymap = Contiene el mapa de teclado, { $size }.
wizard-backups-details-holds-size = Ocupa { $size }.
wizard-backups-details-holds-records = { $count ->
    [one] Hay una copia, { $size }.
   *[other] Hay { $count } copias, { $size } en total.
}
wizard-backups-details-only-copy = Es la única copia.
# What restoring does, for a backup that can be restored. Name REAPER's
# Actions list as REAPER does in your language. The -uninstalled-files form
# is for a backup that names no version, which may hold no record of the
# package to put back, so it says nothing about RABBIT's records.
wizard-backups-details-restore-uninstalled = Al restaurarla, los archivos vuelven a su sitio y RABBIT vuelve a considerar { $package } instalado.
wizard-backups-details-restore-uninstalled-files = Al restaurarla, los archivos vuelven a su sitio.
wizard-backups-details-restore-stopped-partway = La desinstalación se detuvo a medias, así que algunos de sus archivos solo están en esta copia de seguridad. Al restaurarla, vuelven a su sitio.
wizard-backups-details-restore-already-back = { $package } ya está de vuelta. Al restaurarla, sus archivos vuelven a su sitio otra vez.
wizard-backups-details-restore-earlier = Al restaurarla, vuelves a { $version } y RABBIT guarda la versión que tienes ahora como copia de seguridad, para que puedas volver a ella. Tu configuración se queda como está.
wizard-backups-details-restore-earlier-copy = Al restaurarla, vuelve esta copia y RABBIT guarda la versión que tienes ahora como copia de seguridad, para que puedas volver a ella.
wizard-backups-details-restore-earlier-already-back = { $package } ya tiene esta versión. Al restaurarla, sus archivos vuelven a su sitio otra vez.
wizard-backups-details-restore-keymap = Al restaurarlo, vuelve a ser el mapa de teclado de REAPER y RABBIT guarda el actual como copia de seguridad. Los scripts que instalaste desde entonces siguen en la lista de acciones de REAPER.
wizard-backups-details-restore-default-keys = Al restaurarlas, vuelven las teclas propias de REAPER, no un mapa de teclado tuyo, y RABBIT guarda el mapa de teclado actual como copia de seguridad.
# Why it can't be restored, by what RABBIT found. Name the Uninstall tab as
# wizard-packages-tab-uninstall does.
wizard-backups-details-not-here = RABBIT no puede restaurarla aquí.
wizard-backups-details-cannot-restore = RABBIT no puede restaurarla.
wizard-backups-details-status-installed-again = Vuelves a tener { $package }. Desinstala primero el que tienes, en la pestaña «Desinstalar», y luego restaura.
wizard-backups-details-status-not-complete = Falta una parte en la copia de seguridad, así que RABBIT no puede volver a esa versión.
wizard-backups-details-status-nothing-in-it = Ninguno de sus archivos está en esta copia de seguridad, así que no hay nada que restaurar.
wizard-backups-details-status-version-unknown = RABBIT no sabe qué versión era, así que no puede volver a ella.
wizard-backups-details-status-other-language-pack = Hay otro paquete de idioma instalado, e instalar uno quita el otro, así que RABBIT no devuelve este mientras el otro esté instalado.
wizard-backups-details-status-part-missing = Falta una parte en la copia de seguridad, así que RABBIT no puede restaurarla.
wizard-backups-details-status-records-unreadable = RABBIT no puede leer sus registros, así que no puede saber si esta copia de seguridad encaja con ellos.
wizard-backups-details-status-not-yours = Pertenece a otra cuenta, así que RABBIT ni la restaura ni la elimina.
wizard-backups-details-status-not-a-backup = RABBIT nunca mira dentro, y ni lo restaura ni lo elimina.
# macOS: how to remove another account's backup by hand. The backups are in
# the hidden Library folder, so the steps go through Finder's Go menu.
# $folder is the folder that holds the backup, a full macOS path, and $name
# the backup's own folder name; keep both as they are. Name Finder, its Go
# menu and the menu's Go to Folder item as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does. Never
# suggest sudo.
wizard-backups-details-not-yours-finder-macos = Para quitarla, en el Finder elige Ir > Ir a la carpeta, escribe { $folder }, luego selecciona { $name } y pulsa Comando-Suprimir. El Finder pide el nombre y la contraseña de un administrador si los necesita.
# What deleting it loses.
wizard-backups-details-delete-uninstalled = Si eliminas esta copia de seguridad, RABBIT no puede recuperar esta copia.
wizard-backups-details-delete-earlier = Si eliminas esta copia de seguridad, no puedes volver a { $version }.
wizard-backups-details-delete-earlier-copy = Si eliminas esta copia de seguridad, no puedes volver a esta copia.
wizard-backups-details-delete-keymap = Si eliminas esta copia de seguridad, RABBIT no puede recuperar este mapa de teclado.
wizard-backups-details-delete-already-back = Sus archivos también están en el disco, así que al eliminarla no se pierde nada.
wizard-backups-details-delete-other = Si la eliminas, desaparece para siempre.
wizard-backups-details-delete-deleting = Al eliminarla, se termina esa eliminación.
# For rows that go together until a later version of RABBIT: one backup
# holds them all, and RABBIT deletes whole backups. $backups is a list of
# what the other rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-details-together = { $origin ->
    [uninstall] La copia de seguridad de esta desinstalación también contiene { $backups }.
    [update] La copia de seguridad de esta actualización también contiene { $backups }.
   *[other] La misma copia de seguridad también contiene { $backups }.
} Hasta una versión posterior, RABBIT las elimina juntas.
# After the sentence above, when one of those rows can't be deleted.
wizard-backups-details-tied = Una de ellas no se puede eliminar, así que RABBIT tampoco puede eliminar esta.
# The choice below the Backups list: its label, which screen readers read
# with the choice, then the two choices, Restore and Delete. Restore is
# where the tab starts. The choices have no access keys.
wizard-backups-choice-label = Qué hacer con las copias de seguridad marcadas
wizard-backups-choice-restore = Restaurar
wizard-backups-choice-delete = Eliminar
# The choice below the Backups list, Restore or Delete, keeps its own ticks.
# After a switch, VoiceOver on macOS says how many backups are ticked for the
# new choice. $choice is restore or delete, and $count a number.
wizard-backups-ticked-macos = { $choice ->
    [delete] { $count ->
        [0] Ninguna copia de seguridad marcada para eliminar.
        [one] Una copia de seguridad marcada para eliminar.
       *[other] { $count } copias de seguridad marcadas para eliminar.
    }
   *[restore] { $count ->
        [0] Ninguna copia de seguridad marcada para restaurar.
        [one] Una copia de seguridad marcada para restaurar.
       *[other] { $count } copias de seguridad marcadas para restaurar.
    }
}
# Said by VoiceOver on macOS after "Checked" or "Unchecked", when ticking a
# row to delete ticks or unticks the rows from the same backup too, such as
# "SWS 2.14.0.7 from the same uninstall is ticked too." $backups is a list
# of what those rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-also-ticked-macos = También marcado, de la misma { $origin ->
    [uninstall] desinstalación
    [update] actualización
   *[other] copia de seguridad
}: { $backups }.
wizard-backups-also-unticked-macos = También desmarcado, de la misma { $origin ->
    [uninstall] desinstalación
    [update] actualización
   *[other] copia de seguridad
}: { $backups }.
# Why a restore of a backup is refused, a sentence or two each, about the
# backup, which they call "it". Review puts one after "Can't restore <backup>
# yet.", and a row's details after what the backup is. $package is a
# localized package name, $packages a list of them, $path a file or folder
# as the computer writes it, keep it as it is, $detail an error message in
# English, $reason one of the wizard-restore-target-* phrases, and $programs
# a comma-separated list of program names, never translated. Name the
# Uninstall tab as wizard-packages-tab-uninstall does.
wizard-restore-refusal-invalid-id = Su nombre no es uno que RABBIT dé a una copia de seguridad.
wizard-restore-refusal-not-found = Se eliminó o nunca existió. Vuelve a abrir RABBIT para ver las copias de seguridad tal como están ahora.
wizard-restore-refusal-differs-in-case = RABBIT solo la encuentra con un nombre con otras mayúsculas y minúsculas.
wizard-restore-refusal-ends-in-dot-or-space = RABBIT solo la encuentra con un nombre sin el punto o el espacio del final.
wizard-restore-refusal-being-deleted = Su eliminación empezó y no terminó, así que puede que falte una parte.
wizard-restore-refusal-is-link = RABBIT no usa { $path }: es un enlace, y RABBIT nunca sigue enlaces en sus copias de seguridad.
wizard-restore-refusal-no-manifest = No registra cuáles de sus archivos pertenecen a { $package }.
wizard-restore-refusal-no-package-records = No registra ningún paquete, así que RABBIT no puede restaurar solo { $package } desde ella.
wizard-restore-refusal-package-not-in-set = No contiene ningún registro de { $package }.
wizard-restore-refusal-no-keymap-in-set = No contiene ningún mapa de teclado que RABBIT pueda restaurar.
wizard-restore-refusal-damaged-manifest = RABBIT no puede leer su registro de contenido ({ $detail }).
wizard-restore-refusal-records-only = Solo contiene una copia de los registros de RABBIT, así que no hay nada que restaurar.
wizard-restore-refusal-receipt-in-set = Contiene una copia de los registros de RABBIT. Copiarla de vuelta desharía todos los cambios hechos en ellos desde entonces, así que RABBIT no la restaura.
wizard-restore-refusal-reinstalled-since = Vuelves a tener { $package }, una copia instalada después de hacer esta copia de seguridad. Desinstala primero la que tienes, en la pestaña «Desinstalar», y luego restaura.
wizard-restore-refusal-nothing-of-it-in-set = Ninguno de los archivos de { $packages } está en ella, así que no hay nada que restaurar.
# Instead of the sentence above, when nothing in a backup of earlier
# versions belongs to a package.
wizard-restore-refusal-nothing-of-it-in-set-none = Nada de lo que contiene pertenece a un paquete que RABBIT pueda devolver.
wizard-restore-refusal-part-missing = Falta una parte: su copia de { $path } ha desaparecido o no es el archivo que registró.
wizard-restore-refusal-not-complete = No está completa: { $path } no está ni en la copia de seguridad ni en el disco como lo tenía esa versión.
wizard-restore-refusal-version-unknown = RABBIT no sabe qué versión de { $package } contiene.
wizard-restore-refusal-other-language-pack-installed = RABBIT no devuelve { $package } mientras esto esté instalado: { $packages }. Instalar uno quita el otro.
wizard-restore-refusal-target-refused = RABBIT no devuelve { $path }: { $reason }.
wizard-restore-refusal-newer-file-refused = Volver atrás movería { $path }, un archivo de la versión más nueva de { $package }, a una copia de seguridad de la que no podría volver: { $reason }.
wizard-restore-refusal-earlier-file-refused = Su registro de esa versión de { $package } menciona { $path }, un lugar donde RABBIT nunca instala: { $reason }.
wizard-restore-refusal-receipt-unreadable = RABBIT no puede leer sus registros ({ $detail }), así que no puede saber si esta copia de seguridad encaja con ellos.
wizard-restore-refusal-reaper-running = REAPER está abierto. Ciérralo y vuelve a intentarlo.
# The same, for packages every REAPER on the computer can use, such as
# app2clap. $packages is a list of localized package names.
wizard-restore-refusal-reaper-running-every = Hay un REAPER abierto. Cierra todas las copias de REAPER de este equipo, también las portátiles, y vuelve a intentarlo. Cualquier REAPER puede usar { $packages }.
wizard-restore-refusal-keymap-needs-reaper-closed = Devuelve el mapa de teclado de REAPER, y RABBIT solo lo hace con REAPER cerrado.
wizard-restore-refusal-program-running = Cierra estos programas y vuelve a intentarlo: { $programs }.
wizard-restore-refusal-not-writable = RABBIT no puede escribir donde la restauración devuelve los archivos, en { $path } ({ $detail }).
wizard-restore-refusal-lock-held = Otra copia de RABBIT está trabajando en este REAPER. Espera a que termine.
# sudo and root are never translated.
wizard-restore-refusal-as-root = RABBIT se está ejecutando como root, como ocurre con sudo, y así no restaura copias de seguridad. Sal de RABBIT y ábrelo con tu propia cuenta.
wizard-restore-refusal-not-yours = Pertenece a otra cuenta, así que RABBIT no la restaura.
wizard-restore-refusal-changed-while-read = Cambió mientras RABBIT la leía, como cuando otra copia de RABBIT todavía la estaba escribiendo.
wizard-restore-refusal-changed-since-listed = Cambió desde que RABBIT la mostró. Vuelve a abrir RABBIT para ver las copias de seguridad tal como están ahora.
# Why a restore won't put a file back at a place: the phrase that ends
# wizard-restore-refusal-target-refused and the two after it. Start in lower
# case where your language allows. $path is a file or folder, keep it as it
# is; $name a file or folder name; $detail an error message in English.
wizard-restore-target-rabbit-folder = RABBIT nunca devuelve un archivo a su propia carpeta
wizard-restore-target-leads-into-rabbit-folder = el camino hasta allí lleva a la propia carpeta de RABBIT
wizard-restore-target-name-not-as-written = el nombre { $name } en el camino puede abrir otro archivo u otra carpeta distintos de los que nombra
wizard-restore-target-place-is-link = ahí hay un enlace, y RABBIT nunca reemplaza enlaces
wizard-restore-target-through-link = el camino pasa por { $path }, un enlace, y RABBIT nunca sigue enlaces
wizard-restore-target-leads-outside-resource-folder = el camino sale de la carpeta de recursos, { $path }
wizard-restore-target-leads-outside = el camino sale de { $path }
wizard-restore-target-outside-allowed-folders = está fuera de la carpeta de recursos de REAPER y de cualquier otra carpeta donde instala RABBIT
wizard-restore-target-not-plain-path = no es una ruta sencilla dentro de la carpeta de recursos
wizard-restore-target-guard-folder-gone = la carpeta a la que vuelve, { $path }, ya no existe
wizard-restore-target-could-not-check = RABBIT no pudo comprobar { $path } ({ $detail })
wizard-restore-target-two-files-for-one-place = la copia de seguridad contiene más de un archivo para este lugar
wizard-restore-target-outside-without-manifest = la copia de seguridad no registra a dónde pertenece este archivo de fuera de la carpeta de recursos
wizard-restore-target-copy-outside-set = el registro de la copia de seguridad sitúa su copia fuera de la copia de seguridad
wizard-restore-target-keymap-among-version-files = es el mapa de teclado de REAPER, que RABBIT solo devuelve como mapa de teclado por sí mismo

# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = No está disponible: Se requiere instalar { $package }.
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
# Nombra la dependencia de un paso de configuracion que satisface cualquiera
# de varios paquetes.
config-dependency-language-pack = un paquete de idioma
config-reapack-reaper-accessibility-name = Agregar el repositorio de scripts de Toni a ReaPack
config-reapack-reaper-accessibility-description = Agregar el repositorio de scripts de Ttoni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Después de agregarlo, abre el menú de extensiones de Reaper, ReaPack, Browse Packages para obtener scripts y plugings adicionales.
config-reapack-reaper-accessible-fr-name = Agregar el repositorio de Reaper Accessible (francés) a ReaPack
config-reapack-reaper-accessible-fr-description = Agrega el repositorio de ReaPack de Reaper Accessible en francés (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Después de agregarlo, abre el menú extensiones, ReaPack, Browse Packages para obtener los recursos de Reaper Accessible en francés.
config-reapack-reaper-accessible-en-name = Agregar el repositorio de Reaper Accessible (inglés) a ReaPack
config-reapack-reaper-accessible-en-description = Agrega el repositorio de Reaper Accessible en inglés a ReaPack (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Después de agregarlo, abre el menú extensiones, ReaPack, Browse Packages para obtener los recursos de Reaper Accessible en inglés.
config-set-reaper-language-name = Configurar el idioma de REAPER
config-set-reaper-language-description = Indica a REAPER cuál de los paquetes de idioma instalados debe usar, escribiéndolo en reaper.ini. Sin esto tendrías que seleccionar el idioma manualmente en Opciones, Preferencias, General. Puedes instalar varios paquetes de idioma y cambiar entre ellos en REAPER más adelante; aquí solo se elige cuál está activo ahora. Desmarca esta casilla si solo quieres que se instalen los archivos.

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
packages-reaper-language-label = Idioma de REAPER tras la instalación
packages-spanish-variant-label = Traducción de OSARA al español
wizard-packages-osara-keymap-unavailable-note = Selecciona Osara para configurar el comportamiento de su mapa de teclado.
wizard-packages-osara-keymap-preserve-note = Tu mapa de teclado actual se queda como está: RABBIT no toca reaper-kb.ini. Para añadir los nuevos atajos de OSARA conservando los tuyos, ejecuta después la acción de OSARA «Merge OSARA key map» (menú Extensiones, OSARA).
wizard-packages-osara-keymap-replace-note = RABBIT hará una copia de seguridad de tu reaper-kb.ini actual, si existe, y lo reemplazará con el último mapa de teclado de OSARA. Los atajos que hayas cambiado tú no se conservan; la copia de seguridad los guarda.
wizard-packages-osara-keymap-unchanged-note = Tu mapa de teclado es el de OSARA sin cambios tuyos, así que RABBIT hará una copia de seguridad y lo reemplazará con el último mapa de teclado de OSARA. Tus scripts instalados se quedan en la lista de acciones.
wizard-packages-osara-keymap-default-keys-note = Usas los atajos de teclado predeterminados de REAPER en lugar del mapa de teclado de OSARA, así que RABBIT los deja como están. Para cambiar al mapa de teclado de OSARA, marca la casilla de arriba.
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
wizard-review-osara-keymap-replace-unchanged = Guardar tu mapa de teclado de OSARA sin cambios y reemplazarlo con el último de OSARA.
wizard-review-osara-keymap-keep-default-keys = Conservar los atajos de teclado predeterminados de REAPER.
wizard-review-notes-heading = Notas
wizard-review-preflight-prefix = Todavía no se puede instalar

# $path is the selected REAPER resource path.
wizard-review-target = Destino: { $path }
wizard-review-no-target = No seleccionaste un destino.
wizard-review-no-package = No hay un paquete seleccionado.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Listo para desinstalar
wizard-review-uninstall-packages = Paquetes que se van a desinstalar
wizard-review-uninstall-no-package = No hay nada marcado para desinstalar.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package }: { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = Su configuración también se quita y va a la copia de seguridad: { $settings }.
wizard-review-uninstall-close-programs-note = Cierra antes los demás programas de música.
wizard-review-uninstall-elevation-note-windows = Windows te pide permiso una sola vez, antes de quitar nada. Si no lo permites, no se desinstala nada.
wizard-review-uninstall-vendor-window-note = Si aparece una ventana del propio desinstalador de { $package }, respóndela.
# Review's permission note on macOS, when a ticked package needs an
# administrator. $packages is a list of package names. osascript is a program
# name, never translated: macOS names it in the password window. The window
# may offer Touch ID, so don't say a password is the only way. Use the same
# words for 'administrator command' as in the Done and progress lines that
# name it.
wizard-review-uninstall-elevation-note-macos = Antes de quitar nada, macOS pide una vez el nombre y la contraseña de un administrador. Puede ofrecer Touch ID en su lugar. La ventana puede nombrar osascript, la herramienta con la que pregunta RABBIT. Con esa contraseña, RABBIT ejecuta un comando de administrador que desinstala { $packages }. Si cancelas la ventana, no se desinstala nada.
# One line per ticked package that goes to the Trash; the second when your
# account may not move it, so an administrator's password moves it there.
# $package is the localized package name. Name the Trash and Put Back as
# Finder does in your language.
wizard-review-uninstall-trash-note = { $package } va a la Papelera. Hasta que vacíes la Papelera, puedes arrastrarlo de vuelta desde allí.
wizard-review-uninstall-trash-elevated-note-macos = { $package } va a tu Papelera con la contraseña del administrador. Conserva el propietario que tenía, así que el Finder no ofrece «Sacar de la papelera» para él, y vaciar la Papelera puede pedir la contraseña de un administrador.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. RABBIT leaves out what it ticked by itself, such as updates, but the
# list can still hold a setting RABBIT ticked along with one of the user's
# ticks, so the note doesn't say who ticked it. $packages is a comma-separated
# list of names: packages, and for the Install or update tab also settings,
# such as "Set REAPER's language".
wizard-review-inactive-tab-note-install = Lo que está marcado en la pestaña «Instalar o actualizar» no forma parte de esta operación: { $packages }.
wizard-review-inactive-tab-note-uninstall = Lo que está marcado en la pestaña «Desinstalar» no forma parte de esta operación: { $packages }.
# The same note for the Backups tab. $backups is one or both of the two
# messages below, joined as wizard-backups-list-and joins names.
wizard-review-inactive-tab-note-backups = Lo que está marcado en la pestaña «Copias de seguridad» no forma parte de esta operación: { $backups }.
# How many backups are ticked on the Backups tab to restore, and to delete,
# for the note above. $count is a number, never 0.
wizard-review-inactive-tab-backups-restore = { $count ->
    [one] una copia de seguridad para restaurar
   *[other] { $count } copias de seguridad para restaurar
}
wizard-review-inactive-tab-backups-delete = { $count ->
    [one] una copia de seguridad para eliminar
   *[other] { $count } copias de seguridad para eliminar
}
wizard-review-uninstall-reinstall-hint = Puedes volver a instalarlos más adelante desde la pestaña «Instalar o actualizar».
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Todavía no se puede desinstalar: { $reason }
wizard-review-uninstall-not-writable = RABBIT no puede escribir en la carpeta de este REAPER.

# Review before a restore from the Backups tab. The heading names what the
# run does, so it is the first thing read.
wizard-review-restore-heading = Listo para restaurar
# One line per ticked backup. $backup is what its row calls it, $package the
# localized package name, $date a date and time (backup-date-time), and
# $settings yes or no: whether the backup holds the package's settings too.
wizard-review-restore-uninstalled = { $settings ->
    [yes] { $backup } vuelve tal como estaba el { $date }, con tu configuración de { $package }.
   *[no] { $backup } vuelve tal como estaba el { $date }.
}
wizard-review-restore-uninstalled-undated = { $settings ->
    [yes] { $backup } vuelve tal como estaba antes de desinstalarlo, con tu configuración de { $package }.
   *[no] { $backup } vuelve tal como estaba antes de desinstalarlo.
}
# An earlier version. $version is the version it goes back to, and $current
# the one it has now, both version numbers.
wizard-review-restore-earlier = { $package } vuelve de { $current } a { $version }. RABBIT guarda { $current } como copia de seguridad.
wizard-review-restore-earlier-to = { $package } vuelve a { $version }. RABBIT guarda la versión que tienes ahora como copia de seguridad.
wizard-review-restore-earlier-copy = { $package } vuelve a la copia que reemplazó una actualización. RABBIT guarda la versión que tienes ahora como copia de seguridad.
# A key map. $origin is own for your own key map, default-keys for REAPER's
# default keys, and other for any other; $date as above.
wizard-review-restore-keymap = { $origin ->
    [own] Tu mapa de teclado del { $date } reemplaza al actual, que RABBIT guarda como copia de seguridad.
    [default-keys] Las teclas predeterminadas de REAPER del { $date } reemplazan al mapa de teclado actual, que RABBIT guarda como copia de seguridad.
   *[other] El mapa de teclado del { $date } reemplaza al actual, que RABBIT guarda como copia de seguridad.
}
wizard-review-restore-keymap-undated = { $origin ->
    [own] Tu mapa de teclado reemplaza al actual, que RABBIT guarda como copia de seguridad.
    [default-keys] Las teclas predeterminadas de REAPER reemplazan al mapa de teclado actual, que RABBIT guarda como copia de seguridad.
   *[other] Este mapa de teclado reemplaza al actual, que RABBIT guarda como copia de seguridad.
}
# Notes on what the restore changes besides the files. $file is a language
# pack's file name, such as de_DE.ReaperLangPack, never translated; $package
# is the localized package name.
wizard-review-restore-language-back = El idioma de REAPER vuelve a ser { $file }.
wizard-review-restore-language-stays = El idioma de REAPER se queda como está, porque desde entonces se eligió otro.
wizard-review-restore-not-counted = RABBIT ya no considera { $package } instalado por él.
wizard-review-restore-needs-reinstall = Para desinstalar { $package } con RABBIT más adelante, vuelve a instalarlo primero.
# When a REAPER that has to be closed is running; the second when a ticked
# package serves every REAPER on the computer.
wizard-review-restore-close-reaper = Cierra REAPER primero.
wizard-review-restore-close-every-reaper = Cierra primero todas las copias de REAPER.
# The backups ticked for the other choice below the list stay ticked, but
# aren't part of this run. $count is a number.
wizard-review-restore-other-choice-note = { $count ->
    [one] Una copia de seguridad marcada para eliminar no forma parte de esta operación.
   *[other] { $count } copias de seguridad marcadas para eliminar no forman parte de esta operación.
}
# Why the Restore button is disabled. $reason is a sentence, such as
# wizard-review-restore-nothing-ticked. The -backup form names a backup that
# can't come back: $backup as in wizard-backups-row-reference, and $reason
# one of the wizard-restore-refusal-* sentences.
wizard-review-restore-blocked = Todavía no se puede restaurar: { $reason }
wizard-review-restore-blocked-backup = Todavía no se puede restaurar { $backup }. { $reason }
wizard-review-restore-nothing-ticked = No hay nada marcado para restaurar.
wizard-review-restore-not-writable = RABBIT no puede escribir en la carpeta de este REAPER.
# $first and $second name two ticked backups, as in
# wizard-backups-row-reference; $package is the localized package name.
wizard-review-restore-same-package = { $first } y { $second } son copias de seguridad de { $package }. Marca solo una de ellas.
wizard-review-restore-same-file = { $first } y { $second } devuelven el mismo archivo. Marca solo una de ellas.
# When RABBIT couldn't check the restore before it starts: one backup, or
# all of them. $error is the error message, in English.
wizard-review-restore-plan-failed = RABBIT no pudo comprobarla ({ $error }).
wizard-review-restore-check-failed = RABBIT no pudo comprobar la restauración ({ $error }).
# When RABBIT hasn't checked the backups ticked now, so the restore can't
# start yet.
wizard-review-restore-not-checked = RABBIT no ha comprobado lo que está marcado.
# Review before a delete from the Backups tab.
wizard-review-delete-heading = Listo para eliminar copias de seguridad
# $count is a number, the ticked backups, and $size a size, such as "520 MB".
wizard-review-delete-intro = { $count ->
    [one] Esta copia de seguridad se elimina para siempre y libera { $size }:
   *[other] Estas copias de seguridad se eliminan para siempre y liberan { $size }:
}
# One line per ticked backup, saying what is lost with it. $backup is what
# its row calls it, and $date a date and time (backup-date-time).
wizard-review-delete-uninstalled = { $backup }, desinstalado el { $date }: la única copia.
wizard-review-delete-keymap = { $backup } del { $date }: la única copia.
wizard-review-delete-only-copy = { $backup }: la única copia.
wizard-review-delete-earlier = { $backup }: después ya no podrás volver a esa versión.
wizard-review-delete-already-back = { $backup }: sus archivos también están en el disco, así que no se pierde nada.
wizard-review-delete-records = { $backup }: no cambia nada de lo que está instalado.
wizard-review-delete-deleting = { $backup }: termina una eliminación que empezó antes.
wizard-review-delete-other = { $backup }: desaparece para siempre.
# Until a later version, backups from one set go together. $backups is a
# list of what their rows call them, $count how many there are, a number, 2
# or more, $origin uninstall, update or other, for what made the backup, and
# $date a date and time (backup-date-time). The -undated form is for a
# backup whose date RABBIT can't tell.
wizard-review-delete-together = { $origin ->
    [uninstall] { $backups }, de la desinstalación del { $date }, van juntas
    [update] { $backups }, de la actualización del { $date }, van juntas
   *[other] { $backups }, del { $date }, van juntas
}: { $count ->
    [2] una sola copia de seguridad contiene ambas.
   *[other] una sola copia de seguridad contiene las { $count }.
}
wizard-review-delete-together-undated = { $count ->
    [2] { $backups } van juntas: una sola copia de seguridad contiene ambas.
   *[other] { $backups } van juntas: una sola copia de seguridad contiene las { $count }.
}
# $count is a number.
wizard-review-delete-other-choice-note = { $count ->
    [one] Una copia de seguridad marcada para restaurar no forma parte de esta operación.
   *[other] { $count } copias de seguridad marcadas para restaurar no forman parte de esta operación.
}
# Why the Delete button is disabled. $reason is a sentence.
wizard-review-delete-blocked = Todavía no se puede eliminar: { $reason }
wizard-review-delete-nothing-ticked = No hay nada marcado para eliminar.

wizard-progress-heading = Progreso de la instalación
wizard-progress-status-idle = Listo para instalar.
wizard-progress-status-running = Instalando paquetes seleccionados. Esto puede tardar unos minutos.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Deteniendo. RABBIT termina el paso que empezó y luego se cierra.
wizard-progress-details-label = Detalles del progreso
wizard-progress-details-idle = Sin instalación en curso.
wizard-progress-details-starting = Iniciando operaciones de instalación.
wizard-progress-details-cache-prefix = Caché
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops once the step it is on has finished: one package,
# or on macOS the administrator command, which removes several at once.
wizard-progress-uninstall-heading = Desinstalación
wizard-progress-status-uninstall-running = Desinstalando los paquetes marcados.
wizard-progress-details-uninstall-starting = Iniciando la desinstalación.
wizard-progress-status-uninstall-cancelling = Deteniendo. RABBIT termina el paso que empezó y luego se cierra.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Descargando { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = descargando { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = descargando { $count } paquetes… { $downloaded } / { $total }
wizard-progress-status-installing = instalando { $package }…
wizard-progress-status-removing = desinstalando { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Aplicando configuración: { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = Descargando { $package }…
wizard-progress-log-download-completed = se descargó { $package }.
wizard-progress-log-install-started = instalando { $package }…
wizard-progress-log-install-completed = se instaló { $package }.
wizard-progress-log-remove-started = desinstalando { $package }…
wizard-progress-log-remove-completed = se desinstaló { $package }.
wizard-progress-log-remove-failed = no se pudo desinstalar { $package }.
wizard-progress-status-vendor-running = ejecutando el propio desinstalador de { $package }…
wizard-progress-status-vendor-still-waiting = Todavía esperando al propio desinstalador de { $package }. Puede que esté mostrando una pregunta en otra ventana; pulsa Alt+Tab para encontrarla.
wizard-progress-status-vendor-waiting-for-files = El propio desinstalador de { $package } ha terminado y RABBIT está esperando a que se vayan sus archivos. Si otro programa de música tiene { $package } abierto, ciérralo.
# On macOS, said right before the administrator password window appears, and
# again if it has been open a while without an answer. RABBIT puts each on the
# status line and in the log too, in case VoiceOver cuts it off. $packages is
# a list of package names. Use the same words for 'administrator command' as
# in the Done and progress lines that name it.
wizard-progress-status-admin-prompt-macos = macOS pide ahora, una sola vez, el nombre y la contraseña de un administrador. Con ellos, RABBIT ejecuta un comando de administrador que desinstala { $packages }. Si cancelas, no se desinstala nada.
wizard-progress-status-admin-still-waiting-macos = Todavía esperando la ventana de contraseña de administrador. Si no la oyes, comprueba si está detrás de RABBIT.
# While a package goes to the Trash, and in the log once the administrator
# command has worked on a package. $package is the localized package name.
# Name the Trash as Finder does.
wizard-progress-status-trashing = Moviendo { $package } a la Papelera…
wizard-progress-log-admin-done = El comando de administrador ha terminado con { $package }.
wizard-progress-log-configuration-started = aplicando { $step }…
wizard-progress-log-configuration-completed = se aplicó { $step }.

wizard-done-heading = Hecho
wizard-done-status-idle = Todavía no se ejecutó una instalación desde esta ventana.
wizard-done-status-success = ¡RABBIT terminó de hacer su magia! Revisa los detalles a continuación.
wizard-done-status-error = La instalación ha fallado. Revisa el error a continuación.
wizard-reaper-running-title = REAPER sigue abierto
wizard-reaper-running-body = Cierra REAPER y vuelve a pulsar Instalar. RABBIT no puede actualizar REAPER ni sus extensiones mientras REAPER esté abierto.
wizard-error-reaper-running = RABBIT se detuvo porque REAPER estaba abierto. Cierra REAPER y vuelve a ejecutar RABBIT.
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Otra copia de RABBIT abierta está instalando o desinstalando en este REAPER. Espera a que termine y vuelve a ejecutar RABBIT.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Cierra REAPER y vuelve a pulsar Desinstalar. RABBIT no puede desinstalar nada de REAPER mientras REAPER esté abierto.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Cierra todas las copias de REAPER de este ordenador, también las portables, y vuelve a pulsar Desinstalar. Cualquier REAPER de este ordenador puede usar { $packages }.
wizard-reaper-running-uninstall-reaper-body = Cierra todas las copias de REAPER de este ordenador, también las portables, y vuelve a pulsar Desinstalar. El propio desinstalador de REAPER no se ejecuta mientras haya algún REAPER abierto.
# The same on macOS, when RABBIT's administrator command is to move REAPER's
# app: it can't tell one REAPER from another.
wizard-reaper-running-uninstall-reaper-body-macos = Cierra todas las copias de REAPER de este ordenador, también las portables, y vuelve a pulsar Desinstalar. El comando de administrador que mueve REAPER no distingue un REAPER de otro, así que todos tienen que estar cerrados.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Cierra primero estos programas
wizard-programs-running-uninstall-body = Cierra { $programs } y vuelve a pulsar Desinstalar.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = RABBIT se detuvo porque había un REAPER abierto. Cierra todas las copias de REAPER de este ordenador, también las portables, y vuelve a ejecutar RABBIT.
wizard-error-uninstall-not-enough-space = RABBIT se detuvo porque no hay suficiente espacio libre para la copia de seguridad. Libera espacio en la unidad donde está la carpeta de recursos de REAPER y vuelve a ejecutar RABBIT.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } no se ha desinstalado porque otro programa está usando { $path }. Cierra ese programa y vuelve a ejecutar RABBIT.
# The same, for a file a package's installer added outside REAPER's folder.
# Only the JAWS scripts' installer adds such files, and JAWS itself may have
# one open; the program to close would be the user's screen reader, so this
# says to wait and try again instead. $package is the localized package
# name, and $path the file's full path as Windows holds it. Keep JAWS as it
# is.
wizard-error-uninstall-vendor-file-in-use = { $package } no se ha desinstalado porque JAWS u otro programa está usando { $path }. Espera un momento y vuelve a ejecutar RABBIT. Si así no se soluciona, reinicia JAWS primero.
# Why an uninstall didn't start on macOS: RABBIT itself runs as root, as under
# sudo. sudo and root are never translated.
wizard-error-uninstall-as-root = RABBIT se está ejecutando como root, como ocurre con sudo, así que no desinstala nada. Sal de RABBIT y ábrelo con tu propia cuenta. Pide por sí mismo la contraseña de un administrador cuando la necesita.
wizard-done-status-completed-with-errors = La instalación se completó con errores. Revisa los detalles a continuación.
wizard-done-status-cancelled = Has detenido la instalación. La lista de abajo muestra lo que RABBIT terminó antes de pararse.
wizard-done-status-no-packages = No se ha seleccionado ningún paquete para instalar o actualizar.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = RABBIT ha desinstalado { $packages }.
wizard-done-uninstall-status-completed-with-errors = RABBIT ha desinstalado { $packages }, pero no todo ha salido como estaba previsto.
wizard-done-uninstall-status-error = La desinstalación ha fallado.
wizard-done-uninstall-status-cancelled = Has detenido la desinstalación. Los paquetes a los que RABBIT todavía no había llegado siguen instalados.
wizard-done-uninstall-status-no-packages = No se ha desinstalado nada.
wizard-done-uninstall-status-pending-restart = RABBIT ha desinstalado { $packages }. Reinicia el ordenador para terminar.
wizard-done-uninstall-status-declined = No se ha desinstalado nada, porque se rechazó la solicitud de permiso de Windows.
# The same on macOS, where the administrator password window was cancelled.
wizard-done-uninstall-status-declined-macos = No se ha desinstalado nada, porque se canceló la ventana de contraseña de administrador.
# The Done page's first line when the administrator command removed only part
# of a package and nothing else was uninstalled. $packages is a list of
# package names.
wizard-done-uninstall-status-partly-removed = RABBIT solo ha quitado una parte de { $packages }.
wizard-done-uninstall-removed-before-stop = Antes de que lo detuvieras, RABBIT desinstaló { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Para recuperar uno, vuelve a ejecutar RABBIT y márcalo en la pestaña «Instalar o actualizar».
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } ya no estaba instalado, así que no había nada que quitar.
wizard-done-uninstall-left-in-backup = Algunos archivos de { $package } no se pudieron devolver a su sitio y siguen en la copia de seguridad. «Mostrar detalles» los enumera.
wizard-done-uninstall-leftovers = RABBIT todavía encuentra parte de { $package } y no lo toca. «Mostrar detalles» indica qué.
wizard-done-uninstall-language-reset-failed = RABBIT no pudo volver a poner REAPER en inglés después de quitar { $package }. Elige un idioma en las preferencias de REAPER.
wizard-done-uninstall-registry-failed = RABBIT desinstaló { $package }, pero puede que la lista de programas instalados de Windows aún lo muestre. «Mostrar detalles» indica la entrada.
wizard-done-uninstall-pending-restart = Reinicia el ordenador para terminar de quitar { $package }.
wizard-done-uninstall-vendor-cancelled = El propio desinstalador de { $package } se canceló antes de terminar.
wizard-done-uninstall-vendor-failed = El propio desinstalador de { $package } terminó con el código de error { $code }. «Mostrar detalles» indica lo que sigue ahí.
wizard-done-uninstall-vendor-failed-unsure = El propio desinstalador de { $package } terminó con el código de error { $code }, así que puede que { $package } no se haya quitado del todo.
wizard-done-uninstall-vendor-still-there = El propio desinstalador de { $package } terminó, pero { $package } sigue instalado. Puedes desinstalarlo en Configuración de Windows, en Aplicaciones, Aplicaciones instaladas.
wizard-done-uninstall-vendor-still-open = El propio desinstalador de { $package } sigue abierto, así que RABBIT se ha detenido y no ha tocado los paquetes siguientes. Pulsa Alt+Tab para encontrar su ventana y respóndela.
wizard-done-uninstall-vendor-run-failed = RABBIT no ha podido terminar de ejecutar el propio desinstalador de { $package } ({ $error }), así que se ha detenido ahí.
wizard-done-uninstall-elevation-failed = RABBIT no ha obtenido permisos de administrador ({ $error }), así que no se ha desinstalado nada.
wizard-done-uninstall-resource-damaged = Importante: después de ejecutarse el propio desinstalador de { $package }, faltaban estos archivos en la carpeta de recursos de REAPER: { $paths }. RABBIT se ha detenido y ha vuelto a escribir sus registros. No puede recuperar los ajustes de REAPER; si tienes una copia de reaper.ini, vuelve a ponerla en su sitio antes de abrir REAPER.
wizard-done-uninstall-receipt-not-saved = RABBIT ha desinstalado { $package }, pero no ha podido actualizar sus registros. La próxima vez que se ejecute, RABBIT verá igualmente que { $package } ya no está.
wizard-done-uninstall-vendor-lost = RABBIT ha perdido el rastro del propio desinstalador de { $package } después de que empezara. Puede que siga en marcha, así que RABBIT no puede saber qué ha quitado, y se ha detenido ahí.
wizard-done-uninstall-vendor-lost-reaper = Cuando el desinstalador haya terminado, comprueba que reaper.ini sigue en la carpeta de recursos de REAPER: guarda tus ajustes de REAPER.
wizard-done-uninstall-vendor-files-left = El propio desinstalador de { $package } terminó, pero algunos de sus archivos siguen ahí. «Mostrar detalles» los enumera.
wizard-done-uninstall-vendor-not-confirmed = El propio desinstalador de { $package } terminó, pero RABBIT no puede confirmar que { $package } ya no esté.
wizard-done-uninstall-vendor-not-listed = Windows ya no muestra { $package } en la lista de programas instalados, así que tampoco está en Configuración de Windows.
wizard-done-uninstall-working-folder-left = RABBIT no ha podido quitar { $path }, la carpeta desde la que ejecutó el desinstalador de { $package }. Solo un administrador puede borrarla.
wizard-done-uninstall-restart-list-unreadable = RABBIT no ha podido leer qué archivos quita Windows en el próximo reinicio, así que no sabe si los archivos que quedan de { $package } se irán entonces.
wizard-done-uninstall-installed-list-unreadable = RABBIT no ha podido leer la lista de programas instalados de Windows después del desinstalador de { $package }, así que no ha podido comprobar si Windows aún muestra { $package }.
# After the elevated helper was to remove the uninstaller folder and
# registry entry a package's installer left. $package is the localized
# package name, and $error Windows' own error message or RABBIT's in
# English. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does, and "Show
# details" as wizard-done-show-details does.
wizard-done-uninstall-cleanup-refused = RABBIT ha vuelto a comprobar la carpeta del desinstalador de { $package } dentro de Archivos de programa (Program Files) y su entrada del registro, y no se ha fiado de lo que ha encontrado, así que ha devuelto a su sitio lo que había movido.
wizard-done-uninstall-cleanup-failed = RABBIT no ha podido quitar del todo la carpeta del desinstalador y la entrada del registro de { $package } ({ $error }), así que ha devuelto a su sitio lo que había movido. «Mostrar detalles» indica lo que ya no está.
wizard-done-uninstall-cleanup-lost = RABBIT ha perdido el contacto con su programa auxiliar mientras quitaba la carpeta del desinstalador y la entrada del registro de { $package } ({ $error }), así que ha devuelto a su sitio lo que había movido. Puede que parte de esa carpeta o su entrada del registro ya no estén.
# Instead of wizard-done-uninstall-cleanup-failed, when RABBIT removed
# nothing of the folder or the registry entry before it failed, so "Show
# details" lists nothing as gone. $package and $error as above.
wizard-done-uninstall-cleanup-failed-nothing-gone = RABBIT no ha podido quitar la carpeta del desinstalador y la entrada del registro de { $package } ({ $error }), así que ha devuelto a su sitio lo que había movido. No se ha quitado nada de esa carpeta ni de su entrada del registro.
# After a macOS removal: to the Trash, or by the administrator command.
# $package is the localized package name, $path a full macOS path, keep it as
# it is, $error macOS's own error message, and $programs a list of program
# names, never translated. Name the Trash, Put Back and Finder as Finder does,
# "Show details" as wizard-done-show-details does, and System Settings >
# Privacy & Security > App Management as macOS names it in your language.
wizard-done-uninstall-moved-to-trash = RABBIT ha movido { $package } a la Papelera. Está allí como { $path }. Hasta que vacíes la Papelera, puedes arrastrarlo de vuelta desde allí.
wizard-done-uninstall-moved-to-trash-unknown = RABBIT ha movido { $package } a la Papelera. Hasta que vacíes la Papelera, puedes arrastrarlo de vuelta desde allí.
wizard-done-uninstall-trash-owner = Conserva el propietario que tenía, así que el Finder no ofrece «Sacar de la papelera» para él. Vaciar la Papelera puede pedir la contraseña de un administrador, y arrastrarlo de vuelta también.
wizard-done-uninstall-trash-failed = RABBIT no ha podido mover { $package } a la Papelera, así que no ha cambiado nada de él ({ $error }).
wizard-done-uninstall-trash-unclear = RABBIT pidió a macOS que moviera { $path } a la Papelera, pero no puede confirmar que esté allí ahora. Busca { $package } en la Papelera y en su carpeta en el Finder. RABBIT se detuvo ahí.
wizard-done-uninstall-admin-step-failed = El comando de administrador se detuvo en { $path }, así que no se ha quitado nada de { $package }. RABBIT se detuvo ahí.
wizard-done-uninstall-admin-delete-failed = El comando de administrador se detuvo en { $path }, que puede que ahora solo siga ahí en parte. No se ha quitado nada más de { $package }. RABBIT se detuvo ahí.
wizard-done-uninstall-admin-stopped = El comando de administrador terminó sin decir dónde se detuvo, y no se ha quitado nada de { $package }. RABBIT se detuvo ahí.
wizard-done-uninstall-partly-removed = El comando de administrador se detuvo en { $path }, así que { $package } solo se ha quitado en parte. «Mostrar detalles» indica qué se ha quitado y qué sigue ahí. RABBIT se detuvo ahí.
wizard-done-uninstall-partly-removed-unclear = El comando de administrador terminó sin decir dónde se detuvo, y { $package } solo se ha quitado en parte. «Mostrar detalles» indica qué se ha quitado y qué sigue ahí. RABBIT se detuvo ahí.
wizard-done-uninstall-admin-left = El comando de administrador se ejecutó, pero { $path } sigue ahí, o RABBIT no puede ver que se haya quitado, así que { $package } no se ha quitado del todo. «Mostrar detalles» indica qué se ha quitado y qué sigue ahí. RABBIT se detuvo ahí.
wizard-done-uninstall-app-management = Si macOS lo ha impedido: en Ajustes del Sistema > Privacidad y seguridad > Gestión de apps, activa RABBIT. Luego sal de RABBIT, ábrelo de nuevo y vuelve a desinstalar.
wizard-done-uninstall-admin-unclear = El comando de administrador terminó sin decir cómo, pero ya no queda nada de { $package }.
wizard-done-uninstall-installer-records-remain = El instalador de macOS todavía tiene recibos de { $package }, aunque sus archivos ya no están. Eso no hace daño; «Mostrar detalles» los indica.
wizard-done-uninstall-installer-records-unchecked = RABBIT no ha podido comprobar que el instalador de macOS haya olvidado { $package } ({ $error }). Eso no hace daño: sus archivos ya no están.
wizard-done-uninstall-program-started = El comando de administrador encontró abierto uno de estos programas, o no pudo comprobarlo, así que no cambió nada: { $programs }. Cierra los que estén abiertos y vuelve a ejecutar RABBIT.
wizard-done-uninstall-admin-steps-refused = RABBIT no ha podido preparar el comando de administrador para { $package } ({ $error }), así que no ha pedido ninguna contraseña ni ha desinstalado nada.
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

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = ¿Detener la instalación?
wizard-close-during-install-body = RABBIT está instalando. Si lo detienes ahora, RABBIT termina el paso que empezó, conserva lo que ya está instalado, limpia lo que empezó y luego se cierra. Los paquetes a los que todavía no ha llegado se quedan sin instalar. ¿Quieres detener la instalación?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = ¿Detener la desinstalación?
wizard-close-during-uninstall-body = RABBIT está desinstalando. Si lo detienes ahora, RABBIT termina el paso que empezó, luego se detiene y se cierra. Los paquetes a los que todavía no ha llegado siguen instalados. ¿Quieres detener la desinstalación?

# The same question while a restore runs, and while a delete of backups
# runs. No is the focused button here too.
wizard-close-during-restore-title = ¿Detener la restauración?
wizard-close-during-restore-body = RABBIT está restaurando. Si lo detienes ahora, RABBIT termina la copia de seguridad que está restaurando, luego se detiene y se cierra. Las copias de seguridad a las que todavía no ha llegado se quedan como están. ¿Quieres detener la restauración?
wizard-close-during-delete-title = ¿Detener la eliminación de copias de seguridad?
wizard-close-during-delete-body = RABBIT está eliminando copias de seguridad. Si lo detienes ahora, RABBIT termina la copia de seguridad que está eliminando, luego se detiene y se cierra. Las copias de seguridad a las que todavía no ha llegado se conservan. ¿Quieres detener la eliminación?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = ¿Desinstalar los paquetes marcados?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Una copia de seguridad no puede recuperar { $package }. Para tenerlo de nuevo, tendrás que volver a instalarlo.
# Instead of the sentence above, for a package whose files go into a backup
# while RABBIT removes its uninstaller folder in Program Files and its
# registry entry with administrator rights. Review shows this sentence too,
# or the one above when none of the package's files are left to move.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-confirm-uninstall-partly-unbacked = Una copia de seguridad puede recuperar los archivos de { $package }, pero no la carpeta de su desinstalador dentro de Archivos de programa (Program Files) ni su entrada del registro, que RABBIT quita con permisos de administrador.
# Instead of wizard-confirm-uninstall-unbacked, for a package that goes to
# the Trash; the second when your account may not move it, so an
# administrator's password moves it there. $package is the localized
# package name. Name the Trash as Finder does.
wizard-confirm-uninstall-trash = { $package } va a la Papelera. Puedes arrastrarlo de vuelta desde allí hasta que vacíes la Papelera.
wizard-confirm-uninstall-trash-elevated-macos = { $package } va a la Papelera. Puedes arrastrarlo de vuelta desde allí hasta que vacíes la Papelera, pero macOS puede pedir la contraseña de un administrador para ello.
wizard-confirm-uninstall-question = ¿Desinstalar ahora?

# Asked before backups are deleted, every time. The No button is the focused
# one, so Enter never deletes anything. The text is the body, then the
# only-copy sentence when one applies, then the question, as one paragraph.
# $count and $only are numbers: the backups that go, and how many of them
# are the only copy of what they hold. $size is a size, such as "520 MB".
wizard-confirm-delete-backups-title = ¿Eliminar las copias de seguridad para siempre?
wizard-confirm-delete-backups-body = RABBIT elimina { $count ->
    [one] una copia de seguridad
   *[other] { $count } copias de seguridad
} y libera { $size }.
wizard-confirm-delete-backups-only-copy = { $only ->
    [one] Una de ellas es la única copia de lo que contiene.
   *[other] { $only } de ellas son la única copia de lo que contienen.
}
# Instead of the sentence above, when only one backup goes.
wizard-confirm-delete-backups-only-copy-single = Es la única copia de lo que contiene.
wizard-confirm-delete-backups-question = Una copia de seguridad eliminada no se puede recuperar. ¿Eliminar ahora?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = RABBIT se está actualizando
wizard-close-during-self-update-body = RABBIT está reemplazando sus propios archivos de programa. Este paso no se puede detener sin dañar la instalación. Espera a que termine y cierra RABBIT después.

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Actualización de RABBIT disponible
wizard-self-update-prompt-body = RABBIT { $latest } está disponible. Ahora tienes { $current }. ¿Quieres actualizar? RABBIT se va a reiniciar cuando termine la actualización.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = Novedades desde RABBIT { $current }:
wizard-self-update-prompt-update-button = Actualizar ahora
wizard-self-update-prompt-later-button = Ahora no

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = Actualizando RABBIT

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
wizard-summary-osara-keymap-merge-hint = Se ha conservado tu mapa de teclado. Para añadirle los nuevos atajos de OSARA, abre REAPER y ejecuta la acción de OSARA «Merge OSARA key map» (menú Extensiones, OSARA).
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Acción que se planea: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Estado: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Versión instalada: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Arquitectura: { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Quitar también la configuración: { $value }
# One line per file, below its package's line. $path is a full path.
# wizard-summary-left-over can also name a folder, such as a package's
# uninstaller folder in Program Files, or a registry key as regedit shows
# it; keep either as it is, never translated.
wizard-summary-removed-file = Quitado: { $path }
wizard-summary-kept-file = Conservado porque cambió desde que RABBIT lo instaló: { $path }
wizard-summary-kept-link = Conservado porque contiene un enlace que RABBIT nunca sigue: { $path }
wizard-summary-kept-reapack = Conservado porque lo instaló ReaPack, que lleva la cuenta de él: { $path }
wizard-summary-left-over = Sigue ahí: { $path }
wizard-summary-left-in-backup = No se pudo devolver a su sitio, sigue en la copia de seguridad: { $path }
# A file a package's installer added outside REAPER's folder that stays:
# it was there before RABBIT ran that installer, or RABBIT recorded it more
# than once with contents that disagree. "the installer" is the package's
# own installer. $path is a full path as Windows holds it, never
# translated.
wizard-summary-kept-there-before = Conservado porque ya estaba ahí cuando RABBIT ejecutó el instalador: { $path }
wizard-summary-kept-record-unclear = Conservado porque los registros de RABBIT sobre él no coinciden: { $path }
wizard-summary-pending-restart = Se quita al reiniciar el ordenador: { $path }
wizard-summary-vendor-uninstaller = Su propio desinstalador: { $path }
# Below a package removed on macOS, one line per path: what went to the Trash
# and where it is there now, and what the administrator command removed; then
# the installer receipts macOS still lists, and macOS's own words when a step
# failed. On Windows, the line for what was removed with administrator rights
# also names a package's uninstaller folder in Program Files, and each file
# in it, that RABBIT removed that way. $path is a full Windows or macOS path,
# keep it as it is; $ids a comma-separated list of receipt names, never
# translated; $error macOS's own message. Name the Trash as Finder does.
wizard-summary-moved-to-trash = Movido a la Papelera: { $path }
wizard-summary-in-trash-as = Ahora en la Papelera: { $path }
wizard-summary-removed-as-admin = Quitado con permisos de administrador: { $path }
wizard-summary-installer-records-remain = El instalador de macOS aún lo tiene: { $ids }
wizard-summary-macos-said = macOS dijo: { $error }
# What became of a package's entry in Windows' list of installed programs,
# below the package's line. $key is the entry's registry key, such as
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA, and is
# never translated; $error is Windows' own error message.
wizard-summary-registry-removed = Quitada de la lista de programas instalados de Windows: { $key }
wizard-summary-registry-kept = Sigue en la lista de programas instalados de Windows porque no apunta a este REAPER: { $key }
wizard-summary-registry-failed = Sigue en la lista de programas instalados de Windows porque RABBIT no pudo leerla ni quitarla ({ $error }): { $key }
# Below a package whose uninstaller folder and registry entry RABBIT removed
# with administrator rights: the registry entry. $key is the entry's
# registry key as regedit shows it, such as
# HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts,
# and is never translated. The folder and its files are listed with
# wizard-summary-removed-as-admin.
wizard-summary-registry-key-removed = Quitada del registro con permisos de administrador: { $key }
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Copia de seguridad: { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = Para recuperar { $package }, cierra REAPER y ejecuta: { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Informe: { $path }
status-installed-or-checked = Instalado o revisado
status-planned-unattended = Plan desatendido
status-deferred-unattended = Plan diferido
status-skipped-current = Omitido (ya existe)
status-failed = Fallido
status-skipped-dependency-failed = Omitido (falló una dependencia)
status-cancelled = No instalado (lo detuviste)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Desinstalado
status-removed-with-leftovers = Desinstalado, pero parte sigue ahí
status-removed-pending-restart = Desinstalado; el resto se quita al reiniciar el ordenador
status-would-remove = Se desinstalaría
status-remove-not-installed = No estaba instalado
status-remove-blocked = No desinstalado
status-remove-failed = No se pudo desinstalar
status-remove-cancelled = Sigue instalado (lo detuviste)
status-remove-not-reached = Sigue instalado (el proceso se detuvo antes)

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
package-status-cancelled = No instalado: detuviste la configuración antes de que RABBIT llegara a este paquete.
# Aparece tras el nombre del paquete cuando el instalador del fabricante terminó
# con un código distinto de 0. $code es el número. Le sigue una de las frases
# package-status-vendor-exit-* y, si el instalador dejó un registro,
# package-status-vendor-installer-log.
package-status-vendor-installer-failed = El instalador terminó con el código de error { $code }.
# Código 2 de Inno Setup: se canceló antes de empezar a instalar. Como RABBIT
# lo ejecuta de forma desatendida, en la práctica se rechazó el aviso de
# permisos de Windows (Control de cuentas de usuario) que mostró el
# instalador. «Sí» es el botón del aviso.
package-status-vendor-exit-cancelled-before-installing = Eso suele significar que se rechazó el aviso de permisos de Windows, así que no cambió nada. Vuelve a ejecutar RABBIT y elige Sí cuando Windows te pida permiso.
# Código 5 de Inno Setup: se detuvo en una pregunta, como si debe cerrar los
# programas que usan sus archivos, que nadie puede responder mientras RABBIT lo
# ejecuta de forma desatendida. $package es el nombre del paquete.
package-status-vendor-exit-stopped-at-question = Eso significa que se detuvo en una pregunta que no puede hacer mientras RABBIT lo ejecuta de forma desatendida, normalmente porque otro programa tiene abierto uno de sus archivos. Cierra los demás programas, sobre todo otros programas de música y { $package }, y vuelve a ejecutar RABBIT.
# Código 7 de Inno Setup: al prepararse vio que no puede continuar.
package-status-vendor-exit-preparation-failed = Eso significa que, al prepararse, vio que ahora no puede instalar. Reinicia el equipo y vuelve a ejecutar RABBIT.
# Código 8 de Inno Setup: al prepararse vio que Windows debe reiniciarse antes.
package-status-vendor-exit-restart-needed = Eso significa que, al prepararse, vio que Windows tiene que reiniciarse primero. Reinicia el equipo y vuelve a ejecutar RABBIT.
# Cualquier otro código, o un instalador cuyos códigos RABBIT no conoce.
# $package es el nombre del paquete.
package-status-vendor-exit-unknown = Cierra los demás programas y vuelve a ejecutar RABBIT. Si se detiene de nuevo, instala { $package } desde el sitio web de su editor.
# $path es el archivo de registro que escribió el instalador.
package-status-vendor-installer-log = El registro del instalador está en { $path }.
# Aparece tras el nombre del paquete cuando RABBIT no inició su instalador
# porque los propios programas del paquete estaban abiertos. $package es el
# nombre del paquete; $programs, los nombres de archivo de los programas
# abiertos, como «Surge XT.exe».
package-status-blocking-programs-running = RABBIT no inició el instalador porque { $package } está abierto ({ $programs }) y el instalador no puede reemplazar un programa que se está ejecutando. Ciérralo y vuelve a ejecutar RABBIT.
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
config-message-cancelled = Este paso de configuración no se ejecutó: detuviste la configuración antes.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Estado: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Aplicado
config-status-skipped = Omitido
config-status-skipped-dependency-missing = Omitido  (falta dependencia)
config-status-dry-run = Dry ejecutado
config-status-cancelled = No se ejecutó (lo detuviste)
wizard-summary-planned-execution-title = Ejecución desatendida planeada:
wizard-summary-planned-execution-runner =   Se ejecuta: { $runner }
wizard-summary-planned-execution-artifact =   Artefacto: { $artifact }
wizard-summary-planned-execution-program =   Programa: { $program }
wizard-summary-planned-execution-program-zipped =   Programa: el instalador contenido en el archivo .zip, una vez descargado
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

# Modo experto y canales de versiones.
package-channel-reaper-dev = Versiones de desarrollo de landoleet.org
package-channel-osara-pr = Versiones de prueba de pull requests
wizard-window-title-expert = { $title } – Modo experto
wizard-expert-enable-title = ¿Activar el modo experto?
wizard-expert-enable-body = El modo experto te permite instalar versiones preliminares: las versiones de desarrollo de REAPER de landoleet.org y las versiones de prueba de los pull requests de OSARA. No tienen soporte, pueden estropear REAPER o su accesibilidad y solo sirven para hacer pruebas. En esta página aparecerán dos opciones de versión, debajo de la instalación de REAPER. ¿Activar el modo experto?
wizard-expert-disabled-title = El modo experto está desactivado
wizard-expert-disabled-body = El modo experto está desactivado. Los paquetes instalados desde una versión preliminar volverán a su versión normal la próxima vez que instales.
wizard-expert-first-page-title = Modo experto
wizard-expert-first-page-body = El modo experto solo se puede activar o desactivar en la primera página, que es donde añade las opciones de versión. Vuelve a la primera página para cambiarlo.
wizard-expert-busy-body = El modo experto no se puede cambiar mientras RABBIT está instalando ni cuando ya ha terminado.
wizard-expert-reaper-builds-label = Versiones de REAPER
wizard-expert-reaper-builds-stable = Versiones normales
wizard-expert-osara-builds-label = Versiones de OSARA
wizard-expert-osara-builds-snapshot = Snapshots normales
wizard-expert-osara-builds-loading = Buscando versiones de pull requests…
wizard-expert-osara-builds-unavailable = Snapshots normales (no se pudieron obtener las versiones de pull requests)
wizard-expert-osara-builds-pr = Pull request { $number }: { $title }
wizard-expert-osara-builds-pr-untitled = Pull request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = versión de desarrollo
wizard-package-channel-dev-details = { $package } se instalará desde sus versiones de desarrollo: versiones preliminares sin soporte que pueden fallar en cualquier momento.
wizard-package-channel-pr = versión de prueba del pull request { $number }
wizard-package-channel-pr-details = { $package } se instalará desde la versión de prueba del pull request { $number }: código sin revisar ni firmar que GitHub borra a los 90 días.
wizard-package-channel-back = vuelta a la versión normal
wizard-package-channel-back-details = { $package } es ahora una versión preliminar. Esta instalación vuelve a poner la versión normal.
