---
title: Permissões
description: Decida o que o agente pode fazer sem perguntar, com modos de permissão, regras de liberar/perguntar/negar e pastas extras.
---

Toda chamada de ferramenta que o agente faz passa por uma verificação de permissão. Ler dentro do projeto é sempre livre; editar arquivos, rodar comandos, buscar páginas e ler fora do projeto dependem do **modo de permissão** e das suas **regras**. Quando nenhum dos dois resolve, o {{NAME}} pergunta a você.

## Respondendo a um pedido de permissão

Quando o agente precisa da sua aprovação, o painel mostra a ferramenta, o que ela quer fazer e até três opções:

| Opção | Tecla | Efeito |
|---|---|---|
| Yes | `y`, `1` | Executa esta chamada uma vez. |
| Yes, and don't ask again for `<regra>` in this project | `a`, `2` | Executa e salva a regra sugerida em `permissions.allow` no `{{DIR}}/settings.local.json`. |
| Yes, and don't ask again this session | `a`, `2` | Aparece quando nenhuma regra serve (edições, por exemplo). Lembra a ferramenta, no caso de edições, ou a chamada exata, nos outros casos, até a sessão terminar. |
| No, and tell the agent what to do instead | `n`, Esc ou o número dela | Recusa. Escolhê-la pelo número ou com Enter permite digitar uma observação que o agente recebe; Esc ou `n` recusam sem observação. |

A opção "don't ask again" só aparece quando a sua resposta evita a próxima pergunta. Ela não aparece para perguntas que voltam toda vez de qualquer jeito: comandos arriscados (pushes, publicações…), um endereço que nunca apareceu na conversa (veja [Ações que sempre perguntam](#acoes-que-sempre-perguntam)), rodar um comando fora da sandbox, instalar um pacote suspeito, uma chamada que casou com uma regra `ask` e uma chamada sobre a qual um hook pediu para perguntar.

Em uma pasta em que você não confiou, uma regra salva no `{{DIR}}/settings.local.json` vale só naquela execução; ela passa a valer nas seguintes quando você confiar na pasta, e o chat avisa disso ao salvar a regra. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).

As regras sugeridas descrevem o tipo de ação, não os argumentos:

| Pedido sobre | Regra sugerida |
|---|---|
| `cargo test --workspace` | `Bash(cargo test:*)` |
| `gh issue create --title x` | `Bash(gh issue create:*)` |
| `python3 scripts/gen.py --all` | `Bash(python3 scripts/gen.py:*)` |
| `make` | `Bash(make)` |
| Buscar `https://docs.rs/serde/latest` | `WebFetch(domain:docs.rs)` |
| Ler `/home/ana/notes/todo.md` (fora do projeto) | `Read(//home/ana/notes/**)` |

Comandos que não podem ser descritos com segurança (dois comandos juntos, `$(…)`, um script por caminho, `FOO=1 make`) não recebem regra sugerida.

## Modos de permissão

| Modo | Nome em `--mode` e `/mode` | Edições | Comandos e outras ações | Use quando |
|---|---|---|---|---|
| Ask | `ask` (ou `default`) | Pergunta | Pergunta | Você quer aprovar cada mudança. |
| Edits | `edits` (ou `acceptEdits`, `accept-edits`) | Liberadas | Pergunta | Padrão. Você confia nas edições no projeto, não nos comandos. |
| Plan | `plan` | Recusadas | Recusados | O agente só lê, pesquisa e propõe um plano para você aprovar. |
| Bypass | `bypass` (ou `bypassPermissions`, `bypass-permissions`, `yolo`) | Liberadas | Liberados, depois de uma verificação de segurança | Trabalho sem supervisão em um ambiente descartável. |

O nome da segunda coluna é o que o app mostra e grava. Os outros, entre parênteses, são lidos como o mesmo modo, em maiúsculas ou minúsculas, onde quer que um modo seja dado: `--mode`, `/mode`, a configuração `mode` guardada e o `permissionMode` de um subagente.

Em qualquer modo, ler e pesquisar dentro do projeto, nas pastas adicionais e com comandos de shell somente leitura (`ls`, `git status`…) roda sem perguntar. Ler fora delas pergunta, exceto no modo bypass.

Como trocar:

- **Shift+Tab** no chat alterna ask → edits → plan → bypass → ask.
- `/mode` abre um seletor; `/mode plan` (ou `/plan`) troca direto.
- `--mode <nome>` abre o chat ou uma execução `-p` nesse modo.
- O último modo escolhido fica guardado para o próximo chat; `--mode` vence naquela abertura.

> [!WARNING]
> No modo bypass, `permissions.allow` e `permissions.ask` deixam de importar: tudo o que não é negado roda. Antes de cada ação que não seja uma simples leitura, uma verificação de segurança a avalia contra a conversa e seus arquivos de instruções e segura tudo o que considerar arriscado demais. Regras de negação continuam recusando.

Quando um plano fica pronto, o agente pede a sua aprovação; aprovar troca o modo para que ele execute o plano. Em execuções `-p` o plano é impresso e não é executado (rode de novo com `--mode edits`).

## Regras de permissão

As regras ficam na chave `permissions` dos seus [arquivos de configuração](./settings):

```json
{
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm:*)", "Read(*.env)", "mcp__github__delete_repo"]
  }
}
```

- `allow` roda sem perguntar. Só os seus próprios arquivos liberam: regras `allow` em um `{{DIR}}/settings.json` versionado no repositório são ignoradas.
- `ask` sempre pergunta, mesmo quando o modo deixaria a chamada passar.
- `deny` recusa, em qualquer modo, e avisa o agente para não tentar de novo.

### Precedência

1. Uma regra **deny** que casa recusa a chamada.
2. No modo bypass, e para edições ou comandos no modo plan, quem decide é o modo.
3. Uma regra **ask** que casa pergunta (até para leituras).
4. Uma regra **allow** que casa executa a chamada.
5. Caso contrário, quem decide é o modo.

As regras de todos os arquivos, e as de `--allowedTools`/`--disallowedTools`, formam uma lista só; a ordem não importa. `/permissions` (ou `/allowed-tools`) lista as regras como deny, ask e allow, seguidas das pastas extras.

### Sintaxe das regras

Uma regra é o nome de uma ferramenta, opcionalmente seguido de um especificador entre parênteses: `Ferramenta` ou `Ferramenta(especificador)`.

| Regra | Casa com |
|---|---|
| `Bash` | Qualquer comando. |
| `Bash(git status)` | Exatamente `git status` (espaços extras são ignorados). `git status -s` não casa. |
| `Bash(git diff:*)` | `git diff` e tudo que começa com `git diff ` (limite de palavra: não casa `git difftool`). |
| `Bash(npm run * --silent)` | `*` casa qualquer coisa: `npm run build --silent`. |
| `Edit` | Qualquer edição. Regras `Edit` também valem para `Write` e `NotebookEdit`. |
| `Edit(src/**)` | Edições dentro de `src/` no projeto. |
| `Read(*.env)` | Um nome de arquivo em qualquer nível do projeto. Regras `Read` também valem para `Glob`, `Grep` e `LSP`. |
| `Read(//etc/hosts)` | Um caminho absoluto (duas barras no início). |
| `Read(~/notes/*.md)` | Um caminho na sua pasta pessoal. |
| `WebFetch(domain:docs.rs)` | Buscar páginas nesse host. |
| `mcp__jira` | Todas as ferramentas do servidor MCP `jira`. |
| `mcp__github__delete_repo` | Uma ferramenta de um servidor. |
| `mcp__docs__resources(file:///a.md)` | Ler esse recurso do servidor `docs`. |
| `Ferramenta(assunto)` | Qualquer outra ferramenta: o especificador precisa ser igual ao assunto da chamada. |

### Caminhos

Especificadores de caminho funcionam como no `.gitignore`:

- `//caminho` é absoluto, `~/caminho` começa na sua pasta pessoal, `./caminho` começa no projeto.
- Um padrão com `/` começa no projeto (`src/**`, `config/prod.env`).
- Um padrão sem `/` casa esse nome em qualquer nível do projeto (`*.env`, `secrets`).
- `*` e `?` ficam dentro de uma pasta; `**` atravessa pastas.
- Uma regra que casa uma pasta vale para tudo dentro dela.
- Regras deny e ask ignoram maiúsculas e minúsculas (`Read(.env)` também recusa `.ENV`); regras allow não.
- Um `Glob` ou `Grep` sobre uma pasta é recusado quando uma regra deny cobre algo dentro dela.

### Comandos

As regras de shell leem o comando do jeito que o shell vai executá-lo. A linha é analisada em cada comando simples que ela executa, inclusive os que estão dentro de `if`, `for`, `while`, `case`, funções, subshells, `{ …; }`, `$(…)`, crases, `<(…)` e o comando que `find -exec` executa:

- **Comandos compostos** (`&&`, `;`, `|`, quebras de linha, estruturas de controle): todas as partes precisam ser liberadas. Partes somente leitura (`ls`, `cat`, `git status`…) e `cd` para uma pasta dentro do projeto passam sem regra. Uma parte negada recusa o comando inteiro.
- **Comandos escondidos** (`$(…)`, crases, `<(…)`, `bash -c '…'`, `eval`, um programa dado por uma variável) e **redirecionamentos para arquivos** nunca são liberados por uma regra allow. Os comandos dentro de uma substituição são partes próprias, então uma regra deny que cite um deles recusa a linha inteira, e o mesmo vale para o código de `bash -c '…'`, `sh -c "…"` e `eval …`: `bash -c 'rm -rf x'` é recusado por `Bash(rm:*)`. Um programa que as regras não conseguem ver (`$(echo rm) x`, `$X x`, `` `echo rm` x ``, um curinga como `/bin/r? x` ou `{rm,x}`, um shell lendo comandos de um pipe como `echo … | sh`) gera pergunta **toda vez, em todo modo, inclusive bypass**, enquanto existir uma regra deny com especificador; sem ninguém para perguntar, é recusado. Nomes de programa são comparados sem diferenciar maiúsculas (`RM`, `/bin/RM`).
- **Regras deny enxergam através de disfarces**: `sudo rm`, `env rm`, `VAR=1 rm`, `\rm`, `"rm"`, `/bin/rm`, `(rm x)`, `{ rm x; }`, `if true; then rm x; fi`, `f() { rm x; }`, `ls | xargs rm`, `busybox rm`, `find . -exec rm {} \;` casam com `Bash(rm:*)`. Os invólucros desmontados são `sudo`, `doas`, `env`, `exec`, `nice`, `timeout`, `nohup`, `time`, `command`, `builtin`, `stdbuf`, `ionice`, `chrt`, `xargs`, `busybox`, `setsid`, `chroot`, `flock`, `watch`, `script`, `caffeinate`, `xcrun`, `taskset`, `su`, `runuser`, `sg`, `pkexec`, `unbuffer`, `arch` e `strace`; o comando que recebem como texto (`su -c '…'`, `flock f -c '…'`, `script -c '…'`, `watch '…'`, `sg grupo '…'`, `env -S '…'`) é julgado como a própria linha.
- **Redirecionamentos não escondem um programa**: as regras casam com cada comando sem os seus redirecionamentos, então `2>/dev/null rm -rf y`, `<x rm -rf y`, `rm</dev/null -rf y` e `curl<<<x …` casam com `Bash(rm:*)` e `Bash(curl:*)`. Um arquivo lido por um redirecionamento de entrada (`<secrets/k cat`, `cat < secrets/k`) é julgado pelas regras `Read`, seja qual for o comando.
- **Um programa negado entregue a um desconhecido pergunta**: quando um programa que as regras não conhecem recebe uma palavra que nomeia um programa negado (`frobnicate rm -rf x`, `python3 -c 'import os; os.system("rm -rf x")'`, `osascript -e '… rm …'`), a linha é perguntada toda vez, inclusive no bypass. `echo`, `printf`, os programas só de leitura e um subcomando logo depois de `docker`, `npm`, `cargo` e afins (`docker rm`) ficam de fora.
- **Regras allow leem o comando como foi escrito**: `LD_PRELOAD=/tmp/x.so git status` não casa com `Bash(git status:*)`.
- **Uma linha que não dá para analisar com segurança** (aspas não fechadas, aritmética como `$((…))` com um comando dentro, `coproc`, um heredoc cujo corpo executa uma substituição) nunca é liberada por uma regra com especificador e gera pergunta quando existem regras deny ou ask com especificador.

Regras deny são uma lista de melhor esforço: um comando montado durante a execução (um arquivo de script, o código inline de um interpretador, o valor de uma variável) fica invisível para elas. Quem impede um comando de escrever onde não deve é o [sandbox](./sandbox-and-safety).

### Comandos somente leitura

Um comando de shell roda sem perguntar, em qualquer modo, só quando a linha inteira é uma lista ou pipeline simples (sem estrutura de controle, função, subshell, grupo, `!` ou substituição) e cada comando nela:

- é um programa de leitura da lista (`ls`, `cat`, `head`, `grep`, `rg`, `find`, `git status`, `git diff`, `git log`…) usado só com flags listadas para ele;
- não tem prefixo `VAR=valor` nem invólucro (`sudo ls` pergunta);
- redireciona só para `/dev/null` ou entre as próprias saídas (`2>&1`), nunca para um arquivo;
- não tem palavra que o shell mudaria: nada de `$`, crase ou barra invertida fora de aspas simples, nada de `*`, `?`, `[`, `{` ou `<` sem aspas, nem `~` que indique uma pasta pessoal. Padrões entre aspas são palavras comuns: `find . -name '*.rs'` e `grep 'a.*b' notes.txt` são somente leitura, `find . -name *.rs` não é. `git diff HEAD~1` é somente leitura;
- não cita caminho fora do projeto (`/…`, `..`), nem por um link simbólico: `cat ./link`, quando `link` leva para fora do projeto, pergunta como o `Read`, e as regras `Read` julgam para onde o link leva além do nome escrito.

### Ferramentas e seus efeitos

Cada ferramenta embutida diz, a partir da entrada, o que a chamada vai fazer, e a decisão é tomada a partir disso:

| Ferramenta | Tratada como |
|---|---|
| `Read`, `LSP` | leitura daquele arquivo (pergunta fora do projeto); o `LSP` não inicia servidor numa pasta em que você não confiou |
| `Glob`, `Grep` | leitura da pasta pesquisada |
| `Write`, `Edit`, `NotebookEdit` | escrita naquele arquivo (pergunta fora do projeto e das pastas adicionadas, em `.git/` ou em `{{DIR}}/`; ali só uma regra que nomeia o caminho, como `Edit(.git/hooks/**)`, libera, não um `Edit` sozinho) |
| `Bash`, `Monitor` | execução do comando (comandos somente leitura rodam livres) |
| `WebFetch`, `WebSearch` | acesso à rede (regras `WebFetch(domain:…)`) |
| `CronCreate`, `ScheduleWakeup` | trabalho agendado: pergunta |
| `EnterWorktree`, `ExitWorktree` | troca do checkout da sessão: pergunta (recusada no modo plan) |
| `Agent`, `Watch`, `PushNotification`, ferramentas de tarefas e de plano | não precisam de permissão; um subagente segue o próprio modo |
| `AskUserQuestion`, `Skill`, `ToolSearch`, `Advisor`, `BashOutput`, `CronList`, `ListSessions` | não precisam de permissão |
| `REPL` | o script em si não precisa de permissão; cada ferramenta que ele chama é julgada como aquela ferramenta |
| `KillShell`, `CronDelete`, `TaskStop` | não precisam de permissão para o trabalho do próprio agente; `KillShell` sobre qualquer outra coisa pergunta |
| `SendMessage` para outra sessão | pergunta, a menos que uma regra libere `SendMessage` |
| `ListMcpResources`, `ReadMcpResource` | seguem as regras daquele servidor (`mcp__docs`, `mcp__docs__resources(…)`) |
| Ferramentas MCP | perguntam, a menos que uma regra libere o servidor ou a ferramenta |

Suas próprias ferramentas de script em `~/{{DIR}}/tools/*.js` dizem o que fazem com `access` no `spec`: `"read"` roda livre, `"edit"` é tratada como uma edição, `"execute"` (ou sem `access`) pergunta. As ferramentas de script de um projeto sempre perguntam. `access` cobre só a chamada em si: cada função do host que o script usa (`readFile`, `writeFile`, `exec`, `fetch`) também precisa estar listada em `capabilities` no `spec`, e cada uma dessas chamadas é julgada como `Read`, `Write`, `Bash` ou `WebFetch` (veja [Capacidades](./sandbox-and-safety#capacidades)).

### Regras inválidas

Uma regra sem o `)` de fechamento, com especificador vazio (`Bash()`) ou que não começa com o nome de uma ferramenta interrompe a execução com um erro que cita a lista, por exemplo ``permissions.deny: rule "Bash(rm" misses its `)` ``.

## Ações que sempre perguntam

Algumas ações são perguntadas toda vez, seja qual for o modo e as regras (exceto uma regra deny, que as recusa):

- Push para o branch padrão, ou push forçado, que apaga ou reescreve o histórico remoto.
- `cargo publish`, `npm publish`, `pnpm publish`, `yarn publish`.
- `gh pr merge`, `gh release create|delete|edit`, `gh api`.
- `docker push`, `podman push`, `docker image push`.
- Os mesmos comandos com opções globais antes do subcomando (`git -C . push -f`, `npm --prefix . publish`).
- Um comando de shell cujo programa as regras não conseguem ver, enquanto existir uma regra deny de `Bash` com especificador (veja acima).
- Buscar um endereço que nunca apareceu na conversa, depois que a sessão leu conteúdo de fora (páginas web, ferramentas MCP).
- Rodar um comando fora da sandbox, instalar um pacote suspeito e instalar um servidor de linguagem (a menos que uma regra allow como `LSP(install:pyright)` o nomeie). Veja [Sandbox e segurança](./sandbox-and-safety).

Comandos que imprimiriam segredos (`env`, `printenv`, `cat .env`, ler chaves SSH…) são recusados de imediato.

## Opções de linha de comando

| Opção | Efeito |
|---|---|
| `--mode <ask\|edits\|plan\|bypass>` | Começa neste modo. |
| `--allowedTools <regras>` (ou `--allowed-tools`) | Adiciona regras allow nesta execução. Pode repetir; as regras podem ser separadas por espaços ou vírgulas fora dos parênteses: `--allowedTools "Bash(git diff:*) Edit"`. |
| `--disallowedTools <regras>` (ou `--disallowed-tools`) | Adiciona regras deny nesta execução. |
| `--add-dir <caminho>` | Adiciona uma pasta que o agente lê como o projeto. Pode repetir. |

```bash
{{COMMAND}} -p "corrija o teste que está falhando" --mode edits --allowedTools "Bash(cargo test:*)"
{{COMMAND}} --disallowedTools "Bash(git push:*),WebFetch"
```

Em execuções `-p` ninguém pode responder, então toda chamada que perguntaria é recusada e anotada (`denied: the agent asked for permission (use --mode or --allowedTools)`). Veja [Modo headless](./headless).

## Regras sugeridas

Com o recurso `suggested-rules` (ligado por padrão), `/rules` olha suas respostas nas conversas recentes desta pasta e sugere regras para o que você responde sempre do mesmo jeito: uma regra allow depois de 5 ou mais aprovações sem nenhuma recusa, uma regra deny depois de 3 ou mais recusas sem nenhuma aprovação. Regras que você já tem não são sugeridas.

```text
/rules           lista as sugestões, numeradas
/rules keep 2    salva a sugestão 2 em {{DIR}}/settings.local.json
```

## Pastas adicionais

Por padrão o agente lê livremente só dentro do projeto. Para dar acesso a outra pasta (um repositório vizinho, notas compartilhadas):

- `/add-dir <caminho>` para a conversa atual; `/add-dir` sozinho lista as pastas adicionadas.
- `--add-dir <caminho>` ao abrir.
- `permissions.additionalDirectories` nos seus próprios arquivos de configuração, de forma permanente. `~/` começa na sua pasta pessoal; um caminho relativo começa no projeto.

Caminhos digitados em `/add-dir` e `--add-dir` precisam ser pastas existentes; `~/` é expandido e caminhos relativos começam no projeto. Uma pasta adicionada é lida e editada como o projeto e, com a sandbox ligada, comandos na sandbox podem gravar nela.

## Hooks e permissões

Um hook `PreToolUse` pode responder `allow`, `ask` ou `deny` para uma chamada; `deny` recusa, `ask` pergunta a você e `allow` pula a pergunta, a menos que uma regra ask tenha casado. Um hook `PermissionRequest` roda sempre que o {{NAME}} está para perguntar a você e pode responder no seu lugar. Veja [Hooks](./hooks).
