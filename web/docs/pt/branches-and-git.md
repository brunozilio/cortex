---
title: Branches e git
description: Ramifique uma conversa a partir de um prompt anterior, deixe o {{NAME}} preparar o git, isole o trabalho em worktrees, acompanhe um pull request até o merge e rode várias sessões na mesma pasta.
---

O {{NAME}} funciona melhor num repositório git: as mudanças podem ser revisadas com `/diff`, desfeitas e isoladas em worktrees. Esta página trata dos branches de conversa que você cria pelo chat e das funcionalidades de git em volta deles.

## Ramificar a partir de um prompt anterior

> [!NOTE]
> Controlado pela flag `branches`. Padrão: ligada.

O `/branch <n>` começa uma **conversa nova** com tudo o que veio antes do seu n-ésimo prompt desta, e deixa esta conversa como está. Os prompts são numerados a partir de 1, do mais antigo para o mais novo, na ordem em que você os enviou.

```text
/branch 2
Branched: this is a new conversation from before that prompt; the files are as they were. The original stays in /resume.
```

Depois de ramificar:

- o chat mostra a conversa nova, cortada logo antes do prompt escolhido;
- esse prompt volta para a caixa de entrada, para você editá-lo e enviar outra versão;
- a conversa original fica inteira e pode ser reaberta com `/resume`.

Ramificar nunca muda arquivos no disco. Para também voltar os arquivos ao estado daquele prompt, use o `/rewind` (veja [Sessões](./sessions)).

> [!NOTE]
> O `/branch` sem número lista os prompts de onde você pode ramificar, numerados do mais antigo para o mais novo (`Nothing to branch from in this chat yet.` quando não há nenhum). Para continuar numa cópia da conversa inteira, use `/fork`.

A ramificação é recusada enquanto o agente trabalha (`Wait for the agent to finish, or press Esc, before branching.`), para um número que não corresponde a nenhum prompt (`There is no such prompt; /branch lists them.`) e quando a conversa foi compactada depois daquele prompt (`the conversation was compacted since then, so it cannot branch there`).

## Inicializar o git em uma pasta nova

> [!NOTE]
> Controlado pela flag `git-offer`. Padrão: ligada.

Quando você envia a primeira mensagem de uma conversa nova numa pasta que não está dentro de um repositório git, o {{NAME}} segura a mensagem e pergunta:

| Resposta | O que acontece |
|---|---|
| **Yes, initialize git** | Roda `git init` e faz um primeiro commit vazio (`Initial commit`), depois envia a sua mensagem |
| **No, not now** | Envia a sua mensagem sem repositório; a pergunta pode voltar na próxima vez que você iniciar o {{NAME}} aqui |
| **No, and don't offer again** | Desliga a flag `git-offer` nas suas configurações e envia a sua mensagem |

Se o git não conseguir fazer o primeiro commit (geralmente porque `user.name` e `user.email` não estão definidos), o repositório é criado mesmo assim e o {{NAME}} avisa que os agentes rodam sem isolamento em worktree até existir um commit. O `/features git-offer on` traz a oferta de volta.

## Worktrees

Uma worktree do git é um checkout separado do mesmo repositório. O {{NAME}} cria as suas worktrees em `{{DIR}}/worktrees/<id>` na raiz do repositório, destacadas (detached) no commit atual (`HEAD`), então elas têm os commits da branch em que você está. Mudanças não commitadas, em stage ou não, e arquivos não rastreados não vão junto: faça o commit antes se a worktree precisar deles. O resultado do EnterWorktree diz o commit (hash curto e assunto) e quantas mudanças não commitadas ficaram para trás.

As worktrees precisam de um repositório com pelo menos um commit. Sem isso, o trabalho roda na pasta do projeto e o resultado diz `Ran without isolation: this folder is not a git repository with a commit, so no worktree was made.`

### Um subagente na sua própria worktree

Quando o agente inicia um subagente que edita arquivos, pode pedir `isolation: "worktree"`. O subagente então trabalha numa worktree nova. Ao terminar, uma worktree sem commits e sem arquivos não commitados é removida; uma em que o subagente mudou algo é mantida, e o caminho dela é informado para as mudanças serem revisadas e integradas. Veja [Agentes](./agents).

### Mover a sessão inteira para uma worktree

Peça ao agente para trabalhar numa worktree ("faça isso numa worktree separada"). Ele usa duas ferramentas:

- **EnterWorktree** cria uma worktree e move a sessão para lá: a partir do próximo passo, arquivos e comandos ficam na worktree, assim como os prompts seguintes da sessão, até ela sair.
- **ExitWorktree** volta ao checkout do projeto, com `action: "keep"` para deixar a worktree no disco (o caminho é informado) ou `action: "remove"` para apagá-la. A remoção é recusada enquanto a worktree tem arquivos não commitados ou commits próprios, a menos que `discard_changes` seja true.

Só o agente principal pode mover a sessão, e uma worktree por vez. Uma sessão retomada volta para a sua worktree; se a worktree foi apagada, a retomada falha com `The session worktree no longer exists; restore it before resuming`.

## Acompanhar um pull request até o merge

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-pr-follow`. Padrão: ligada. Com a flag desligada, a skill `follow-pr` fica escondida do agente.

A skill embutida `follow-pr` mantém um pull request andando até ele ser integrado ou fechado. Peça depois de abrir um PR:

```text
acompanhe o PR até ele ser integrado
```

Ela precisa da CLI do GitHub (`gh`) autenticada; se `gh auth status` falhar, o agente avisa e para. Depois ele:

1. encontra o PR que você indicou, ou o PR do branch atual;
2. inicia um **Monitor** que confere o PR a cada 60 segundos e imprime uma linha só quando muda o estado, o status de merge, as checagens que falharam, as revisões ou os comentários, de modo que cada mudança acorda o agente;
3. numa checagem que falhou, lê o log (`gh run view <id> --log-failed`), corrige a causa, roda a mesma checagem localmente, faz commit e push;
4. numa revisão ou comentário novo, muda o código quando é pedida uma mudança, ou responde uma pergunta com `gh pr comment`; nunca resolve a conversa de um revisor;
5. quando o PR é integrado ou fechado, para o monitor e avisa você.

O agente pergunta antes de qualquer coisa que não dá para desfazer, como um force push, e nunca faz o merge do PR a menos que você tenha pedido.

## Várias sessões na mesma pasta

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-session-claims`. Padrão: ligada.

Você pode rodar várias sessões do {{NAME}} no mesmo projeto. Cada vez que o agente edita ou escreve um arquivo, a sessão registra uma reivindicação sobre ele. Se outra sessão que ainda está rodando mudou o mesmo arquivo há menos de 10 minutos, o resultado da ferramenta avisa o agente:

```text
<other-session>Another session (process 48121) changed this file 3 min ago. Its work and yours may overwrite each other: re-read the file before editing it again, and tell the user if you both need it.</other-session>
```

As edições da própria sessão nunca contam como conflito. As reivindicações de sessões que terminaram, ou com mais de uma hora, são descartadas. Elas ficam na pasta temporária do sistema, em `{{COMMAND}}-claims/`, um arquivo por projeto.

> [!TIP]
> Para duas sessões que não devem mexer nos arquivos uma da outra, ponha uma delas para trabalhar numa worktree.

## Relacionados

- [Sessões](./sessions): `/resume`, `/fork`, `/rewind`.
- [Verificação e revisão](./verification-and-review): checagens que rodam antes de o agente terminar.
- [Skills](./skills): como skills embutidas como a `follow-pr` são carregadas.
