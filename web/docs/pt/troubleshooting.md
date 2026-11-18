---
title: Solução de problemas
description: Confira a instalação com o doctor, encontre os logs e resolva os problemas mais comuns.
---

Quando algo não funciona, comece pelo doctor: ele confere de uma vez as peças de que o {{NAME}} precisa e diz o que corrigir. Se não bastar, o arquivo de log tem os detalhes.

## Rodar o doctor

Num terminal, na pasta do projeto:

```bash
{{COMMAND}} doctor
```

Ou, no chat:

```text
/doctor
```

```text
All checks passed:
  ✓ version      0.3.1
  ✓ providers    Anthropic API, Grok Subscription
  ✓ models       42 available
  ✓ settings     6 permission rules, 2 hooks
  ✓ trust        /home/ana/rocket is trusted
  ✓ mcp servers  3 configured
  ✓ declared providers none (~/{{DIR}}/providers/*.js adds one)
  ✓ sandbox      on: commands write only inside the project, added folders, temp and caches
  ✓ git          git version 2.47.0
  ✓ ripgrep      ripgrep 14.1.1 (Grep searches with it)
```

Quando uma verificação falha, a primeira linha conta os problemas (`1 problem(s):`) e a verificação aparece marcada com `✗`. O `{{COMMAND}} doctor` sai com 0 quando todas passam e com 1 caso contrário, então dá para usá-lo em scripts. Use `-C <caminho>` para conferir outra pasta.

### O que cada verificação significa

| Verificação | Passa quando | Quando falha |
|---|---|---|
| `version` | Sempre; mostra a versão instalada | |
| `providers` | Pelo menos um provedor está conectado; lista os provedores | `none connected: run {{COMMAND}} login`. Veja [Modelos e provedores](./models-and-providers). |
| `models` | Os provedores conectados oferecem pelo menos um modelo; mostra quantos | `none available: connect a provider`, ou o erro que o provedor devolveu |
| `settings` | Todos os arquivos de configuração daqui são lidos; mostra as regras de permissão e os hooks encontrados | O arquivo com problema e o motivo, seguidos de `(fix the file it names)`. Veja [Configurações](./settings). |
| `trust` | Se esta pasta é confiável; quando não é, quantos itens do projeto foram pulados, e que `/trust` (ou `--trust` com `--print`) confia nela | Só quando as pastas confiáveis não podem ser lidas. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta). |
| `mcp servers` | A configuração de MCP é lida; mostra quantos servidores | O erro na configuração. Veja [MCP](./mcp). |
| `declared providers` | Todos os arquivos em `~/{{DIR}}/providers/` e `{{DIR}}/providers/` carregaram | Cada arquivo que não carregou, e por quê |
| `sandbox` | Sempre; diz se os comandos estão confinados | |
| `git` | `git` roda; mostra a versão | `` `git` was not found on PATH `` |
| `ripgrep` | Sempre; mostra a versão do `rg`, ou que não está instalado | |

A linha `sandbox` diz:

| Linha | Significado |
|---|---|
| `on: commands write only inside the project, added folders, temp and caches` | O sandbox confina os comandos |
| `off in your settings (/features experimental-sandbox on)` | Ele está disponível e desligado; o comando na linha o liga |
| `not on this system: commands run unconfined` | Este sistema não tem sandbox |
| `not offered` | O recurso de sandbox não é oferecido |

Veja [Sandbox e segurança](./sandbox-and-safety).

A linha `declared providers` diz `none (~/{{DIR}}/providers/*.js adds one)` quando não há nenhum e `<n> loaded` quando todos carregaram. Um arquivo que não carregou reprova a verificação com uma destas mensagens:

- `<arquivo>: the project's provider <id> waits: add it to enabledProviders in your settings`
- `<arquivo>: left out: <motivo>`, por exemplo um `wire` errado, um `base` que não é `https`, um id que um provedor embutido já usa, ou `the default export is not a provider`

Veja [Provedores declarados](./models-and-providers#provedores-declarados).

> [!NOTE]
> O `ripgrep` é opcional. Quando o `rg` está no `PATH`, a ferramenta `Grep` o usa para achar os arquivos que casam, o que é bem mais rápido em projetos grandes; sem ele, o `Grep` busca com o motor embutido e dá os mesmos resultados. Nos dois casos a verificação passa, com `not installed (Grep uses its built-in search, slower on large projects)` quando ele falta. O `git` é necessário para o `/diff` e para worktrees.

## Logs

O {{NAME}} escreve um arquivo de log na pasta local de dados:

| Sistema | Arquivo de log |
|---|---|
| macOS | `~/Library/Application Support/{{NAME}}/logs/{{COMMAND}}.log` |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}/logs/{{COMMAND}}.log`, ou `~/.config/{{COMMAND}}/logs/{{COMMAND}}.log` |
| Windows | `%LOCALAPPDATA%\{{NAME}}\logs\{{COMMAND}}.log` |

Por padrão, só avisos e erros são gravados. Defina `{{ENV}}_LOG` para registrar mais, com um filtro como `info`, `debug` ou `app=debug`:

```bash
{{ENV}}_LOG=debug {{COMMAND}} -p "hello"
```

Um filtro que não pode ser lido volta para `warn`. Cada linha traz o nome da thread, e um crash é gravado no log com a thread em que aconteceu.

Quando o arquivo passa de 5 MiB, ele é movido para `{{COMMAND}}.log.1` na próxima vez que o {{NAME}} inicia, e um arquivo novo começa; o `.log.1` anterior é substituído.

> [!TIP]
> Ao relatar um problema, anexe o fim do arquivo de log e a saída do `{{COMMAND}} doctor`.

## Problemas comuns

### Nenhum modelo disponível

```text
{{COMMAND}}: no model is available: run `{{COMMAND}} login` first
```

Nenhum provedor está conectado, ou os provedores conectados não oferecem modelos. Rode `{{COMMAND}} login` e depois `{{COMMAND}} models` para ver o que é oferecido. No chat, `/model` diz `No models yet. Run {{COMMAND}} login to connect a provider.`

### Um nome de modelo não é encontrado

```text
{{COMMAND}}: no model named sonnet: `{{COMMAND}} models` lists them
```

Um modelo pode ser indicado pelo id completo (`anthropic:claude-sonnet-5-5`), pela parte depois dos dois-pontos ou pelo rótulo. Confira a grafia com `{{COMMAND}} models`. No chat, a mensagem é `No model named <nome>. /model lists them.`

### O login no navegador não termina

- Se o navegador não abrir, copie a URL que o {{NAME}} imprime.
- O ChatGPT Subscription espera na porta 1455, o Kiro Subscription na porta 49153 e o Antigravity Subscription na porta 51121. Feche o que mais estiver usando essa porta e tente de novo.
- O Kiro aceita logins com Google, GitHub, Builder ID e de organização (IAM Identity Center). Um login de organização falha quando o portal do Kiro não informa a URL inicial da organização (que precisa ser `https`) ou a sua região AWS; confira a URL inicial e tente de novo.
- O GitHub Copilot recusa tokens clássicos `ghp_`.

### O agente não consegue fazer nada no modo print

```text
denied: the agent asked for permission (use --mode or --allowedTools)
```

No modo print ninguém pode responder a um pedido de permissão, então todo pedido é negado. Permita o que a execução precisa com `--allowedTools`, ou escolha um `--mode`. Veja [Modo headless](./headless#escolher-o-que-o-agente-pode-fazer).

### O plano não é executado

```text
plan not carried out: run again with --mode edits to implement it
```

A execução estava no modo plan. Rode de novo com `--mode edits`.

### O agente para depois de passos demais

```text
The agent stopped after too many steps without finishing.
```

A execução usou todos os turnos: 200 por padrão, ou `--max-turns` no modo print. Divida a tarefa, ou aumente `--max-turns`.

### A chamada ao modelo falhou ou uma conta está ocupada

```text
The model call failed; going on with <modelo>.
Account …aaaa is busy or signed out; going on with …bbbb.
```

Não são erros: um [modelo de fallback](./models-and-providers#modelos-de-fallback) ou [outra conta](./accounts#dividir-o-trabalho-entre-contas) assumiu. Se as chamadas continuarem falhando, confira o `{{COMMAND}} doctor` e o log.

### O cache esfriou

```text
cache cold: next turn rewrites 48k tokens
```

O cache de prompt do provedor expira depois de uns cinco minutos sem chamadas, então o próximo turno paga para gravá-lo de novo. Veja [Cache de prompt](./models-and-providers#cache-de-prompt).

### Um arquivo de configuração tem erro

A verificação `settings` mostra o arquivo e o problema, por exemplo um valor JSON inválido ou `bashOutputLimit must be a whole number of characters`. Corrija esse arquivo, ou inspecione-o com `{{COMMAND}} config list` (`--project` para o do projeto). No chat, um `mode` ou `effort` salvo que não é conhecido aparece como `The saved effort "..." is not low, medium, high, xhigh, max or auto; using the default.`

### Um servidor MCP do projeto não inicia

Um servidor do `{{DIR}}/mcp.json` de um projeto espera a sua aprovação. O `{{COMMAND}} mcp` o lista como `waiting for approval` e diz como aprová-lo (numa pasta em que você não confiou ele aparece como `skipped: folder not trusted`; confie na pasta antes):

```bash
{{COMMAND}} mcp approve <nome>
```

### `-c` começa uma conversa nova

```text
{{COMMAND}}: no earlier conversation in this folder; starting a new one
```

O `-c` continua a última conversa da pasta atual. Rode na mesma pasta, ou use `-r` para escolher uma.

### Shift+Enter não adiciona uma linha

```text
/terminal-setup
```

Alt+Enter e Ctrl+J adicionam uma linha em qualquer terminal. Shift+Enter funciona nos terminais que o informam: Ghostty, kitty, WezTerm, e iTerm2 com *Report modifiers using CSI u*. Nos outros terminais, mapeie Shift+Enter para enviar `\n` nas configurações de teclas do terminal. O {{NAME}} pede essas teclas ao terminal quando consegue. Veja [Atalhos de teclado](./keyboard-shortcuts).

### Cores ou animações estranhas

- As cores seguem o fundo do terminal, lido de `COLORFGBG`. Defina `{{ENV}}_THEME=light` ou `{{ENV}}_THEME=dark` para escolher.
- `{{ENV}}_REDUCED_MOTION=1` desliga as animações.

## Começar do zero

`/factory-reset` apaga tudo o que o {{NAME}} guarda nesta máquina e sai:

```text
/factory-reset
```

Ele pergunta antes, com *No, keep everything* como primeira opção; nada é apagado a menos que você escolha *Yes, erase and quit*, que lista as pastas que remove:

- a pasta de dados: sessões, memória, logins (chaves e tokens) e configurações;
- a pasta local de dados com os logs, quando é uma pasta diferente (no Windows);
- `~/{{DIR}}`: suas configurações, skills, agentes, plugins e provedores declarados.

O programa continua instalado, e as pastas dos projetos (incluindo o `{{DIR}}` de cada projeto) ficam intactas. Se uma pasta não puder ser apagada, o {{NAME}} diz qual e por quê.

> [!CAUTION]
> Um factory reset não pode ser desfeito. Você vai precisar conectar seus provedores de novo com `{{COMMAND}} login`.

Se ainda não há nada guardado, ele diz `{{NAME}} keeps nothing on this machine yet.`

Veja [Arquivos e diretórios](./files-and-directories) para saber o que fica em cada pasta.
