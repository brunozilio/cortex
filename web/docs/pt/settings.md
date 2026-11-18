---
title: Configurações
description: Todas as chaves que o {{NAME}} lê dos arquivos de configuração, onde esses arquivos ficam, como se combinam e como alterá-los.
---

O {{NAME}} é configurado com arquivos JSON chamados `settings.json`. Você pode ter um só seu, um por projeto que vai para o repositório e é compartilhado com o time, e um privado por projeto que fica só na sua máquina. Esta página explica onde eles ficam, como se combinam e lista todas as chaves que podem conter.

## Início rápido

Crie `~/{{DIR}}/settings.json` com o que você quer em todos os projetos:

```json
{
  "outputStyle": "concise",
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Bash(git diff:*)"],
    "deny": ["Read(.env)"]
  }
}
```

Ou altere uma chave pelo terminal, sem abrir um editor:

```bash
{{COMMAND}} config set outputStyle concise
{{COMMAND}} config set permissions.allow '["Bash(cargo test:*)"]'
{{COMMAND}} config get permissions.allow
```

As próximas execuções já usam a mudança; não é preciso reiniciar nada.

## Arquivos de configuração

| Arquivo | Alcance | Tratado como seu | Uso típico |
|---|---|---|---|
| `~/{{DIR}}/settings.json` | Você, em todos os projetos | Sim | Suas preferências, modelos e permissões. `{{COMMAND}} config` e `/features` gravam aqui. |
| `~/{{DIR}}/settings.local.json` | Você, em todos os projetos | Sim | O mesmo que o anterior, se preferir um segundo arquivo. |
| `{{DIR}}/settings.json` no projeto (ou em qualquer pasta acima) | Todos que clonam o projeto | Não: vai para o repositório | Regras do time: regras de negação e de pergunta, hooks, estilo de resposta. |
| `{{DIR}}/settings.local.json` no projeto | Você, neste projeto | Sim, depois que você confia na pasta | Suas escolhas privadas no projeto. As respostas "não perguntar de novo", `/rules keep` e `{{COMMAND}} config --project` gravam aqui. |
| `~/{{DIR}}/managed-settings.json` | Você, em todos os projetos | Sim | Lido por último, depois de todos os outros. |

> [!IMPORTANT]
> O `{{DIR}}/settings.local.json` de um projeto só conta como seu em uma pasta em que você confia (veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta)). Até lá ele é lido como o `settings.json` versionado e só pode restringir; então uma regra salva com "não perguntar de novo" vale só naquela execução e passa a valer nas seguintes quando a pasta for confiável; o chat avisa disso ao salvar a regra. Deixe o arquivo fora do controle de versão: em uma pasta confiável, o que estiver nele pode conceder permissões.

Uma pasta `{{DIR}}/` acima do projeto (acima da pasta que contém o `.git`, ou acima da pasta atual quando não há repositório) é lida como um arquivo versionado, seja qual for o nome do arquivo e não importa em que você confiou: pode restringir, nunca conceder.

### Quais pastas são lidas

Para uma execução em uma pasta, o {{NAME}} lê, nesta ordem:

1. `~/{{DIR}}/settings.json` e depois `~/{{DIR}}/settings.local.json`.
2. Para cada pasta, da raiz do sistema de arquivos até a pasta atual: `{{DIR}}/settings.json` e depois `{{DIR}}/settings.local.json`. Uma pasta alcançada duas vezes (sua pasta pessoal costuma ser também uma pasta acima do projeto) é lida uma vez só.
3. `~/{{DIR}}/managed-settings.json`.

Arquivos que não existem são ignorados. Um arquivo que não é JSON válido interrompe a execução com um erro que cita o arquivo.

Um valor do tipo errado, ou fora das opções aceitas, não interrompe a execução: esse valor é deixado de fora (numa lista, só aquele item; em `hooks`, aquele hook), o resto do arquivo continua valendo, e `/permissions` e `/hooks` mostram um aviso com o arquivo, o caminho da chave e o que era esperado:

```text
/home/eu/{{DIR}}/settings.json: permissions.deny[1]: expected string, found 5
/home/eu/{{DIR}}/settings.json: hooks.Stop[0].hooks[0].type: expected one of "command", "script", "http", "prompt", "agent", found "shell"
```

Uma chave que o {{NAME}} não conhece é citada do mesmo jeito (`theme: unknown key, ignored`) e fora isso ignorada. Um valor do tipo certo que o {{NAME}} mesmo assim não consegue usar, como uma regra que ele não entende, um matcher de hook que não é regex ou um nome de modelo com espaços, interrompe a execução com um erro que cita o arquivo e a chave, por exemplo `permissions.deny: rule "Bash(rm" misses its )`.

### JSON Schema

Os arquivos de configuração e o `mcp.json` têm um JSON Schema publicado, para que o editor complete as chaves e sublinhe os erros enquanto você digita. O {{NAME}} copia os schemas para a sua pasta:

| Arquivo | Schema |
|---|---|
| `settings.json`, `settings.local.json`, `managed-settings.json` | `~/{{DIR}}/schemas/settings.schema.json` |
| `mcp.json` | `~/{{DIR}}/schemas/mcp.schema.json` |

Aponte um arquivo para o schema dele com a chave `$schema`. O {{NAME}} ignora essa chave, e ela nunca é citada como desconhecida. Em `~/{{DIR}}/settings.json` um caminho relativo funciona:

```json
{
  "$schema": "./schemas/settings.schema.json",
  "outputStyle": "concise"
}
```

O `{{DIR}}/settings.json` de um projeto fica em outro lugar, então use o caminho completo da sua cópia (por exemplo `"$schema": "/home/eu/{{DIR}}/schemas/settings.schema.json"`) ou associe o padrão de nome do arquivo ao schema nas configurações do seu editor. Os schemas correspondem à versão do {{NAME}} que os gravou e são atualizados junto com ele, a menos que você tenha editado a sua cópia. No código-fonte, eles ficam em `content/schemas/`.

### Como os valores se combinam

- **Valores únicos** (`outputStyle`, `agent`, `advisorModel`, `judgeModel`, `escalationModel`, `fallbackModel`, `smallModel`, `teammateMode`, `jsonSchema`, `bashOutputLimit`, `enableAllProjectMcpServers`): vale o último arquivo lido. Na prática a pasta mais próxima de onde você trabalha vence as pastas acima dela, e `managed-settings.json` vence todas. Definir `advisorModel`, `judgeModel`, `smallModel`, `escalationModel`, `fallbackModel`, `agent` ou `jsonSchema` como `null` apaga um valor definido por um arquivo anterior.
- **Listas e mapas se somam**: regras de permissão, `verification.allow`, `permissions.additionalDirectories`, `hooks`, `lspServers`, `enabledProviders`, `enabledMcpServers`, `disabledMcpServers`, `sandbox.allowWrite`, `sandbox.allowUnixSockets`, `sandbox.allowRead`.
- **Chaves de liga/desliga** (`features`, `scripts`, `sandbox.network`, `sandbox.required`): cada arquivo muda só os nomes que menciona.

### O que um arquivo do repositório não pode fazer

O `{{DIR}}/settings.json` de um projeto vem de quem escreveu o repositório, então ele pode restringir o {{NAME}}, mas não ampliar o que ele pode fazer:

| Chave | Em um `settings.json` do repositório |
|---|---|
| `permissions.allow` | Ignorada. Só os seus arquivos liberam. |
| `permissions.ask`, `permissions.deny` | Respeitadas. |
| `permissions.additionalDirectories` | Ignorada. |
| `verification.allow` | Ignorada. |
| `escalationModel` | Ignorada. |
| `enabledProviders` | Ignorada. |
| `enableAllProjectMcpServers`, `enabledMcpServers`, `disabledMcpServers` | Ignoradas. |
| `hooks` | Ignorados até você confiar na pasta. Veja [Hooks](./hooks#hooks-do-projeto-e-confianca-na-pasta). |
| `scripts.*` | Só pode desligar acessos. Não consegue dar acesso aos scripts do próprio projeto. |
| `features.*` | Pode desligar um recurso comum e ligar um recurso de segurança (`experimental-sandbox`, `redact-secrets`, `experimental-dependency-guard`), nunca o contrário. |
| `sandbox.network` | Só pode desligar a rede. |
| `sandbox.allowWrite` | Ignorada. |
| `sandbox.allowUnixSockets` | Ignorada. |
| `sandbox.allowRead` | Ignorada. |
| `sandbox.required` | Só pode ligá-la. |
| `lspServers` | `command`, `args` e `env` são descartados; o projeto só pode mapear extensões, marcadores de raiz ou desativar um servidor. |
| Todo o resto | Respeitado como nos seus próprios arquivos. |

## Alterando as configurações

### `{{COMMAND}} config`

`{{COMMAND}} config` lê e edita um arquivo de configuração por chave com pontos.

| Comando | O que faz |
|---|---|
| `{{COMMAND}} config list` | Mostra o arquivo inteiro. |
| `{{COMMAND}} config get <chave>` | Mostra um valor, por exemplo `{{COMMAND}} config get sandbox.network`. Falha quando a chave não está definida. |
| `{{COMMAND}} config set <chave> <valor>` | Define um valor. O valor é lido como JSON (`true`, `3`, `["Bash(ls:*)"]`, `{"network": false}`); o que não for JSON é guardado como texto. Objetos intermediários que faltam são criados. |
| `{{COMMAND}} config unset <chave>` | Remove uma chave. |

Sem opções, edita `~/{{DIR}}/settings.json`. Com `--project`, edita `{{DIR}}/settings.local.json` na pasta atual (ou na pasta indicada com `-C`).

```bash
{{COMMAND}} config set features.experimental-sandbox false
{{COMMAND}} config --project set permissions.deny '["Bash(rm:*)"]'
{{COMMAND}} config unset outputStyle
```

### No chat

- `/config` (ou `/settings`) abre o seletor **Settings**: Model, Permission mode, Effort, Output style, Vim keys, Status line, além de Features, Verification e Budget. Escolher um item executa o comando correspondente (`/model`, `/mode`, `/effort`, `/output-style`, `/vim`, `/statusline`, `/features`, `/verification`, `/budget`). Essas escolhas ficam guardadas para você; veja Preferências guardadas do chat, mais abaixo.
- `/features <nome> on|off` grava `features.<nome>` em `~/{{DIR}}/settings.json`. Veja [Recursos experimentais](./experimental-features).
- Responder **Yes, and don't ask again for …** a um pedido de permissão, ou usar `/rules keep <n>`, adiciona uma regra em `{{DIR}}/settings.local.json`. Veja [Permissões](./permissions).
- `/permissions` mostra as regras e pastas em vigor; `/hooks` mostra os hooks.

## Referência

### Chaves de primeiro nível

| Chave | Tipo | Padrão | Descrição |
|---|---|---|---|
| `outputStyle` | texto | `"default"` | Como o agente fala. Um estilo embutido (`default`, `concise`, `proactive`, `explanatory`, `learning`, sem diferenciar maiúsculas) ou o nome de um estilo próprio em `{{DIR}}/output-styles/<nome>.md` (letras, dígitos, `-` e `_`). O estilo escolhido com `/output-style` vence esta chave. Veja [Estilos de resposta](./output-styles). |
| `agent` | texto ou `null` | nenhum | Executa a conversa principal como este agente (um nome listado em `/agents`), com as instruções, ferramentas e modelo dele. Um nome desconhecido faz a execução falhar. Veja [Agentes](./agents). |
| `bashOutputLimit` | inteiro, de 1000 a 1000000 | `30000` | Caracteres da saída de um comando Bash que o agente vê. Acima disso, o agente recebe o começo e o fim da saída sob uma primeira linha como `[Output cut: 81234 characters in 2010 lines, over the limit of 30000; the first 15000 and the last 15000 are shown. The full output is in /tmp/…/bash-123-4.txt: Read or Grep it for the rest.]`, e a saída inteira é salva nesse arquivo. O chat mostra a mesma linha acima da saída dobrada. Um valor fora do intervalo vai para o extremo mais próximo. |
| `advisorModel` | texto ou `null` | nenhum | Modelo do conselheiro que o agente consulta. Um id ou nome de modelo, sem espaços. Um papel definido com `/model advisor <modelo>` vence. |
| `judgeModel` | texto ou `null` | nenhum | Modelo do juiz que confere o trabalho terminado contra os critérios de verificação. Um papel definido com `/model review <modelo>` vence. Veja [Verificação e revisão](./verification-and-review). |
| `smallModel` | texto ou `null` | o modelo pequeno do provedor | Modelo para tarefas de fundo: títulos, escolha de esforço, leitura da web. Um papel definido com `/model small <modelo>` vence. |
| `teammateMode` | `process`, `tmux`, `iterm2` ou `auto` | `process` | Onde o líder inicia os teammates: em segundo plano, imprimindo num log (`process`), num painel novo da janela do tmux em que o líder roda (`tmux`), num painel novo do iTerm2 (`iterm2`), ou `auto`: tmux dentro do tmux, senão iTerm2 dentro do iTerm2, senão em segundo plano. Veja [Equipes de agentes](./agents#equipes-de-agentes). |
| `escalationModel` | texto ou `null` | nenhum | Para onde vai uma chamada em que o agente falha repetidamente quando o recurso `experimental-escalation` está ligado (senão, vai para o Advisor). Lido só dos seus próprios arquivos. |
| `fallbackModel` | texto ou lista de textos | nenhum | Modelo, ou cadeia de modelos, tentado quando o escolhido está ocupado, desconectado ou inacessível. `--fallback-model` e `/model fallbacks` vencem esta chave. Veja [Modelos e provedores](./models-and-providers). |
| `jsonSchema` | objeto JSON Schema ou `null` | nenhum | Dá ao agente uma ferramenta `StructuredOutput` cujo `result` precisa seguir este schema. Útil com `-p`; veja [Modo headless](./headless). Um schema inválido faz a execução falhar. |
| `enabledProviders` | lista de textos | `[]` | Ids dos provedores declarados pelo projeto em `{{DIR}}/providers/` que você permite carregar. Lido só dos seus próprios arquivos. |
| `enableAllProjectMcpServers` | booleano | `false` | Inicia sem perguntar todos os servidores MCP declarados no `{{DIR}}/mcp.json` do projeto. Lido só dos seus próprios arquivos. |
| `enabledMcpServers` | lista de textos | `[]` | Servidores MCP do projeto que você aprovou. `{{COMMAND}} mcp approve` e `/mcp approve` gravam aqui. Lido só dos seus próprios arquivos. |
| `disabledMcpServers` | lista de textos | `[]` | Servidores MCP do projeto que você desligou; vence as duas chaves acima. Lido só dos seus próprios arquivos. |
| `hooks` | objeto | `{}` | Comandos, prompts ou chamadas HTTP executados em eventos. Os hooks de todos os arquivos se somam; os hooks de um projeto são ignorados até você confiar na pasta dele. Veja [Hooks](./hooks#hooks-do-projeto-e-confianca-na-pasta). |
| `lspServers` | objeto | `{}` | Adiciona, altera ou desativa servidores de linguagem. Veja [`lspServers`](#lspservers) e [Inteligência de código](./code-intelligence). |
| `permissions` | objeto | `{}` | Regras de liberar, perguntar e negar, e pastas extras. Veja [`permissions`](#permissions). |
| `verification` | objeto | `{}` | Regras para o juiz de verificação. Veja [`verification`](#verification). |
| `scripts` | objeto | tudo `true` para os seus scripts, tudo `false` para os de um projeto | O que ferramentas, skills, hooks e provedores em JavaScript podem fazer. Veja [`scripts`](#scripts). |
| `sandbox` | objeto | rede ligada | Onde comandos na sandbox podem gravar e se acessam a rede. Veja [`sandbox`](#sandbox). |
| `features` | objeto | todos os recursos ligados | Liga ou desliga recursos pelo nome. Veja [`features`](#features). |

`$schema` indica o [JSON Schema](#json-schema) do arquivo. Qualquer outra chave que o {{NAME}} não conhece é ignorada com um aviso. `trustProjectHooks`, que versões anteriores liam, não faz mais nada: os hooks de um projeto rodam quando você confia na pasta dele, e o chat avisa isso até você remover a chave.

> [!NOTE]
> As definições de servidores MCP (`mcpServers`) não ficam no `settings.json`: ficam em `{{DIR}}/mcp.json` (do projeto) e `~/{{DIR}}/mcp.json` (seus). Veja [MCP](./mcp). O comando da linha de status é uma preferência do chat (veja Preferências guardadas do chat, abaixo, e [Linha de status](./statusline)).

### `permissions`

| Chave | Tipo | Padrão | Descrição |
|---|---|---|---|
| `permissions.allow` | lista de regras | `[]` | Chamadas de ferramenta que rodam sem perguntar. Lida só dos seus próprios arquivos. |
| `permissions.ask` | lista de regras | `[]` | Chamadas que sempre perguntam, mesmo quando o modo deixaria passar. |
| `permissions.deny` | lista de regras | `[]` | Chamadas recusadas, em qualquer modo. |
| `permissions.additionalDirectories` | lista de pastas | `[]` | Pastas que o agente lê como se fossem o projeto. `~/` começa na sua pasta pessoal; um caminho relativo começa no projeto. Lida só dos seus próprios arquivos. |

```json
{
  "permissions": {
    "allow": ["Bash(npm run test:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)", "mcp__jira"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm:*)", "Read(*.env)", "mcp__github__delete_repo"],
    "additionalDirectories": ["~/notes", "../shared"]
  }
}
```

A sintaxe e a precedência das regras estão na página [Permissões](./permissions).

### `verification`

| Chave | Tipo | Padrão | Descrição |
|---|---|---|---|
| `verification.allow` | lista de regras | `[]` | Regras que o juiz de verificação pode executar sem perguntar (por exemplo o comando de testes que ele usa para conferir o trabalho). Lida só dos seus próprios arquivos. |

```json
{ "verification": { "allow": ["Bash(cargo test:*)", "Bash(npm test)"] } }
```

### `scripts`

| Chave | Tipo | Seus scripts | Scripts de um projeto | Descrição |
|---|---|---|---|---|
| `scripts.files` | booleano | `true` | `false` | Scripts podem ler e gravar arquivos (`readFile`, `writeFile`). |
| `scripts.processes` | booleano | `true` | `false` | Scripts podem iniciar processos (`exec`). |
| `scripts.network` | booleano | `true` | `false` | Scripts podem acessar a rede (`fetch`). |

O padrão depende de onde o script está:

- **Os seus scripts** são os que ficam em `~/{{DIR}}/` e os que vêm com o {{NAME}}. Eles têm os três tipos de acesso, a não ser que um arquivo de configuração desligue algum.
- **Os scripts de um projeto** são os que ficam na pasta `{{DIR}}/` do projeto: as ferramentas, skills, hooks, provedores e servidores MCP em script dele. Eles não têm **nenhum** dos três até que um dos seus próprios arquivos de configuração libere.

Os seus próprios arquivos são `~/{{DIR}}/settings.json`, `~/{{DIR}}/settings.local.json`, `~/{{DIR}}/managed-settings.json` e o `{{DIR}}/settings.local.json` de um projeto numa pasta em que você confia. Um valor definido neles vale para os dois grupos: `true` libera o acesso também para os scripts do projeto, `false` tira o acesso também dos seus. Um `{{DIR}}/settings.json` versionado só pode desligar acessos.

Um script barrado recebe um erro que cita a configuração, por exemplo `scripts may not touch files here (settings: scripts.files)`.

Essas configurações são o limite externo. Todo script também precisa listar o que usa (`readFile`, `writeFile`, `exec`, `fetch`): uma ferramenta de script em `spec.capabilities`, um hook ou script de skill em `export const capabilities = [...]`, um servidor MCP em JavaScript no `capabilities` do seu export default. Cada chamada dessas é então julgada pelas suas regras de permissão e pelo modo, como a ferramenta correspondente, e onde ninguém pode ser consultado uma chamada que perguntaria é recusada; veja "Capacidades" em [Sandbox e segurança](./sandbox-and-safety).

Para deixar os scripts do projeto em que você está trabalhando lerem arquivos e rodarem comandos, mas sem acesso à rede, coloque isto no `{{DIR}}/settings.local.json` daquele projeto, para valer só ali:

```json
{ "scripts": { "files": true, "processes": true } }
```

Para desligar a rede de todos os scripts, inclusive os seus:

```json
{ "scripts": { "network": false } }
```

> [!NOTE]
> Versões anteriores davam aos scripts de um projeto os três tipos de acesso por padrão. Se uma ferramenta ou um hook do projeto passar a falhar com `settings: scripts.…`, libere o que ele precisa em um dos seus próprios arquivos, como mostrado acima.

### `sandbox`

| Chave | Tipo | Padrão | Descrição |
|---|---|---|---|
| `sandbox.network` | booleano | `true` | `false` bloqueia a rede para comandos na sandbox. Um arquivo do repositório só pode desligá-la. |
| `sandbox.allowWrite` | lista de pastas | `[]` | Pastas extras em que comandos na sandbox podem gravar. Caminhos absolutos ou `~/…`; caminhos relativos são ignorados. Lida só dos seus próprios arquivos. |
| `sandbox.allowUnixSockets` | lista de caminhos de socket | `[]` | macOS: sockets de serviço a que comandos na sandbox podem se conectar (os sockets de Docker, Podman, containerd, BuildKit, Colima, tmux e screen são recusados fora isso). Caminhos absolutos ou `~/…`. Lida só dos seus próprios arquivos. |
| `sandbox.allowRead` | lista de caminhos | `[]` | macOS: caminhos que comandos na sandbox podem ler embora sejam depósitos de credenciais (`~/.ssh`, `~/.aws`, `~/.kube`, `~/.npmrc`, `~/.gnupg`, …) ou a pasta de dados do app, ilegíveis fora isso. Um caminho dentro de um depósito, como `~/.ssh/known_hosts`, abre só esse caminho. Caminhos absolutos ou `~/…`. Lida só dos seus próprios arquivos. Veja [Sandbox](./sandbox-and-safety#sandbox). |
| `sandbox.required` | booleano | `true` no Linux e no macOS, `false` nos demais | `true` recusa os comandos, e não inicia servidores MCP que rodam como processo, quando este sistema não consegue oferecer a sandbox, em vez de rodá-los sem confinamento. Desligue nas suas próprias configurações com `{ "sandbox": { "required": false } }`. Um arquivo do repositório só pode ligá-la. |

```json
{ "sandbox": { "allowWrite": ["~/.m2", "/opt/data"], "network": true } }
```

A sandbox é o recurso `experimental-sandbox` (macOS; Linux 5.13 ou mais novo). Servidores MCP que rodam como processo também ficam na sandbox, cada um com a sua entrada `sandbox` no `mcp.json`; veja "Servidores MCP na sandbox" em [Sandbox e segurança](./sandbox-and-safety).

### `lspServers`

Um mapa do nome do servidor para a sua configuração. Usar um nome embutido (`rust-analyzer`, `typescript`, `pyright`, `gopls`, `clangd`) altera esse servidor; um nome novo adiciona um (precisa de `command` e `extensionToLanguage`).

| Chave | Tipo | Descrição |
|---|---|---|
| `lspServers.<nome>.command` | texto | Programa a iniciar. Ignorado em um arquivo do repositório. |
| `lspServers.<nome>.args` | lista de textos | Argumentos. Ignorado em um arquivo do repositório. |
| `lspServers.<nome>.extensionToLanguage` | objeto | Extensão de arquivo para id de linguagem, por exemplo `{".zig": "zig"}`. |
| `lspServers.<nome>.rootMarkers` | lista de textos | Arquivos que marcam a raiz do projeto para o servidor. |
| `lspServers.<nome>.env` | objeto | Variáveis de ambiente do servidor, somadas às embutidas. Ignorado em um arquivo do repositório. |
| `lspServers.<nome>.disabled` | booleano | `true` remove o servidor. |

```json
{
  "lspServers": {
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" }, "rootMarkers": ["build.zig"] },
    "clangd": { "disabled": true }
  }
}
```

### `features`

Todo recurso fica ligado até você desligá-lo. Cada chave é `features.<nome>` com `true` ou `false`; um valor que não é booleano é deixado de fora com um aviso. `/features` lista todos com o estado atual, e [Recursos experimentais](./experimental-features) explica cada um.

| Chave | Padrão | O que controla |
|---|---|---|
| `features.experimental-sandbox` | `true` | Confina os comandos de Bash e Monitor às pastas do projeto (macOS; Linux com Landlock). Recurso de segurança. |
| `features.redact-secrets` | `true` | Esconde tokens, chaves e os valores do `.env` do projeto na saída das ferramentas. Recurso de segurança. |
| `features.experimental-dependency-guard` | `true` | Pergunta antes de instalar um pacote inexistente, recém-publicado ou com nome parecido com um popular. Recurso de segurança. |
| `features.destructive-snapshot` | `true` | Guarda arquivos não versionados antes de `rm`, `git clean`, `git reset --hard` e similares, para o `/rewind`. |
| `features.suggested-rules` | `true` | `/rules` sugere regras de permissão a partir das respostas que você repete. |
| `features.budget` | `true` | `/budget` interrompe as chamadas de modelo de uma sessão ao atingir um limite de gasto. |
| `features.cache-warning` | `true` | Contagem regressiva do cache de prompt na linha de status. |
| `features.experimental-escalation` | `true` | Envia uma chamada que falha repetidamente ao Advisor ou ao `escalationModel`. |
| `features.experimental-learning` | `true` | Oferece uma lição para guardar depois de uma execução em que você corrigiu o agente. |
| `features.team-memory` | `true` | Recupera memórias versionadas em `{{DIR}}/memory`; `/memory share <id>`. |
| `features.branches` | `true` | `/branch` começa uma conversa a partir de antes de um prompt anterior. |
| `features.git-offer` | `true` | Oferece `git init` em uma pasta que não é repositório. |
| `features.experimental-session-claims` | `true` | Avisa quando outra sessão na mesma pasta editou o mesmo arquivo. |
| `features.experimental-pr-follow` | `true` | A skill `follow-pr` acompanha um pull request. |
| `features.experimental-affected-tests` | `true` | Roda em segundo plano os testes que cobrem um arquivo editado. |
| `features.experimental-output-cleanup` | `true` | O Bash roda sem cores e descarta códigos de escape e linhas repetidas. |
| `features.experimental-output-filters` | `true` | O Bash esconde testes que passaram e progresso de ferramentas comuns, guardando a saída inteira em um arquivo. |
| `features.experimental-output-savings` | `true` | `/gain` mostra quanto a saída dos comandos encolheu. |
| `features.experimental-web-main-content` | `true` | O WebFetch coloca o conteúdo principal da página primeiro. |
| `features.experimental-web-cleanup` | `true` | O WebFetch descarta partes ocultas e links de menu repetidos. |
| `features.experimental-web-links` | `true` | O WebFetch numera os links e lista cada endereço uma vez. |
| `features.experimental-web-small-pages-direct` | `true` | O WebFetch entrega direto ao agente páginas de até 6.000 caracteres. |
| `features.experimental-web-formats` | `true` | O WebFetch pede Markdown e compacta JSON e feeds. |
| `features.experimental-web-empty-page-error` | `true` | O WebFetch trata como erro uma página quase sem texto. |
| `features.auto-review` | `true` | Um revisor somente leitura confere as mudanças antes do fim de uma execução que alterou arquivos. |
| `features.experimental-follow-through` | `true` | Um modelo pequeno manda o agente de volta quando ele encerra um turno prometendo mais trabalho. |
| `features.project-criteria` | `true` | Julga as execuções contra `{{DIR}}/verification.md` quando você não define critérios. |
| `features.experimental-memory-anchors` | `true` | Memórias citam seus arquivos e aparecem quando esses arquivos são lidos ou editados. |
| `features.experimental-structured-learnings` | `true` | Decisões são guardadas como o que foi decidido, o que foi descartado e cuidados. |
| `features.experimental-learning-check` | `true` | `/check` compara o branch com as memórias sobre os arquivos que ele toca. |
| `features.experimental-blast-radius` | `true` | Depois de uma edição, aponta os arquivos que chamam os símbolos alterados. |
| `features.experimental-code-map` | `true` | Indexa símbolos com tree-sitter e aponta cada prompt para o código relacionado. |
| `features.experimental-memory-seed` | `true` | `/memory seed` oferece pedidos repetidos como memórias. |
| `features.experimental-memory-scorecard` | `true` | Uma requisição a cada 20 roda sem memória recuperada, para o `/memory score`. |
| `features.experimental-load-on-call` | `true` | Uma ferramenta que espera atrás do ToolSearch e que o agente chama pelo nome é carregada e executada, em vez de recusada. |
| `features.experimental-code-mode` | `true` | A REPL vira a única ferramenta do agente: ele escreve JavaScript que chama todas as outras ferramentas como funções. |
| `features.memory-search` | `true` | A ferramenta `memory_search`. |
| `features.schedules` | `true` | `/loop` e `/schedules`. |
| `features.plugins` | `true` | Instalar, atualizar, listar e remover plugins. |
| `features.trace` | `true` | `/trace` e `/console`. |
| `features.channels` | `true` | Servidores MCP que ficam abertos e mandam mensagens ao agente. |
| `features.declared-providers` | `true` | Carrega os provedores declarados em pastas `providers/`. |

```json
{ "features": { "experimental-code-map": false, "git-offer": false } }
```

### Preferências guardadas do chat

As escolhas que você faz no chat não vão para o `settings.json`. Elas ficam guardadas por usuário na pasta de dados (`users/local/settings.json`, veja [Arquivos e pastas](./files-and-directories)) e são lidas de volta quando o chat abre. Normalmente você as altera com comandos, não à mão.

| Chave | Tipo | Definida por | Descrição |
|---|---|---|---|
| `model` | texto | `/model`, `--model` | O modelo inicial. Também usado pelo `-p` quando `--model` não é informado. |
| `mode` | texto | `/mode`, Shift+Tab | `ask`, `edits`, `plan` ou `bypass` (as outras grafias de [Modos de permissão](./permissions#modos-de-permissao) também são lidas). `--mode` vence naquela abertura. |
| `effort` | texto | `/effort` | `low`, `medium`, `high`, `xhigh`, `max` ou `auto`. |
| `outputStyle` | texto | `/output-style` | Vence o `outputStyle` dos arquivos de configuração. |
| `vim` | `"on"` ou `"off"` | `/vim` | Teclas do vim no prompt. |
| `statusLine` | texto | `/statusline` | Comando que desenha a linha de status; vazio para a padrão. Veja [Linha de status](./statusline). |
| `roles` | objeto | `/model <papel> <modelo>` | Modelo por papel: `plan`, `explore`, `small`, `review`, `advisor`. |
| `fallbacks` | lista de textos | `/model fallbacks` | Cadeia de fallback deste usuário. |
| `history` | objeto | envio de prompts | Os últimos 100 prompts por pasta, para o histórico do prompt. |
| `sessions` | objeto | conversas | A última conversa por pasta, para o `-c`. |
| `titles` | objeto | conversas | O título da última conversa por pasta. |
Um `mode` ou `effort` guardado com valor desconhecido é ignorado com um aviso. Se o arquivo deixar de ser JSON válido, ele é movido para `settings.json.corrupt-<hora>` e o chat começa com os padrões.

## Exemplo completo

`~/{{DIR}}/settings.json`:

```json
{
  "outputStyle": "explanatory",
  "advisorModel": "opus",
  "judgeModel": "sonnet",
  "smallModel": "haiku",
  "escalationModel": "opus",
  "fallbackModel": ["sonnet", "gpt-5"],
  "enabledProviders": ["acme-llm"],
  "enableAllProjectMcpServers": false,
  "enabledMcpServers": ["jira"],
  "disabledMcpServers": ["sonar"],
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Bash(git diff:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm -rf:*)", "Read(*.env)"],
    "additionalDirectories": ["~/notes"]
  },
  "verification": { "allow": ["Bash(cargo test:*)"] },
  "scripts": { "files": true, "processes": true, "network": false },
  "sandbox": { "network": true, "allowWrite": ["~/.m2"] },
  "features": { "experimental-code-map": false, "git-offer": false },
  "lspServers": {
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" } }
  },
  "hooks": {
    "PostToolUse": [
      { "matcher": "Edit|Write", "hooks": [{ "type": "command", "command": "cargo fmt" }] }
    ]
  }
}
```

O `{{DIR}}/settings.json` de um projeto, versionado no repositório:

```json
{
  "outputStyle": "concise",
  "agent": "reviewer",
  "jsonSchema": null,
  "permissions": {
    "ask": ["Bash(npm publish:*)"],
    "deny": ["Read(secrets/**)"]
  },
  "features": { "experimental-sandbox": true }
}
```
