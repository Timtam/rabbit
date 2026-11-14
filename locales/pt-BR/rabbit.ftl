app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = sim
common-no = não

# What happens to a package in this run, the first thing its row says. An
# unticked row on either tab gets action-keep, so keep it neutral: on the
# Uninstall tab it only means the package isn't marked for removal.
action-install = Será instalado
action-update = Será atualizado
action-keep = Não será alterado
action-uninstall = Será desinstalado
# On the Backups tab: what happens to a backup ticked to restore, or to
# delete. The last is for a backup that is the only copy of what it holds,
# such as software you uninstalled.
action-restore = Será restaurado
action-delete = Será excluído
action-delete-only-copy = Será excluído (única cópia)

package-reaper = REAPER
package-osara = OSARA
package-sws = Extensão SWS
package-reapack = ReaPack
package-reakontrol = ReaKontrol
package-jaws-scripts = Scripts do Snowman para JAWS no REAPER
package-ffmpeg = FFmpeg para melhor suporte a vídeo
package-surge-xt = Surge XT
package-app2clap = app2clap
package-langpack-es = REAPER em espanhol
package-langpack-de = REAPER em alemão
package-langpack-fr = REAPER em francês
package-langpack-es-variant-rae = REAPER Accesible español (es_ES)
package-langpack-es-variant-pma = Team PMA (es_MX)

package-reaper-description = O REAPER é a estação de trabalho de áudio digital (DAW) sobre a qual todo o resto aqui é construído. O RABBIT pode instalar ou atualizar o REAPER para você.
package-osara-description = O OSARA é a extensão de acessibilidade de código aberto que torna o REAPER utilizável com leitores de tela. NVDA, JAWS e Narrador no Windows e VoiceOver no macOS são amplamente utilizados; alguns outros leitores de tela do Windows também podem funcionar. Instale o OSARA se você depende de um leitor de tela para usar o REAPER.
package-sws-description = A Extensão SWS é um pacote mantido há muitos anos pela comunidade, com ações, scripts e utilitários extras que ampliam os recursos do REAPER. Para ter o REAPER o mais acessível possível, seja no Windows ou no Mac, você deve instalar o SWS junto com o OSARA.
package-reapack-description = O ReaPack é um gerenciador de pacotes de código aberto. Com ele é possível pesquisar, instalar, acompanhar e atualizar scripts e extensões de terceiros de dentro do próprio REAPER. Instale-o se quiser usar scripts compartilhados pela comunidade do REAPER.
package-reakontrol-description = O ReaKontrol oferece integração de código aberto com os teclados Komplete Kontrol da Native Instruments. Instale-o se você tiver um teclado da série S MK2, série A, M-32 ou Kontrol MK3.
package-jaws-scripts-description = Os scripts do Snowman melhoram a forma como o JAWS lida com várias janelas do REAPER, além de oferecer suporte ampliado a Braille e muitos outros recursos. Observe que esses scripts devem ser usados junto com o OSARA; eles não são uma alternativa a ele. Para a melhor acessibilidade com o JAWS, você deve instalar os dois.
package-ffmpeg-description = As bibliotecas compartilhadas do FFmpeg permitem que o decodificador de vídeo do REAPER importe e reproduza formatos comuns de vídeo e áudio. O RABBIT instala a pasta bin da compilação GPL-shared da BtbN em UserPlugins; como o número da revisão não pode ser obtido apenas pelos nomes das DLLs, versões do FFmpeg instaladas por fora são exibidas com o marcador `<major>.0.0`.
package-surge-xt-description = O Surge XT é um sintetizador híbrido gratuito e de código aberto. O RABBIT executa o instalador do fabricante para você — ele instala os formatos VST3, CLAP, AU (somente macOS) e standalone para todo o sistema, para que o REAPER e outras DAWs possam carregar o Surge XT. Acompanha o canal de compilações noturnas (nightly), porque a versão estável mais recente (1.3.4) é de agosto de 2024 e, na prática, o projeto agora é distribuído pelas compilações noturnas. Apenas para instalações padrão do REAPER: os dados de fábrica ficam fora de qualquer pasta portátil do REAPER.
package-app2clap-description = O app2clap é um plug-in CLAP para Windows que captura o áudio de outros aplicativos e o traz para o REAPER (ou qualquer host CLAP) como um plug-in inserido em uma faixa — útil para gravar ou processar o som de um navegador, reprodutor de mídia ou outro programa. O RABBIT baixa a compilação mais recente e instala o app2clap.clap na sua pasta CLAP de usuário, sem precisar de direitos de administrador. Somente Windows. Apenas para instalações padrão do REAPER: ele é instalado fora de qualquer pasta portátil do REAPER.
package-langpack-es-description = Traduz a própria interface do REAPER para o espanhol, incluindo a extensão SWS. Mantido por Javier Robledo para a comunidade hispânica do REAPER e distribuído pelo reaperespa.com — o RABBIT baixa a versão atual e a instala como es_ES.ReaperLangPack. O OSARA usa esse nome de arquivo para escolher a sua própria tradução em espanhol. Após a instalação, escolha o idioma nas preferências do REAPER (ou deixe que o RABBIT faça isso por você).
package-langpack-de-description = Traduz a própria interface do REAPER para o alemão, incluindo a extensão SWS. Mantido por MrData e publicado no REAPER Stash — o RABBIT baixa a versão atual e a instala como de_DE.ReaperLangPack. Após a instalação, escolha o idioma nas preferências do REAPER (ou deixe que o RABBIT faça isso por você).
package-langpack-fr-description = Traduz a própria interface do REAPER para o francês, incluindo a extensão SWS. Mantido por Lee Julien e Pierre-Marie Curt para o ReaperAccessible, a comunidade francófona de usuários de leitores de tela, e publicado no repositório ReaPack deles — o RABBIT baixa a versão atual e a instala como fr_CA.ReaperLangPack. O OSARA usa esse nome de arquivo para escolher a sua própria tradução em francês, e a tradução fr_CA dele está quase completa, enquanto a fr_FR ainda deixa cerca de um terço das mensagens em inglês; o pacote em si é o mesmo francês padrão nos dois casos. Após a instalação, escolha o idioma nas preferências do REAPER (ou deixe que o RABBIT faça isso por você).

# Text named by the uninstall plans in the package manifests.
#
# The note for a package RABBIT can't uninstall yet.
uninstall-not-yet = O RABBIT ainda não consegue desinstalar este pacote.

# The two sentences Review shows for a package about to be uninstalled: what
# goes, and what stays. With "Also remove settings" ticked, another sentence
# replaces the one about what stays.
uninstall-summary-sws-removes = Remove a extensão SWS.
uninstall-summary-sws-keeps = Os seus grooves e as suas configurações do SWS, como as suas ações cíclicas, são mantidos.
uninstall-summary-reapack-removes = Remove a extensão ReaPack.
uninstall-summary-reapack-keeps = As suas configurações e repositórios do ReaPack são mantidos, assim como a lista do que o ReaPack instalou.
uninstall-summary-reakontrol-removes = Remove a extensão ReaKontrol.
uninstall-summary-reakontrol-keeps = Os seus mapas de FX são mantidos, e o RABBIT não mexe no seu software da Native Instruments.
uninstall-summary-ffmpeg-removes = Remove os arquivos do FFmpeg que o RABBIT instalou em UserPlugins. Depois disso, o REAPER não pode mais usar o FFmpeg para abrir ou renderizar arquivos de mídia.
uninstall-summary-ffmpeg-keeps = Os arquivos do FFmpeg dos quais o RABBIT não tem registro, como os deixados por versões anteriores, são mantidos.
uninstall-summary-app2clap-removes = Remove o app2clap da sua pasta CLAP. Todos os programas em que você usa plug-ins CLAP perdem o app2clap, não só o REAPER.
uninstall-summary-app2clap-keeps = A pasta CLAP e os outros plug-ins que estão nela são mantidos.
uninstall-summary-langpack-removes = Remove o pacote de idioma que o RABBIT instalou. Se o REAPER estiver usando esse pacote, o RABBIT volta a deixar o REAPER em inglês.
uninstall-summary-langpack-keeps = Os pacotes de idioma que você mesmo adicionou são mantidos.
uninstall-summary-osara-removes = Remove a extensão OSARA, as traduções dela e a cópia do mapa de teclas dela na pasta KeyMaps.
uninstall-summary-osara-removes-windows = Remove a extensão OSARA, as traduções dela, o desinstalador dela e a cópia do mapa de teclas dela na pasta KeyMaps. Se este OSARA aparecer na lista de programas instalados do Windows, o RABBIT o remove dessa lista.
uninstall-summary-osara-keeps = O seu mapa de teclas continua como está, assim como as configurações do OSARA e os backups do seu mapa de teclas que o OSARA fez.
uninstall-summary-surge-removes = O próprio desinstalador do Surge XT o remove para todos os programas de música deste computador. Um backup não consegue restaurá-lo.
uninstall-summary-surge-keeps = Os seus próprios patches, na pasta Surge XT, dentro da sua pasta Documentos, são mantidos.
uninstall-summary-reaper-windows-removes = O próprio desinstalador do REAPER remove o programa. Ele também remove as associações de arquivos do REAPER e o ReaRoute para todas as cópias do REAPER neste computador.
uninstall-summary-reaper-keeps = As suas configurações, extensões e projetos são mantidos.
# The JAWS scripts: two plug-in files in REAPER's UserPlugins folder, and
# script files their installer adds to JAWS's settings folder, which RABBIT
# removes only while they are as the installer left them. Keep
# %APPDATA%\Freedom Scientific\JAWS as it is.
uninstall-summary-jaws-scripts-removes = Remove os dois arquivos de plug-in dos scripts da pasta UserPlugins do REAPER, e todos os arquivos de script que o RABBIT viu o instalador deles adicionar à pasta de configurações do JAWS.
uninstall-summary-jaws-scripts-keeps = Os arquivos de script que o RABBIT não tem certeza de que o instalador adicionou, e os que foram alterados desde então, como as suas próprias atribuições de teclas, continuam na pasta de configurações do JAWS, %APPDATA%\Freedom Scientific\JAWS.
# The same two sentences on macOS, where Surge XT has no uninstaller of its
# own and REAPER is an app. Name the Trash and the Library folder as Finder
# does in your language.
uninstall-summary-surge-macos-removes = Remove os aplicativos e plug-ins do Surge XT, e os dados de fábrica dele na pasta Biblioteca. Um backup não consegue restaurá-los.
uninstall-summary-reaper-macos-removes = Move o aplicativo REAPER para o Lixo.

# A short description of a file or folder that stays, shown next to its path,
# for example "Data\Grooves (your grooves)". Start in lower case where the
# language allows.
uninstall-kept-sws-grooves = os seus grooves
uninstall-kept-sws-settings = configurações do SWS
uninstall-kept-sws-cycle-actions = as suas ações cíclicas
uninstall-kept-reapack-settings = configurações do ReaPack e os seus repositórios
uninstall-kept-reapack-data = a lista do que o ReaPack instalou, e o cache de downloads dele
uninstall-kept-reakontrol-fx-maps = os seus mapas de FX
uninstall-kept-app2clap-clap-folder = a pasta CLAP, compartilhada com outros plug-ins
uninstall-kept-osara-key-map = o seu mapa de teclas
uninstall-kept-osara-replaced-key-map = o seu mapa de teclas de antes de o OSARA substituí-lo
uninstall-kept-osara-merged-key-map = o seu mapa de teclas de antes de o OSARA adicionar os atalhos dele
uninstall-kept-osara-settings = as configurações do OSARA, em um arquivo compartilhado por todas as extensões
uninstall-kept-surge-user-data = os seus patches e todo o resto que você salvou com o Surge XT
uninstall-kept-reaper-resource = a pasta de recursos do REAPER, com as suas configurações, extensões e mapas de teclas
# On macOS, in the Library folder of your home folder.
uninstall-kept-surge-settings-macos = as suas configurações do Surge XT
# On macOS, in the Library folder of your home folder: a copy of Surge XT's
# factory data put there by hand, which Surge XT reads instead of the one it
# installs for everyone. Say "factory data" as in
# uninstall-summary-surge-macos-removes.
uninstall-kept-surge-factory-copy-macos = a sua própria cópia dos dados de fábrica do Surge XT

# Shown before SWS is uninstalled while OSARA, or ReaPack, is installed.
uninstall-warning-sws-osara-keymap = Se você usa o mapa de teclas do OSARA, alguns dos atalhos dele executam ações do SWS. Esses atalhos deixam de funcionar até você instalar o SWS novamente.
uninstall-warning-sws-reapack-scripts = Muitos scripts do REAPER, entre eles scripts de acessibilidade, precisam do SWS. Os scripts que você instalou com o ReaPack podem deixar de funcionar.

# A note on what stays behind after ReaPack is uninstalled.
uninstall-note-reapack-packages-stay = Os scripts e extensões que você instalou com o ReaPack continuam onde estão, mas nada mais os mantém atualizados.
# A note on what stays behind after Surge XT is uninstalled.
uninstall-note-surge-fonts-stay = As fontes que o Surge XT instalou são mantidas: o instalador dele as marca para nunca serem removidas.
# A note on the JAWS script files that stay after the JAWS scripts are
# uninstalled, and how to remove them by hand. Keep %APPDATA%, the folder
# and file names, and "reaper" (the start of the file names) exactly as they
# are. For the example language folder, use JAWS's folder for your language:
# enu for English, deu for German, esn for Spanish, fra for French, ita for
# Italian.
uninstall-note-jaws-scripts-by-hand = Para remover os arquivos de script que ficaram, exclua os arquivos cujos nomes começam com reaper das pastas de idioma em %APPDATA%\Freedom Scientific\JAWS, como 2025\Settings\ptb. Copie antes reaper.jkm, reaper.jcf e reaper.ini para outro lugar, se quiser manter as suas atribuições de teclas e configurações.

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
uninstall-by-hand-jaws-scripts = Para remover os scripts para JAWS manualmente:
    1. Feche o REAPER.
    2. Exclua Reaper_ComAccess32.dll e Reaper_ComAccess64.dll da pasta %APPDATA%\REAPER\UserPlugins.
    3. Exclua a pasta %ProgramFiles(x86)%\Reaper_JawsScripts. O Windows pede a permissão de um administrador.
    4. Para remover também os arquivos de script, exclua os arquivos cujos nomes começam com reaper das pastas de idioma em %APPDATA%\Freedom Scientific\JAWS, como 2025\Settings\ptb. Copie antes reaper.jkm, reaper.jcf e reaper.ini para outro lugar, se quiser manter as suas atribuições de teclas e configurações.
uninstall-manual-reaper-portable = O RABBIT não remove um REAPER portátil: os arquivos do próprio REAPER e os seus ficam na mesma pasta. Para removê-lo manualmente:
    1. Feche o REAPER.
    2. Copie para fora da pasta do REAPER portátil tudo o que quiser manter, como projetos que você salvou nela, os seus mapas de teclas e as suas configurações.
    3. Exclua a pasta do REAPER portátil.
    Os backups que o RABBIT fez para este REAPER também estão nessa pasta, em RABBIT\backups, e são excluídos junto com ela.

# Why RABBIT didn't start a program's own uninstaller, or didn't remove a
# package's uninstaller folder and registry entry. RABBIT does both with
# administrator rights. Nothing has changed at that point. RABBIT shows one
# of these, then uninstall-vendor-refused-nothing-changed, or, for the folder
# and registry entry, the steps to remove the package by hand.
# { $package } is the package's name, such as Surge XT, REAPER or
# JAWS-for-REAPER scripts.
# { $path } is a file, a folder or a value as Windows holds it; keep it as it
# is.
uninstall-vendor-refused-not-listed = O Windows não lista { $package } entre os programas instalados para todos os usuários, então o RABBIT não consegue encontrar o desinstalador dele.
uninstall-vendor-refused-unclear-command = O RABBIT não consegue identificar qual programa o Windows indica como o desinstalador de { $package }: { $path }
uninstall-vendor-refused-not-a-full-path = O Windows indica o desinstalador de { $package } por um caminho em que o RABBIT não confia: { $path }
uninstall-vendor-refused-unknown-folder = O RABBIT não consegue encontrar uma pasta de que precisa para verificar o desinstalador de { $package }.
uninstall-vendor-refused-wrong-folder = O desinstalador de { $package } não está na pasta onde deveria estar: { $path }
uninstall-vendor-refused-foreign-file = Ao lado do desinstalador de { $package } há um arquivo que não deveria estar lá: { $path }
uninstall-vendor-refused-folder-or-link = Ao lado do desinstalador de { $package } há uma pasta ou um link que não deveria estar lá: { $path }
uninstall-vendor-refused-untrusted-owner = Ao lado do desinstalador de { $package } há um arquivo que não foi colocado lá por um administrador: { $path }
uninstall-vendor-refused-install-location-missing = O Windows não informa em qual pasta { $package } está instalado.
uninstall-vendor-refused-install-location-mismatch = O Windows informa que { $package } está instalado em { $path }, e não no REAPER que você escolheu.
uninstall-vendor-refused-not-in-install-folder = O desinstalador de { $package } não está na pasta do programa: { $path }
uninstall-vendor-refused-not-below-program-files = { $package } não está instalado em uma pasta própria em Arquivos de Programas (Program Files): { $path }
uninstall-vendor-refused-copy-differs = A cópia que o RABBIT fez do desinstalador de { $package } não é igual ao original.
uninstall-vendor-refused-unreadable = O RABBIT não conseguiu ler { $path } para verificar o desinstalador de { $package }.
uninstall-vendor-refused-not-a-vendor-plan = O RABBIT não tem nenhum desinstalador de { $package } que tenha permissão para executar.
uninstall-vendor-refused-unsupported = O RABBIT só pode executar o desinstalador de { $package } no Windows.
# { $path } is the name of a value in the package's registry entry.
uninstall-vendor-refused-unexpected-registry-value = A entrada de { $package } no Registro do Windows contém um valor que o RABBIT não espera: { $path }
# { $path } is the text of a value in the package's registry entry.
uninstall-vendor-refused-registry-value-mismatch = A entrada de { $package } no Registro do Windows aponta para um local que o RABBIT não espera: { $path }
# The package's registry entry has subkeys, which its installer doesn't write.
uninstall-vendor-refused-registry-key-has-subkeys = A entrada de { $package } no Registro do Windows contém mais do que o RABBIT espera.
# { $path } is the package's uninstaller folder in Program Files.
uninstall-vendor-refused-untrusted-folder = A pasta de { $package } não foi criada por um administrador: { $path }
# Follows each of the messages above. { $package } as above.
uninstall-vendor-refused-nothing-changed = Nada foi alterado. Você pode desinstalar { $package } nas Configurações do Windows, em Aplicativos, Aplicativos instalados.

# $reason is one of the localized "wizard-package-row-unavailable-*" strings
# explaining *why* the row is unavailable. Appended to the row's main summary
# in the package CheckListBox.
wizard-package-row-unavailable-suffix = (indisponível: { $reason })
wizard-package-row-unavailable-portable = destino REAPER portátil
wizard-package-row-unavailable-version-check = a verificação de versão on-line falhou

# Review-page note carrying the full error for a package whose latest-version
# check failed; its row is disabled with the short reason above.
wizard-version-check-failed-note = { $package }: a verificação da versão mais recente falhou ({ $message }). A instalação ou atualização deste pacote está desativada nesta execução.

detect-installed = Instalado
detect-not-installed = Não instalado
detect-version-unknown = Versão desconhecida
detect-source-receipt = Registro do RABBIT
detect-source-files = Presença de arquivos em UserPlugins
detect-source-reapack-registry = Registro do ReaPack

# $package is the localized package display name.
status-package-installed = { $package } instalado

wizard-step-target = Destino
wizard-step-version-check = Verificação de versões
wizard-step-packages = Pacotes
wizard-step-reapack-acknowledgement = Doação ao ReaPack
wizard-step-review = Revisão
wizard-step-progress = Progresso
wizard-step-done = Concluído

# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-button-back = Voltar
wizard-button-back-mnemonic = V
wizard-button-next = Avançar
wizard-button-next-mnemonic = A
wizard-button-install = Instalar
wizard-button-install-mnemonic = I
wizard-button-uninstall = Desinstalar
wizard-button-uninstall-mnemonic = D
wizard-button-close = Fechar
wizard-button-close-mnemonic = F
wizard-button-cancel = Cancelar
wizard-button-cancel-mnemonic = C

wizard-target-heading = Escolha uma tarefa
wizard-target-language-label = Idioma
wizard-target-language-restart-note = Ao mudar o idioma, o RABBIT é reiniciado para que o novo idioma tenha efeito.
wizard-locale-name-en-US = Inglês (Estados Unidos)
wizard-locale-name-de-DE = Alemão (Alemanha)
wizard-locale-name-es-ES = Espanhol (Espanha)
wizard-locale-name-fr-FR = Francês (França)
wizard-locale-name-it-IT = Italiano (Itália)
wizard-locale-name-pt-BR = Português (Brasil)
wizard-target-choice-label = Local de instalação
wizard-target-details-label = Detalhes do destino
wizard-target-empty = Nenhum destino de instalação do REAPER foi selecionado.
wizard-target-portable-choice = Criar ou atualizar uma versão portátil do REAPER
wizard-target-portable-folder-label = Pasta portátil
wizard-target-portable-folder-message = Escolha uma pasta portátil do REAPER, se já tiver uma, ou uma pasta vazia para criar uma nova versão portátil.
wizard-target-portable-folder-browse-label = Procurar…
wizard-target-portable-pending-details = Use o botão Procurar para indicar o local de uma versão portátil existente, se você tiver uma, ou para escolher uma pasta vazia se quiser criar uma nova versão portátil do REAPER.
wizard-target-custom-portable-label = Pasta portátil do REAPER
wizard-target-custom-portable-app-path-label = Caminho do aplicativo REAPER
wizard-target-custom-portable-path-label = Caminho de recursos portátil
wizard-target-custom-portable-version-label = Versão do REAPER
wizard-target-custom-portable-writable-label = Gravável
wizard-target-custom-portable-note = O RABBIT criará aqui o caminho de recursos do REAPER, caso ele não exista.

# $version is the REAPER version or an unknown-version label and $path is the resource path.
wizard-target-row = REAPER { $version } em { $path }

# $app_path is the REAPER application path, $path is the REAPER resource path,
# $version is the REAPER version or an unknown-version label, and $writable
# is yes/no.
wizard-target-details = Local de instalação do REAPER: { $app_path }
    Versão: { $version }
    Caminho de recursos: { $path }
    Gravável: { $writable }

wizard-packages-heading = Escolha o que instalar, atualizar ou desinstalar
wizard-packages-list-label = Instalar ou atualizar: pacotes e configurações
wizard-packages-row-checked = Marcado
wizard-packages-row-unchecked = Desmarcado
wizard-packages-choice-shown = Nova opção abaixo da lista: { $choice }.
wizard-packages-choice-hidden = Opção removida: { $choice }.
wizard-packages-tree-group-label = Pacotes
wizard-additional-software-tree-group-label = Programas adicionais
wizard-language-tree-group-label = Pacotes de idioma
wizard-configuration-tree-group-label = Configuração

# The two tabs on the Packages page. Screen readers read the selected tab's
# name when the page opens, so keep both short.
wizard-packages-tab-install = Instalar ou atualizar
wizard-packages-tab-uninstall = Desinstalar
# The third tab, where RABBIT's backups are restored or deleted. Keep it
# short too.
wizard-packages-tab-backups = Backups
# macOS only: VoiceOver says this once, the first time the Packages page opens.
# Name the Tab and Shift keys as Apple's page of Mac keyboard shortcuts does,
# and write Control out in full.
wizard-packages-tabs-switch-hint-macos = Pressione Control + Tab ou Control + Shift + Tab para alternar entre as guias.
# The label right before the Uninstall tab's list. Screen readers read it when
# focus enters the list, so it names the tab first.
wizard-uninstall-list-label = Desinstalar: software que o RABBIT pode remover
# One row of the Uninstall tab. $package is the localized package name,
# $action is action-uninstall or action-keep, and $installed is the installed
# version or an unknown-version label.
wizard-uninstall-row = { $package }: { $action }. Você tem { $installed }
# Why an Uninstall row can't be ticked: the short phrase that goes into
# wizard-package-row-unavailable-suffix at the end of the row.
wizard-uninstall-row-unavailable-not-yet = o RABBIT ainda não consegue desinstalá-lo
wizard-uninstall-row-unavailable-not-supported = o RABBIT não consegue desinstalá-lo
wizard-uninstall-row-unavailable-manual = remova manualmente
wizard-uninstall-row-unavailable-reapack = gerenciado pelo ReaPack
wizard-uninstall-row-unavailable-reapack-check-failed = não foi possível verificar os registros do ReaPack
wizard-uninstall-row-unavailable-no-receipt = o RABBIT não instalou esta cópia
wizard-uninstall-row-unavailable-no-trusted-uninstaller = nenhum desinstalador em que o RABBIT possa confiar
wizard-uninstall-row-unavailable-unsafe-path = um arquivo que o RABBIT nunca remove
# macOS: the app has to be moved to the Trash in Finder. Name the Trash and
# Finder as macOS does in your language.
wizard-uninstall-row-unavailable-by-hand-macos = mova-o para o Lixo no Finder
# The details text of an Uninstall row that can't be ticked. $package is the
# localized package name, $path a file path, $reapack_package the name ReaPack
# knows the package by, and $reason an error message. Keep file paths as they are.
wizard-uninstall-details-reapack = O ReaPack gerencia { $package }: ele instalou { $path } como { $reapack_package }. Em vez disso, desinstale-o pelo ReaPack.
wizard-uninstall-details-reapack-check-failed = O RABBIT não conseguiu verificar os registros do ReaPack sobre { $path } ({ $reason }), então não mexe em { $package }.
wizard-uninstall-details-portable = { $package } atende a todas as cópias do REAPER neste computador, então o RABBIT o remove somente da instalação padrão do REAPER, não de um REAPER portátil.
wizard-uninstall-details-no-receipt = O RABBIT não instalou esta cópia de { $package }, então o mais provável é que ela seja sua, e o RABBIT não mexe nela.
wizard-uninstall-details-no-trusted-uninstaller = O RABBIT não encontrou nenhum desinstalador de { $package } em que possa confiar.
wizard-uninstall-details-unsafe-path = O RABBIT não vai remover { $path }. Ele prefere não mexer em { $package } a remover só uma parte dele.
# The details text of an Uninstall row whose package RABBIT leaves alone
# because of the uninstaller folder and registry entry its installer left:
# RABBIT doesn't trust what it found there, or finds none of the package's
# files for this account. The package's steps to remove it by hand follow.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-cleanup-refused = O RABBIT não confia no que encontrou na pasta do desinstalador de { $package } em Arquivos de Programas (Program Files) nem na entrada dele no Registro do Windows, então não mexe em { $package }.
wizard-uninstall-details-no-files-for-this-account = O RABBIT não encontra nenhum dos arquivos de { $package } na sua conta. Se outra conta instalou { $package }, desinstale-o a partir dessa conta; até lá, o RABBIT não mexe na pasta do desinstalador dele em Arquivos de Programas (Program Files) nem na entrada dele no Registro do Windows.
# The details text of a macOS app RABBIT can't move to the Trash, by itself or
# with an administrator's password: first why, then how to move it there in
# Finder. $package is the localized package name, $path the app's full macOS
# path, keep it as it is, and $error macOS's own error message. Name Finder,
# the Trash, File > Get Info and Locked as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does: Finder's
# menus show it only as symbols.
wizard-uninstall-details-by-hand-not-movable-macos = A sua conta não tem permissão para mover { $path }: ele pertence a outra conta ou ao macOS, ou está em uma pasta que você não pode alterar.
wizard-uninstall-details-by-hand-locked-macos = { $path } está bloqueado.
wizard-uninstall-details-by-hand-not-the-app-macos = { $path } não é o aplicativo { $package } que o RABBIT espera encontrar ali, então o RABBIT não mexe nele.
wizard-uninstall-details-by-hand-unchecked-macos = O RABBIT não conseguiu verificar quem tem permissão para mover { $path } ({ $error }).
wizard-uninstall-details-by-hand-no-trash-folder-macos = Só um administrador pode mover { $path }, e o seu Lixo não é uma pasta sua que o RABBIT possa verificar.
wizard-uninstall-details-by-hand-other-disk-macos = Só um administrador pode mover { $path }, e ele está em outro disco, não no mesmo disco do seu Lixo.
wizard-uninstall-details-by-hand-finder-macos = Para remover { $package }, selecione-o no Finder e pressione Command + Delete. O Finder pede o nome e a senha de um administrador, se precisar.
wizard-uninstall-details-by-hand-unlock-macos = Para remover { $package }, selecione-o no Finder e escolha Arquivo > Obter Informações. Desmarque Bloqueado e feche essa janela, depois pressione Command + Delete. O Finder pede o nome e a senha de um administrador, se precisar. Se Bloqueado não estiver marcado, ou se o Finder ainda não conseguir movê-lo, ele tem um bloqueio que o Finder não consegue remover.
wizard-uninstall-details-by-hand-not-the-app-finder-macos = Se for o aplicativo { $package } que você quer remover, selecione-o no Finder e pressione Command + Delete.
# The details text of an Uninstall row that can be ticked, one sentence per
# line. $package is the localized package name, $packages a comma-separated
# list of package names, and $reason an error message.
wizard-uninstall-details-files = O RABBIT move os arquivos para um backup em vez de excluí-los.
wizard-uninstall-details-dependents-stay = Estes pacotes são instalados por cima de { $package } e são mantidos: { $packages }. Eles voltam a funcionar assim que você instalar { $package } novamente.
wizard-uninstall-details-machine-wide = { $package } atende a todas as cópias do REAPER e a todos os outros programas deste computador, então todos eles o perdem.
wizard-uninstall-details-reapack-owned-extensions = O ReaPack instalou estes itens, e eles são mantidos, mas sem atualizações, depois que ele for removido: { $packages }.
wizard-uninstall-details-reapack-records-unreadable = O RABBIT não conseguiu ler os registros do ReaPack para informar o que o ReaPack ainda gerencia ({ $reason }).
wizard-uninstall-details-no-receipt-note = O RABBIT não instalou esta cópia. Ele remove os arquivos que se sabe que { $package } instala.
wizard-uninstall-details-reapack-orphan = O ReaPack instalou { $package } no passado. O ReaPack não está mais instalado, então o RABBIT pode removê-lo; os registros do ReaPack sobre ele são mantidos.
# A note on a package whose installer added files outside REAPER's folder,
# when RABBIT has no record of which ones. $package is the localized
# package name.
wizard-uninstall-details-no-vendor-record = O RABBIT não tem registro de quais arquivos o instalador de { $package } adicionou fora da pasta do REAPER, então todos eles são mantidos.
# The details text of an Uninstall row whose package its own uninstaller
# removes, with administrator rights. The second line follows the reason
# RABBIT can't trust a package's own uninstaller; $package is the localized
# package name.
wizard-uninstall-details-vendor = O Windows pede a sua permissão antes que qualquer coisa seja removida.
wizard-uninstall-details-vendor-by-hand = Você pode desinstalar { $package } por conta própria nas Configurações do Windows, em Aplicativos, Aplicativos instalados.
# The details text of an Uninstall row whose package's installer left an
# uninstaller folder in Program Files and a registry entry, which RABBIT
# removes with administrator rights once the package's files are in the
# backup. It follows wizard-uninstall-details-files, or takes its place
# when none of the package's files are left to move. $package is the
# localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-uninstall-details-machine-cleanup = Só um administrador pode remover a pasta do desinstalador de { $package } em Arquivos de Programas (Program Files) e a entrada dele no Registro do Windows, então o Windows pede a sua permissão antes que qualquer coisa seja removida. Um backup não consegue restaurá-las.
# The details text of an Uninstall row on macOS, by how the package goes:
# deleted for good with an administrator's password; to the Trash, moved as
# you; or into your Trash with an administrator's password, because your
# account may not move it. Name the Trash and Put Back as Finder does in your
# language. The password window may offer Touch ID, so don't say a password
# is the only way. The Trash line follows the package's own sentence, which
# already says that it goes to the Trash, so it only says how to get it back.
# The administrator line follows the package's own sentence, which already
# says a backup can't bring it back, so it doesn't say that again.
wizard-uninstall-details-elevated-macos = O macOS pede uma vez o nome e a senha de um administrador, antes que qualquer coisa seja removida. Para tê-lo de volta, você precisa instalá-lo novamente.
wizard-uninstall-details-trash = Enquanto você não esvaziar o Lixo, pode arrastá-lo de volta de lá.
wizard-uninstall-details-trash-elevated-macos = A sua conta não tem permissão para movê-lo, então o macOS pede uma vez o nome e a senha de um administrador, antes que qualquer coisa seja removida, e depois o RABBIT o move para o seu Lixo. Ele mantém o proprietário que tinha, então o Finder não oferece o comando Colocar de Volta para ele, e esvaziar o Lixo pode pedir a senha de um administrador.
# The one row, without a checkbox, of an Uninstall list with nothing in it,
# and its details text.
wizard-uninstall-empty = O RABBIT não encontrou nada que possa desinstalar neste REAPER.
wizard-uninstall-empty-details = Nenhum software que o RABBIT sabe desinstalar está instalado neste REAPER.
# Stands in for the Uninstall list when RABBIT's records for the REAPER can't
# be read. $error is the error message.
wizard-uninstall-unreadable = O RABBIT não conseguiu ler os registros dele para este REAPER ({ $error }), então não consegue listar o que poderia desinstalar.
# The Uninstall tab's "Also remove settings" option: a heading, the checkbox
# and a read-only note below it. The checkbox can be ticked only while a
# ticked package has settings RABBIT can remove.
wizard-uninstall-settings-heading = Configurações
wizard-uninstall-remove-settings = Remover também as configurações
wizard-uninstall-settings-note-none-ticked = Marque um pacote para ver quais configurações dele o RABBIT pode remover. Se você não marcar esta opção, as configurações são mantidas.
wizard-uninstall-settings-note-none-removable = Nenhum dos pacotes marcados tem configurações que o RABBIT possa remover.
# $settings is one line per ticked package, each a
# wizard-uninstall-settings-package line.
wizard-uninstall-settings-note-removes = Com esta opção marcada, o RABBIT também remove estas configurações. Elas vão para o backup junto com o resto, então o backup pode restaurá-las.
    { $settings }
# $package is the localized package name, and $settings a comma-separated list
# of what goes, such as "your grooves, SWS settings".
wizard-uninstall-settings-package = { $package }: { $settings }
# Said by VoiceOver on macOS when a tick in the Uninstall list makes "Also
# remove settings" usable or unusable: that happens below the list, away from
# the row being ticked. $option is the checkbox's name, as
# wizard-uninstall-remove-settings reads.
wizard-uninstall-settings-available = Agora disponível abaixo da lista: { $option }.
wizard-uninstall-settings-unavailable = Não está mais disponível: { $option }.

# The Backups tab. The label right before its list names the tab first, as
# the other tabs' labels do; screen readers read it when focus enters the
# list. $size is a size, one of the size-* messages, such as "742 MB". The
# -empty form is for a list with no backups.
wizard-backups-list-label = Backups: o que o RABBIT pode restaurar ou excluir, { $size } no total
wizard-backups-list-label-empty = Backups: o que o RABBIT pode restaurar ou excluir
# The one row, without a checkbox, of a Backups list with nothing in it, and
# its details text. On macOS, VoiceOver says the row's sentence after the
# tab's name instead.
wizard-backups-empty = O RABBIT não tem backups deste REAPER.
wizard-backups-empty-details = O RABBIT faz um backup quando desinstala um pacote, quando uma atualização substitui os arquivos de um pacote e quando o mapa de teclas do OSARA substitui o seu. Nada disso aconteceu ainda neste REAPER, ou os backups dele foram excluídos. O que o próprio instalador ou desinstalador de um pacote, o Lixo ou a senha de um administrador alterou nunca tem backup.
# Stands in for the Backups list when RABBIT can't read its backups folder.
# $error is the error message, in English.
wizard-backups-unreadable = O RABBIT não conseguiu ler os backups deste REAPER.
wizard-backups-unreadable-details = O RABBIT não conseguiu ler a pasta de backups dele ({ $error }), então não consegue listar o que poderia restaurar ou excluir.
# The group headings of the Backups list, rows without a checkbox, then the
# details text each shows. The details say what backups are for, and what
# they never hold.
wizard-backups-group-removed = Software que você desinstalou
wizard-backups-group-removed-details = Quando o RABBIT desinstala um pacote, ele move os arquivos para cá em vez de excluí-los. Cada um destes backups é a única cópia. O que o próprio desinstalador de um pacote, o Lixo ou a senha de um administrador removeu não está aqui.
wizard-backups-group-versions = Versões anteriores
wizard-backups-group-versions-details = Quando uma atualização substitui os arquivos de um pacote, o RABBIT guarda aqui os antigos. Atualizações feitas pelo próprio instalador de um pacote, como o do REAPER, não guardam nenhum.
wizard-backups-group-keymaps = Mapas de teclas
wizard-backups-group-keymaps-details = Quando o mapa de teclas do OSARA substitui o seu, o RABBIT guarda aqui o antigo. Normalmente só o mais antigo é o seu.
wizard-backups-group-other = Outros backups
wizard-backups-group-other-details = O RABBIT não consegue restaurá-los aqui. Você pode excluí-los.
# One row of the Backups list, read as a list row. $backup is what the row
# calls the backup (one of the wizard-backups-name-* messages, or a package's
# name and version), $action is action-restore, action-delete,
# action-delete-only-copy or action-keep, $when one of the
# wizard-backups-when-* messages or backup-date-unknown, and $size a size,
# such as "4 MB".
wizard-backups-row = { $backup }: { $action }. { $when }, { $size }
# The row that stands for every old copy of RABBIT's records, read as a list
# row. $action as above, $count a number, the copies, and $size a size.
wizard-backups-row-records = Cópias antigas dos registros do RABBIT: { $action }. { $count ->
    [one] Uma cópia
   *[other] { $count } cópias
}, { $size }
# A backup named in a sentence on Review, so that two backups with the same
# name can be told apart. $backup and $when as in wizard-backups-row.
wizard-backups-row-reference = { $backup } ({ $when })
# Names joined in a sentence: $rest is the names but the last, separated by
# commas, and $last the last one, as in "FFmpeg, ReaPack and SWS".
wizard-backups-list-and = { $rest } e { $last }
# What a row calls a package's backup: its localized name, $package, and the
# version in the backup, $version, a version number.
wizard-backups-name-package = { $package } { $version }
# What a row calls a key map: your own; one whose kind RABBIT didn't record,
# as in backups from earlier versions of RABBIT and those a restore made;
# REAPER's own default keys; an earlier OSARA key map you hadn't changed.
wizard-backups-name-keymap-own = O seu mapa de teclas
wizard-backups-name-keymap = Mapa de teclas
wizard-backups-name-keymap-default = As teclas padrão do REAPER
wizard-backups-name-keymap-osara = Um mapa de teclas anterior do OSARA
# What a row calls the other backups. "RABBIT's records" is RABBIT's list of
# what it installed.
wizard-backups-name-records = Cópias antigas dos registros do RABBIT
wizard-backups-name-records-copy = Uma cópia dos registros do RABBIT
wizard-backups-name-not-a-backup = Não é um backup do RABBIT
wizard-backups-name-deleting = Exclusão não concluída
wizard-backups-name-no-manifest = Backup sem registro do conteúdo
wizard-backups-name-damaged = Backup danificado
wizard-backups-name-before-restore = Arquivos substituídos por uma restauração
wizard-backups-name-update-files = Outros arquivos substituídos por uma atualização
wizard-backups-name-uninstall-files = Outros arquivos movidos por uma desinstalação
wizard-backups-name-keymap-files = Outros arquivos guardados com um mapa de teclas
wizard-backups-name-unknown = Backup de outra versão do RABBIT
# When a backup was made, as its row says it after the action. $date is a
# date and time (backup-date-time), and $version a version number.
# "Replaced by OSARA's" means by OSARA's key map.
wizard-backups-when-uninstalled = Desinstalado em { $date }
wizard-backups-when-replaced-by = Substituído pela versão { $version } em { $date }
wizard-backups-when-replaced = Substituído em { $date }
wizard-backups-when-keymap-osara = Substituído pelo do OSARA em { $date }
wizard-backups-when-keymap-restore = Substituído por uma restauração em { $date }
wizard-backups-when-made = Criado em { $date }
# A date and time, as rows, details and Review say when a backup was made,
# such as "November 9, 2026, 2:05 PM". $year, $month (1 to 12), $day,
# $hour (0 to 23) and $hour12 (1 to 12) are numbers; $minute is two digits,
# such as "05"; $period is am or pm. Use the 24-hour clock with $hour and
# $minute, or the 12-hour one with $hour12, $minute and $period, as your
# language does. Name the month by the select on $month.
backup-date-time = { $day } de { $month ->
    [1] janeiro
    [2] fevereiro
    [3] março
    [4] abril
    [5] maio
    [6] junho
    [7] julho
    [8] agosto
    [9] setembro
    [10] outubro
    [11] novembro
   *[12] dezembro
} de { $year }, { $hour }:{ $minute }
# Instead of a row's when-text, for a backup whose date RABBIT can't tell.
backup-date-unknown = Data desconhecida
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
wizard-backups-unavailable-not-here = não dá para restaurar aqui
wizard-backups-unavailable-installed-again = você tem { $package } de novo
wizard-backups-unavailable-not-complete = parte dele não está no backup
wizard-backups-unavailable-nothing-in-it = nenhum dos arquivos está neste backup
wizard-backups-unavailable-version-unknown = o RABBIT não sabe qual versão era
wizard-backups-unavailable-other-language-pack = outro pacote de idioma está instalado
wizard-backups-unavailable-part-missing = falta uma parte
wizard-backups-unavailable-records-unreadable = o RABBIT não consegue ler os registros dele
wizard-backups-unavailable-not-yours = pertence a outra conta
wizard-backups-unavailable-not-a-backup = o RABBIT não mexe nisso
wizard-backups-unavailable-receipt-in-set = contém uma cópia dos registros do RABBIT
wizard-backups-unavailable-file-refused = o RABBIT não devolve um dos arquivos
wizard-backups-unavailable-cannot-restore = o RABBIT não consegue restaurá-lo
wizard-backups-unavailable-tied = vai junto com um backup que o RABBIT não pode excluir
# The details text of a Backups row, a sentence or two per line, in this
# order: what the backup is; when it was made; what it holds and how big;
# whether it is the only copy; what restoring does, or why it can't; what
# deleting loses. $backup is what the row calls the backup, $package the
# localized package name, $version a version number, $date a date and time
# (backup-date-time), $size a size, such as "4 MB", and $count a number.
#
# What the backup is.
wizard-backups-details-uninstalled = { $backup }, como estava antes de você desinstalar.
wizard-backups-details-earlier = { $backup }, uma versão anterior.
wizard-backups-details-keymap-own = O seu mapa de teclas, de antes de o mapa de teclas do OSARA substituí-lo.
wizard-backups-details-keymap-default = As teclas padrão do REAPER, de antes de o mapa de teclas do OSARA substituí-las.
wizard-backups-details-keymap-osara = Um mapa de teclas anterior do OSARA, de antes de um mais novo substituí-lo.
wizard-backups-details-keymap-unknown = Um mapa de teclas de antes de o do OSARA substituí-lo. Pode ser o seu ou um mapa de teclas do OSARA mais antigo: o RABBIT não registrou qual.
wizard-backups-details-keymap-restore = O mapa de teclas que uma restauração substituiu. Pode ser o seu ou o do OSARA.
wizard-backups-details-keymap-other = Um mapa de teclas que o RABBIT guardou.
wizard-backups-details-records = Cópias da lista do que o RABBIT instalou, de execuções anteriores. Excluí-las não muda nada do que está instalado.
wizard-backups-details-records-copy = Uma cópia da lista do que o RABBIT instalou, de uma execução anterior.
wizard-backups-details-deleting = A exclusão deste backup começou e não terminou, então parte dele pode já ter sumido.
wizard-backups-details-not-a-backup = Isto não é um backup do RABBIT: é um link, ou uma pasta com um nome que o RABBIT nunca dá a um backup.
wizard-backups-details-no-manifest = Um backup sem registro do conteúdo, como os que um RABBIT anterior deixava quando parava antes da hora.
wizard-backups-details-damaged = Um backup cujo registro de conteúdo não pode ser lido.
wizard-backups-details-before-restore = Arquivos que uma restauração substituiu, guardados para que ela possa ser desfeita.
wizard-backups-details-update-files = Arquivos que uma atualização substituiu e que nenhuma outra linha aqui devolve.
wizard-backups-details-uninstall-files = Arquivos que uma desinstalação moveu e que não pertencem a nenhum pacote que ela registrou.
wizard-backups-details-keymap-files = Arquivos guardados com um mapa de teclas que nenhuma outra linha aqui devolve.
wizard-backups-details-unknown = Um backup feito por outra versão do RABBIT, por um motivo que esta versão não conhece.
# When it was made.
wizard-backups-details-when-uninstalled = Você o desinstalou em { $date }.
wizard-backups-details-when-updated-to = Uma atualização para a versão { $version } a substituiu em { $date }.
wizard-backups-details-when-updated = Uma atualização a substituiu em { $date }.
wizard-backups-details-when-restored-to = A volta para a versão { $version } a substituiu em { $date }.
wizard-backups-details-when-restored = Uma restauração a substituiu em { $date }.
wizard-backups-details-when-osara = O mapa de teclas do OSARA o substituiu em { $date }.
wizard-backups-details-when-keymap-restored = Um mapa de teclas restaurado o substituiu em { $date }.
wizard-backups-details-when-made = O RABBIT fez este backup em { $date }.
wizard-backups-details-when-records = A cópia mais recente é de { $date }.
wizard-backups-details-when-unknown = O RABBIT não consegue saber quando este backup foi feito.
# What it holds and how big it is. The settings are named only when the
# uninstall recorded that it moved them; backups from earlier versions of
# RABBIT don't say, so the -older form says "anything else".
wizard-backups-details-holds-with-settings = Ele contém os arquivos e as suas configurações de { $package }, { $size }.
wizard-backups-details-holds-files = Ele contém os arquivos, { $size }.
wizard-backups-details-holds-older = Ele contém os arquivos e tudo mais que aquela desinstalação moveu, { $size }.
wizard-backups-details-holds-replaced = Ele contém os arquivos substituídos, { $size }.
wizard-backups-details-holds-keymap = Ele contém o mapa de teclas, { $size }.
wizard-backups-details-holds-size = Ele ocupa { $size }.
wizard-backups-details-holds-records = { $count ->
    [one] Há uma cópia, { $size }.
   *[other] Há { $count } cópias, { $size } no total.
}
wizard-backups-details-only-copy = É a única cópia.
# What restoring does, for a backup that can be restored. Name REAPER's
# Actions list as REAPER does in your language. The -uninstalled-files form
# is for a backup that names no version, which may hold no record of the
# package to put back, so it says nothing about RABBIT's records.
wizard-backups-details-restore-uninstalled = A restauração devolve os arquivos, e o RABBIT volta a contar { $package } como instalado.
wizard-backups-details-restore-uninstalled-files = A restauração devolve os arquivos.
wizard-backups-details-restore-stopped-partway = A desinstalação parou no meio, então alguns arquivos estão só neste backup. A restauração os devolve.
wizard-backups-details-restore-already-back = { $package } já está de volta. A restauração devolve os arquivos outra vez.
wizard-backups-details-restore-earlier = A restauração volta para a versão { $version } e guarda a versão que você tem agora como backup, para que você possa avançar de novo. As suas configurações continuam como estão.
wizard-backups-details-restore-earlier-copy = A restauração devolve esta cópia e guarda a versão que você tem agora como backup, para que você possa avançar de novo.
wizard-backups-details-restore-earlier-already-back = { $package } já está nesta versão. A restauração devolve os arquivos outra vez.
wizard-backups-details-restore-keymap = A restauração o torna de novo o mapa de teclas do REAPER e guarda o atual como backup. Os scripts que você instalou desde então continuam na lista de ações do REAPER.
wizard-backups-details-restore-default-keys = A restauração traz de volta as teclas próprias do REAPER, não um mapa de teclas seu, e guarda o mapa de teclas atual como backup.
# Why it can't be restored, by what RABBIT found. Name the Uninstall tab as
# wizard-packages-tab-uninstall does.
wizard-backups-details-not-here = O RABBIT não consegue restaurá-lo aqui.
wizard-backups-details-cannot-restore = O RABBIT não consegue restaurá-lo.
wizard-backups-details-status-installed-again = Você tem { $package } de novo. Primeiro desinstale o que você tem, na guia Desinstalar, e depois restaure.
wizard-backups-details-status-not-complete = Parte dele não está no backup, então o RABBIT não consegue voltar para essa versão.
wizard-backups-details-status-nothing-in-it = Nenhum dos arquivos está neste backup, então não há nada para restaurar.
wizard-backups-details-status-version-unknown = O RABBIT não sabe qual versão era, então não consegue voltar para ela.
wizard-backups-details-status-other-language-pack = Outro pacote de idioma está instalado, e instalar um remove o outro, então o RABBIT não traz este de volta enquanto o outro estiver instalado.
wizard-backups-details-status-part-missing = Falta uma parte no backup, então o RABBIT não consegue restaurá-lo.
wizard-backups-details-status-records-unreadable = O RABBIT não consegue ler os registros dele, então não consegue saber se este backup combina com eles.
wizard-backups-details-status-not-yours = Ele pertence a outra conta, então o RABBIT não o restaura nem o exclui.
wizard-backups-details-status-not-a-backup = O RABBIT nunca olha dentro, nem o restaura nem o exclui.
# macOS: how to remove another account's backup by hand. The backups are in
# the hidden Library folder, so the steps go through Finder's Go menu.
# $folder is the folder that holds the backup, a full macOS path, and $name
# the backup's own folder name; keep both as they are. Name Finder, its Go
# menu and the menu's Go to Folder item as Finder does in your language, and
# Command-Delete as Apple's page of Mac keyboard shortcuts does. Never
# suggest sudo.
wizard-backups-details-not-yours-finder-macos = Para removê-lo, no Finder escolha Ir > Ir para a Pasta, digite { $folder }, depois selecione { $name } e pressione Command + Delete. O Finder pede o nome e a senha de um administrador, se precisar.
# What deleting it loses.
wizard-backups-details-delete-uninstalled = Se você excluir este backup, o RABBIT não consegue recuperar esta cópia.
wizard-backups-details-delete-earlier = Se você excluir este backup, não poderá voltar para a versão { $version }.
wizard-backups-details-delete-earlier-copy = Se você excluir este backup, não poderá voltar para esta cópia.
wizard-backups-details-delete-keymap = Se você excluir este backup, o RABBIT não consegue recuperar este mapa de teclas.
wizard-backups-details-delete-already-back = Os arquivos também estão no disco, então excluí-lo não faz perder nada.
wizard-backups-details-delete-other = Se você excluí-lo, ele some para sempre.
wizard-backups-details-delete-deleting = Excluí-lo conclui essa exclusão.
# For rows that go together until a later version of RABBIT: one backup
# holds them all, and RABBIT deletes whole backups. $backups is a list of
# what the other rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-details-together = { $origin ->
    [uninstall] O backup desta desinstalação também contém { $backups }.
    [update] O backup desta atualização também contém { $backups }.
   *[other] O mesmo backup também contém { $backups }.
} Até uma versão futura, o RABBIT os exclui juntos.
# After the sentence above, when one of those rows can't be deleted.
wizard-backups-details-tied = Um deles não pode ser excluído, então o RABBIT também não pode excluir este.
# The choice below the Backups list, Restore or Delete, keeps its own ticks.
# After a switch, VoiceOver on macOS says how many backups are ticked for the
# new choice. $choice is restore or delete, and $count a number.
wizard-backups-ticked-macos = { $choice ->
    [delete] { $count ->
        [0] Nenhum backup marcado para excluir.
        [one] Um backup marcado para excluir.
       *[other] { $count } backups marcados para excluir.
    }
   *[restore] { $count ->
        [0] Nenhum backup marcado para restaurar.
        [one] Um backup marcado para restaurar.
       *[other] { $count } backups marcados para restaurar.
    }
}
# Said by VoiceOver on macOS after "Checked" or "Unchecked", when ticking a
# row to delete ticks or unticks the rows from the same backup too, such as
# "SWS 2.14.0.7 from the same uninstall is ticked too." $backups is a list
# of what those rows call their backups, $count how many there are, a
# number, and $origin uninstall, update or other, for what made the backup.
wizard-backups-also-ticked-macos = Também marcado, { $origin ->
    [uninstall] da mesma desinstalação
    [update] da mesma atualização
   *[other] do mesmo backup
}: { $backups }.
wizard-backups-also-unticked-macos = Também desmarcado, { $origin ->
    [uninstall] da mesma desinstalação
    [update] da mesma atualização
   *[other] do mesmo backup
}: { $backups }.
# Why a restore of a backup is refused, a sentence or two each, about the
# backup, which they call "it". Review puts one after "Can't restore <backup>
# yet.", and a row's details after what the backup is. $package is a
# localized package name, $packages a list of them, $path a file or folder
# as the computer writes it, keep it as it is, $detail an error message in
# English, $reason one of the wizard-restore-target-* phrases, and $programs
# a comma-separated list of program names, never translated. Name the
# Uninstall tab as wizard-packages-tab-uninstall does.
wizard-restore-refusal-invalid-id = O nome dele não é um que o RABBIT dá a um backup.
wizard-restore-refusal-not-found = Ele foi excluído ou nunca existiu. Abra o RABBIT de novo para ver os backups como estão agora.
wizard-restore-refusal-differs-in-case = O RABBIT só o encontra com um nome com outras maiúsculas e minúsculas.
wizard-restore-refusal-ends-in-dot-or-space = O RABBIT só o encontra com um nome sem o ponto ou o espaço no final.
wizard-restore-refusal-being-deleted = A exclusão dele começou e não terminou, então parte dele pode ter sumido.
wizard-restore-refusal-is-link = O RABBIT não usa { $path }: é um link, e o RABBIT nunca segue links nos backups dele.
wizard-restore-refusal-no-manifest = Ele não registra quais dos arquivos pertencem a { $package }.
wizard-restore-refusal-no-package-records = Ele não registra pacotes, então o RABBIT não consegue restaurar só { $package } a partir dele.
wizard-restore-refusal-package-not-in-set = Ele não contém nenhum registro de { $package }.
wizard-restore-refusal-no-keymap-in-set = Ele não contém nenhum mapa de teclas que o RABBIT possa restaurar.
wizard-restore-refusal-damaged-manifest = O RABBIT não consegue ler o registro do conteúdo dele ({ $detail }).
wizard-restore-refusal-records-only = Ele contém só uma cópia dos registros do RABBIT, então não há nada para restaurar.
wizard-restore-refusal-receipt-in-set = Ele contém uma cópia dos registros do RABBIT. Copiá-la de volta desfaria todas as mudanças feitas neles desde então, então o RABBIT não o restaura.
wizard-restore-refusal-reinstalled-since = Você tem { $package } de novo, uma cópia instalada depois deste backup. Primeiro desinstale a que você tem, na guia Desinstalar, e depois restaure.
wizard-restore-refusal-nothing-of-it-in-set = Nenhum dos arquivos de { $packages } está nele, então não há nada para restaurar.
# Instead of the sentence above, when nothing in a backup of earlier
# versions belongs to a package.
wizard-restore-refusal-nothing-of-it-in-set-none = Nada nele pertence a um pacote que o RABBIT possa devolver.
wizard-restore-refusal-part-missing = Falta uma parte: a cópia de { $path } sumiu, ou não é o arquivo registrado.
wizard-restore-refusal-not-complete = Ele não está completo: { $path } não está nem no backup nem no disco como aquela versão o tinha.
wizard-restore-refusal-version-unknown = O RABBIT não sabe qual versão de { $package } ele contém.
wizard-restore-refusal-other-language-pack-installed = O RABBIT não traz { $package } de volta enquanto isto estiver instalado: { $packages }. Instalar um remove o outro.
wizard-restore-refusal-target-refused = O RABBIT não devolve { $path }: { $reason }.
wizard-restore-refusal-newer-file-refused = Voltar moveria { $path }, um arquivo da versão mais nova de { $package }, para um backup de onde ele não poderia voltar: { $reason }.
wizard-restore-refusal-earlier-file-refused = O registro dele daquela versão de { $package } cita { $path }, um lugar onde o RABBIT nunca instala: { $reason }.
wizard-restore-refusal-receipt-unreadable = O RABBIT não consegue ler os registros dele ({ $detail }), então não consegue saber se este backup combina com eles.
wizard-restore-refusal-reaper-running = O REAPER está aberto. Feche-o e tente de novo.
# The same, for packages every REAPER on the computer can use, such as
# app2clap. $packages is a list of localized package names.
wizard-restore-refusal-reaper-running-every = Há um REAPER aberto. Feche todas as cópias do REAPER neste computador, incluindo as portáteis, e tente de novo. Qualquer REAPER pode usar { $packages }.
wizard-restore-refusal-keymap-needs-reaper-closed = Ele devolve o mapa de teclas do REAPER, o que o RABBIT só faz com o REAPER fechado.
wizard-restore-refusal-program-running = Feche estes programas e tente de novo: { $programs }.
wizard-restore-refusal-not-writable = O RABBIT não consegue gravar onde a restauração devolve os arquivos, em { $path } ({ $detail }).
wizard-restore-refusal-lock-held = Outra cópia do RABBIT está trabalhando neste REAPER. Aguarde até que ela termine.
# sudo and root are never translated.
wizard-restore-refusal-as-root = O RABBIT está sendo executado como root, como acontece com sudo, e assim não restaura backups. Encerre o RABBIT e abra-o com a sua própria conta.
wizard-restore-refusal-not-yours = Ele pertence a outra conta, então o RABBIT não o restaura.
wizard-restore-refusal-changed-while-read = Ele mudou enquanto o RABBIT o lia, como quando outra cópia do RABBIT ainda estava gravando nele.
wizard-restore-refusal-changed-since-listed = Ele mudou desde que o RABBIT o listou. Abra o RABBIT de novo para ver os backups como estão agora.
# Why a restore won't put a file back at a place: the phrase that ends
# wizard-restore-refusal-target-refused and the two after it. Start in lower
# case where your language allows. $path is a file or folder, keep it as it
# is; $name a file or folder name; $detail an error message in English.
wizard-restore-target-rabbit-folder = o RABBIT nunca devolve um arquivo para a própria pasta
wizard-restore-target-leads-into-rabbit-folder = o caminho até lá leva para a pasta do RABBIT
wizard-restore-target-name-not-as-written = o nome { $name } no caminho pode abrir outro arquivo ou outra pasta que não os que ele nomeia
wizard-restore-target-place-is-link = é um link, e o RABBIT nunca substitui links
wizard-restore-target-through-link = o caminho passa por { $path }, um link, e o RABBIT nunca segue links
wizard-restore-target-leads-outside-resource-folder = o caminho sai da pasta de recursos, { $path }
wizard-restore-target-leads-outside = o caminho sai de { $path }
wizard-restore-target-outside-allowed-folders = fica fora da pasta de recursos do REAPER e de qualquer outra pasta onde o RABBIT instala
wizard-restore-target-not-plain-path = não é um caminho simples dentro da pasta de recursos
wizard-restore-target-guard-folder-gone = a pasta para onde ele volta, { $path }, não existe mais
wizard-restore-target-could-not-check = o RABBIT não conseguiu verificar { $path } ({ $detail })
wizard-restore-target-two-files-for-one-place = o backup contém mais de um arquivo para este lugar
wizard-restore-target-outside-without-manifest = o backup não registra para onde vai este arquivo de fora da pasta de recursos
wizard-restore-target-copy-outside-set = o registro do backup põe a cópia dele fora do backup
wizard-restore-target-keymap-among-version-files = é o mapa de teclas do REAPER, que o RABBIT só devolve como um mapa de teclas à parte

# $package is the localized package name the configuration step depends on.
wizard-configuration-row-unavailable = Indisponível: requer que { $package } esteja instalado.
wizard-configuration-row-already-applied = Já aplicado neste destino do REAPER.
# Short status tag appended in parentheses to a configuration row's tree label
# when the row isn't actionable. Kept terse so the tree label stays readable;
# the longer sentence in `wizard-configuration-row-unavailable` /
# `wizard-configuration-row-already-applied` is still surfaced in the details
# pane and as the row's accessible reason.
# $reason is one of the "wizard-configuration-row-status-*" strings below.
wizard-configuration-row-summary-suffix = ({ $reason })
# $package is the localized name of the dependency package.
wizard-configuration-row-status-requires = requer { $package }
wizard-configuration-row-status-already-applied = já aplicado
# Names the dependency of a configuration step that any one of several
# packages satisfies, so the step doesn't advertise one arbitrary package.
config-dependency-language-pack = um pacote de idioma
config-reapack-reaper-accessibility-name = Adicionar o repositório ReaPack REAPER Accessibility do Toni
config-reapack-reaper-accessibility-description = Adiciona o repositório ReaPack REAPER Accessibility de Toni Barth (https://github.com/Timtam/reapack/raw/master/index.xml). Depois de adicionado, vá ao menu Extensions, ReaPack, Browse Packages para obter scripts e plug-ins acessíveis extras.
config-reapack-reaper-accessible-fr-name = Adicionar o repositório ReaPack REAPER Accessible (FR)
config-reapack-reaper-accessible-fr-description = Adiciona o repositório ReaPack em francês do REAPER Accessible (https://github.com/reaperaccessible/rap_fr/raw/main/index.xml). Depois de adicionado, vá ao menu Extensions, ReaPack, Browse Packages para obter os recursos em francês do REAPER Accessible.
config-reapack-reaper-accessible-en-name = Adicionar o repositório ReaPack REAPER Accessible (EN)
config-reapack-reaper-accessible-en-description = Adiciona o repositório ReaPack em inglês do REAPER Accessible (https://github.com/reaperaccessible/rap_en/raw/main/index.xml). Depois de adicionado, vá ao menu Extensions, ReaPack, Browse Packages para obter os recursos em inglês do REAPER Accessible.
config-set-reaper-language-name = Definir o idioma do REAPER
config-set-reaper-language-description = Informa ao REAPER qual dos pacotes de idioma instalados ele deve usar, gravando essa escolha no reaper.ini. Sem isso, você teria que selecionar o idioma por conta própria em Options, Preferences, General. Você pode instalar vários pacotes de idioma e alternar entre eles no REAPER depois; isto apenas escolhe qual fica ativo agora. Desmarque se quiser apenas que os arquivos sejam instalados.

wizard-reapack-ack-heading = Aviso de doação do ReaPack
wizard-reapack-ack-body = O ReaPack é um software livre distribuído sob a licença LGPL. O seu autor, Christian Fillion, aceita doações opcionais para apoiar o desenvolvimento contínuo. Christian também mantém a extensão SWS e, no passado, incluiu código especificamente para melhorar a compatibilidade com o OSARA. Qualquer apoio que você puder enviar é muito merecido.
wizard-reapack-ack-link-label = Abrir a página de doação do ReaPack
wizard-reapack-ack-confirm-label = Não doar desta vez, apenas instalar ou atualizar o ReaPack
cli-reapack-ack-prompt-summary = O ReaPack é um software livre (LGPL). O seu autor, Christian Fillion, aceita doações opcionais em https://reapack.com/donate para apoiar o desenvolvimento contínuo.
cli-reapack-ack-flag-required = O ReaPack faz parte do plano desta execução, mas falta a confirmação do aviso de doação. Execute novamente com --accept-reapack-donation-notice para confirmar que você leu https://reapack.com/donate e quer que o RABBIT instale ou atualize o ReaPack.

wizard-version-check-heading = Verificando as versões mais recentes
wizard-version-check-status-pending = Preparando a verificação das versões mais recentes…
# $package is the localized package display name.
wizard-version-check-status-checking = Verificando { $package }…
wizard-version-check-status-progress = Verificando versões… ({ $done } de { $total })
# $error_count is the number of failed checks.
wizard-version-check-status-error = { $error_count } verificação(ões) de versão falhou(aram). Use Voltar para tentar outro destino ou feche o RABBIT.
wizard-version-check-progress-label = Progresso
wizard-version-check-error-heading = Verificações com falha
# $package is the localized package display name; $message is the failure message.
wizard-version-check-error-line = { $package }: { $message }
wizard-package-details-label = Detalhes do pacote
# $package is the localized package display name. Heads the release-notes block in the package details pane.
wizard-package-whats-new-heading = Novidades em { $package }:
wizard-packages-osara-keymap-heading = Mapa de teclas do OSARA
wizard-packages-osara-keymap-replace-label = Substituir o seu mapa de teclas atual pelo mapa de teclas mais recente do OSARA
packages-reaper-language-label = Idioma do REAPER após a instalação
packages-spanish-variant-label = Tradução do OSARA para o espanhol
wizard-packages-osara-keymap-unavailable-note = Selecione o OSARA para configurar o comportamento do mapa de teclas dele.
wizard-packages-osara-keymap-preserve-note = O seu mapa de teclas atual continua como está: o RABBIT não vai mexer no reaper-kb.ini. Para adicionar os novos atalhos do OSARA mantendo os seus, execute depois a ação "Merge OSARA key map" do OSARA (menu Extensions, OSARA).
wizard-packages-osara-keymap-replace-note = O RABBIT fará um backup do seu reaper-kb.ini atual, se existir, e o substituirá pelo mapa de teclas mais recente do OSARA. Os atalhos que você mesmo alterou não são mantidos; o backup os preserva.
wizard-packages-osara-keymap-unchanged-note = O seu mapa de teclas é o mapa de teclas do OSARA, sem nenhuma alteração sua, então o RABBIT fará um backup dele e o substituirá pelo mapa de teclas mais recente do OSARA. Os scripts que você instalou continuam na lista de ações.
wizard-packages-osara-keymap-default-keys-note = Você está usando os atalhos de teclado padrão do REAPER em vez do mapa de teclas do OSARA, então o RABBIT os mantém como estão. Para passar a usar o mapa de teclas do OSARA, marque a caixa acima.
wizard-package-details-handling-prefix = Tratamento
wizard-package-handling-automatic = O RABBIT pode instalar este pacote diretamente.
wizard-package-handling-unattended = O RABBIT pode instalar este pacote sem intervenção, incluindo executar o instalador dele quando necessário.
wizard-package-handling-planned = O RABBIT foi projetado para executar ele mesmo o instalador ou a rotina de configuração deste pacote e concluir a instalação sem intervenção, mas esta versão ainda apenas descreve as etapas em vez de executá-las.
wizard-package-handling-manual = O RABBIT vai baixar este pacote e informar as etapas manuais após a execução.
wizard-package-handling-unavailable = Este pacote não está disponível para a plataforma ou arquitetura selecionada.

# $package is the localized package display name, $action is the localized planned action, $installed is the installed version or unknown, and $available is the available version or unknown.
wizard-package-row = { $package }: { $action }. Você tem { $installed }. A mais recente é { $available }

wizard-review-heading = Revise o que você pediu para o RABBIT fazer
wizard-review-target-prefix = Destino
wizard-review-package-heading = Pacotes selecionados
wizard-review-osara-keymap-heading = Mapa de teclas do OSARA
wizard-review-osara-keymap-preserve = Manter o seu mapa de teclas atual.
wizard-review-osara-keymap-replace = Fazer backup do seu mapa de teclas atual e depois substituí-lo pelo mais recente do OSARA.
wizard-review-osara-keymap-replace-unchanged = Fazer backup do seu mapa de teclas do OSARA sem alterações e depois substituí-lo pelo mais recente do OSARA.
wizard-review-osara-keymap-keep-default-keys = Manter os atalhos de teclado padrão do REAPER.
wizard-review-notes-heading = Observações
wizard-review-preflight-prefix = Ainda não é possível instalar

# $path is the selected REAPER resource path.
wizard-review-target = Destino: { $path }
wizard-review-no-target = Nenhum destino selecionado.
wizard-review-no-package = Nenhum pacote selecionado.

# $package is the localized package display name and $action is the localized planned action.
wizard-review-package = { $package }: { $action }

# Review in uninstall mode. The heading names the mode, so it is the first
# thing read.
wizard-review-uninstall-heading = Pronto para desinstalar
wizard-review-uninstall-packages = Pacotes para desinstalar
wizard-review-uninstall-no-package = Nada está marcado para desinstalar.
# One line per package. $package is the localized package name, $removes the
# sentence on what goes, and $stays the one on what stays, or
# wizard-review-uninstall-settings-removed.
wizard-review-uninstall-package = { $package }: { $removes } { $stays }
# Replaces the sentence on what stays when "Also remove settings" is ticked.
# $settings is a comma-separated list of what goes, such as "your grooves, SWS
# settings".
wizard-review-uninstall-settings-removed = As configurações dele também vão para o backup: { $settings }.
wizard-review-uninstall-close-programs-note = Feche primeiro os outros programas de música.
# Review's permission note, when a ticked package needs administrator rights.
# Then one line per ticked package its own uninstaller removes; $package is
# the localized package name.
wizard-review-uninstall-elevation-note-windows = O Windows pede a sua permissão uma vez, antes que qualquer coisa seja removida. Se você não permitir, nada é desinstalado.
wizard-review-uninstall-vendor-window-note = Se aparecer uma janela do próprio desinstalador de { $package }, responda a ela.
# Review's permission note on macOS, when a ticked package needs an
# administrator. $packages is a list of package names. osascript is a program
# name, never translated: macOS names it in the password window. The window
# may offer Touch ID, so don't say a password is the only way. Use the same
# words for 'administrator command' as in the Done and progress lines that
# name it.
wizard-review-uninstall-elevation-note-macos = Antes que qualquer coisa seja removida, o macOS pede uma vez o nome e a senha de um administrador. Ele pode oferecer o Touch ID no lugar da senha. A janela pode mencionar o osascript, a ferramenta que o RABBIT usa para pedir. Com essa senha, o RABBIT executa um único comando de administrador que desinstala { $packages }. Se você cancelar a janela, nada é desinstalado.
# One line per ticked package that goes to the Trash; the second when your
# account may not move it, so an administrator's password moves it there.
# $package is the localized package name. Name the Trash and Put Back as
# Finder does in your language.
wizard-review-uninstall-trash-note = { $package } vai para o Lixo. Enquanto você não esvaziar o Lixo, pode arrastá-lo de volta de lá.
wizard-review-uninstall-trash-elevated-note-macos = { $package } vai para o seu Lixo com a senha do administrador. Ele mantém o proprietário que tinha, então o Finder não oferece o comando Colocar de Volta para ele, e esvaziar o Lixo pode pedir a senha de um administrador.
# What is ticked on the tab that isn't running stays ticked, but isn't part of
# this run. RABBIT leaves out what it ticked by itself, such as updates, but the
# list can still hold a setting RABBIT ticked along with one of the user's
# ticks, so the note doesn't say who ticked it. $packages is a comma-separated
# list of names: packages, and for the Install or update tab also settings,
# such as "Set REAPER's language".
wizard-review-inactive-tab-note-install = O que está marcado na guia Instalar ou atualizar não faz parte desta execução: { $packages }.
wizard-review-inactive-tab-note-uninstall = O que está marcado na guia Desinstalar não faz parte desta execução: { $packages }.
wizard-review-uninstall-reinstall-hint = Você pode instalá-los novamente mais tarde na guia Instalar ou atualizar.
# Why the Uninstall button is disabled. $reason is a sentence, such as
# wizard-review-uninstall-not-writable.
wizard-review-uninstall-blocked = Ainda não é possível desinstalar: { $reason }
wizard-review-uninstall-not-writable = O RABBIT não consegue gravar na pasta deste REAPER.

# Review before a restore from the Backups tab. The heading names what the
# run does, so it is the first thing read.
wizard-review-restore-heading = Pronto para restaurar
# One line per ticked backup. $backup is what its row calls it, $package the
# localized package name, $date a date and time (backup-date-time), and
# $settings yes or no: whether the backup holds the package's settings too.
wizard-review-restore-uninstalled = { $settings ->
    [yes] { $backup } volta como estava em { $date }, com as suas configurações de { $package }.
   *[no] { $backup } volta como estava em { $date }.
}
wizard-review-restore-uninstalled-undated = { $settings ->
    [yes] { $backup } volta como estava antes da desinstalação, com as suas configurações de { $package }.
   *[no] { $backup } volta como estava antes da desinstalação.
}
# An earlier version. $version is the version it goes back to, and $current
# the one it has now, both version numbers.
wizard-review-restore-earlier = { $package } volta da versão { $current } para a versão { $version }. O RABBIT guarda a versão { $current } como backup.
wizard-review-restore-earlier-to = { $package } volta para a versão { $version }. O RABBIT guarda a versão que você tem agora como backup.
wizard-review-restore-earlier-copy = { $package } volta para a cópia que uma atualização substituiu. O RABBIT guarda a versão que você tem agora como backup.
# A key map. $origin is own for your own key map, default-keys for REAPER's
# default keys, and other for any other; $date as above.
wizard-review-restore-keymap = { $origin ->
    [own] O seu mapa de teclas de { $date } substitui o atual, que o RABBIT guarda como backup.
    [default-keys] As teclas padrão do REAPER de { $date } substituem o mapa de teclas atual, que o RABBIT guarda como backup.
   *[other] O mapa de teclas de { $date } substitui o atual, que o RABBIT guarda como backup.
}
wizard-review-restore-keymap-undated = { $origin ->
    [own] O seu mapa de teclas substitui o atual, que o RABBIT guarda como backup.
    [default-keys] As teclas padrão do REAPER substituem o mapa de teclas atual, que o RABBIT guarda como backup.
   *[other] Este mapa de teclas substitui o atual, que o RABBIT guarda como backup.
}
# Notes on what the restore changes besides the files. $file is a language
# pack's file name, such as de_DE.ReaperLangPack, never translated; $package
# is the localized package name.
wizard-review-restore-language-back = O idioma do REAPER volta a ser { $file }.
wizard-review-restore-language-stays = O idioma do REAPER continua como está, porque outro foi escolhido desde então.
wizard-review-restore-not-counted = O RABBIT deixa de contar { $package } como instalado por ele.
wizard-review-restore-needs-reinstall = Para desinstalar { $package } com o RABBIT depois, instale-o de novo antes.
# When a REAPER that has to be closed is running; the second when a ticked
# package serves every REAPER on the computer.
wizard-review-restore-close-reaper = Feche o REAPER antes.
wizard-review-restore-close-every-reaper = Feche todas as cópias do REAPER antes.
# The backups ticked for the other choice below the list stay ticked, but
# aren't part of this run. $count is a number.
wizard-review-restore-other-choice-note = { $count ->
    [one] Um backup marcado para excluir não faz parte desta execução.
   *[other] { $count } backups marcados para excluir não fazem parte desta execução.
}
# Why the Restore button is disabled. $reason is a sentence, such as
# wizard-review-restore-nothing-ticked. The -backup form names a backup that
# can't come back: $backup as in wizard-backups-row-reference, and $reason
# one of the wizard-restore-refusal-* sentences.
wizard-review-restore-blocked = Ainda não é possível restaurar: { $reason }
wizard-review-restore-blocked-backup = Ainda não é possível restaurar { $backup }. { $reason }
wizard-review-restore-nothing-ticked = Nada está marcado para restaurar.
wizard-review-restore-not-writable = O RABBIT não consegue gravar na pasta deste REAPER.
# $first and $second name two ticked backups, as in
# wizard-backups-row-reference; $package is the localized package name.
wizard-review-restore-same-package = { $first } e { $second } são backups de { $package }. Marque só um deles.
wizard-review-restore-same-file = { $first } e { $second } devolvem o mesmo arquivo. Marque só um deles.
# When RABBIT couldn't check the restore before it starts: one backup, or
# all of them. $error is the error message, in English.
wizard-review-restore-plan-failed = O RABBIT não conseguiu verificá-lo ({ $error }).
wizard-review-restore-check-failed = O RABBIT não conseguiu verificar a restauração ({ $error }).
# When RABBIT hasn't checked the backups ticked now, so the restore can't
# start yet.
wizard-review-restore-not-checked = O RABBIT não verificou o que está marcado.
# Review before a delete from the Backups tab.
wizard-review-delete-heading = Pronto para excluir backups
# $count is a number, the ticked backups, and $size a size, such as "520 MB".
wizard-review-delete-intro = { $count ->
    [one] Este backup é excluído para sempre e libera { $size }:
   *[other] Estes backups são excluídos para sempre e liberam { $size }:
}
# One line per ticked backup, saying what is lost with it. $backup is what
# its row calls it, and $date a date and time (backup-date-time).
wizard-review-delete-uninstalled = { $backup }, desinstalado em { $date }: a única cópia.
wizard-review-delete-keymap = { $backup } de { $date }: a única cópia.
wizard-review-delete-only-copy = { $backup }: a única cópia.
wizard-review-delete-earlier = { $backup }: depois disso, você não poderá voltar para ela.
wizard-review-delete-already-back = { $backup }: os arquivos também estão no disco, então nada se perde.
wizard-review-delete-records = { $backup }: nada do que está instalado muda.
wizard-review-delete-deleting = { $backup }: isto conclui uma exclusão que começou antes.
wizard-review-delete-other = { $backup }: este backup some para sempre.
# Until a later version, backups from one set go together. $backups is a
# list of what their rows call them, $count how many there are, a number, 2
# or more, $origin uninstall, update or other, for what made the backup, and
# $date a date and time (backup-date-time). The -undated form is for a
# backup whose date RABBIT can't tell.
wizard-review-delete-together = { $origin ->
    [uninstall] { $backups }, da desinstalação de { $date }, vão juntos
    [update] { $backups }, da atualização de { $date }, vão juntos
   *[other] { $backups }, de { $date }, vão juntos
}: { $count ->
    [2] um único backup contém os dois.
   *[other] um único backup contém todos os { $count }.
}
wizard-review-delete-together-undated = { $count ->
    [2] { $backups } vão juntos: um único backup contém os dois.
   *[other] { $backups } vão juntos: um único backup contém todos os { $count }.
}
# $count is a number.
wizard-review-delete-other-choice-note = { $count ->
    [one] Um backup marcado para restaurar não faz parte desta execução.
   *[other] { $count } backups marcados para restaurar não fazem parte desta execução.
}
# Why the Delete button is disabled. $reason is a sentence.
wizard-review-delete-blocked = Ainda não é possível excluir: { $reason }
wizard-review-delete-nothing-ticked = Nada está marcado para excluir.

wizard-progress-heading = Progresso da instalação
wizard-progress-status-idle = Pronto para instalar.
wizard-progress-status-running = Instalando os pacotes selecionados. Isso pode levar alguns minutos.
# Shown from the moment the user confirms the stop until the pipeline finishes
# the step it is on. Not "stopped": nothing has stopped yet when it appears.
wizard-progress-status-cancelling = Interrompendo. O RABBIT está concluindo a etapa que começou e depois será fechado.
wizard-progress-details-label = Detalhes do progresso
wizard-progress-details-idle = Nenhuma instalação em andamento.
wizard-progress-details-starting = Iniciando a operação de configuração.
wizard-progress-details-cache-prefix = Cache
# The progress page while an uninstall runs: its heading, the first status
# line, the first line of the log, and the status line once the user asked to
# stop. An uninstall stops once the step it is on has finished: one package,
# or on macOS the administrator command, which removes several at once.
wizard-progress-uninstall-heading = Desinstalando
wizard-progress-status-uninstall-running = Desinstalando os pacotes marcados.
wizard-progress-details-uninstall-starting = Iniciando a desinstalação.
wizard-progress-status-uninstall-cancelling = Interrompendo. O RABBIT está concluindo a etapa que começou e depois será fechado.

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Baixando { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Baixando { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Baixando { $count } pacotes… { $downloaded } / { $total }
wizard-progress-status-installing = Instalando { $package }…
wizard-progress-status-removing = Desinstalando { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Aplicando etapa de configuração: { $step }

# Running log lines appended to the progress details text control.
# $package is the localized package display name and $step the localized
# configuration step name.
wizard-progress-log-download-started = Baixando { $package }…
wizard-progress-log-download-completed = { $package } baixado.
wizard-progress-log-install-started = Instalando { $package }…
wizard-progress-log-install-completed = { $package } instalado.
wizard-progress-log-remove-started = Desinstalando { $package }…
wizard-progress-log-remove-completed = { $package } desinstalado.
wizard-progress-log-remove-failed = Não foi possível desinstalar { $package }.
# For a package its own uninstaller removes: the status and log line while
# it runs, the line said once when it runs for a long time, and the line
# said once when its files are still there long after it ended. $package
# is the localized package name. Name Alt+Tab as Windows does.
wizard-progress-status-vendor-running = Executando o próprio desinstalador de { $package }…
wizard-progress-status-vendor-still-waiting = Ainda aguardando o próprio desinstalador de { $package }. Ele pode estar mostrando uma pergunta em outra janela; pressione Alt+Tab para encontrá-la.
wizard-progress-status-vendor-waiting-for-files = O próprio desinstalador de { $package } terminou, e o RABBIT está aguardando que os arquivos dele sejam removidos. Se outro programa de música estiver com { $package } aberto, feche esse programa.
# On macOS, said right before the administrator password window appears, and
# again if it has been open a while without an answer. RABBIT puts each on the
# status line and in the log too, in case VoiceOver cuts it off. $packages is
# a list of package names. Use the same words for 'administrator command' as
# in the Done and progress lines that name it.
wizard-progress-status-admin-prompt-macos = O macOS agora pede uma vez o nome e a senha de um administrador. Com eles, o RABBIT executa um único comando de administrador que desinstala { $packages }. Se você cancelar, nada é desinstalado.
wizard-progress-status-admin-still-waiting-macos = Ainda aguardando a janela de senha de administrador. Se você não a ouvir, verifique se ela está atrás do RABBIT.
# While a package goes to the Trash, and in the log once the administrator
# command has worked on a package. $package is the localized package name.
# Name the Trash as Finder does.
wizard-progress-status-trashing = Movendo { $package } para o Lixo…
wizard-progress-log-admin-done = O comando de administrador terminou com { $package }.
wizard-progress-log-configuration-started = Aplicando { $step }…
wizard-progress-log-configuration-completed = { $step } aplicado.

wizard-done-heading = Concluído
wizard-done-status-idle = Nenhuma instalação foi executada a partir desta janela ainda.
wizard-done-status-success = O RABBIT terminou de fazer a sua mágica! Confira os detalhes abaixo.
wizard-done-status-error = A instalação falhou. Confira o erro abaixo.
wizard-reaper-running-title = O REAPER ainda está aberto
wizard-reaper-running-body = Feche o REAPER e pressione Instalar novamente. O RABBIT não consegue atualizar o REAPER nem as extensões dele enquanto o REAPER estiver em execução.
wizard-error-reaper-running = O RABBIT parou porque o REAPER estava aberto. Feche o REAPER e execute o RABBIT novamente.
# Why a run stopped when another RABBIT is installing or uninstalling in the
# same REAPER.
wizard-error-lock-held = Outra cópia do RABBIT está instalando ou desinstalando neste REAPER. Aguarde até que ela termine e execute o RABBIT novamente.
# Shown instead of starting an uninstall while REAPER is open. Name the
# button as wizard-button-uninstall does.
wizard-reaper-running-uninstall-body = Feche o REAPER e pressione Desinstalar novamente. O RABBIT não consegue desinstalar nada do REAPER enquanto o REAPER estiver em execução.
# The same, for packages every REAPER on the computer can load, such as
# app2clap. $packages is a comma-separated list of localized package names.
wizard-reaper-running-uninstall-every-body = Feche todas as cópias do REAPER neste computador, incluindo as portáteis, e pressione Desinstalar novamente. Todas as cópias do REAPER neste computador podem usar { $packages }.
# The same when REAPER itself is the only such package: its own uninstaller
# needs every REAPER closed.
wizard-reaper-running-uninstall-reaper-body = Feche todas as cópias do REAPER neste computador, incluindo as portáteis, e pressione Desinstalar novamente. O próprio desinstalador do REAPER não é executado enquanto houver alguma cópia do REAPER aberta.
# The same on macOS, when RABBIT's administrator command is to move REAPER's
# app: it can't tell one REAPER from another.
wizard-reaper-running-uninstall-reaper-body-macos = Feche todas as cópias do REAPER neste computador, incluindo as portáteis, e pressione Desinstalar novamente. O comando de administrador que move o REAPER não consegue distinguir uma cópia do REAPER de outra, então todas elas precisam estar fechadas.
# Shown instead of starting an uninstall while a program that has to be
# closed first is running. $programs is a comma-separated list of program
# names, such as "Surge XT.exe"; they are never translated.
wizard-programs-running-uninstall-title = Feche primeiro estes programas
wizard-programs-running-uninstall-body = Feche { $programs } e pressione Desinstalar novamente.
# Why an uninstall stopped, on the Done page.
wizard-error-uninstall-any-reaper-running = O RABBIT parou porque uma cópia do REAPER estava aberta. Feche todas as cópias do REAPER neste computador, incluindo as portáteis, e execute o RABBIT novamente.
wizard-error-uninstall-not-enough-space = O RABBIT parou porque não há espaço livre suficiente para o backup. Libere espaço na unidade onde fica a pasta de recursos do REAPER e execute o RABBIT novamente.
# $package is the localized package name, and $path the full path of the
# file another program has open.
wizard-error-uninstall-file-in-use = { $package } não foi desinstalado, porque outro programa está usando { $path }. Feche esse programa e execute o RABBIT novamente.
# The same, for a file a package's installer added outside REAPER's folder.
# Only the JAWS scripts' installer adds such files, and JAWS itself may have
# one open; the program to close would be the user's screen reader, so this
# says to wait and try again instead. $package is the localized package
# name, and $path the file's full path as Windows holds it. Keep JAWS as it
# is.
wizard-error-uninstall-vendor-file-in-use = { $package } não foi desinstalado, porque o JAWS ou outro programa está usando { $path }. Aguarde um momento e execute o RABBIT novamente. Se isso não resolver, reinicie primeiro o JAWS.
# Why an uninstall didn't start on macOS: RABBIT itself runs as root, as under
# sudo. sudo and root are never translated.
wizard-error-uninstall-as-root = O RABBIT está sendo executado como root, como acontece com sudo, então ele não desinstala nada. Encerre o RABBIT e abra-o com a sua própria conta. Ele mesmo pede a senha de um administrador quando precisa dela.
wizard-done-status-completed-with-errors = A instalação foi concluída com erros. Confira os detalhes abaixo.
wizard-done-status-cancelled = Você interrompeu a instalação. A lista abaixo mostra o que o RABBIT concluiu antes de parar.
wizard-done-status-no-packages = Nenhum pacote foi selecionado para instalação ou atualização.
# The Done page after an uninstall. Its first line, one of the five below,
# also ends the progress page. $packages is a comma-separated list of the
# localized names of the packages that were uninstalled.
wizard-done-uninstall-status-success = O RABBIT desinstalou { $packages }.
wizard-done-uninstall-status-completed-with-errors = O RABBIT desinstalou { $packages }, mas nem tudo saiu como planejado.
wizard-done-uninstall-status-error = A desinstalação falhou.
wizard-done-uninstall-status-cancelled = Você interrompeu a desinstalação. Os pacotes aos quais o RABBIT ainda não tinha chegado continuam instalados.
wizard-done-uninstall-status-no-packages = Nada foi desinstalado.
# The Done page's first line when a removal finishes only after a restart,
# and when the Windows permission prompt was declined.
wizard-done-uninstall-status-pending-restart = O RABBIT desinstalou { $packages }. Reinicie o computador para concluir.
wizard-done-uninstall-status-declined = Nada foi desinstalado, porque o pedido de permissão do Windows foi recusado.
# The same on macOS, where the administrator password window was cancelled.
wizard-done-uninstall-status-declined-macos = Nada foi desinstalado, porque a janela de senha de administrador foi cancelada.
# The Done page's first line when the administrator command removed only part
# of a package and nothing else was uninstalled. $packages is a list of
# package names.
wizard-done-uninstall-status-partly-removed = O RABBIT removeu apenas parte de { $packages }.
wizard-done-uninstall-removed-before-stop = Antes de você interromper a desinstalação, o RABBIT desinstalou { $packages }.
# There is no way back to the package list from the Done page. Name the tab
# as wizard-packages-tab-install does.
wizard-done-uninstall-reinstall-hint = Para recuperar um deles, execute o RABBIT novamente e marque-o na guia Instalar ou atualizar.
# What to look at after an uninstall, one sentence per package. $package is
# the localized package name. Name "Show details" as wizard-done-show-details
# does.
wizard-done-uninstall-not-installed = { $package } já não estava instalado, então não havia nada para remover.
wizard-done-uninstall-left-in-backup = Alguns arquivos de { $package } não puderam ser restaurados e continuam no backup. O botão Mostrar detalhes lista esses arquivos.
wizard-done-uninstall-leftovers = O RABBIT ainda encontra partes de { $package } e não mexe nelas. O botão Mostrar detalhes indica quais são.
wizard-done-uninstall-language-reset-failed = O RABBIT não conseguiu voltar a deixar o REAPER em inglês depois de remover { $package }. Escolha um idioma nas preferências do REAPER.
wizard-done-uninstall-registry-failed = O RABBIT desinstalou { $package }, mas a lista de programas instalados do Windows ainda pode mostrá-lo. O botão Mostrar detalhes indica a entrada.
# After a package's own uninstaller ran, or was to run. $package is the
# localized package name, $code a number, $error an error message in
# English, $path a full path, and $paths a comma-separated list of full
# paths. Name "Show details" as wizard-done-show-details does, and Alt+Tab
# as Windows does.
wizard-done-uninstall-pending-restart = Reinicie o computador para concluir a remoção de { $package }.
wizard-done-uninstall-vendor-cancelled = O próprio desinstalador de { $package } foi cancelado antes de terminar.
wizard-done-uninstall-vendor-failed = O próprio desinstalador de { $package } terminou com o código de erro { $code }. O botão Mostrar detalhes lista o que ainda está lá.
wizard-done-uninstall-vendor-failed-unsure = O próprio desinstalador de { $package } terminou com o código de erro { $code }, então { $package } pode não ter sido removido por completo.
wizard-done-uninstall-vendor-still-there = O próprio desinstalador de { $package } terminou, mas { $package } ainda está instalado. Você pode desinstalá-lo nas Configurações do Windows, em Aplicativos, Aplicativos instalados.
wizard-done-uninstall-vendor-still-open = O próprio desinstalador de { $package } ainda está aberto, então o RABBIT parou e não mexeu nos pacotes seguintes. Pressione Alt+Tab para encontrar a janela dele e responda a ela.
wizard-done-uninstall-vendor-run-failed = O RABBIT não conseguiu concluir a execução do próprio desinstalador de { $package } ({ $error }), então parou aí.
wizard-done-uninstall-elevation-failed = O RABBIT não conseguiu obter direitos de administrador ({ $error }), então nada foi desinstalado.
wizard-done-uninstall-resource-damaged = Importante: depois que o próprio desinstalador de { $package } foi executado, estes arquivos estavam faltando na pasta de recursos do REAPER: { $paths }. O RABBIT parou e gravou os registros dele novamente. Ele não consegue restaurar as configurações do REAPER; se você tiver uma cópia do reaper.ini, coloque-a de volta no lugar antes de abrir o REAPER.
wizard-done-uninstall-receipt-not-saved = O RABBIT desinstalou { $package }, mas não conseguiu atualizar os registros dele. Na próxima vez que o RABBIT for executado, ele vai perceber mesmo assim que { $package } não está mais instalado.
wizard-done-uninstall-vendor-lost = O RABBIT perdeu o rastro do próprio desinstalador de { $package } depois que ele foi iniciado. Ele ainda pode estar em execução, então o RABBIT não consegue saber o que ele removeu, e parou aí.
wizard-done-uninstall-vendor-lost-reaper = Quando o desinstalador terminar, verifique se o reaper.ini ainda está na pasta de recursos do REAPER: ele guarda as suas configurações do REAPER.
wizard-done-uninstall-vendor-files-left = O próprio desinstalador de { $package } terminou, mas alguns arquivos dele ainda estão lá. O botão Mostrar detalhes lista esses arquivos.
wizard-done-uninstall-vendor-not-confirmed = O próprio desinstalador de { $package } terminou, mas o RABBIT não consegue confirmar que { $package } foi removido.
wizard-done-uninstall-vendor-not-listed = O Windows não lista mais { $package } entre os programas instalados, então ele também não aparece nas Configurações do Windows.
wizard-done-uninstall-working-folder-left = O RABBIT não conseguiu remover { $path }, a pasta de onde executou o desinstalador de { $package }. Só um administrador pode excluí-la.
wizard-done-uninstall-restart-list-unreadable = O RABBIT não conseguiu ler quais arquivos o Windows remove na próxima vez que o computador for reiniciado, então não sabe se os arquivos que restam de { $package } serão removidos nesse momento.
wizard-done-uninstall-installed-list-unreadable = O RABBIT não conseguiu ler a lista de programas instalados do Windows depois de executar o desinstalador de { $package }, então não pôde verificar se o Windows ainda lista { $package }.
# After the elevated helper was to remove the uninstaller folder and
# registry entry a package's installer left. $package is the localized
# package name, and $error Windows' own error message or RABBIT's in
# English. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does, and "Show
# details" as wizard-done-show-details does.
wizard-done-uninstall-cleanup-refused = O RABBIT verificou novamente a pasta do desinstalador de { $package } em Arquivos de Programas (Program Files) e a entrada dele no Registro do Windows, não confiou no que encontrou e então restaurou o que tinha movido.
wizard-done-uninstall-cleanup-failed = O RABBIT não conseguiu remover por completo a pasta do desinstalador de { $package } e a entrada dele no Registro do Windows ({ $error }), então restaurou o que tinha movido. O botão Mostrar detalhes lista o que já foi removido.
wizard-done-uninstall-cleanup-lost = O RABBIT perdeu o contato com o programa auxiliar dele enquanto removia a pasta do desinstalador de { $package } e a entrada dele no Registro do Windows ({ $error }), então restaurou o que tinha movido. Parte dessa pasta ou a entrada dele no Registro pode já ter sido removida.
# Instead of wizard-done-uninstall-cleanup-failed, when RABBIT removed
# nothing of the folder or the registry entry before it failed, so "Show
# details" lists nothing as gone. $package and $error as above.
wizard-done-uninstall-cleanup-failed-nothing-gone = O RABBIT não conseguiu remover a pasta do desinstalador de { $package } e a entrada dele no Registro do Windows ({ $error }), então restaurou o que tinha movido. Nada dessa pasta nem da entrada dele no Registro foi removido.
# After a macOS removal: to the Trash, or by the administrator command.
# $package is the localized package name, $path a full macOS path, keep it as
# it is, $error macOS's own error message, and $programs a list of program
# names, never translated. Name the Trash, Put Back and Finder as Finder does,
# "Show details" as wizard-done-show-details does, and System Settings >
# Privacy & Security > App Management as macOS names it in your language.
wizard-done-uninstall-moved-to-trash = O RABBIT moveu { $package } para o Lixo. Lá, ele está em { $path }. Enquanto você não esvaziar o Lixo, pode arrastá-lo de volta de lá.
wizard-done-uninstall-moved-to-trash-unknown = O RABBIT moveu { $package } para o Lixo. Enquanto você não esvaziar o Lixo, pode arrastá-lo de volta de lá.
wizard-done-uninstall-trash-owner = Ele mantém o proprietário que tinha, então o Finder não oferece o comando Colocar de Volta para ele. Esvaziar o Lixo pode pedir a senha de um administrador, e arrastá-lo de volta também.
wizard-done-uninstall-trash-failed = O RABBIT não conseguiu mover { $package } para o Lixo, então nada nele foi alterado ({ $error }).
wizard-done-uninstall-trash-unclear = O RABBIT pediu ao macOS que movesse { $path } para o Lixo, mas não consegue confirmar que ele esteja lá agora. Procure { $package } no Lixo e na pasta dele no Finder. O RABBIT parou aí.
wizard-done-uninstall-admin-step-failed = O comando de administrador parou em { $path }, então nada de { $package } foi removido. O RABBIT parou aí.
wizard-done-uninstall-admin-delete-failed = O comando de administrador parou em { $path }, que agora pode estar presente só em parte. Nada mais de { $package } foi removido. O RABBIT parou aí.
wizard-done-uninstall-admin-stopped = O comando de administrador terminou sem dizer onde parou, e nada de { $package } foi removido. O RABBIT parou aí.
wizard-done-uninstall-partly-removed = O comando de administrador parou em { $path }, então { $package } foi removido só em parte. O botão Mostrar detalhes lista o que foi removido e o que ainda está lá. O RABBIT parou aí.
wizard-done-uninstall-partly-removed-unclear = O comando de administrador terminou sem dizer onde parou, e { $package } foi removido só em parte. O botão Mostrar detalhes lista o que foi removido e o que ainda está lá. O RABBIT parou aí.
wizard-done-uninstall-admin-left = O comando de administrador foi executado, mas { $path } ainda está lá, ou o RABBIT não consegue ver que ele foi removido, então { $package } não foi removido por completo. O botão Mostrar detalhes lista o que foi removido e o que ainda está lá. O RABBIT parou aí.
wizard-done-uninstall-app-management = Se o macOS recusou a operação: em Ajustes do Sistema > Privacidade e Segurança > Gerenciamento de App, ative o RABBIT. Depois, encerre o RABBIT, abra-o de novo e desinstale novamente.
wizard-done-uninstall-admin-unclear = O comando de administrador terminou sem dizer como, mas tudo de { $package } foi removido.
wizard-done-uninstall-installer-records-remain = O instalador do macOS ainda lista recibos de instalação de { $package }, embora os arquivos dele tenham sido removidos. Isso não causa nenhum problema; o botão Mostrar detalhes os indica.
wizard-done-uninstall-installer-records-unchecked = O RABBIT não conseguiu verificar se o instalador do macOS esqueceu { $package } ({ $error }). Isso não causa nenhum problema: os arquivos dele foram removidos.
wizard-done-uninstall-program-started = O comando de administrador encontrou um destes programas em execução, ou não conseguiu verificá-los, então não alterou nada: { $programs }. Feche os que estiverem abertos e depois execute o RABBIT novamente.
wizard-done-uninstall-admin-steps-refused = O RABBIT não conseguiu montar o comando de administrador para { $package } ({ $error }), então não pediu nenhuma senha e não desinstalou nada.
wizard-done-show-details = Mostrar detalhes
# Mnemonic messages are single-character native access keys. Choose a character
# from the translated label when possible.
wizard-done-launch-reaper = Abrir o REAPER e fechar o RABBIT
wizard-done-launch-reaper-mnemonic = A
wizard-done-open-resource = Abrir a pasta de recursos (somente para manutenção manual avançada)
wizard-done-open-resource-mnemonic = P
wizard-done-no-reaper-app = Não há nenhum aplicativo REAPER executável conhecido para este destino.
wizard-done-launch-reaper-error-prefix = Não foi possível iniciar o REAPER
wizard-done-open-resource-error-prefix = Não foi possível abrir a pasta de recursos
wizard-done-self-update-apply-running = Aplicando a atualização do RABBIT…
wizard-done-self-update-error-prefix = A atualização automática do RABBIT falhou
wizard-done-self-update-relaunch-prefix = RABBIT reiniciado
wizard-self-update-status-checking = Procurando atualizações do RABBIT…

# Asked when the user closes the window while an install is running. The No
# button is the focused one, so a stray Enter never ends a running install.
wizard-close-during-install-title = Interromper a instalação?
wizard-close-during-install-body = O RABBIT está instalando. Se você interromper agora, o RABBIT conclui a etapa que começou, mantém o que já foi instalado, desfaz o que havia começado e então é fechado. Os pacotes aos quais ele ainda não chegou continuam sem ser instalados. Interromper a instalação?

# The same question while an uninstall runs. No is the focused button here
# too.
wizard-close-during-uninstall-title = Interromper a desinstalação?
wizard-close-during-uninstall-body = O RABBIT está desinstalando. Se você interromper agora, o RABBIT conclui a etapa que começou, depois para e é fechado. Os pacotes aos quais ele ainda não chegou continuam instalados. Interromper a desinstalação?

# Asked before an uninstall that a backup can't undo, or that a package's
# manifest asks to confirm. The No button is the focused one, so Enter never
# starts it.
wizard-confirm-uninstall-title = Desinstalar os pacotes marcados?
# $package is the localized package name.
wizard-confirm-uninstall-unbacked = Um backup não consegue restaurar { $package }. Para tê-lo de volta, você precisa instalá-lo novamente.
# Instead of the sentence above, for a package whose files go into a backup
# while RABBIT removes its uninstaller folder in Program Files and its
# registry entry with administrator rights. Review shows this sentence too,
# or the one above when none of the package's files are left to move.
# $package is the localized package name. Name Program Files as
# uninstall-vendor-refused-not-below-program-files does.
wizard-confirm-uninstall-partly-unbacked = Um backup consegue restaurar os arquivos de { $package }, mas não a pasta do desinstalador dele em Arquivos de Programas (Program Files) nem a entrada dele no Registro do Windows, que o RABBIT remove com direitos de administrador.
# Instead of wizard-confirm-uninstall-unbacked, for a package that goes to
# the Trash; the second when your account may not move it, so an
# administrator's password moves it there. $package is the localized
# package name. Name the Trash as Finder does.
wizard-confirm-uninstall-trash = { $package } vai para o Lixo. Você pode arrastá-lo de volta de lá até esvaziar o Lixo.
wizard-confirm-uninstall-trash-elevated-macos = { $package } vai para o Lixo. Você pode arrastá-lo de volta de lá até esvaziar o Lixo, mas o macOS pode pedir a senha de um administrador para isso.
wizard-confirm-uninstall-question = Desinstalar agora?

# Asked before backups are deleted, every time. The No button is the focused
# one, so Enter never deletes anything. The text is the body, then the
# only-copy sentence when one applies, then the question, as one paragraph.
# $count and $only are numbers: the backups that go, and how many of them
# are the only copy of what they hold. $size is a size, such as "520 MB".
wizard-confirm-delete-backups-title = Excluir backups para sempre?
wizard-confirm-delete-backups-body = O RABBIT exclui { $count ->
    [one] um backup
   *[other] { $count } backups
} e libera { $size }.
wizard-confirm-delete-backups-only-copy = { $only ->
    [one] Um deles é a única cópia do que contém.
   *[other] { $only } deles são a única cópia do que contêm.
}
# Instead of the sentence above, when only one backup goes.
wizard-confirm-delete-backups-only-copy-single = Ele é a única cópia do que contém.
wizard-confirm-delete-backups-question = Um backup excluído não pode ser recuperado. Excluir agora?

# Shown instead when the user closes the window while RABBIT is replacing its
# own files. Not a question: that swap has no safe interruption point.
wizard-close-during-self-update-title = O RABBIT está se atualizando
wizard-close-during-self-update-body = O RABBIT está substituindo os próprios arquivos de programa. Esta etapa não pode ser interrompida sem danificar a instalação. Aguarde a conclusão e depois feche o RABBIT.

# Modal dialog shown once per session when a startup self-update check finds a
# newer release. Title is short; body uses the same { $current } / { $latest }
# placeholders as the status-line variant below.
wizard-self-update-prompt-title = Atualização do RABBIT disponível
wizard-self-update-prompt-body = O RABBIT { $latest } está disponível. Você tem atualmente a versão { $current }. Atualizar agora? O RABBIT será reiniciado quando a atualização terminar.

# Shown in the update prompt when the release notes could be fetched. The
# heading doubles as the accessible name of the read-only notes box, so it
# has to read well on its own; $current is the running version, since the
# notes cover every release published since then.
wizard-self-update-prompt-notes-heading = Novidades desde o RABBIT { $current }:
wizard-self-update-prompt-update-button = Atualizar agora
wizard-self-update-prompt-later-button = Agora não

# Title of the modeless window shown while RABBIT downloads and installs
# its own update. It doubles as the accessible name of the progress log
# inside it, so it has to read well on its own.
wizard-self-update-progress-title = Atualizando o RABBIT

# $current is the running RABBIT version, $latest is the version offered by the
# release manifest, $channel is the release channel id (e.g. "stable").
self-update-status-update-available = Atualização do RABBIT disponível: { $current } → { $latest } (canal { $channel }). Reinicie o RABBIT para ver a pergunta novamente.
self-update-status-up-to-date = O RABBIT está atualizado (versão atual { $current }, canal { $channel }).

# $version is the version that the apply pipeline targeted but did not write.
self-update-apply-no-files-replaced = A atualização automática não substituiu nenhum arquivo (versão de destino { $version }).
# $count is the number of files swapped on disk, $root is the install directory,
# $version is the new RABBIT version that is now in place.
self-update-apply-replaced-summary = { $count } arquivo(s) substituído(s) em { $root }; reinicie o RABBIT para usar a versão { $version }.

# $signed / $unsigned are counts of binaries that produced each verdict.
self-update-apply-signature-summary-signed-only = Verificação de assinatura: { $signed } assinado(s).
self-update-apply-signature-summary-unsigned-only = Verificação de assinatura: { $unsigned } sem assinatura.
self-update-apply-signature-summary-mixed = Verificação de assinatura: { $signed } assinado(s), { $unsigned } sem assinatura.

# $pid is the OS process id of the other RABBIT install holding the lock.
self-update-lock-blocking = Outra instalação do RABBIT está em andamento (PID { $pid }). A aplicação está pausada até que ela termine.

# Summary and report lines shown in the wizard progress/done views and saved outcome reports.
wizard-summary-target = Destino: { $path }
wizard-summary-portable = Destino portátil: { $value }
wizard-summary-dry-run = Simulação: { $value }
wizard-summary-packages-selected = Pacotes selecionados: { $packages }
wizard-summary-cache = Cache: { $path }
wizard-summary-planned-app = Caminho previsto do aplicativo: { $path }
wizard-summary-error = Erro: { $message }
wizard-summary-error-antivirus = O software de segurança do Windows bloqueou este download. Normalmente é um falso positivo: o instalador tem assinatura de código e o RABBIT já o havia verificado com o checksum do fabricante. Às vezes o Defender sinaliza um instalador recém-compilado que ele ainda não viu com frequência — as versões de desenvolvimento (snapshots) do OSARA são o caso mais comum. Para continuar: abra a Segurança do Windows, vá em Proteção contra vírus e ameaças, depois em Histórico de proteção, selecione o item bloqueado e escolha Permitir; em seguida, execute o RABBIT novamente. Você também pode adicionar a pasta de downloads do RABBIT às exclusões, ou instalar este pacote manualmente a partir do site do fabricante. O RABBIT nunca desativa a sua proteção antivírus.
wizard-summary-resource-items-created = Itens de recursos criados: { $count }
wizard-summary-packages-installed-or-checked = Pacotes instalados ou verificados: { $count }
wizard-summary-packages-current = Pacotes já atualizados: { $count }
wizard-summary-packages-manual = Pacotes que exigem atenção manual: { $count }
wizard-summary-backup-files-created = Arquivos de backup criados: { $count }
wizard-summary-backup-file = Arquivo de backup: { $path }
wizard-summary-receipt-backup = Backup do registro: { $path }
wizard-summary-backup-manifest = Manifesto do backup: { $path }
wizard-summary-osara-keymap-merge-hint = O seu mapa de teclas foi mantido. Para adicionar a ele os novos atalhos do OSARA, abra o REAPER e execute a ação "Merge OSARA key map" do OSARA (menu Extensions, OSARA).
wizard-summary-package-message = { $package }: { $message }
# $action is one of the localized "action-*" labels (Install/Update/Keep).
wizard-summary-package-plan-action =   Ação planejada: { $action }
# $status is one of the localized "status-*" labels.
wizard-summary-package-status =   Status: { $status }
# $version is the version RABBIT just installed (or confirmed already current).
wizard-summary-package-installed-version =   Versão instalada: { $version }
# $architecture is the detected REAPER architecture (x64, arm64, …).
wizard-summary-architecture = Arquitetura: { $architecture }
# "Show details" after an uninstall. $value is common-yes or common-no.
wizard-summary-uninstall-remove-settings = Remover também as configurações: { $value }
# One line per file, below its package's line. $path is a full path.
# wizard-summary-left-over can also name a folder, such as a package's
# uninstaller folder in Program Files, or a registry key as regedit shows
# it; keep either as it is, never translated.
wizard-summary-removed-file = Removido: { $path }
wizard-summary-kept-file = Mantido, porque foi alterado desde que o RABBIT o instalou: { $path }
wizard-summary-kept-link = Mantido, porque contém um link que o RABBIT nunca segue: { $path }
wizard-summary-kept-reapack = Mantido, porque o ReaPack o instalou e mantém o controle dele: { $path }
wizard-summary-left-over = Ainda presente: { $path }
wizard-summary-left-in-backup = Não pôde ser restaurado, continua no backup: { $path }
# A file a package's installer added outside REAPER's folder that stays:
# it was there before RABBIT ran that installer, or RABBIT recorded it more
# than once with contents that disagree. "the installer" is the package's
# own installer. $path is a full path as Windows holds it, never
# translated.
wizard-summary-kept-there-before = Mantido, porque já estava lá quando o RABBIT executou o instalador: { $path }
wizard-summary-kept-record-unclear = Mantido, porque os registros do RABBIT sobre ele não coincidem: { $path }
# Below a package its own uninstaller removed: a file Windows removes at the
# next restart, and the uninstaller itself. $path is a full path, never
# translated.
wizard-summary-pending-restart = Será removido quando você reiniciar o computador: { $path }
wizard-summary-vendor-uninstaller = O próprio desinstalador dele: { $path }
# Below a package removed on macOS, one line per path: what went to the Trash
# and where it is there now, and what the administrator command removed; then
# the installer receipts macOS still lists, and macOS's own words when a step
# failed. On Windows, the line for what was removed with administrator rights
# also names a package's uninstaller folder in Program Files, and each file
# in it, that RABBIT removed that way. $path is a full Windows or macOS path,
# keep it as it is; $ids a comma-separated list of receipt names, never
# translated; $error macOS's own message. Name the Trash as Finder does.
wizard-summary-moved-to-trash = Movido para o Lixo: { $path }
wizard-summary-in-trash-as = Agora no Lixo: { $path }
wizard-summary-removed-as-admin = Removido com direitos de administrador: { $path }
wizard-summary-installer-records-remain = Ainda listado pelo instalador do macOS: { $ids }
wizard-summary-macos-said = O macOS informou: { $error }
# What became of a package's entry in Windows' list of installed programs,
# below the package's line. $key is the entry's registry key, such as
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA, and is
# never translated; $error is Windows' own error message.
wizard-summary-registry-removed = Removido da lista de programas instalados do Windows: { $key }
wizard-summary-registry-kept = Ainda na lista de programas instalados do Windows, porque a entrada não aponta para este REAPER: { $key }
wizard-summary-registry-failed = Ainda na lista de programas instalados do Windows, porque o RABBIT não conseguiu ler nem remover a entrada ({ $error }): { $key }
# Below a package whose uninstaller folder and registry entry RABBIT removed
# with administrator rights: the registry entry. $key is the entry's
# registry key as regedit shows it, such as
# HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Reaper_JawsScripts,
# and is never translated. The folder and its files are listed with
# wizard-summary-removed-as-admin.
wizard-summary-registry-key-removed = Removido do Registro do Windows com direitos de administrador: { $key }
# $id is the backup set's name, such as uninstall-unix-1782841261.
wizard-summary-uninstall-backup = Conjunto de backup: { $id }
# One line per uninstalled package. $package is the localized package name,
# and $command the complete rabbit restore-backup command that brings it
# back, which is never translated.
wizard-summary-uninstall-restore = Para restaurar { $package }, feche o REAPER e execute: { $command }
# $path is the full path of the report RABBIT saved.
wizard-summary-uninstall-report = Relatório: { $path }
status-installed-or-checked = Instalado ou verificado
status-planned-unattended = Planejado sem intervenção
status-deferred-unattended = Adiado sem intervenção
status-skipped-current = Ignorado (já atualizado)
status-failed = Falhou
status-skipped-dependency-failed = Ignorado (falha em dependência)
status-cancelled = Não instalado (você interrompeu)
# What became of a package in an uninstall, after its name in "Show details".
status-removed = Desinstalado
status-removed-with-leftovers = Desinstalado, mas uma parte ainda está lá
status-removed-pending-restart = Desinstalado; o resto será removido quando você reiniciar o computador
status-would-remove = Seria desinstalado
status-remove-not-installed = Não estava instalado
status-remove-blocked = Não desinstalado
status-remove-failed = Não foi possível desinstalar
status-remove-cancelled = Ainda instalado (você interrompeu)
status-remove-not-reached = Ainda instalado (a execução parou antes)

# Per-package status messages surfaced on the wizard's Done page next to the
# package name (e.g. "OSARA: <message>"). The wrapper template
# `wizard-summary-package-message` already prefixes the package name, so each
# of these strings is just the message body.
package-status-extension-binary-installed = Binário único da extensão tratado pelo instalador do RABBIT.
# $installed is the on-disk version; $available is the latest upstream version.
package-status-skipped-current = A versão instalada { $installed } é a atual ou mais nova que a versão disponível { $available }.
package-status-skipped-content-unchanged = A cópia instalada é idêntica à publicada na origem.
# $error is the failure text. $dependency is the package (e.g. REAPER) that failed first.
package-status-install-failed = A instalação falhou: { $error }
package-status-skipped-dependency-failed = Ignorado porque { $dependency }, do qual ele depende, não foi instalado com sucesso.
package-status-cancelled = Não instalado: você interrompeu a configuração antes de o RABBIT chegar a este pacote.
# Shown after the package's name when its vendor installer ended with an
# exit code other than 0. $code is the number. One of the
# package-status-vendor-exit-* sentences follows, then
# package-status-vendor-installer-log when the installer left a log.
package-status-vendor-installer-failed = O instalador terminou com o código de saída { $code }.
# Inno Setup's exit code 2: cancelled before it began installing. RABBIT runs
# it unattended, so in practice the user declined the Windows permission
# prompt (User Account Control) the installer raised. "Yes" is the prompt's
# button.
package-status-vendor-exit-cancelled-before-installing = Isso geralmente significa que o pedido de permissão do Windows foi recusado, então nada foi alterado. Execute o RABBIT novamente e escolha Sim quando o Windows pedir permissão.
# Inno Setup's exit code 5: it stopped at a question, such as whether to close
# programs that use its files, that nobody can answer while RABBIT runs it
# unattended. $package is the package's name.
package-status-vendor-exit-stopped-at-question = Isso significa que ele parou em uma pergunta que não pode fazer enquanto o RABBIT o executa sem intervenção, geralmente porque outro programa está com um dos arquivos dele aberto. Feche os outros programas, especialmente outros programas de música e { $package }, e depois execute o RABBIT novamente.
# Inno Setup's exit code 7: its preparation found it can't continue.
package-status-vendor-exit-preparation-failed = Isso significa que, ao se preparar, ele descobriu que não pode instalar agora. Reinicie o computador e depois execute o RABBIT novamente.
# Inno Setup's exit code 8: its preparation found Windows must restart first.
package-status-vendor-exit-restart-needed = Isso significa que, ao se preparar, ele descobriu que o Windows precisa ser reiniciado primeiro. Reinicie o computador e depois execute o RABBIT novamente.
# Any other exit code, or an installer whose codes RABBIT doesn't know.
# $package is the package's name.
package-status-vendor-exit-unknown = Feche os outros programas e execute o RABBIT novamente. Se o instalador parar de novo, instale { $package } a partir do site do fabricante.
# $path is the log file the installer wrote.
package-status-vendor-installer-log = O log do instalador está em { $path }.
# Shown after the package's name when RABBIT didn't start its installer
# because the package's own programs were open. $package is the package's
# name; $programs lists the open programs' file names, such as "Surge XT.exe".
package-status-blocking-programs-running = O RABBIT não iniciou o instalador porque { $package } está aberto ({ $programs }), e o instalador não consegue substituir um programa em execução. Feche-o e depois execute o RABBIT novamente.
# $automation is one of the "package-automation-*" labels (vendor installer / archive extraction / ...).
package-status-dry-run-would-run-unattended = Simulação: o RABBIT baixaria e executaria esta { $automation } sem intervenção.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-staged = Esta versão ainda não implementa a execução sem intervenção planejada para: { $automation }. O RABBIT preparou o arquivo no cache, mas não o executou.
# $automation is one of the "package-automation-*" labels.
package-status-deferred-unattended-not-staged = Esta versão ainda não implementa a execução sem intervenção planejada para: { $automation }. O RABBIT não baixou nem executou o arquivo.
package-status-unattended-installed = O RABBIT executou o instalador original sem intervenção, verificou os caminhos de destino esperados e atualizou o registro do RABBIT.
package-status-osara-unattended-keymap-backed-up = O RABBIT executou o instalador original sem intervenção, fez backup do reaper-kb.ini, aplicou a substituição pelo mapa de teclas do OSARA e atualizou o registro do RABBIT.
package-status-osara-unattended-keymap-replaced = O RABBIT executou o instalador original sem intervenção, aplicou a substituição pelo mapa de teclas do OSARA e atualizou o registro do RABBIT.

# Short automation-kind labels interpolated into the per-package status
# messages above.
package-automation-installer = instalação pelo instalador do fabricante
package-automation-archive = extração de arquivo compactado
package-automation-disk-image = instalação por imagem de disco
package-automation-extension-binary = instalação direta de arquivo

# Per-configuration-step status messages surfaced on the wizard's Done page.
# `wizard-summary-configuration-message = { $step }: { $message }` is the
# wrapper template — the `*-message` keys below are the message body only.
# $name is the human-readable remote name; $url is the index XML URL.
config-message-reapack-remote-already-present = O repositório ReaPack { $name } ({ $url }) já está configurado no reapack.ini.
config-message-reapack-remote-added = Repositório ReaPack { $name } ({ $url }) adicionado ao reapack.ini.
config-message-reapack-remote-created-file = O reapack.ini foi criado com o repositório ReaPack { $name } ({ $url }). O ReaPack adicionará os repositórios padrão dele na próxima vez que o REAPER for aberto.
config-message-reapack-remote-dry-run = Adicionaria o repositório ReaPack { $name } ({ $url }) ao reapack.ini.
config-message-reaper-language-already-selected = O REAPER já estava configurado para usar { $file }.
config-message-reaper-language-selected = Idioma do REAPER definido como { $file }.
config-message-reaper-language-dry-run = Definiria o idioma do REAPER como { $file }.
# $step is the configuration step id (e.g. `reapack-add-reaper-accessibility-remote`).
config-message-skipped = A etapa de configuração { $step } não foi selecionada.
# $step is the configuration step id; $dependency is the dependency package id.
config-message-skipped-dependency-missing = A etapa de configuração { $step } foi ignorada porque o pacote do qual ela depende, { $dependency }, não estava instalado e não faz parte deste plano.
config-message-applied-no-op = Etapa de configuração aplicada sem alterações.
config-message-cancelled = Esta etapa de configuração não foi executada: você interrompeu a configuração antes.

# Per-configuration-step status sub-line on the Done page. Sibling to
# `wizard-summary-package-status` which handles per-package items.
wizard-summary-configuration-message = { $step }: { $message }
wizard-summary-configuration-status =   Status: { $status }

# Configuration step status labels used in the summary's "  Status: …" line.
config-status-applied = Aplicado
config-status-skipped = Ignorado
config-status-skipped-dependency-missing = Ignorado (dependência ausente)
config-status-dry-run = Simulação
config-status-cancelled = Não executado (você interrompeu)
wizard-summary-planned-execution-title = Execução sem intervenção planejada:
wizard-summary-planned-execution-runner =   Executor: { $runner }
wizard-summary-planned-execution-artifact =   Arquivo: { $artifact }
wizard-summary-planned-execution-program =   Programa: { $program }
wizard-summary-planned-execution-program-zipped =   Programa: o instalador dentro do .zip, depois que ele for baixado
wizard-summary-planned-execution-arguments =   Argumentos: { $arguments }
wizard-summary-planned-execution-working-directory =   Diretório de trabalho: { $path }
wizard-summary-planned-execution-verify =   Verificar: { $path }
wizard-summary-manual-title = { $title }:
wizard-summary-manual-step =   { $step }
wizard-summary-manual-note =   Observação: { $note }
wizard-summary-status-finished = Concluído. { $installed } item(ns) de pacote instalado(s) ou verificado(s); { $manual } exige(m) atenção manual.
wizard-summary-status-finished-with-errors = Concluído com erros. { $installed } item(ns) de pacote instalado(s) ou verificado(s); { $failed } falhou(aram).

wizard-planned-runner-launch-installer = Executar o instalador
wizard-planned-runner-extract-archive = Extrair o arquivo compactado e executar o instalador contido nele
wizard-planned-runner-extract-archive-copy-osara = Extrair o arquivo compactado e copiar os arquivos do instalador do OSARA
wizard-planned-runner-mount-disk-image = Montar a imagem de disco e executar o instalador contido nela
wizard-planned-runner-mount-disk-image-copy-app = Montar a imagem de disco e copiar o pacote de aplicativo contido nela
wizard-planned-runner-mount-disk-image-run-pkg = Montar a imagem de disco e executar o instalador pkg contido nela

# Expert mode and release channels. Expert mode is a hidden, documented
# unlock (Ctrl+Shift+E / Cmd+Shift+E, or RABBIT_EXPERT=1) that offers pre-release
# builds of REAPER and OSARA.
package-channel-reaper-dev = Versões de desenvolvimento do landoleet.org
package-channel-osara-pr = Versões de teste de pull requests
wizard-window-title-expert = { $title } – Modo avançado
wizard-expert-enable-title = Ativar o modo avançado?
wizard-expert-enable-body = O modo avançado permite instalar versões de pré-lançamento: as versões de desenvolvimento do REAPER do landoleet.org e as versões de teste de pull requests do OSARA. Elas não têm suporte, podem quebrar o REAPER ou a acessibilidade dele e servem apenas para testes. Duas opções de versão aparecem nesta página, abaixo da instalação do REAPER. Ativar o modo avançado?
wizard-expert-disabled-title = O modo avançado está desativado
wizard-expert-disabled-body = O modo avançado está desativado. Qualquer pacote instalado a partir de uma versão de pré-lançamento volta para a versão normal na próxima vez que você instalar.
wizard-expert-first-page-title = Modo avançado
wizard-expert-first-page-body = O modo avançado só pode ser ativado ou desativado na primeira página, onde ele adiciona as opções de versão. Volte para a primeira página para alterá-lo.
wizard-expert-busy-body = O modo avançado não pode ser alterado enquanto o RABBIT está instalando ou depois que ele terminou.
wizard-expert-reaper-builds-label = Versões do REAPER
wizard-expert-reaper-builds-stable = Versões normais
wizard-expert-osara-builds-label = Versões do OSARA
wizard-expert-osara-builds-snapshot = Snapshots normais
wizard-expert-osara-builds-loading = Procurando versões de pull requests…
wizard-expert-osara-builds-unavailable = Snapshots normais (não foi possível listar as versões de pull requests)
wizard-expert-osara-builds-pr = Pull request { $number }: { $title }
wizard-expert-osara-builds-pr-untitled = Pull request { $number }
wizard-package-row-channel-suffix = { $row } ({ $channel })
wizard-package-channel-dev = versão de desenvolvimento
wizard-package-channel-dev-details = { $package } será instalado a partir das versões de desenvolvimento: versões de pré-lançamento sem suporte, que podem quebrar a qualquer momento.
wizard-package-channel-pr = versão de teste do pull request { $number }
wizard-package-channel-pr-details = { $package } será instalado a partir da versão de teste do pull request { $number }: código não revisado e sem assinatura, que o GitHub apaga após 90 dias.
wizard-package-channel-back = de volta à versão normal
wizard-package-channel-back-details = { $package } está atualmente em uma versão de pré-lançamento. Esta instalação restaura a versão normal.
