---
title: Variáveis de ambiente
description: As variáveis de ambiente que o {{NAME}} lê, as variáveis padrão que ele respeita e as que ele define para os programas que inicia.
---

Quase tudo no {{NAME}} se configura nos [arquivos de configuração](./settings). As variáveis de ambiente cobrem o que precisa ser conhecido antes de ler esses arquivos (logs, cores, onde ficam os dados) e algumas chaves de liga/desliga úteis em scripts e CI.

```bash
{{ENV}}_LOG=debug {{COMMAND}}
{{ENV}}_THEME=light {{ENV}}_REDUCED_MOTION=1 {{COMMAND}}
{{ENV}}_STREAM_IDLE_SECONDS=600 {{COMMAND}} -p "resuma os TODOs em aberto"
```

## Variáveis do {{NAME}}

| Variável | Valores | Padrão | Para que serve |
|---|---|---|---|
| `{{ENV}}_LOG` | Um filtro de log: `error`, `warn`, `info`, `debug`, `trace`, ou por módulo (`agent_engine=debug,warn`) | `warn` | Quanto vai para o arquivo de log `logs/{{COMMAND}}.log` na [pasta de dados local](./files-and-directories#pasta-de-dados). Um filtro vazio ou inválido volta para `warn`. |
| `{{ENV}}_THEME` | `light`, `dark` (sem diferenciar maiúsculas) | detectado | Força as cores claras ou escuras do chat em vez de detectar o fundo do terminal. |
| `{{ENV}}_REDUCED_MOTION` | Qualquer valor não vazio diferente de `0` | não definida | Desliga as animações do chat. |
| `{{ENV}}_CONSOLE_DIR` | Uma pasta com um `index.html` | não definida | Serve a página do [console de trace](./trace-console) a partir desta pasta em vez da cópia embutida no binário. |
| `{{ENV}}_STREAM_IDLE_SECONDS` | Um número inteiro de segundos acima de zero | `300` | Quanto tempo a resposta de um modelo pode ficar em silêncio no meio do stream antes de o pedido ser abandonado e tratado como falha. Qualquer outro valor volta ao padrão. |
| `{{ENV}}_OTLP_ENDPOINT` | Uma URL `http://` ou `https://`, como `http://localhost:4318` | não definida: nada é exportado | Manda cada run terminado a este coletor OpenTelemetry como OTLP/HTTP JSON, em `<endpoint>/v1/traces`. Veja [Exportar para o OpenTelemetry](./trace-console#exportar-para-o-opentelemetry). |
| `{{ENV}}_OTLP_HEADERS` | Pares `nome=valor` separados por vírgulas | não definida | Cabeçalhos acrescentados a cada pedido de exportação, como uma chave de API. Lidos só quando `{{ENV}}_OTLP_ENDPOINT` está definida. |
| `{{ENV}}_TEAM_NAME` | O nome de uma equipe | definida pelo líder | A equipe a que um teammate pertence. O líder a define em cada teammate que inicia; veja [Equipes de agentes](./agents#equipes-de-agentes). |
| `{{ENV}}_AGENT_NAME` | O nome de um teammate | definida pelo líder | O nome do teammate na equipe, definido junto com `{{ENV}}_TEAM_NAME`. |
| `{{ENV}}_TEAMMATE_COMMAND` | O caminho de um programa | não definida: o `{{COMMAND}}` em execução | O programa com que o líder inicia teammates, como `<programa> … teammate`. |
| `{{ENV}}_SECRETS` | `keychain` (sem diferenciar maiúsculas) | não definida: a chave fica em `secrets.key` | Com `keychain`, a chave que abre suas chaves de API e tokens salvos passa de `secrets.key` para o chaveiro do sistema. Sem a variável (ou com qualquer outro valor), ela fica no arquivo e o chaveiro nunca é escrito; um binário recompilado ou reinstalado tem uma assinatura nova, e o macOS pediria a senha do chaveiro a cada início. Se uma versão anterior moveu a chave para o chaveiro, ela volta para o arquivo uma vez (o macOS pergunta uma vez); se o chaveiro não responder nessa hora, uma chave nova é criada e as chaves salvas antes precisam ser informadas de novo. Com `keychain`, um chaveiro que deixa de responder é um erro, não uma chave nova; defina `{{ENV}}_SECRETS=file` para sair dele. Veja [Arquivos e diretórios](./files-and-directories). |

## Variáveis do controle remoto

| Variável | Lida por | Para que serve |
|---|---|---|
| `CLOUDFLARE_API_TOKEN` | `{{COMMAND}} remote up` | Um API token com a permissão **Account · Workers Scripts · Edit**, usado no lugar do login da Cloudflare pelo navegador (para uma máquina sem navegador). Veja [Controle remoto](./remote-control). |
| `RELAY_LISTEN` | o programa `relay` | O endereço e a porta em que ele escuta, como `127.0.0.1:8080`; `relay --listen` faz o mesmo. O padrão é `0.0.0.0:8080`. Veja [Rodar seu próprio relay](./remote-control#rodar-seu-proprio-relay). |

## Variáveis padrão que ele respeita

| Variável | Usada para |
|---|---|
| `HOME` | Sua pasta pessoal: `~/{{DIR}}` (configurações, skills, agentes, plugins…), o `~/` em regras e caminhos, e o `~` mostrado para o projeto na tela de boas-vindas. No Windows é usada a pasta do perfil do usuário. |
| `XDG_CONFIG_HOME` | Linux: a [pasta de dados](./files-and-directories) é `$XDG_CONFIG_HOME/{{COMMAND}}`; `~/.config/{{COMMAND}}` quando não definida ou vazia. |
| `APPDATA` | Windows: a pasta de dados é `%APPDATA%\{{NAME}}`; `~\AppData\Roaming\{{NAME}}` quando não definida. |
| `LOCALAPPDATA` | Windows: a pasta de dados locais (sessões, logs, chaves) é `%LOCALAPPDATA%\{{NAME}}`; `~\AppData\Local\{{NAME}}` quando não definida. Também é onde o Git Bash é procurado (`Programs\Git\bin\bash.exe`). |
| `ProgramFiles` | Windows: onde o Git Bash é procurado (`Git\bin\bash.exe`). Sem Git Bash, os comandos rodam no Windows PowerShell. |
| `SHELL` | macOS e Linux: o shell que roda os comandos do agente (`$SHELL -c …`); `/bin/sh` quando não definida ou vazia. Hooks e scripts sempre usam `/bin/sh`. |
| `VISUAL`, `EDITOR` | O editor que o Ctrl+G abre para escrever o prompt, e no qual `/memory edit` abre o `{{INSTRUCTIONS}}` do projeto. Primeiro `VISUAL`, depois `EDITOR`, depois `vi`. O valor pode ter argumentos (`code --wait`). |
| `COLORFGBG` | Detecta se o fundo do terminal é claro ou escuro quando `{{ENV}}_THEME` não está definida; escuro quando falta. |
| `COLORTERM` | `truecolor` ou `24bit` liga cores reais (true colour) no chat. |
| `PATH` | Onde servidores de linguagem (`rust-analyzer`, `gopls`…) e outros programas são encontrados. |
| `TMPDIR` | macOS e Linux: a [pasta temporária](./files-and-directories#arquivos-temporarios) para saídas longas de ferramentas, o prompt em edição e a permissão de gravação da sandbox. |
| `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY` (ou em minúsculas) | As requisições aos provedores e às páginas web passam por esse proxy. Com uma delas definida, as requisições web do agente deixam para o proxy a verificação de que um site não está neste computador nem na sua rede privada. |

As formas `${VAR}` e `${VAR:-padrão}` nos arquivos `mcp.json` são preenchidas a partir do ambiente; uma variável não definida e sem padrão fica como está escrita. Veja [MCP](./mcp).

[Hooks](./hooks) HTTP podem usar `$VAR` ou `${VAR}` em seus `headers`, mas só para as variáveis listadas no `allowed_env_vars` do hook; as outras ficam vazias.

> [!NOTE]
> Chaves e tokens de provedores não são lidos do ambiente. Conecte provedores com `{{COMMAND}} login` (ou `/login`); as credenciais ficam seladas na pasta de dados. Veja [Contas](./accounts).

## Variáveis que ele define para outros programas

| Variável | Definida para | Valor |
|---|---|---|
| `{{ENV}}_PROJECT_DIR` | Hooks de comando | A pasta do projeto em que a execução trabalha. |
| `{{ENV}}_PLUGIN_ROOT` | Configuração de plugins | Não é uma variável de ambiente: `${{{ENV}}_PLUGIN_ROOT}` nos servidores MCP, servidores de linguagem e hooks de um plugin é trocado pela pasta do plugin. Veja [Plugins](./plugins). |
| `NO_COLOR=1`, `CLICOLOR=0`, `CARGO_TERM_COLOR=never`, `CARGO_TERM_PROGRESS_WHEN=never`, `PY_COLORS=0` | Os comandos de shell do agente | Só com o recurso `experimental-output-cleanup` ligado (o padrão), para que a saída chegue sem cores nem barras de progresso. |
| `CARGO_TARGET_DIR` | Testes em segundo plano em projetos Rust | `<projeto>/target/agent-tests`, para não travar os seus próprios builds. Recurso `experimental-affected-tests`. |
| `GOBIN` | Instalação de um servidor de linguagem Go | Uma pasta temporária dentro do cache de servidores de linguagem. |
| `TERM=xterm-256color`, `COLORTERM=truecolor` | O terminal que o console de trace abre | Valores fixos. |
| `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` | Comandos git que o {{NAME}} roda por conta própria (status, diffs) | Para o git nunca pedir senha nem pegar um lock que faria os seus próprios comandos git esperarem. |
| `APP_NOTIFICATION_TITLE`, `APP_NOTIFICATION_MESSAGE` | Notificações da área de trabalho no Windows | O título e o texto da notificação. |
| Variáveis de `env` | Servidores MCP (stdio) e servidores de linguagem | O que você colocar no `env` do servidor em `mcp.json` ou `lspServers`. |

No Windows com Git Bash, `BASH_ENV` e `ENV` são removidas do ambiente dos comandos do agente, para que nenhum perfil de inicialização rode antes deles.

Todo o resto do seu ambiente é repassado sem mudanças aos comandos, hooks e servidores que o {{NAME}} inicia.
