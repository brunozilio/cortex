---
title: Chat
description: Use o chat no terminal: escreva prompts, mencione arquivos, anexe imagens, rode comandos de shell, oriente o agente e acompanhe o que ele faz.
---

O chat é onde você trabalha com o {{NAME}}. Você digita um prompt, o agente trabalha, e você acompanha, responde às perguntas dele e o orienta no caminho. Esta página cobre tudo o que o prompt e a tela fazem. Só as teclas estão em [Atalhos de teclado](./keyboard-shortcuts); os comandos, em [Comandos de barra](./slash-commands).

## Abrir um chat

```bash
{{COMMAND}}                          # chat na pasta atual
{{COMMAND}} "fix the failing test"   # abre o chat e envia este prompt
{{COMMAND}} -C ../outro-projeto      # chat em outra pasta
{{COMMAND}} -c                       # continua a última conversa desta pasta
{{COMMAND}} --inline                 # chat no rodapé do terminal
```

As outras flags de início (`-m`, `--mode`, `--add-dir`, `-r`, `--fork-session`, `--channels`…) estão na [Referência da CLI](./cli-reference).

## A tela

| Área | O que mostra |
|---|---|
| Conversa | Seus prompts (marcados com `>`), as respostas do agente em Markdown com código destacado, o raciocínio (`✻ Thinking…`), chamadas de ferramentas (`●` com o resultado em `⎿`), sub-agentes, planos, diffs e avisos. |
| Painel | O que pede sua atenção: uma pergunta ou pedido de permissão, um seletor, sugestões de `@` ou `/`, a lista de tarefas do agente ou, num chat novo, algumas dicas. |
| Prompt | Uma caixa arredondada onde você digita. Cresce até 10 linhas. |
| Linha de status | Uma linha no rodapé (veja [Linha de status](#linha-de-status)). |

Conteúdo longo fica dobrado: a saída de uma ferramenta nas 3 primeiras linhas (`… +N lines (ctrl+o to expand)`), o raciocínio nas 3 últimas linhas enquanto o agente pensa e numa linha `✻ Thought` quando termina, diffs e edições em 20 linhas, um sub-agente nos 3 últimos passos. `Ctrl+O` desdobra ou dobra tudo de uma vez.

As cores seguem o fundo do terminal (de `COLORFGBG`, ou `{{ENV}}_THEME=light|dark`) e usam true color quando `COLORTERM` indica; `{{ENV}}_REDUCED_MOTION=1` desliga as animações. Veja [Variáveis de ambiente](./environment-variables).

## Escrever um prompt

Digite e aperte `Enter` para enviar. O prompt vazio mostra uma dica do que dá para fazer: `Try "explain this project"` num chat novo, `Reply to go on · / for commands` depois de uma resposta, `Say what to do next, or /retry` depois de uma falha.

### Prompts com várias linhas

| Tecla | Efeito |
|---|---|
| `Alt+Enter` | Nova linha (funciona em qualquer terminal). |
| `Ctrl+J` | Nova linha (funciona em qualquer terminal). |
| `Shift+Enter` | Nova linha, nos terminais que informam essa tecla. |
| `Ctrl+G` | Edita o prompt no seu editor (`$VISUAL`, senão `$EDITOR`, senão `vi`). |

O {{NAME}} pede ao terminal que informe teclas com modificadores quando possível. Se `Shift+Enter` não fizer nada, rode `/terminal-setup`: use um terminal que informe a tecla (Ghostty, kitty, WezTerm, iTerm2 com "Report modifiers using CSI u") ou mapeie `Shift+Enter` para enviar `\n` nas configurações de teclas do terminal.

Com `Ctrl+G` o prompt abre num arquivo Markdown temporário; quando o editor termina com sucesso, o texto dele (sem espaços no fim) substitui o prompt. Se o editor falhar, o prompt fica como estava.

### Colagens longas

Uma colagem de 5 linhas ou mais, ou de 1.000 caracteres ou mais, vira um marcador como `[Pasted text #1 +6 lines]`. Você pode digitar em volta dele; o texto inteiro é enviado no lugar.

### Histórico de prompts

`↑` na primeira linha do prompt traz prompts anteriores, `↓` na última linha avança de novo até voltar ao prompt vazio. O histórico é guardado por pasta (os últimos 100 prompts) e sobrevive a reinícios. `Ctrl+R` busca nele: digite parte de um prompt, `Ctrl+R` de novo para resultados mais antigos, `Enter` (ou `Tab`, `→`) pega o resultado, `Esc` sai.

### Teclas do vim

`/vim` liga ou desliga a edição no estilo vim (a escolha fica guardada). `Esc` passa para o modo normal e `i` volta a inserir. As teclas aceitas estão em [Atalhos de teclado](./keyboard-shortcuts#modo-vim).

## Mencionar arquivos com @

Digite `@` e parte do nome de um arquivo: o painel oferece até 8 arquivos do projeto, ordenados por quanto o nome do próprio arquivo bate, depois o caminho, depois letras na ordem. `↑`/`↓` escolhem, `Tab` completa para `@caminho/do/arquivo `.

```text
por que @src/main.rs sai cedo?
```

Ao enviar, o texto de cada arquivo mencionado vai junto, dentro de um bloco `<file path="...">`:

- Até 5 arquivos diferentes por prompt.
- Até 50.000 caracteres de cada; arquivos maiores são cortados e marcados com `[… cut]`.
- Uma menção que não é um arquivo legível fica como está.
- Pontuação no fim (`,` `.` `:` `;` `)` `?` `!`) não faz parte do caminho.

A lista de arquivos vem do git (arquivos rastreados mais os não rastreados que não estão ignorados), então as sugestões precisam de um repositório git. Uma menção digitada à mão funciona em qualquer pasta.

## Anexar imagens

| Como | Resultado |
|---|---|
| `Ctrl+V` | Anexa a imagem da área de transferência (no macOS via `osascript`, no Linux via `wl-paste` ou `xclip`). |
| Arrastar um arquivo de imagem para o terminal | O caminho é reconhecido, com aspas ou escapes, incluindo `~/` ou `file://`. |
| Colar o caminho de um arquivo de imagem | O mesmo que arrastar. |

Cada imagem entra como um marcador como `[Image #1]`. Só vão junto as imagens cujos marcadores ainda estão no prompt quando você envia; apagar o marcador tira a imagem. PNG, JPEG, GIF e WebP são aceitos, até 20 MiB cada. Se a área de transferência não tiver imagem, o chat avisa "No image on the clipboard."

Arrastar um arquivo que não é imagem insere o caminho dele como texto.

## Rodar comandos de shell com !

Comece um prompt com `!` para rodar um comando você mesmo, sem o agente:

```text
!cargo test -p domain
```

Digitar `!` com o prompt vazio entra no **modo shell**: a borda e a marca do prompt ficam rosa (`! `) e a dica diz "Run a shell command · its output goes with your next message · esc leaves". `Backspace` ou `Esc` com o prompt vazio saem do modo shell. Colar um texto que começa com `!` num prompt vazio também entra nele.

O comando roda na pasta do projeto e a saída aparece na conversa como uma entrada de ferramenta `!`, marcada como falha quando ele termina com erro. O comando e a saída não são enviados sozinhos: vão, uma única vez, no começo do seu próximo prompt, apresentados como "I ran this in the shell myself", para o agente ver o que você viu.

Comandos que o agente deixou rodando em segundo plano aparecem em `/bashes`; `/bashes kill <id>` para um deles. `Ctrl+B` manda para segundo plano o comando que o agente está rodando.

## Guardar uma nota com #

Comece um prompt com `#` para salvar uma nota na memória do projeto em vez de enviá-la:

```text
# os testes rodam com cargo nextest
```

O chat responde `Remembered: os testes rodam com cargo nextest`. Veja [Memória](./memory).

## Comandos de barra e skills

Um prompt que começa com `/` seguido de uma palavra com cara de comando é um comando. Digitar `/` mostra no painel os comandos, os apelidos e as suas skills; `↑`/`↓` escolhem e `Tab` completa. Um caminho como `/Users/me/notes.txt o que é isto?` é enviado como prompt normal. Um erro de digitação ganha sugestão: `/modle is not a command or skill. Did you mean /model?`

Qualquer `/nome` que não seja embutido roda a skill ou o comando personalizado com esse nome, com o resto da linha como argumentos. Veja [Comandos de barra](./slash-commands) e [Skills](./skills).

## Enquanto o agente trabalha

Uma linha com um indicador mostra o que o agente está fazendo, o tempo e os tokens escritos: `Working… (12s · ↓ 1.2k tokens · esc to interrupt)`.

### Interromper

`Esc` ou `Ctrl+C` param a execução atual. O que estava na fila volta para o prompt, para você editar.

### Orientar no meio do caminho

Não é preciso esperar. Digite uma mensagem e aperte `Enter` enquanto o agente trabalha: ela aparece como `› sua mensagem  (queued)` e é entregue ao agente dentro da mesma execução, no próximo passo dele. A dica do prompt diz "Type to message the agent as it works".

- `↑` com o prompt vazio pega de volta as mensagens ainda não entregues, para você editar (a dica diz "↑ edits what is queued · esc interrupts").
- Se a execução terminar antes de o agente pegar a mensagem, ela vai como o próximo prompt.
- Se você interromper a execução, as mensagens da fila voltam para o prompt.

### Fazer uma pergunta paralela

`/btw <pergunta>` pergunta algo sem interromper e sem entrar na conversa. A resposta aparece no painel; `Esc`, `Enter` ou `Espaço` fecham. Enquanto o agente trabalha, a pergunta paralela enxerga a conversa até o último passo concluído do agente.

### Responder perguntas e pedidos de permissão

Quando o agente precisa de você, o painel fica amarelo e mostra há quanto tempo ele espera (`the agent waits for your answer · 8s`); o cronômetro da execução pausa enquanto isso.

| Pedido | Teclas |
|---|---|
| Permissão (`Allow Bash: cargo test?`) | `y` sim · `a` sim e não perguntar de novo (quando oferecido) · `n` não · `↑`/`↓` + `Enter` · `Esc` nega |
| "No, and tell the agent what to do instead" | Digite a nota, `Enter` envia, `Esc` só diz não |
| Perguntas do agente | Um número escolhe, `↑`/`↓` + `Enter`, `Espaço` marca numa escolha múltipla, `Esc` pula |
| Aprovação de plano | **Yes, and auto-accept edits**, **Yes, and manually approve edits**, **No, keep planning**; um número ou `↑`/`↓` + `Enter`; `Esc` continua planejando |

`PgUp`/`PgDn` rolam a conversa enquanto um pedido espera, por exemplo para ler um plano longo. Se o terminal não estiver em foco quando o agente pergunta ou termina, o chat toca o sino do terminal.

### Tarefas

Para trabalhos com vários passos, o agente mantém uma lista de tarefas, mostrada no painel como `Tasks  2 of 5 done · ctrl+t hides`, com `✓` feita, `●` em andamento e `○` pendente. `Ctrl+T` mostra ou esconde; `/todos` (ou `/tasks`) imprime a lista na conversa.

## Depois de uma execução

- **Título**: depois da primeira execução, a conversa ganha um título curto, mostrado na linha de status, no título da janela do terminal e no `/resume`.
- **Próximo prompt sugerido**: uma sugestão pode aparecer em cinza no prompt vazio, seguida de `(tab)`; `Tab` aceita.
- **Aviso**: o agente pode acrescentar um aviso `Heads-up:` sobre algo que você precisa saber.
- **Falha**: a linha de status mostra `✗ last run failed` e o chat diz `/retry sends it again from where it started.` Quando o provedor estava indisponível, o chat tenta de novo sozinho uma vez depois de 30 segundos (`↻ Retrying in 30s · the provider was unavailable · esc cancels`); `Esc` com o prompt vazio cancela.

## Revisar alterações

| Comando | O que mostra |
|---|---|
| `/diff` | As alterações não commitadas do projeto, na própria conversa, mais os arquivos não rastreados (`● New, not yet in git: …`). |
| `/diffs` | Um navegador de diffs em tela cheia. |

O navegador de diffs começa nas alterações não commitadas; `Tab` alterna o escopo: uncommitted → unstaged → staged → branch (tudo desde que a branch saiu de `origin/HEAD`, `main`, `master` ou do upstream). Teclas:

| Tecla | Ação |
|---|---|
| `↑` / `↓` | Arquivo anterior / próximo |
| `k` / `j` | Rola uma linha para cima / baixo |
| `PgUp` / `PgDn` ou `Espaço` | Rola uma página |
| `n` / `p` | Próximo / anterior trecho (hunk) |
| `Tab` | Próximo escopo |
| `r` | Recarregar |
| `Esc` ou `q` | Fechar |

Os dois precisam de um repositório git.

## Sessões em paralelo

`←` com o prompt vazio (fora do modo shell) abre a lista **Sessions**: a conversa atual (`· here`) e as que você deixou rodando, cada uma marcada `working`, `needs you`, `done` ou `idle`, e depois as conversas anteriores desta pasta, com quando foram usadas pela última vez. O `/fleet` (ou `/sessions`) também a abre. `↑`/`↓` movem; `Enter` troca para a sessão escolhida, ou continua a conversa anterior escolhida, e a que você deixa continua trabalhando; `n` começa uma sessão nova e deixa a atual trabalhando sozinha; `Esc` volta.

## Linha de status

A linha do rodapé mostra, separados por `·`:

1. O título da conversa, quando já tem um.
2. O modelo.
3. O modo de permissão (`ask`, `edits` em amarelo, `plan` em azul, `bypass` em vermelho).
4. O esforço (`medium effort`, ou `auto effort`).
5. `✗ last run failed`, depois de uma falha.
6. Marcadores de recursos em verde: um orçamento (`$0.42/$5.00`), verificação (`verify (yours)`), a contagem regressiva do cache de prompt (`cache 1:30`, depois `cache cold: …`).
7. O contexto restante (`72% context left`).
8. Comandos em segundo plano (`2 background`).
9. O custo da sessão (`$0.42`).

À direita, quando cabe: `shift+tab mode · /help`. A linha também mostra `Press Ctrl+C again to quit` e `Press Esc again to clear the prompt` (ou, com o prompt vazio, `Press Esc again to go back to an earlier prompt`) quando essas ações estão armadas.

`/statusline <comando>` troca a linha pela saída do seu próprio comando; veja [Linha de status](./statusline).

## Mouse e rolagem

Em tela cheia o chat captura o mouse: a roda rola 3 linhas, `PgUp`/`PgDn` rolam 10. Arrastar seleciona texto, e soltar copia para a área de transferência (com `pbcopy`, `wl-copy` ou `xclip`).

## Modo inline

```bash
{{COMMAND}} --inline
```

Com `--inline` o chat ocupa só as 16 últimas linhas do terminal em vez da tela inteira. Cada troca concluída é impressa acima dele, no histórico de rolagem do próprio terminal, assim que o agente fica parado, então você rola e seleciona com o terminal como sempre (o mouse não é capturado).

## Sair

`Ctrl+C` com o prompt vazio arma a saída (`Press Ctrl+C again to quit`); aperte de novo em até 2 segundos. `Ctrl+D` com o prompt vazio, `/exit` ou `/quit` saem na hora. Depois de uma conversa, o terminal mostra como continuá-la: `{{COMMAND}} -c`.
