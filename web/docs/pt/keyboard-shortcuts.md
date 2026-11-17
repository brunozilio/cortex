---
title: Atalhos de teclado
description: Todas as teclas que o chat entende, no prompt, nos seletores, nas perguntas, no navegador de diffs e no modo vim.
---

Esta página lista todos os atalhos do chat. As teclas dependem do que está em foco: o prompt, um seletor, uma pergunta do agente, o navegador de diffs ou a lista de sessões. Dentro do chat, `/help` mostra uma linha com as mais usadas.

> [!NOTE]
> No macOS, combinações com `Cmd` só chegam ao chat se o terminal as repassar. `Shift+Enter` só funciona em terminais que informam essa tecla; `/terminal-setup` explica como conseguir.

## As mais usadas

| Tecla | Ação |
|---|---|
| `Enter` | Envia o prompt |
| `Alt+Enter` / `Ctrl+J` | Nova linha |
| `Esc` | Interrompe o agente |
| `Esc` `Esc` | Volta atrás (rewind), com o prompt vazio |
| `Shift+Tab` | Próximo modo de permissão |
| `Ctrl+R` | Busca prompts anteriores |
| `Ctrl+G` | Edita o prompt no `$EDITOR` |
| `Ctrl+V` | Anexa a imagem da área de transferência |
| `Ctrl+O` | Expande ou dobra saídas |
| `Ctrl+L` | Redesenha a tela |
| `PgUp` / `PgDn` | Rola |
| `Ctrl+C` | Interrompe, limpa ou sai |

## Em qualquer lugar

Funcionam seja o que for que estiver em foco.

| Tecla | Ação |
|---|---|
| `Ctrl+C` | Se o agente está trabalhando, interrompe. Senão, se o prompt tem texto, limpa. Senão arma a saída; um segundo `Ctrl+C` em até 2 segundos sai. |
| `Ctrl+O` | Expande ou dobra raciocínio, saídas de ferramentas, passos de sub-agentes e diffs. |
| `Ctrl+T` | Mostra ou esconde a lista de tarefas do agente. |
| `Ctrl+B` | Manda para segundo plano o comando que o agente está rodando. |

## Prompt: enviar e quebrar linhas

| Tecla | Ação |
|---|---|
| `Enter` | Envia o prompt (ou roda o comando). Enquanto o agente trabalha, a mensagem entra na fila e é entregue a ele na mesma execução. |
| `Alt+Enter` | Insere uma nova linha. |
| `Shift+Enter` | Insere uma nova linha, nos terminais que informam essa tecla. |
| `Ctrl+J` | Insere uma nova linha. |
| `Tab` | Aceita o próximo prompt sugerido mostrado no prompt vazio; senão completa o arquivo `@` ou o comando `/` destacado. |
| `Shift+Tab` | Alterna o modo de permissão: ask → edits → plan → bypass → ask. |
| `Ctrl+G` | Abre o prompt no `$VISUAL`, `$EDITOR` ou `vi`; o texto editado o substitui. |
| `Ctrl+V` | Anexa a imagem da área de transferência como `[Image #n]`. |
| `Ctrl+R` | Busca no histórico de prompts. |
| `Ctrl+L` | Limpa e redesenha a tela. |
| `Ctrl+D` | Sai, quando o prompt está vazio. |

## Prompt: mover e apagar

| Tecla | Ação |
|---|---|
| `←` / `→` | Move um caractere. |
| `Alt+←` / `Alt+→`, `Ctrl+←` / `Ctrl+→` | Move uma palavra. |
| `Alt+B` / `Alt+F` | Move uma palavra para trás / para a frente. |
| `Home` / `Ctrl+A`, `Cmd+←` | Início da linha. |
| `End` / `Ctrl+E`, `Cmd+→` | Fim da linha. |
| `↑` / `↓` | Move entre as linhas de um prompt com várias linhas. |
| `Backspace` | Apaga o caractere antes do cursor. |
| `Delete` | Apaga o caractere sob o cursor. |
| `Alt+Backspace`, `Ctrl+W` | Apaga a palavra antes do cursor. |
| `Alt+D`, `Alt+Delete`, `Ctrl+Delete` | Apaga a palavra depois do cursor. |
| `Cmd+Backspace` | Apaga até o início da linha. |
| `Ctrl+U` | Limpa o prompt inteiro. |
| `Esc` `Esc` | Limpa o prompt (quando tem texto): o primeiro `Esc` arma (`Press Esc again to clear the prompt`), o segundo limpa. Com o prompt vazio a linha diz `Press Esc again to go back to an earlier prompt`, e um segundo `Esc` dentro de um segundo abre a lista de rewind. |

## Prompt: histórico, sugestões e começos especiais

| Tecla | Ação |
|---|---|
| `↑` na primeira linha | Traz o prompt anterior do histórico desta pasta. |
| `↓` na última linha | Vai ao próximo prompt do histórico, e depois volta ao prompt vazio. |
| `↑` / `↓` com sugestões abertas | Escolhe entre os arquivos `@` ou comandos `/` oferecidos; a lista dá a volta. |
| `↑` com o prompt vazio enquanto o agente trabalha | Pega de volta as mensagens da fila ainda não entregues ao agente, para editar. |
| `!` com o prompt vazio | Entra no modo shell. |
| `Backspace` ou `Esc` com o prompt vazio no modo shell | Sai do modo shell. |
| `←` com o prompt vazio | Abre a lista de sessões. |
| `Esc` enquanto o agente trabalha | Interrompe. |
| `Esc` com o prompt vazio durante a contagem de uma nova tentativa automática | Cancela a nova tentativa. |
| `Esc` `Esc` com o prompt vazio (em até 1 segundo) | Abre o seletor de rewind. |
| `PgUp` / `PgDn` | Rola a conversa 10 linhas. |

## Busca no histórico (Ctrl+R)

O prompt mostra `search '<busca>': <resultado>`.

| Tecla | Ação |
|---|---|
| Digitar | Refina a busca (sem diferenciar maiúsculas); aparece o resultado mais recente. |
| `Backspace` | Apaga um caractere da busca. |
| `Ctrl+R` | Próximo resultado mais antigo. |
| `Enter`, `Tab`, `→` | Coloca o resultado no prompt. |
| `Esc`, `Ctrl+G` | Sai da busca. |

## Seletores

Os seletores abrem em `/model`, `/mode`, `/effort`, `/output-style`, `/config`, `/resume`, `/rewind`, `/login`, `/logout`, `/accounts` e `/model <papel>`.

| Tecla | Ação |
|---|---|
| `↑` / `↓` | Move (dá a volta). |
| `Enter` | Escolhe. |
| `Esc` | Fecha sem escolher. |

## Campo de chave de API

Aparece depois de escolher em `/login` um provedor que usa chave de API.

| Tecla | Ação |
|---|---|
| Digitar ou colar | Informa a chave (mostrada como pontos). |
| `Backspace` | Apaga um caractere. |
| `Enter` | Conecta. |
| `Esc` | Cancela. |

## Perguntas e pedidos de permissão

| Tecla | Pedido de permissão | Pergunta | Aprovação de plano |
|---|---|---|---|
| `y` | Sim | | |
| `a` | Sim, e não perguntar de novo (quando oferecido) | | |
| `n` | Não | | |
| `1`–`9` | Escolhe a opção | Escolhe a opção (marca/desmarca numa escolha múltipla) | Escolhe a opção |
| `↑` / `↓` | Move | Move | Move |
| `Espaço` | | Marca/desmarca a opção numa escolha múltipla | |
| `Enter` | Confirma | Confirma | Confirma |
| `Esc` | Nega | Pula | Continua planejando |
| `PgUp` / `PgDn` | Rola a conversa | Rola a conversa | Rola a conversa |

Depois de **No, and tell the agent what to do instead**, digite sua nota: `Enter` envia (uma nota vazia só diz não), `Backspace` apaga, `Esc` diz não sem nota.

## Pergunta paralela (/btw)

| Tecla | Ação |
|---|---|
| `Esc`, `Enter`, `Espaço` | Fecha a resposta. |

## Navegador de diffs (/diffs)

| Tecla | Ação |
|---|---|
| `↑` / `↓` | Arquivo anterior / próximo. |
| `k` / `j` | Rola uma linha para cima / baixo. |
| `PgUp` | Rola uma página para cima. |
| `PgDn`, `Espaço` | Rola uma página para baixo. |
| `n` / `p` | Próximo / anterior trecho (hunk). |
| `Tab` | Próximo escopo: uncommitted → unstaged → staged → branch. |
| `r` | Recarrega. |
| `Esc`, `q` | Fecha. |

## Lista de sessões

Aberta com `←` no prompt vazio, ou com `/fleet` (`/sessions`).

| Tecla | Ação |
|---|---|
| `↑` / `↓` | Move (dá a volta). |
| `Enter` | Troca para a sessão escolhida, ou retoma a conversa anterior escolhida; a que você deixa continua trabalhando. |
| `n` | Começa uma sessão nova; a atual continua trabalhando. |
| `Esc` | Volta ao chat. |

## Modo vim

`/vim` liga ou desliga as teclas do vim; a escolha fica guardada. No modo vim o prompt começa no modo de inserção, onde as teclas funcionam como sempre. `Esc` passa para o modo normal (então, com o vim ligado, interromper o agente com `Esc` exige um segundo `Esc`; `Ctrl+C` sempre interrompe).

Teclas do modo normal:

| Tecla | Ação |
|---|---|
| `h` / `l`, `←` / `→` | Esquerda / direita. `Backspace` também move para a esquerda. |
| `w` | Para o começo da próxima palavra. |
| `b` | Para o começo da palavra anterior. |
| `0`, `^` | Início da linha. |
| `$` | Fim da linha. |
| `j` / `k` | Linha de baixo / de cima. |
| `x` | Apaga o caractere sob o cursor. |
| `D` | Apaga até o fim da linha. |
| `dd` | Limpa o prompt. |
| `dw` | Apaga a próxima palavra. |
| `i` | Insere antes do cursor. |
| `a` | Insere depois do cursor. |
| `I` | Insere no início da linha. |
| `A` | Insere no fim da linha. |
| `o` | Abre uma linha nova abaixo e insere. |

Outras letras não fazem nada no modo normal. `Enter`, `↑`/`↓` e as teclas com `Ctrl` funcionam como no modo de inserção.

## Mouse

Em tela cheia (sem `--inline`):

| Ação | Efeito |
|---|---|
| Roda | Rola 3 linhas. |
| Arrastar | Seleciona texto; soltar copia para a área de transferência. |
