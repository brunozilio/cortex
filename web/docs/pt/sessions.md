---
title: Sessões
description: Continue, retome, faça fork, ramifique, volte atrás, repita, compacte, exporte e copie conversas, e veja onde elas ficam guardadas.
---

Cada conversa com o {{NAME}} é uma sessão: ela é salva enquanto você trabalha, ganha um título e pode ser continuada depois, copiada, cortada até um prompt anterior ou resumida. Esta página mostra como.

## Continuar de onde parou

Ao sair depois de uma conversa, o terminal mostra:

```text
Continue this conversation with: {{COMMAND}} -c
```

| Comando | O que faz |
|---|---|
| `{{COMMAND}} -c` / `--continue` | Continua a última conversa desta pasta. Se não houver nenhuma, avisa e começa uma nova. |
| `{{COMMAND}} -r` / `--resume` | Abre o chat no seletor das conversas anteriores desta pasta. |
| `{{COMMAND}} -r <id>` | Continua a conversa com esse id de sessão. |
| `{{COMMAND}} -c --fork-session` | Continua numa cópia, deixando a original como está (também com `-r`). |

`-c` e `-r` também funcionam com `-p` para respostas únicas; veja [Headless](./headless).

```bash
{{COMMAND}} -c "agora escreva a documentação"
{{COMMAND}} -r 2b0c…e41 --fork-session
```

Ao continuar uma conversa, o chat diz `Continuing the last conversation in this folder.` (ou `Continuing an earlier conversation.`) e mostra um resumo curto: `Where you left off: …`.

> [!TIP]
> `/status` mostra o id da sessão atual.

## Retomar uma conversa

Dentro do chat, `/resume` (apelido `/continue`) abre **Resume a conversation**: as conversas anteriores desta pasta, da mais recente para a mais antiga, cada uma com o título (ou `Untitled`) e quando foi usada pela última vez (`just now`, `5m ago`, `2h ago`, `3d ago`).

```text
/resume
/resume bug login
```

Com palavras, só aparecem as conversas cujo título contém todas elas (sem diferenciar maiúsculas). A lista inclui conversas rodadas nesta pasta por outros clientes, como o navegador, mas deixa de fora a atual. `↑`/`↓` e `Enter` escolhem; `Esc` fecha. O `/resume` fica indisponível enquanto o agente trabalha.

## Títulos

Depois da primeira execução, uma tarefa em segundo plano dá à conversa um título curto. O título aparece no começo da linha de status, no título da janela do terminal (`<título> · {{NAME}}`), no `/resume`, no nome do arquivo do `/export` e no [console de trace](./trace-console).

## Começar do zero

`/clear` (também `/new`, `/reset`) começa uma conversa nova com o mesmo modelo. A anterior fica guardada e aparece no `/resume`.

## Fazer fork de uma conversa

`/fork` faz a próxima mensagem continuar numa cópia desta conversa; a original fica como está e pode ser retomada depois. Precisa de pelo menos uma mensagem antes. `/branch` sem argumento lista os prompts de onde você pode ramificar. Na linha de comando, `--fork-session` com `-c` ou `-r` faz o fork da conversa que você continua.

## Ramificar a partir de um prompt anterior

`/branch <n>` começa uma conversa nova a partir de antes do prompt número `n` desta, mantendo a original:

```text
/branch 2
```

O chat corta a conversa para antes daquele prompt, coloca o prompt na entrada para você editar e avisa que a original continua no `/resume`. Os arquivos no disco não mudam. Os prompts são numerados do mais antigo para o mais novo. Ramificar precisa do recurso `branches` e fica indisponível enquanto o agente trabalha. Veja [Branches e git](./branches-and-git).

## Voltar atrás (rewind)

Cada prompt que você envia é um checkpoint. Voltar atrás leva para antes de um deles:

- `Esc` `Esc` com o prompt vazio (o segundo toque em até um segundo), ou
- `/rewind` (apelido `/undo`).

Um seletor **Rewind to before…** lista os prompts desta conversa, do mais recente para o mais antigo. Escolher um:

1. Restaura os arquivos que o agente mudou desde aquele prompt.
2. Corta a conversa para antes dele.
3. Devolve aquele prompt à entrada, para você mudar e enviar de novo.

O chat informa o que aconteceu, por exemplo `Rewound: 3 files restored. Changes made with Bash are not undone.`

> [!WARNING]
> O rewind restaura o que as ferramentas de arquivo do agente mudaram. Alterações feitas com comandos Bash não são desfeitas. O recurso `destructive-snapshot` guarda os arquivos não rastreados antes de comandos como `rm -r` ou `git reset --hard`, para o rewind poder trazê-los de volta; veja [Sandbox e segurança](./sandbox-and-safety).

Um arquivo que você mudou depois que o agente o deixou não é sobrescrito: o rewind o deixa como está e o lista com o motivo (`Left as they are: notes.txt (it changed after the agent left it)`). Um arquivo maior que 10 MiB, ou que não pôde ser lido, não tem a versão anterior guardada: a ferramenta que o gravou diz isso no resultado (`/rewind cannot put back the earlier version of …`), e um rewind o deixa como está e o lista com esse motivo. Um arquivo que não consegue voltar (uma pasta que virou só leitura, um disco cheio) é listado com o erro, e todos os outros arquivos continuam sendo restaurados. A conversa é salva antes da lista de checkpoints, então um rewind que falha ao salvar nunca perde os checkpoints deixando a conversa como estava; rode-o de novo, e os arquivos que já voltaram contam como restaurados. Os arquivos voltam inteiros (uma queda no meio nunca deixa um arquivo pela metade), mantêm suas permissões, e um link posto no lugar de um arquivo é substituído em vez de seguido. Um arquivo que o agente editou por um link simbólico é salvo e restaurado para onde o link leva, e o link continua link. O estado de cada arquivo é salvo antes da primeira escrita do agente nele, então um prompt interrompido por uma queda o guarda; mas o que o agente deixou em cada arquivo nunca foi anotado, então um rewind depois da queda não distingue as suas correções posteriores do trabalho do agente: ele lista esses arquivos como deixados como estão, e você pode restaurá-los à mão.

Um arquivo de conversa cuja última linha foi cortada no meio de uma gravação (mesmo no meio de um caractere) é reparado na próxima gravação: a linha cortada é descartada e as gravações continuam.

Se a conversa foi compactada depois daquele prompt, os arquivos são restaurados mas a conversa fica como está (`the conversation was compacted since, so it is kept`). O rewind fica indisponível enquanto o agente trabalha.

## Repetir (retry)

`/retry` envia o último prompt de novo, de onde ele começou: volta para antes daquele prompt (restaurando arquivos) e o envia mais uma vez. Sem checkpoint, só reenvia o último prompt. Depois de uma execução que falhou, o chat sugere o comando (`/retry sends it again from where it started.`), e quando o provedor estava indisponível ele tenta de novo sozinho uma vez depois de 30 segundos.

## Compactar a conversa

Conversas longas enchem a janela de contexto do modelo. Compactar troca o histórico por um resumo para você poder continuar:

```text
/compact
/compact mantenha as decisões sobre a API e os TODOs em aberto
```

O texto depois de `/compact` diz ao resumo o que manter. O chat mostra `Compacting the conversation…` enquanto trabalha. A compactação também acontece sozinha quando o contexto chega perto do limite. `/context` mostra quanto está usado e quanto está livre antes de compactar; a linha de status mostra `NN% context left`.

O `/compact` fica indisponível enquanto o agente trabalha.

## Exportar e copiar

| Comando | O que faz |
|---|---|
| `/export` | Grava a conversa em Markdown na pasta do projeto, com o nome do título (`fix-the-login-page.md`) ou `conversation.md`. |
| `/export <caminho>` | Grava nesse caminho (relativo à pasta do projeto). |
| `/copy` | Copia a última resposta do agente para a área de transferência (`pbcopy`, `wl-copy` ou `xclip`). |

A exportação começa com o título como cabeçalho e traz seus prompts como citações, as respostas, uma linha por chamada de ferramenta (com `(failed)` quando falhou), sub-agentes, planos e diffs. Avisos, raciocínio e erros ficam de fora.

## Sessões em paralelo

`←` com o prompt vazio abre a lista de sessões; `n` ali começa uma sessão nova enquanto a atual continua trabalhando. Veja [Chat](./chat#sessoes-em-paralelo).

## Onde as sessões ficam

As sessões ficam na sua pasta de dados, não no projeto:

| Sistema | Pasta de dados |
|---|---|
| macOS | `~/Library/Application Support/{{NAME}}` |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}`, por padrão `~/.config/{{COMMAND}}` |
| Windows | `%LOCALAPPDATA%\{{NAME}}` para `native-sessions/`, `%APPDATA%\{{NAME}}` para `app.db` e `users/` |

Dentro dela:

| Caminho | Guarda |
|---|---|
| `native-sessions/` | A conversa, o trace e os checkpoints de cada sessão. |
| `app.db` | Um banco SQLite com a lista de conversas (pasta, título, status, último uso) que o `/resume`, a lista de sessões e o console de trace mostram, e as suas memórias. |
| `users/` | As configurações do chat por usuário: último modelo, modo, esforço, histórico de prompts, a última sessão de cada pasta e o título dela. |

Veja [Arquivos e pastas](./files-and-directories). O `/factory-reset` apaga tudo isso.
