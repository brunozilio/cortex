---
title: Referência da CLI
description: Todos os comandos, subcomandos e opções do programa {{COMMAND}}.
---

`{{COMMAND}}` abre o chat na pasta atual. Com um prompt, o chat começa com ele; com `--print`, responde uma vez e sai; com um subcomando, faz só aquilo.

```bash
{{COMMAND}} [OPTIONS] [PROMPT]...
{{COMMAND}} [OPTIONS] <COMMAND>
```

## Usos comuns

```bash
{{COMMAND}}                                   # chat no projeto atual
{{COMMAND}} "why does the build fail?"        # chat, começando com este prompt
{{COMMAND}} -p "explain src/main.rs"          # uma resposta, impressa
{{COMMAND}} -c                                # continua a última conversa daqui
{{COMMAND}} -r                                # escolhe uma conversa anterior desta pasta
{{COMMAND}} -m claude-opus-5-5 --mode plan    # começa com este modelo, no modo plan
{{COMMAND}} -C ../other-project               # trabalha em outra pasta
{{COMMAND}} login                             # conecta um provedor
{{COMMAND}} models                            # lista os modelos
{{COMMAND}} doctor                            # confere esta instalação aqui
```

## Subcomandos

| Comando | O que faz |
|---|---|
| `{{COMMAND}} login` | Conecta um provedor de LLM com chave de API ou assinatura; lista os provedores para escolher |
| `{{COMMAND}} logout` | Esquece as credenciais de um provedor |
| `{{COMMAND}} models` | Lista os modelos que os provedores conectados oferecem |
| `{{COMMAND}} mcp` | Os servidores MCP que uma sessão aqui pode usar: listar, mostrar, adicionar, remover, aprovar, negar, login, logout |
| `{{COMMAND}} plugin` | Os plugins em `~/{{DIR}}/plugins`: listar, instalar, atualizar, remover |
| `{{COMMAND}} remote` | Controle remoto pelo celular, pelo seu relay: status, relay, pair, devices, revoke |
| `{{COMMAND}} doctor` | Confere se esta instalação funciona na pasta |
| `{{COMMAND}} eval` | Roda os evals em `{{DIR}}/evals` e os pontua |
| `{{COMMAND}} teammate` | Trabalha como teammate da equipe de um líder, a partir da caixa de entrada; iniciado pelo líder, não à mão (veja [Equipes de agentes](./agents#equipes-de-agentes)) |
| `{{COMMAND}} config` | Lê ou muda as configurações: as suas, ou as deste projeto com `--project` |
| `{{COMMAND}} help` | Imprime a ajuda, ou a ajuda de um subcomando |

As opções globais (`--cwd`, `--model` e as demais) vêm antes do subcomando: `{{COMMAND}} -C ../app doctor`.

### login

```bash
{{COMMAND}} login
```

Imprime os provedores, cada um com a forma de conectar (`paste an API key`, `sign in with the browser`, `sign in with a code`), e pergunta `Choose 1-<n>:`. A chave é digitada oculta; um login abre o navegador. Termina com `Connected <provedor> (<conta>).` Conectar um provedor que já está conectado adiciona outra conta. Veja [Modelos e provedores](./models-and-providers) e [Contas](./accounts).

### logout

```bash
{{COMMAND}} logout
```

Lista os provedores conectados e pergunta `Forget which?`. O provedor é esquecido com todas as contas dele. Sem nada conectado, imprime `No provider is connected.`

### models

```bash
{{COMMAND}} models
```

Imprime uma linha por modelo: o id, completado até 40 caracteres, e depois o rótulo.

### mcp

```bash
{{COMMAND}} mcp [COMMAND]
```

Sem comando, lista os servidores. Todo comando imprime a lista de servidores no fim: o nome, de onde vem (`host`, `project`, `user` ou `plugin <nome>`), o estado (`ready`, `waiting for approval`, `off` ou `skipped: folder not trusted`) e a URL ou o comando. Veja [MCP](./mcp).

| Comando | Argumentos | O que faz |
|---|---|---|
| `mcp list` | | Lista os servidores, de onde vêm e se iniciam |
| `mcp get` | `<NAME>` | Mostra a entrada de um servidor, com os segredos ocultos |
| `mcp add` | `<NAME> <TARGET> [ARGS]...` | Adiciona um servidor seu: uma URL (HTTP) ou um comando e seus argumentos |
| `mcp remove` | `<NAME>` | Remove um servidor seu |
| `mcp approve` | `<NAME>` | Deixa um servidor do `{{DIR}}/mcp.json` deste projeto iniciar |
| `mcp deny` | `<NAME>` | Desliga um servidor do `{{DIR}}/mcp.json` deste projeto |
| `mcp login` | `<NAME>` | Faz login num servidor remoto pelo navegador (OAuth) |
| `mcp logout` | `<NAME>` | Esquece o login de um servidor remoto |

Argumentos e opções de `mcp add`:

| Argumento ou opção | Valor | Padrão | Descrição |
|---|---|---|---|
| `<NAME>` | texto | | O nome do servidor: letras, dígitos, `-` e `_` |
| `<TARGET>` | texto | | Uma URL começando com `https://` ou `http://` para um servidor HTTP; qualquer outra coisa é o comando a iniciar |
| `[ARGS]...` | texto | | Os argumentos do comando; escreva `--` antes deles quando algum começa com `-` |
| `--force` | | | Substitui um servidor seu que tem esse nome |
| `-e`, `--env` | `KEY=VALUE` | | Uma variável de ambiente para um servidor de comando (pode repetir). `${VAR}` é lido quando o servidor inicia. |
| `-H`, `--header` | `"Name: value"` | | Um cabeçalho para um servidor HTTP (pode repetir) |
| `--transport` | `http` ou `sse` | `http` | `sse` para um servidor no transporte antigo HTTP+SSE |

As opções podem vir em qualquer lugar antes de `--`; tudo depois de `--` é do servidor:

```bash
{{COMMAND}} mcp add -H "Authorization: Bearer $TOKEN" docs https://mcp.example.com/mcp
{{COMMAND}} mcp add --transport sse legacy https://old.example.com/sse
{{COMMAND}} mcp add -e API_KEY='${API_KEY}' files -- npx -y @example/files-mcp --root .
{{COMMAND}} mcp get files
{{COMMAND}} mcp approve jira
```

### plugin

```bash
{{COMMAND}} plugin [COMMAND]
```

Sem comando, lista os plugins instalados. Veja [Plugins](./plugins).

| Comando | Argumentos e opções | O que faz |
|---|---|---|
| `plugin list` | | Lista os plugins instalados: nome e descrição |
| `plugin install` | `<SOURCE>` `[--name <NAME>]` | Instala um plugin de uma URL git ou de um caminho. `--name` define o nome da pasta; senão ele vem da origem. |
| `plugin update` | `<NAME>` | Puxa os commits mais recentes de um plugin |
| `plugin remove` | `<NAME>` | Remove a pasta de um plugin |

Depois de instalar, um chat novo ou `/reload` carrega o plugin.

```bash
{{COMMAND}} plugin install https://github.com/example/{{COMMAND}}-plugin.git
{{COMMAND}} plugin install ./my-plugin --name mine
{{COMMAND}} plugin update mine
```

### remote

```bash
{{COMMAND}} remote [COMMAND]
```

Acompanhe e conduza as sessões desta máquina pelo celular: a mesma página do `/console` (sessões, execuções, o terminal), instalada na tela inicial do celular. O celular chega à máquina por um relay que você mesmo roda (o programa `relay` deste repositório, atrás de HTTPS); a máquina se conecta a ele e não abre porta nenhuma. Tudo entre o celular e a máquina é cifrado de ponta a ponta: o relay repassa bytes que não consegue ler. Sem comando, mostra o relay, o id desta máquina e os aparelhos pareados.

| Comando | Argumentos | O que faz |
|---|---|---|
| `remote status` | | O relay, o id desta máquina e os aparelhos pareados |
| `remote up` | `[--account <ID>]` | Publica o relay como Worker na sua conta Cloudflare (workers.dev) e passa a usá-lo |
| `remote logout` | | Esquece o login da Cloudflare que o `remote up` guarda |
| `remote relay` | `<https://HOST>` | Define um relay que você roda em outro lugar, pelo qual os celulares chegam a esta máquina |
| `remote pair` | | Imprime um QR code e um link que pareiam um celular, em até dez minutos, uma vez |
| `remote devices` | | Lista os aparelhos pareados |
| `remote revoke` | `<NOME>` | Esquece um aparelho; as conexões abertas dele fecham em segundos |

A máquina fica alcançável enquanto um chat está aberto com o controle remoto ligado (`/remote`); ele liga sozinho quando há um relay definido e um celular pareado. No celular, abra o link, espere a página conectar e use **Adicionar à Tela de Início**. Um celular pareado pode abrir o terminal, que roda comandos como você: revogue um celular perdido.

```bash
{{COMMAND}} remote relay https://relay.example.com
{{COMMAND}} remote pair
{{COMMAND}} remote revoke "iPhone"
```

#### Sem servidor: `remote up`

`{{COMMAND}} remote up` publica o relay como um Worker na sua própria conta Cloudflare, pela API da Cloudflare, e o define como o relay desta máquina. Não precisa de servidor, nem de domínio, nem de instalar nada: o relay ganha um endereço fixo, `https://{{COMMAND}}-relay.<seu subdomínio>.workers.dev`, e continua rodando na Cloudflare; nesta máquina só fica aberto o chat. Cabe no plano gratuito da Cloudflare (um Worker e um Durable Object por máquina).

Na primeira vez, ele abre o navegador para entrar na Cloudflare: escolha a conta e autorize. O login fica no cofre de credenciais do {{NAME}} e se renova sozinho, então o próximo `remote up` não pergunta de novo; `{{COMMAND}} remote logout` o esquece. Numa máquina sem navegador, defina `CLOUDFLARE_API_TOKEN` com um API token com a permissão **Account · Workers Scripts · Edit**. Se o login alcança mais de uma conta, escolha uma com `--account <id>`. Uma conta que nunca abriu Workers & Pages ainda não tem subdomínio workers.dev: abra uma vez no painel. Rode `remote up` de novo depois de atualizar o {{NAME}} para publicar a página nova; os celulares pareados continuam funcionando.

A Cloudflare transporta o tráfego e entrega a página ao celular, mas o que passa entre o celular e a máquina continua cifrado de ponta a ponta.

```bash
{{COMMAND}} remote up
CLOUDFLARE_API_TOKEN=… {{COMMAND}} remote up --account 0123abcd…
```

### doctor

```bash
{{COMMAND}} doctor
```

Roda as verificações da instalação para a pasta e sai com 0 quando todas passam, 1 caso contrário. A linha `trust` diz se a pasta é confiável e quantos itens do projeto estão sendo ignorados enquanto ela não é; uma pasta não confiável não conta como verificação que falhou. Veja [Solução de problemas](./troubleshooting#rodar-o-doctor).

### eval

```bash
{{COMMAND}} eval [OPTIONS] [NAMES]...
```

| Argumento ou opção | Descrição |
|---|---|
| `[NAMES]...` | Só estes casos (todos por padrão) |
| `--json` | Imprime as pontuações em JSON |

Cada caso é `{{DIR}}/evals/<nome>/case.json`, com uma `fixture/` opcional. `--model`, passado antes de `eval`, vale para todos os casos. Sai com 0 quando todos os casos passaram, 1 caso contrário. Veja [Evals](./evals).

### config

```bash
{{COMMAND}} config [--project] <COMMAND>
```

Lê ou muda o seu arquivo de configurações, `~/{{DIR}}/settings.json`. Com `--project`, trabalha no `{{DIR}}/settings.local.json` deste projeto. `--project` pode vir antes ou depois do comando. Veja [Configurações](./settings).

| Comando | Argumentos | O que faz |
|---|---|---|
| `config list` | | Imprime todas as configurações, em JSON |
| `config get` | `<KEY>` | Imprime uma configuração. A chave é um caminho com pontos, como `permissions.allow`. |
| `config set` | `<KEY> <VALUE>` | Define uma. O valor é lido como JSON (`true`, `["Bash(ls:*)"]`), ou senão como texto. |
| `config unset` | `<KEY>` | Remove uma |

```bash
{{COMMAND}} config set permissions.allow '["Bash(ls:*)"]'
{{COMMAND}} config set outputStyle concise
{{COMMAND}} config get permissions.allow
{{COMMAND}} config --project set permissions.allow '["Bash(cargo test:*)"]'
{{COMMAND}} config unset outputStyle
```

`set` e `unset` criam o arquivo e a pasta quando preciso e imprimem `Set <key> in <file>.` ou `Removed <key> from <file>.` Um `get` de chave que não está definida falha com `<key> is not set in <file>`.

## Opções

### Prompt e saída

| Opção | Curta | Valor | Padrão | Exige | Descrição |
|---|---|---|---|---|---|
| `[PROMPT]...` | | texto | | | O que perguntar. As palavras são unidas com espaços. Abre o chat com ele, a menos que `--print` seja passado. |
| `--print` | `-p` | | desligado | | Responde uma vez, imprime a resposta e sai. Com `--input-format text` precisa de um prompt (`--print needs a prompt`). Veja [Modo headless](./headless). |
| `--output-format` | | `text`, `json` ou `stream-json` | `text` | `--print` | `text` transmite a resposta; `json` imprime um objeto de resultado; `stream-json` imprime cada evento como uma linha JSON, e depois o resultado |
| `--input-format` | | `text` ou `stream-json` | `text` | `--print` | `text` pega o prompt da linha de comando; `stream-json` lê mensagens de usuário como linhas JSON na entrada padrão e responde uma de cada vez |
| `--max-turns` | | número | 200 | `--print` | O máximo de turnos do modelo que uma execução pode usar |
| `--verification` | | texto | | `--print` | Julga o trabalho por estes critérios antes de terminar; o comando sai com 1 quando eles não são atendidos |
| `--trust` | | | desligado | `--print` | Confia na pasta do projeto só nesta chamada, para que os hooks, scripts, servidores MCP e configurações locais dela carreguem. Nada é guardado. Sem ela, uma execução com `--print` roda sem confiança e uma linha no stderr diz o que foi ignorado. Na sua pasta pessoal ela é ignorada, com uma linha no stderr avisando. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta). |

### Modelo e comportamento

| Opção | Curta | Valor | Padrão | Descrição |
|---|---|---|---|---|
| `--model` | `-m` | id ou nome do modelo | o último usado | O modelo. O id completo, a parte depois dos dois-pontos ou o rótulo. |
| `--fallback-model` | | id ou nome do modelo | | O modelo com que seguir quando o escolhido está sobrecarregado. Tem prioridade sobre os fallbacks definidos no chat e nas configurações. |
| `--mode` | | `ask`, `edits`, `plan` ou `bypass` | | O modo de permissão. `default` também é lido como `ask`, `acceptEdits` e `accept-edits` como `edits`, `bypassPermissions`, `bypass-permissions` e `yolo` como `bypass`, em maiúsculas ou minúsculas. |
| `--allowedTools` | | regras | | Regras permitidas sem perguntar, como `Bash(git diff:*) Edit`. Pode repetir. Também `--allowed-tools`. |
| `--disallowedTools` | | regras | | Regras recusadas, como `Bash(rm:*)`. Pode repetir. Também `--disallowed-tools`. |
| `--append-system-prompt` | | texto | | Texto adicionado ao fim do prompt de sistema |

As regras de `--allowedTools` e `--disallowedTools` são separadas por espaços ou vírgulas fora dos parênteses. Veja [Permissões](./permissions).

### Conversas

| Opção | Curta | Valor | Conflita com | Descrição |
|---|---|---|---|---|
| `--continue` | `-c` | | `--resume` | Continua a última conversa desta pasta. Se não houver nenhuma, aparece `no earlier conversation in this folder; starting a new one`. |
| `--resume` | `-r` | id opcional | `--continue` | Continua uma conversa anterior: o id dela, ou, sem id, escolha uma desta pasta no chat |
| `--fork-session` | | | | Com `-c` ou `-r`: continua numa cópia daquela conversa, deixando-a como está |

Veja [Sessões](./sessions).

### Pastas e terminal

| Opção | Curta | Valor | Padrão | Descrição |
|---|---|---|---|---|
| `--cwd` | `-C` | caminho | a pasta atual | A pasta do projeto |
| `--add-dir` | | caminho | | Deixa o agente ler esta pasta como se fosse o projeto. Pode repetir. Caminhos relativos partem da pasta do projeto; `~/` parte da sua pasta pessoal. A pasta precisa existir. |
| `--inline` | | | desligado | Chat na parte de baixo do terminal, com a conversa no histórico de rolagem do próprio terminal, em vez de tela cheia |
| `--channels` | | nomes, separados por vírgula | | Servidores MCP a manter ativos como canais: o que eles enviam (mensagens de um chat, um webhook) chega ao agente como evento. Veja [Canais](./channels). |

### Ajuda e versão

| Opção | Curta | Descrição |
|---|---|---|
| `--help` | `-h` | Imprime a ajuda. `{{COMMAND}} <comando> --help` imprime a ajuda de um subcomando. |
| `--version` | `-V` | Imprime a versão |

## Códigos de saída

| Código | Quando |
|---|---|
| `0` | Sucesso |
| `1` | Uma falha: execução que falhou, verificação ou eval reprovado, verificação do doctor que falhou, ou um erro como uma pasta ilegível. A mensagem sai na saída de erro como `{{COMMAND}}: <mensagem>`. |
| `2` | A linha de comando está errada: opção ou valor desconhecido, argumento faltando, ou opção usada sem aquela que ela exige. Também quando a entrada ou a saída padrão não é um terminal e `--print` não foi dado (`no terminal: use -p for print mode`). |

## Variáveis de ambiente

| Variável | Efeito |
|---|---|
| `{{ENV}}_LOG` | O que vai para o arquivo de log, como um filtro do tipo `warn` ou `app=debug`. Padrão `warn`. Veja [Solução de problemas](./troubleshooting#logs). |
| `{{ENV}}_THEME` | `light` ou `dark`: as cores do chat, em vez de seguir o terminal |
| `{{ENV}}_REDUCED_MOTION` | `1` desliga as animações do chat |

Veja [Variáveis de ambiente](./environment-variables) para a lista completa.
