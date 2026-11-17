---
title: Memória
description: Como o {{NAME}} lembra fatos, procedimentos e trabalhos anteriores por projeto, como ler e editar essa memória com /memory e como compartilhá-la com a equipe.
---

O {{NAME}} mantém uma memória para cada projeto. Depois de um turno que ensinou ao agente algo que vale guardar, ele salva uma nota curta; no começo dos prompts seguintes, as notas que combinam com o que você pediu são trazidas de volta para a conversa. Você pode listar, buscar, adicionar, compartilhar e desfazer memórias pelo chat.

## Como a memória funciona

Cada memória tem uma ou duas frases e é de um de três tipos:

| Tipo | Guarda | Aparece para o agente como |
|---|---|---|
| Semântica | Um fato duradouro sobre o projeto ou sobre você | Facts |
| Procedural | Como algo é feito aqui: comandos, fluxos de trabalho | Procedures |
| Episódica | O que aconteceu num turno, no máximo uma por turno | Episodes |

Quando você envia um prompt, o {{NAME}} ranqueia as memórias do projeto em relação a ele e acrescenta as que mais combinam à conversa, agrupadas em **Facts**, **Procedures** e **Episodes**, com o id de cada uma entre colchetes. São lembrados até 40 fatos e procedimentos e 8 episódios. Os episódios perdem peso com o tempo (o peso cai pela metade a cada 30 dias, e cada vez que um é lembrado ele dura mais).

Quando a memória do projeto tem mais do que foi lembrado, o agente também ganha a ferramenta `memory_search`. Nesse caso ele lembra menos memórias de saída (15 e 3) e busca quando você se refere a um trabalho anterior ou quando a tarefa depende de uma decisão passada. Uma busca devolve até 8 resultados por padrão (no máximo 25) e nunca devolve uma memória que já está na conversa.

No fim de um turno, se aprendeu algo novo, o agente escreve as mudanças que quer: memórias para lembrar, esquecer, revisar ou reclassificar. O {{NAME}} as aplica:

- o texto é limpo: tokens, chaves e valores depois de nomes como `password` ou `token` viram `[redacted]`, e a memória é cortada em 600 caracteres;
- uma memória nova com o mesmo texto de outra do mesmo tipo não é adicionada duas vezes, e uma memória nova quase igual a uma mais antiga é fundida com ela;
- uma memória esquecida fica guardada à parte por 30 dias para você poder trazê-la de volta.

Quando uma execução mudou a memória, o chat diz quanto: `Memory: 1 added, 1 changed, 1 forgotten (/memory lists it).` Se as mudanças não puderam ser salvas, ele diz `What the run learned was not remembered: <reason>`.

O agente é orientado de que as memórias lembradas são contexto, não instruções, que eram verdadeiras quando foram escritas e que ele deve conferir se um arquivo, função ou flag citado por uma memória ainda existe antes de confiar nela.

> [!NOTE]
> A memória é mantida pelo {{NAME}}, não em arquivos do projeto. O agente é orientado a não escrever arquivos de memória próprios e a não guardar o que o repositório já registra (estrutura do código, histórico do git, {{INSTRUCTIONS}}).

## Onde a memória fica guardada

A sua memória de um projeto é um arquivo JSON na pasta de dados local do {{NAME}}, em `memory/local/<projeto>.json`, onde `<projeto>` é o caminho do projeto com todo caractere que não seja letra ou dígito trocado por `-`. Veja [Arquivos e diretórios](./files-and-directories) para saber onde fica a pasta de dados no seu sistema.

Se esse arquivo não puder ser lido, o {{NAME}} o renomeia para `<projeto>.json.corrupt-<hora>` e começa com uma memória vazia, sem sobrescrever nada.

As memórias compartilhadas com a equipe ficam no próprio projeto, em `{{DIR}}/memory/*.md` (veja [Memória da equipe](#memoria-da-equipe)).

## Adicionar uma memória você mesmo

Comece uma mensagem com `#` para guardá-la como fato, sem enviá-la ao agente:

```text
# O banco de staging é zerado toda noite às 02:00 UTC
```

O chat responde `Remembered: …`. Memórias adicionadas assim recebem a tag `user`.

## O comando /memory

| Comando | O que faz |
|---|---|
| `/memory` | Lista as memórias do projeto com seus ids |
| `/memory <palavras>` | Busca na memória; os resultados mostram uma pontuação e o id |
| `/memory optimize` | Pede ao agente para arrumar a memória inteira (juntar, reescrever, descartar) com o modelo atual |
| `/memory undo` | Devolve a memória ao estado de antes do último `/memory optimize` |
| `/memory forgotten` | Lista as memórias esquecidas que ainda podem ser restauradas |
| `/memory restore <id>` | Traz de volta uma memória esquecida |
| `/memory share <id>` | Escreve uma memória em `{{DIR}}/memory` para a equipe |
| `/memory edit` | Abre o arquivo de instruções do projeto no seu editor |
| `/memory seed` | Oferece como memórias frases que você escreveu em sessões anteriores |
| `/memory seed keep <n> [<n>…]` | Guarda as frases oferecidas com esses números |
| `/memory score` | Compara execuções com e sem memória |

Qualquer coisa depois de `/memory` que não seja uma dessas palavras é uma busca; então `/memory undo the font` busca por "undo the font".

O `/memory edit` abre o `{{INSTRUCTIONS}}` na raiz do projeto, ou o `{{DIR}}/{{INSTRUCTIONS}}` quando só ele existe. Veja [Arquivos de instruções](./instructions-files).

### Otimizar e desfazer

```text
/memory optimize
Memory optimized: 48 → 31 memories. /memory undo puts it back.
```

O `/memory optimize` precisa de um modelo (escolha um com `/model`). O `/memory undo` restaura a memória como estava logo antes da última otimização; quando a memória muda de outro jeito depois disso, não há mais o que desfazer e ele responde `nothing to undo in the memory`.

### Esquecer e restaurar

Quando o agente esquece uma memória, ela fica guardada por 30 dias:

```text
/memory forgotten
Forgotten (/memory restore <id>, for 30 days):
  [12] The release script lives in tools/release.sh

/memory restore 12
Restored [12] The release script lives in tools/release.sh
```

## Memória da equipe

> [!NOTE]
> Controlada pela flag `team-memory`. Padrão: ligada.

Um projeto pode versionar memórias em `{{DIR}}/memory/`. Cada arquivo `.md` ali é lido como uma memória e lembrado para todo mundo que trabalha no projeto, ao lado das memórias de cada um. As memórias da equipe têm ids a partir de 1000000 e não podem ser editadas pelo chat; mude o arquivo.

Compartilhe uma das suas memórias:

```text
/memory share 7
Shared [7] as …/{{DIR}}/memory/7-run-make-check-before-pushing.md: commit it and the team's agents recall it too.
```

O nome do arquivo é o id seguido das seis primeiras palavras da memória. Um arquivo tem esta cara:

```markdown
---
kind: procedural
tags: [release, ci]
touches: [scripts/release.sh]
---
Release with `make release`; never tag by hand.
```

| Campo do front matter | Significado |
|---|---|
| `kind` | `semantic` (o padrão), `procedural` ou `episodic` |
| `tags` | Assuntos, numa lista entre colchetes |
| `touches` | Arquivos ou pastas de que a memória trata (veja [Memórias ligadas a arquivos](#memorias-ligadas-a-arquivos)) |
| `author`, `date` | Quem escreveu e quando; aparece junto da memória como `(autor, data)` |

Um arquivo sem front matter é um fato. O corpo passa pela mesma limpeza de segredos de qualquer memória e é cortado em 2.000 caracteres. Com a [`experimental-structured-learnings`](./learning#decisoes-estruturadas) ligada, o `/memory share` também escreve o seu `user.name` do git como `author` e a data de hoje, e uma decisão é escrita com as seções `## Decided`, `## Tried and ruled out` e `## Watch out`.

Com a `team-memory` desligada, os arquivos em `{{DIR}}/memory` não são lembrados (o compartilhamento continua escrevendo neles).

## Memórias ligadas a arquivos

> [!EXPERIMENTAL]
> Controlada pela flag `experimental-memory-anchors`. Padrão: ligada.

Quando o agente salva uma memória sobre um código específico, ele pode listar os arquivos ou pastas que ela **toca** (caminhos relativos ao projeto, no máximo 8). Depois, na primeira vez numa execução em que o agente lê, edita ou escreve um desses arquivos (ou um arquivo dentro de uma dessas pastas), as memórias sobre ele que ainda não tinham sido lembradas são acrescentadas ao resultado da ferramenta, até 4 por vez:

```text
<memory>Kept from earlier sessions about src/retry.rs; follow it unless the user says otherwise, and say why when you go against it:
- [4] Keep retries at 4: the upstream rate limit resets every 5 attempts
</memory>
```

Uma memória salva de novo com o mesmo texto ganha os novos arquivos na sua lista.

## Semear a memória com sessões anteriores

> [!EXPERIMENTAL]
> Controlada pela flag `experimental-memory-seed`. Padrão: ligada.

O `/memory seed` lê os prompts que você enviou nas sessões recentes deste projeto e oferece as frases que você escreveu, com palavras parecidas, em duas ou mais delas, pulando as que a memória já tem. São oferecidas até 10, as mais repetidas primeiro:

```text
/memory seed
Said in two or more past sessions (/memory seed keep <n> [<n>…] remembers them):
  1. run the tests with cargo nextest (3 sessions)
  2. never push to main directly (2 sessions)

/memory seed keep 1 2
```

Só contam frases de 12 a 240 caracteres com pelo menos três palavras significativas; perguntas, prompts com blocos de código e linhas que começam com `$`, `/`, `!`, `#`, `>` ou `|` são ignorados. As frases guardadas viram memórias procedurais com a tag `seeded`.

Com a flag desligada, o `/memory seed` responde `/memory seed is turned off (experimental-memory-seed); /features turns it on.`

## Medir se a memória ajuda

> [!EXPERIMENTAL]
> Controlada pela flag `experimental-memory-scorecard`. Padrão: ligada.

Com esta flag ligada, a cada 20 requisições num projeto uma roda **sem** memória lembrada (o agente ainda pode salvar memórias novas). Para cada execução concluída, o {{NAME}} conta as suas correções (mensagens que você enviou enquanto o agente trabalhava e pedidos de permissão recusados) e o custo. O `/memory score` compara os dois grupos:

```text
/memory score
With memory: 57 runs, 12.3 corrections per 100 runs, $0.41 per run
Without memory: 3 runs, 33.3 corrections per 100 runs, $0.52 per run
Not enough runs without memory yet: one run in 20 goes without, and 5 are needed to compare.
```

Desligue a flag se não quiser que nenhuma requisição rode sem memória.

## Desligar a memória

- Defina `"memory": false` sob `features` para parar a memória do projeto de vez (veja [Feature flags](./experimental-features#desligar-uma-funcionalidade-interna-inteira)), ou `"memory-search": false` para manter a lembrança mas tirar a ferramenta `memory_search`.
- `"autoMemoryEnabled": false` nas configurações, ou a variável de ambiente `{{ENV}}_DISABLE_AUTO_MEMORY` com `1`, `true` ou `yes`, desliga a memória persistente dos [agentes](./agents) que declaram um escopo `memory` (subagentes e o agente escolhido com a configuração `agent`). Isso não afeta a memória do projeto.

## Relacionados

- [Aprendizado](./learning): lições oferecidas depois de uma execução, `/check`, decisões estruturadas.
- [Arquivos de instruções](./instructions-files): regras que o agente sempre lê.
- [Feature flags](./experimental-features): todas as flags citadas aqui.
