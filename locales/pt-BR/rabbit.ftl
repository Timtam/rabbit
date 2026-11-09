app-title = REAPER Accessibility Bootstrap & Bundle Installation Tool
app-short-name = RABBIT

common-yes = sim
common-no = não

action-install = Será instalado
action-update = Será atualizado
action-keep = Não será alterado

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

wizard-packages-heading = Escolha os pacotes
wizard-packages-list-label = Pacotes para instalar ou atualizar
wizard-packages-row-checked = Marcado
wizard-packages-row-unchecked = Desmarcado
wizard-packages-choice-shown = Nova opção abaixo da lista: { $choice }.
wizard-packages-choice-hidden = Opção removida: { $choice }.
wizard-packages-tree-group-label = Pacotes
wizard-additional-software-tree-group-label = Programas adicionais
wizard-language-tree-group-label = Pacotes de idioma
wizard-configuration-tree-group-label = Configuração
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

# Live per-package status line on the progress page.
# $package is the localized package display name (e.g. "REAPER", "OSARA").
wizard-progress-status-downloading = Baixando { $package }…
# $downloaded and $total are human-readable byte counts (e.g. "12.4 MB", "30.0 MB").
wizard-progress-status-downloading-with-bytes = Baixando { $package }… { $downloaded } / { $total }
wizard-progress-status-downloading-many = Baixando { $count } pacotes… { $downloaded } / { $total }
wizard-progress-status-installing = Instalando { $package }…
# $step is the localized configuration step name.
wizard-progress-status-configuring = Aplicando etapa de configuração: { $step }

# Running log lines appended to the progress details text control.
wizard-progress-log-download-started = Baixando { $package }…
wizard-progress-log-download-completed = { $package } baixado.
wizard-progress-log-install-started = Instalando { $package }…
wizard-progress-log-install-completed = { $package } instalado.
wizard-progress-log-configuration-started = Aplicando { $step }…
wizard-progress-log-configuration-completed = { $step } aplicado.

wizard-done-heading = Concluído
wizard-done-status-idle = Nenhuma instalação foi executada a partir desta janela ainda.
wizard-done-status-success = O RABBIT terminou de fazer a sua mágica! Confira os detalhes abaixo.
wizard-done-status-error = A instalação falhou. Confira o erro abaixo.
wizard-reaper-running-title = O REAPER ainda está aberto
wizard-reaper-running-body = Feche o REAPER e pressione Instalar novamente. O RABBIT não consegue atualizar o REAPER nem as extensões dele enquanto o REAPER estiver em execução.
wizard-error-reaper-running = O RABBIT parou porque o REAPER estava aberto. Feche o REAPER e execute o RABBIT novamente.
wizard-done-status-completed-with-errors = A instalação foi concluída com erros. Confira os detalhes abaixo.
wizard-done-status-cancelled = Você interrompeu a instalação. A lista abaixo mostra o que o RABBIT concluiu antes de parar.
wizard-done-status-no-packages = Nenhum pacote foi selecionado para instalação ou atualização.
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
status-installed-or-checked = Instalado ou verificado
status-planned-unattended = Planejado sem intervenção
status-deferred-unattended = Adiado sem intervenção
status-skipped-current = Ignorado (já atualizado)
status-failed = Falhou
status-skipped-dependency-failed = Ignorado (falha em dependência)
status-cancelled = Não instalado (você interrompeu)

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
