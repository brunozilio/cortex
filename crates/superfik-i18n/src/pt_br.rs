//! Português (Brasil).

use superfik_core::{
    Activity, AgentRole, AgentStatus, Command, ContextSlice, Effort, EngineKind, EngineProblem, OutputStyle,
};

use crate::text::plural;
use crate::{Catalog, Text};

const MONTHS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
const WEEKDAYS: [&str; 7] =
    ["Segunda-feira", "Terça-feira", "Quarta-feira", "Quinta-feira", "Sexta-feira", "Sábado", "Domingo"];

#[derive(Debug)]
pub struct Portuguese;

impl Catalog for Portuguese {
    fn text(&self, text: Text<'_>) -> String {
        let fixed = match text {
            Text::BrowserProfiles => "Perfis de laboratório",
            Text::BrowserProfilesCaption => "Sem perfil: as páginas veem este computador como ele é.",
            Text::BrowserProfileNew => "Novo perfil",
            Text::BrowserProfileDefaultName(n) => return format!("Perfil {n}"),
            Text::BrowserProfileReal => "Valores reais",
            Text::BrowserProfileUse => "Usar",
            Text::BrowserProfileEdit => "Editar",
            Text::BrowserProfileDelete => "Excluir",
            Text::BrowserProfileSave => "Salvar perfil",
            Text::BrowserProfileCancel => "Cancelar",
            Text::BrowserProfileRestart => "As alterações salvas entram em vigor após reiniciar o Superfik.",
            Text::BrowserProfileEnabled => "Perfil ativo",
            Text::BrowserProfileCanvas => "Canvas",
            Text::BrowserProfileAudio => "Áudio",
            Text::BrowserProfileHttps => "HTTPS até o proxy",
            Text::BrowserProfileRtcSystem => "Sistema",
            Text::BrowserProfileRtcProxy => "Sem UDP direto",
            Text::BrowserProfileRtcDisabled => "Desligado",
            Text::BrowserProfileDefaultsHint => {
                "Campos vazios mantêm os valores reais deste computador. Separe as fontes por vírgulas."
            }
            Text::BrowserProfilePasswordHint => "Manter a salva",
            Text::BrowserProfileField(field) => match field {
                superfik_core::browser_profiles::ProfileField::Name => "Nome",
                superfik_core::browser_profiles::ProfileField::Cpu => "Threads de CPU",
                superfik_core::browser_profiles::ProfileField::Memory => "Memória (GiB)",
                superfik_core::browser_profiles::ProfileField::Seed => "Semente",
                superfik_core::browser_profiles::ProfileField::GpuVendor => "Fabricante",
                superfik_core::browser_profiles::ProfileField::GpuRenderer => "Renderizador (WebGL)",
                superfik_core::browser_profiles::ProfileField::GpuArchitecture => "Arquitetura (WebGPU)",
                superfik_core::browser_profiles::ProfileField::GpuDevice => "Dispositivo (WebGPU)",
                superfik_core::browser_profiles::ProfileField::GpuDescription => "Descrição (WebGPU)",
                superfik_core::browser_profiles::ProfileField::Fonts => "Fontes locais",
                superfik_core::browser_profiles::ProfileField::Width => "Largura",
                superfik_core::browser_profiles::ProfileField::Height => "Altura",
                superfik_core::browser_profiles::ProfileField::Scale => "Escala (DPR)",
                superfik_core::browser_profiles::ProfileField::Depth => "Profundidade de cor",
                superfik_core::browser_profiles::ProfileField::ProxyHost => "Host",
                superfik_core::browser_profiles::ProfileField::ProxyPort => "Porta",
                superfik_core::browser_profiles::ProfileField::ProxyUsername => "Usuário",
                superfik_core::browser_profiles::ProfileField::ProxyPassword => "Senha",
            },
            Text::BrowserProfileActual => "Valor real",
            Text::BrowserProfileInUse => "Em uso nesta conversa",
            Text::BrowserProfileOff => "Desativado",
            Text::BrowserProfileAllActual => "Nada alterado ainda",
            Text::BrowserProfileNewCaption => "Hardware, tela, GPU e proxy fixos para uma conversa.",
            Text::BrowserProfileCreate => "Criar",
            Text::BrowserProfileHardware => "Hardware",
            Text::BrowserProfileScreen => "Tela",
            Text::BrowserProfileGpu => "GPU",
            Text::BrowserProfileFingerprint => "Impressão digital",
            Text::BrowserProfileProxy => "Proxy",
            Text::BrowserProfileWebRtc => "WebRTC",
            Text::BrowserProfileSeedCaption => "Mantém canvas e áudio estáveis.",
            Text::SettingsMenu => "Ajustes…",
            Text::SettingsGeneral => "Geral",
            Text::SettingsSound => "Som",
            Text::SettingsPlaySounds => "Tocar sons da interface",
            Text::SettingsSoundPack => "Pacote de sons",
            Text::SettingsPulseTrackpad => "Vibrar o trackpad",
            Text::SettingsHapticsCaption => "Um toque leve ao deslizar o medidor de esforço.",
            Text::SettingsFillOnLoad => "Preencher logins ao carregar a página",
            Text::SettingsFillOnLoadCaption => {
                "Quando um único login do cofre combina com a página, ele entra sozinho."
            }
            Text::SettingsSounds => "Sons",
            Text::SettingsHaptics => "Feedback háptico",
            Text::SettingsLanguage => "Idioma",
            Text::SettingsTheme => "Tema",
            Text::SettingsTitle => "Ajustes",
            Text::SettingsGeneralCaption => "Idioma e aparência.",
            Text::SettingsSoundCaption => "Como o Superfik soa e responde enquanto você trabalha.",
            Text::SettingsLanguageCaption => "Menus, rótulos e mensagens.",
            Text::SettingsThemeCaption => "Seguir o sistema, ou ficar sempre claro ou escuro.",
            Text::SettingsPlaySoundsCaption => "Sinais curtos enquanto você trabalha.",
            Text::SettingsSoundPackCaption => "O instrumento que toca os sinais.",
            Text::SettingsVoice => "Voz",
            Text::SettingsVoiceCaption => "Fale com o agente e ouça a resposta. Tudo roda neste computador.",
            Text::SettingsVoiceEnabled => "Ativar voz",
            Text::SettingsVoiceEnabledCaption => "Mostra o microfone no composer.",
            Text::SettingsVoiceChunk => "Tempo de resposta",
            Text::SettingsVoiceChunkCaption => "Menor é mais rápido; maior, mais preciso.",
            Text::SettingsVoiceSpeaker => "Voz do assistente",
            Text::SettingsWakeEnabled => "Palavra de ativação",
            Text::SettingsWakeEnabledCaption => "Ouve o microfone o tempo todo esperando a palavra.",
            Text::SettingsWakeWord => "Palavra",
            Text::SettingsWakeWordCaption => "Diga-a para começar a falar com o agente.",
            Text::SettingsVoiceAddress => "Como te chamar",
            Text::SettingsVoiceAddressCaption => "O agente te chama assim quando fala.",
            Text::VoiceListening => "Ouvindo…",
            Text::VoiceWaiting => "Pensando…",
            Text::VoiceSpeaking => "Falando…",
            Text::VoicePreparing(percent) => return format!("Baixando os modelos de voz… {percent}%"),
            Text::VoiceNoMicrophone => {
                "O Superfik não consegue usar o microfone. Permita em Ajustes do Sistema › Privacidade."
            }
            Text::VoiceFailed(why) => return format!("A voz parou: {why}"),
            Text::SettingsBrowser => "Navegador",
            Text::SettingsBrowserCaption => "Perfis de laboratório e logins salvos para os cards do navegador.",
            Text::AiSubscription => "Assinatura",
            Text::AiApiKey => "Chave de API",
            Text::AiLocalSelect => "Selecionar modelo",
            Text::AiLocalOther => "Outro modelo…",
            Text::AiLocalModels => "Modelos locais",
            Text::AiLocalCaption => "Baixe modelos MLX do Hugging Face e execute neste Mac.",
            Text::AiLocalEmpty => "Nenhum modelo local baixado.",
            Text::AiLocalModelId => "Modelo do Hugging Face",
            Text::AiLocalDownload => "Baixar modelo",
            Text::AiLocalDownloading => "Baixando…",
            Text::AiLocalPreparing => "Preparando o MLX (só na primeira vez)…",
            Text::AiLocalFetching(done, total) => {
                return format!(
                    "Baixando… {}% · {} de {}",
                    crate::text::percent(done, total),
                    crate::text::size(done, ','),
                    crate::text::size(total, ',')
                );
            }
            Text::AiLocalFetched(done) => return format!("Baixando… {}", crate::text::size(done, ',')),
            Text::AiLocalInvalidId => "Informe um modelo do Hugging Face: org/nome.",
            Text::AiConnectionMethod => "Conecte com uma assinatura ou chave de API.",
            Text::SettingsAi => "IA",
            Text::SettingsAiCaption => "Conecte suas contas de IA e gerencie os modelos locais.",
            Text::SettingsIntegrations => "Integrações",
            Text::SettingsIntegrationsCaption => "Conecte os serviços com que seus agentes trabalham.",
            Text::SettingsVaults => "Cofres",
            Text::SettingsVaultsCaption => "Deixe os agentes usarem senhas e tokens sem nunca vê-los.",
            Text::SettingsServer => "Servidor",
            Text::SettingsServerCaption => "Trabalhe em equipe num servidor do Superfik e gerencie quem entra nele.",
            Text::OrganizationMembers => "Membros",
            Text::ServerLocal => "Este computador",
            Text::ServerLocalCaption => {
                "O Superfik roda só aqui. Conecte-se ao servidor da sua equipe para trabalharem juntos."
            }
            Text::ServerConnectHeading => "Conectar a um servidor",
            Text::ServerAddress => "Endereço",
            Text::ServerAddressCaption => "Onde o servidor da equipe responde. Sem http:// ou https://, é https://.",
            Text::ServerUsername => "Usuário",
            Text::ServerPassword => "Senha",
            Text::ServerNewPassword => "Nova senha",
            Text::ServerNewPasswordCaption => "10 caracteres ou mais",
            Text::ServerCode => "Código de primeiro acesso",
            Text::ServerCodeCaption => "O código que um administrador te passou",
            Text::ServerConnect => "Conectar",
            Text::ServerConnecting => "Conectando…",
            Text::ServerFirstAccess => "Primeiro acesso",
            Text::ServerHavePassword => "Já tenho senha",
            Text::ServerAddressInvalid => {
                "Isso não é um endereço de servidor. Escreva um domínio, host:porta, ou um endereço que comece com https:// ou http://."
            }
            Text::ServerInsecure => {
                "Este endereço é http:// simples: sua senha e seu acesso iriam sem criptografia. Conecte mesmo assim só numa rede em que você confia."
            }
            Text::ServerAllowInsecure => "Conectar sem criptografia",
            Text::ServerTryHttp => {
                "Sem esquema, o endereço é tentado por https://. Um servidor instalado sem domínio fala http://: escreva http:// antes do endereço."
            }
            Text::ServerConnectedAs(name, address) => return format!("{name} em {address}"),
            Text::ServerDisconnect => "Desconectar",
            Text::ServerHeading => "Servidor",
            Text::ServerDomain => "Domínio",
            Text::ServerDomainCaption => "Onde os usuários acessam este servidor; vale na hora",
            Text::ServerSave => "Salvar",
            Text::ServerUsers => "Usuários",
            Text::ServerPending => "primeiro acesso pendente",
            Text::ServerNewCode => "Novo código",
            Text::ServerMakeAdmin => "Tornar admin",
            Text::ServerMakeMember => "Tornar membro",
            Text::ServerAddUser => "Adicionar usuário",
            Text::ServerAddUserCaption => "Ele entra uma vez com o código mostrado e escolhe a senha",
            Text::ServerAdd => "Adicionar",
            Text::ServerCodeFor(name) => return format!("Código de primeiro acesso de {name}"),
            Text::ServerCodeShownCaption => "Vale uma vez, por 7 dias. Ele só aparece agora.",
            Text::ServerCopy => "Copiar",
            Text::ServerDone => "Pronto",
            Text::RoleOwner => "Dono",
            Text::RoleAdmin => "Administrador",
            Text::RoleMember => "Membro",
            Text::RoleGuest => "Convidado",
            Text::RemoveMember => "Remover",
            Text::CommentBy(name) => return format!("{name} · comentário"),
            Text::Commented(name) => return format!("{name} deixou um comentário."),
            Text::CommentNeedsOrganization => "Comentários ficam em sessões dos projetos da sua organização.",
            Text::SharedWithTeam(team) => return format!("Compartilhada com {team}."),
            Text::NoSuchTeam(team) => return format!("Nenhum time se chama {team}."),
            Text::TeamsAdminsOnly => "Só donos e administradores criam times.",
            Text::OrganizationTeams => "Times",
            Text::TeamMembers(n) => return plural(n, "membro", "membros"),
            Text::TeamEdit => "Membros",
            Text::TeamDone => "Pronto",
            Text::TeamWho(team) => return format!("Quem está em {team}"),
            Text::TeamHint => "Crie um com /team <nome>; compartilhe uma sessão com /share <time>",
            Text::OrganizationRequired => {
                "Sua empresa configurou este app para o servidor dela. Conecte-se em Ajustes › Servidor."
            }
            Text::UnlockReason => "desbloquear o Superfik",
            Text::LockTitle => "O Superfik está bloqueado",
            Text::LockCaption => "Desbloqueie para continuar de onde parou.",
            Text::UnlockWith(method) => return format!("Desbloquear com {method}"),
            Text::LockCancelled => "O desbloqueio foi cancelado.",
            Text::LockUnavailable => "Este computador não tem como desbloquear: o bloqueio foi deixado de lado.",
            Text::SettingsLock(method) => return format!("Bloquear com {method}"),
            Text::SettingsLockCaption(method) => return format!("Pede o {method} sempre que o Superfik abre."),
            Text::UpdateRequiredTitle => "Atualização necessária",
            Text::UpdateRequiredCaption(version) => return format!("O Superfik {version} não é mais compatível."),
            Text::UpdateAvailableTitle => "Atualização disponível",
            Text::UpdateAvailableCaption(version) => return format!("O Superfik {version} está pronto para baixar."),
            Text::UpdateButton => "Atualizar",
            Text::UpdatingTitle => "Atualizando o Superfik",
            Text::UpdatingCaption => "Seus agentes continuam trabalhando o tempo todo.",
            Text::UpdateChecking => "Procurando atualizações",
            Text::UpdateStarting => "Iniciando o download",
            Text::UpdateVerifying => "Verificando",
            Text::UpdateInstalling => "Instalando",
            Text::UpdatePercent(percent) => return format!("{percent}%"),
            Text::UpdateReadyTitle => "Atualização pronta",
            Text::UpdateReadyCaption => "Reinicie para concluir. Leva alguns segundos.",
            Text::UpdateVersion(version) => return format!("Superfik {version}"),
            Text::UpdateRestart => "Reiniciar para atualizar",
            Text::UpdateFailedTitle => "A atualização não terminou",
            Text::UpdateRetry => "Tentar de novo",
            Text::LinkCopied => "Copiado",
            Text::SettingsLogins => "Importar logins",
            Text::ClearCookies => "Limpar cookies",
            Text::ClearCookiesCaption => "Sai de todos os sites nos cards.",
            Text::ClearCookiesButton => "Limpar",
            Text::ClearingCookies => "Limpando",
            Text::CookiesCleared => "Limpo",
            Text::LoginsImporting => "Importando",
            Text::LoginsImported => "Importado",
            Text::LoginsImportAgain => "Importar de novo",
            Text::LoginsInUse => "Sessões novas já entram com estes logins.",
            Text::LoginsImport => "Importar",
            Text::LoginsNoBrowser => "Nenhum navegador para importar",
            Text::LoginsNoBrowserCaption => "Chrome, Dia, Arc, Brave, Edge, Chromium ou Vivaldi.",
            Text::ThemeSystem => "Sistema",
            Text::ThemeLight => "Claro",
            Text::ThemeDark => "Escuro",
            Text::SettingsOutputStyle => "Estilo das respostas",
            Text::SettingsOutputStyleCaption => "Como o agente escreve.",
            Text::SettingsSuggestions => "Sugerir o próximo prompt",
            Text::SettingsSuggestionsCaption => "Após cada resposta, em cinza; Tab aceita.",
            Text::SettingsSafetyCheck => "Verificar ações autônomas",
            Text::SettingsSafetyCheckCaption => "Segura edições e comandos arriscados.",
            Text::SettingsHeadsUp => "Vale saber",
            Text::SettingsHeadsUpCaption => "Aponta o que importa após um trabalho longo.",
            Text::OutputStyleName(style) => match style {
                OutputStyle::Default => "Padrão",
                OutputStyle::Concise => "Conciso",
                OutputStyle::Proactive => "Proativo",
                OutputStyle::Explanatory => "Explicativo",
                OutputStyle::Learning => "Aprendizado",
            },
            Text::NewSession => "Nova sessão",
            Text::ArchivedSessions => "Arquivadas",
            Text::AskAllow => "Permitir",
            Text::AskAlways => "Sempre permitir",
            Text::AskDeny => "Negar",
            Text::AskNext => "Próxima",
            Text::AskBack => "Voltar",
            Text::AskAnswer => "Responder",
            Text::AskSkip => "Pular",
            Text::AskPickMany => "Escolha quantas quiser",
            Text::PermissionAsk => "Pedir antes",
            Text::PermissionEdits => "Aceitar edições",
            Text::PermissionPlan => "Só planejar",
            Text::PermissionBypass => "Acesso total",
            Text::AskPermission(tool) => return format!("{tool} pede sua permissão"),
            Text::AskOf(n, total) => return format!("{n} de {total}"),
            Text::Research => "Pesquisa",
            Text::Context => "Contexto",
            Text::ContextMeasuring => "Medindo…",
            Text::ContextEmpty => "Sem contexto ainda",
            Text::Memory => "Memória",
            Text::MemoryEpisodic => "Episódica",
            Text::MemorySemantic => "Semântica",
            Text::MemoryProcedural => "Procedural",
            Text::SearchMemory => "Buscar na memória",
            Text::MemoryEmpty => "Nada lembrado ainda",
            Text::MemoryNoMatch => "Nada corresponde",
            Text::Remember => "Lembrar",
            Text::RememberPlaceholder => "Algo que vale lembrar…",
            Text::Forget => "Esquecer",
            Text::OptimizeMemory => "Otimizar",
            Text::OptimizingMemory => "Otimizando…",
            Text::UndoOptimize => "Desfazer",
            Text::CompactContext => "Compactar contexto",
            Text::Compacting => "Compactando…",
            Text::ContextFree(tokens) => return format!("{tokens} livres"),
            Text::MemoryActivated(n) => return format!("{n} ativadas"),
            Text::ComposerEmpty => "Pergunte algo, ou digite / para skills, plugins e comandos.",
            Text::ComposerFollowUp => "Continue a conversa...",
            Text::References => "Referências",
            Text::Notes => "Notas",
            Text::NewNote => "Nova nota",
            Text::NotePlaceholder => "Título, depois linhas; - [ ] vira item",
            Text::NoteMore(n) => return format!("+{n} linhas"),
            Text::Skills => "Skills",
            Text::Repository => "Repositório",
            Text::OpenRepositoryTitle => "Abra um repositório",
            Text::OpenRepositoryHint => "Escolha a pasta de trabalho",
            Text::AddRepository => "Adicionar repositório",
            Text::PullRequests => "Pull requests",
            Text::PullRequest => "Pull request",
            Text::ReviewApproved => "Aprovado",
            Text::ReviewChangesRequested => "Mudanças pedidas",
            Text::ReviewRequired => "Aguardando review",
            Text::OpenPullRequest => "Abrir pull request",
            Text::PullRequestSheetTitle => "Abrir um pull request",
            Text::PullRequestTitlePlaceholder => "Título",
            Text::PullRequestBodyPlaceholder => "Descreva o que mudou e por quê…",
            Text::WriteWithAgent => "Escrever com agente",
            Text::DraftToggle => "Rascunho",
            Text::CreatePullRequest => "Criar pull request",
            Text::Cancel => "Cancelar",
            Text::ReviewBack => "Voltar ao canvas",
            Text::ReviewStacked => "Empilhado",
            Text::ReviewSplit => "Lado a lado",
            Text::ReviewAddComment => "Comentar",
            Text::ReviewAskAgent => "Perguntar ao agente",
            Text::ReviewApprove => "Aprovar",
            Text::ReviewRequestChanges => "Pedir mudanças",
            Text::ReviewPublishSheetTitle => "Publicar review",
            Text::ReviewPublish => "Publicar",
            Text::ReviewSummaryPlaceholder => "Resumo (opcional)…",
            Text::ReviewPending => "Pendente",
            Text::ReviewResolved => "Resolvido",
            Text::ReviewAgentBadge => "Agente",
            Text::ReviewSave => "Salvar",
            Text::ReviewNoFiles => "Nenhum arquivo alterado",
            Text::ReviewWithAgent => "Revisar com agente",
            Text::PublishingReview => "Publicando o review…",
            Text::NoLoginForm => "Esta página não tem um formulário de login para preencher.",
            Text::NotNow => "Agora não",
            Text::PasskeyElsewhere => "Usar outro dispositivo",
            Text::PasskeySaved(account) => return format!("Passkey salva em {account}."),
            Text::SavePasskeyTo(account) => return format!("Salvar passkey em {account}"),
            Text::LoginPageChanged => "A página mudou antes de o login ser preenchido.",
            Text::NoSavedLogin => "Nenhum login do cofre está salvo para esta página.",
            Text::NoWebPages => "Páginas web ainda não estão disponíveis neste sistema.",
            Text::NoModels => {
                "Nenhum modelo ainda. Conecte uma conta de IA ou instale um modelo local em Ajustes › IA."
            }
            Text::CreatingPullRequest => "Abrindo o pull request…",
            Text::NoPullRequests => "Nenhum pull request aberto",
            Text::NoSkills => "Nenhuma skill",
            Text::Commands => "Comandos",
            Text::AllSkillsAdded => "Todas as skills adicionadas",
            Text::Draft => "Rascunho",
            Text::OpenRepositoryMenu => "Abrir repositório…",
            Text::ChooseRepositoryMenu => "Escolher repositório…",
            Text::OpenPullRequestMenu => "Abrir pull request…",
            Text::GitHubSignIn => "Entre com `gh auth login` para listar o GitHub",
            Text::GitHubSignInHere => "Entrar com GitHub",
            Text::GitHubCode(code) => return format!("Digite {code} no GitHub para entrar."),
            Text::Private => "Privado",
            Text::NotInstalled => "Não instalado",
            Text::Soon => "Em breve",
            Text::ChooseRepositoryToStart => "Escolha um repositório para começar.",
            Text::NothingToCompact => "Nada para compactar agora.",
            Text::NothingToAsk => "Nada foi dito nesta sessão ainda; pergunte direto ao agente.",
            Text::Recap(text) => return format!("Onde você parou: {text}"),
            Text::HeadsUp { urgent, title, learn, explain } => {
                let tag = if urgent { "Atenção" } else { "Vale saber" };
                return format!("{tag} · {title}\n{learn}\n\n{explain}");
            }
            Text::BusySwitch => "Espere as sessões em andamento terminarem antes de trocar de branch.",
            Text::DialogOpenRepository => "Abrir um repositório",
            Text::DialogAddReferences => "Adicionar referências",
            Text::DialogAttachRepository => "Anexar um repositório",
            Text::ImagesFilter => "Imagens",
            Text::Plugins => "Plugins",
            Text::Connections => "Plugins",
            Text::InstallPlugin => "Instalar",
            Text::UninstallPlugin => "Remover plugin",
            Text::AddAccount => "Adicionar conta",
            Text::AccountNotUsed => "Não usar",
            Text::SignInWithBrowser => "Entrar pelo navegador",
            Text::Browser => "Navegador",
            Text::BrowserAddress => "Pesquise ou digite um endereço",
            Text::BrowserAgent => "Agente",
            Text::RoutinesTitle => "Rotinas",
            Text::RoutinesCaption => {
                "Agentes que trabalham sozinhos, numa agenda, e prestam contas aqui. Neste computador rodam \
                 enquanto o Superfik está aberto; num servidor, o tempo todo."
            }
            Text::RoutinesHeading => "Rotinas",
            Text::RoutinesNone => "Nenhuma rotina ainda",
            Text::RoutineNew => "Nova rotina",
            Text::RoutineRunNow => "Rodar agora",
            Text::RoutinePause => "Pausar",
            Text::RoutineResume => "Retomar",
            Text::RoutineEdit => "Editar",
            Text::RoutineDelete => "Apagar",
            Text::RoutineName => "Nome",
            Text::RoutineNamePlaceholder => "Triagem da manhã",
            Text::RoutineBrief => "O que fazer",
            Text::RoutineBriefPlaceholder => "Veja os novos erros do Sentry e proponha uma correção para cada um.",
            Text::RoutineWhen => "Quando",
            Text::CadenceHourly => "De hora em hora",
            Text::CadenceDaily => "Todo dia",
            Text::CadenceWeekly => "Toda semana",
            Text::RoutineAutonomy => "Autonomia",
            Text::AutonomyDraft => "Propor",
            Text::AutonomyBounded => "Editar arquivos",
            Text::AutonomyAuto => "Agir sozinho",
            Text::RoutineSave => "Salvar",
            Text::RoutineCancel => "Cancelar",
            Text::RoutineEvery(60) => "A cada hora",
            Text::RoutineEvery(n) => return format!("A cada {n} minutos"),
            Text::RoutineDaily(time) => return format!("Todo dia às {time}"),
            Text::RoutineWeekly(day, time) => {
                return format!("Toda semana, {} às {time}", self.weekday(day).to_lowercase());
            }
            Text::RoutinePaused => "Pausada",
            Text::ReportsHeading => "Relatórios",
            Text::ReportsNone => "Nada rodou ainda",
            Text::ReportRunning => "Rodando",
            Text::ReportFailed => "Falhou",
            Text::ReportStopped => "Parou",
            Text::ReportNeedsInput => "Precisa de você",
            Text::ReportContinue => "Continuar numa sessão",
            Text::ReportBranch(branch) => return format!("No branch {branch}"),
            Text::RoutineStartFrom => "Começar de",
            Text::RoutineAccounts => "Contas que pode usar",
            Text::RoutineAccountsNone => "Conecte Sentry, Linear, GitHub ou Stripe em Ajustes › Integrações.",
            Text::TemplateProductionUpkeep => "Sustentação",
            Text::TemplateReleaseNotes => "Notas de versão",
            Text::TemplateRevenueDigest => "Resumo de receita",
            Text::TemplateCustomerSupport => "Atendimento",
            Text::PairWithQr => "Parear com QR code",
            Text::ChannelOpening => "Conectando…",
            Text::ChannelPairing => {
                "No celular: WhatsApp › Dispositivos conectados › Conectar um dispositivo, e escaneie."
            }
            Text::ChannelLinked(account) => return format!("Conectado como {account}"),
            Text::ChannelLoggedOut => "Desconectado pelo celular. Remova e adicione de novo para parear.",
            Text::ChannelFailed(reason) => return format!("Não foi possível conectar: {reason}"),
            Text::ChannelWebhook(path) => return format!("Webhook: endereço do seu servidor + {path}"),
            Text::CopyWebhook => "Copiar",
            Text::DraftsHeading => "Aguardando aprovação",
            Text::DraftSend => "Enviar",
            Text::DraftDiscard => "Descartar",
            Text::RoutineAnswers => "Responder mensagens",
            Text::RoutineAnswersDetail => "Roda assim que uma das suas contas de canal recebe uma mensagem",
            Text::RoutineWebhook => "Webhook",
            Text::RoutineWebhookDetail => "Roda quando outro serviço chama o endereço dela no seu servidor",
            Text::RoutineWebhookCall => "Como chamar",
            Text::RoutineCopyCall => "Copiar chamada",
            Text::UseCliLogin => "Usar o login do CLI",
            Text::AccountName => "Nome da conta",
            Text::AccountNamePlaceholder => "trabalho, pessoal…",
            Text::Connect => "Conectar",
            Text::Disconnect => "Desconectar",
            Text::NoAccounts => "Nenhuma conta conectada",
            Text::PluginAccounts => "Contas",
            Text::AddPluginFolder => "Adicionar de uma pasta…",
            Text::DialogAddPlugin => "Escolher a pasta do plugin",
            Text::SigningIn => "Aguardando o navegador…",
            Text::PluginReady => "Pronto: escolha com / no composer",
            Text::StoreInstalled => "Instalados",
            Text::CategoryDevelopment => "Ferramentas de desenvolvimento",
            Text::CategoryProductivity => "Produtividade",
            Text::CategoryData => "Dados e pagamentos",
            Text::CategoryWeb => "Web e navegador",
            Text::CategoryOther => "Outros",
            Text::WelcomeAboard => "Bem-vindo a bordo",
            Text::GetStarted => "Primeiros passos",
            Text::GetStartedCanvas => "Contexto num canvas",
            Text::GetStartedCanvasBody => "Veja tudo o que o agente vê, ao redor do composer.",
            Text::GetStartedAgents => "Agentes à vista",
            Text::GetStartedAgentsBody => "Acompanhe cada sub-agente ao vivo, passo a passo.",
            Text::GetStartedMemory => "Memória que fica",
            Text::GetStartedMemoryBody => "Cada projeto lembrado entre sessões.",
            Text::GetStartedReview => "Revise sem sair daqui",
            Text::GetStartedReviewBody => "Leia diffs e revise pull requests, com ou sem agente.",
            Text::Continue => "Continuar",
            Text::AgentTitle(n) => return format!("Agente {n}"),
            Text::Steps(n) => return plural(n, "passo", "passos"),
            Text::ReadingFile(path) => return format!("Lendo {path}"),
            Text::SaveLoginTo(account) => return format!("Salvar em {account}"),
            Text::LoginSaved(account) => return format!("Login salvo em {account}."),
            Text::ReadFiles(n) => return format!("Leu {}", plural(n, "arquivo", "arquivos")),
            Text::UsageLimitResets(time) => return format!("Limite de uso atingido · volta às {time}"),
            Text::RetryTurn => "Tentar de novo",
            Text::Recentre => "Centralizar",
            Text::UsageLimitReached => "Limite de uso atingido",
            Text::ServiceUnavailable => "O serviço está indisponível agora",
            Text::OpeningRepository(slug) => return format!("Abrindo {slug}…"),
            Text::ContinuingShared(owner) => return format!("Continuando a sessão de {owner} numa cópia sua."),
            Text::SharedWithYou => "Compartilhadas com você",
            Text::AskApprover => "Pedir a um admin",
            Text::WaitingForApprover => "Esperando um admin. Você ainda pode responder.",
            Text::DelegatedAllowed(by) => return format!("{by} permitiu."),
            Text::DelegatedDenied(by) => return format!("{by} negou."),
            Text::ApprovalWaiting(name) => {
                return format!("{name} espera uma aprovação em Ajustes › Servidor.");
            }
            Text::WaitingForYourApproval => "Esperando sua aprovação",
            Text::MemoryShared => "Compartilhada com todos neste projeto. Os agentes deles vão lembrar disso.",
            Text::PublishHow => "Diga qual skill ou agente seu: /publish <nome>.",
            Text::Reconnecting => "A conexão com o servidor caiu. Reconectando…",
            Text::ServerSignedOut => "O servidor não aceita mais este login. Entre de novo.",
            Text::PublishNotAllowed => "Só donos e administradores publicam na biblioteca da organização.",
            Text::Published(name) => return format!("Publicando {name} na biblioteca da organização…"),
            Text::OrganizationLibrary => "Biblioteca",
            Text::Here(names) => return format!("{names} aqui"),
            Text::PolicyUnpriced(model) => {
                return format!(
                    "O custo de {model} é desconhecido. Escolha um modelo com tarifa revisada para usar o orçamento da organização."
                );
            }
            Text::PolicyModel(model) => return format!("Sua organização não permite {model}. Escolha outro modelo."),
            Text::PolicyMode(mode) => return format!("Sua organização permite no máximo “{mode}”."),
            Text::PolicyBudget(n) => {
                return format!("Você chegou ao orçamento deste mês, US$ {n}. Um administrador pode aumentá-lo.");
            }
            Text::OrganizationPolicy => "Política",
            Text::PolicyMaxMode => "O máximo que um agente faz sozinho",
            Text::PolicyMaxModeCaption => "Ninguém escolhe um modo mais solto",
            Text::PolicyAnyMode => "Qualquer",
            Text::PolicyBudgetTitle => "Orçamento mensal por membro",
            Text::PolicyBudgetCaption => "As execuções param quando um membro o gasta",
            Text::PolicyUnlimited => "Nenhum",
            Text::PolicyModelCaption => "Permitido para membros",
            Text::UsageThisMonth => "Gasto neste mês",
            Text::UsageLine(dollars, runs) => {
                return format!("{dollars} · {}", plural(runs, "execução", "execuções"));
            }
            Text::LibrarySkillBy(name) => return format!("Skill · publicada por {name}"),
            Text::LibraryAgentBy(name) => return format!("Agente · publicado por {name}"),
            Text::CheckingOutPullRequest(n) => return format!("Abrindo #{n}…"),
            Text::LoadingPullRequest(n) => return format!("Carregando #{n}…"),
            Text::ReviewPublished(n) => return format!("Review publicado em #{n}."),
            Text::AgentReviewReady(n) => {
                return format!("O agente deixou {} para aprovar.", plural(n, "comentário", "comentários"));
            }
        };
        fixed.to_owned()
    }

    fn activity(&self, activity: &Activity, elapsed: &str) -> String {
        match activity {
            Activity::Thinking => "Pensando".into(),
            Activity::SearchingWeb => "Pesquisando na web".into(),
            Activity::Reading => "Lendo arquivos".into(),
            Activity::Writing => "Escrevendo".into(),
            Activity::Done => format!("Trabalhou por {elapsed}"),
            Activity::Failed(_) => "Parou com um erro".into(),
            Activity::Cancelled => "Interrompido".into(),
        }
    }

    fn today(&self) -> &'static str {
        "Hoje"
    }

    fn yesterday(&self) -> &'static str {
        "Ontem"
    }

    fn weekday(&self, index: usize) -> &'static str {
        WEEKDAYS[index % 7]
    }

    fn date(&self, month: u32, day: u32, year: Option<i64>) -> String {
        let month = MONTHS[(month.clamp(1, 12) - 1) as usize];
        match year {
            Some(year) => format!("{day} de {month} de {year}"),
            None => format!("{day} de {month}"),
        }
    }

    fn effort(&self, effort: Effort) -> &'static str {
        match effort {
            Effort::Low => "Baixo",
            Effort::Medium => "Médio",
            Effort::High => "Alto",
            Effort::XHigh => "Extra",
            Effort::Max => "Ultracode",
        }
    }

    fn command(&self, command: Command) -> &'static str {
        match command {
            Command::New | Command::Clear => "Inicia uma nova sessão",
            Command::Context => "Mostra a janela de contexto",
            Command::Memory => "Mostra a memória do projeto",
            Command::Notes => "Mostra as notas do projeto",
            Command::Skills => "Mostra as skills",
            Command::Plugins => "Mostra os plugins",
            Command::Commands => "Mostra esta lista",
            Command::Repository => "Escolhe o repositório",
            Command::Model => "Escolhe o modelo",
            Command::Compact => "Compacta o contexto",
            Command::Mode => "Escolhe o modo de permissão",
            Command::Upload => "Anexa arquivos",
            Command::Effort(_) => "Define o esforço",
            Command::Browser => "Abre ou fecha o browser",
            Command::Share => "Compartilha a sessão com o time, ou volta a deixá-la privada",
            Command::Publish => "Publica uma skill ou um agente para a organização",
            Command::Voice => "Fala com o agente e ouve a resposta",
            Command::Btw => "Faz uma pergunta paralela sem parar o agente",
            Command::Rename => "Dá nome à sessão pela conversa, ou como você digitar",
        }
    }

    fn role(&self, role: AgentRole) -> &'static str {
        match role {
            AgentRole::Explorer => "Explorador",
            AgentRole::Builder => "Construtor",
            AgentRole::Reviewer => "Revisor",
            AgentRole::Tester => "Testador",
        }
    }

    fn idle_summary(&self, role: AgentRole) -> &'static str {
        match role {
            AgentRole::Explorer => "Mapeando o código",
            AgentRole::Builder => "Rascunhando a mudança",
            AgentRole::Reviewer => "Conferindo o diff",
            AgentRole::Tester => "Rodando as verificações",
        }
    }

    fn context_slice(&self, slice: &ContextSlice) -> Option<&'static str> {
        Some(match slice {
            ContextSlice::SystemPrompt => "Prompt do sistema",
            ContextSlice::Tools => "Ferramentas",
            ContextSlice::Mcp => "MCP",
            ContextSlice::Agents => "Agentes",
            ContextSlice::Memory => "Memória",
            ContextSlice::Skills => "Skills",
            ContextSlice::Messages => "Mensagens",
            ContextSlice::Other(_) => return None,
        })
    }

    fn waiting_turn(&self) -> &'static str {
        "Aguardando a vez"
    }

    fn status(&self, status: &AgentStatus) -> Option<&'static str> {
        match status {
            AgentStatus::Ready => Some("Pronto"),
            AgentStatus::Queued => Some("Na fila"),
            AgentStatus::Failed => Some("Falhou"),
            AgentStatus::Running { .. } | AgentStatus::Done { .. } => None,
        }
    }

    fn step_verb(&self, verb: &str) -> Option<&'static str> {
        Some(match verb {
            "Read" => "Leu",
            "Edit" => "Editou",
            "Write" => "Escreveu",
            "Search" => "Pesquisou",
            "Fetch" => "Acessou",
            "Agent" => "Agente",
            "Skill" => "Skill",
            "Plan" => "Plano",
            "Replied" => "Respondeu",
            _ => return None,
        })
    }

    fn engine_problem(&self, problem: EngineProblem, engine: EngineKind) -> String {
        let name = engine.label();
        match (problem, engine) {
            (EngineProblem::NotInstalled, EngineKind::Native) => {
                "O motor de IA integrado está indisponível. Reinicie o Superfik e tente de novo.".to_owned()
            }
            (EngineProblem::NotInstalled, _) => format!("{name} não está instalado nesta máquina."),
            (EngineProblem::NotLoggedIn, EngineKind::Native) => {
                "Conecte uma conta em Configurações → IA e tente de novo.".to_owned()
            }
            (EngineProblem::NotLoggedIn, _) => format!("{name} não está conectado."),
            (EngineProblem::SessionNotFound, _) => {
                format!("A conversa anterior do {name} não foi encontrada; sua próxima mensagem começa uma nova.")
            }
        }
    }
}
