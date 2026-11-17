---
title: Estilos de saída
description: Mude como o agente fala e trabalha com os estilos de saída embutidos, ou escreva o seu.
---

Um estilo de saída muda como o agente se comunica: quanto ele fala, se explica o que faz, se pede para você escrever código. O trabalho continua igualmente cuidadoso; só muda a forma de relatar e de colaborar.

```text
/output-style concise
```

## Os estilos embutidos

| Estilo | Em uma linha |
|---|---|
| `default` | O jeito de sempre do {{NAME}}: trabalha e depois relata. |
| `concise` | Começa pelo resultado e pula preâmbulos e narração. |
| `explanatory` | Explica as escolhas e os padrões da base de código enquanto trabalha. |
| `learning` | Faz pausas para você escrever pequenos trechos de código que importam. |
| `proactive` | Age na hora, interrompe pouco e prefere fazer a planejar. |

### default

Nenhum estilo é acrescentado ao prompt de sistema: o agente segue as instruções padrão de entrega do trabalho e de escrita das respostas.

### concise

O agente mantém as respostas curtas e diretas, fazendo o trabalho com o mesmo cuidado:

- A primeira frase diz o que aconteceu ou qual é a resposta; nada de "Vou…" no começo nem recapitulação no fim.
- Relata resultados, decisões e o que você precisa fazer, não cada passo que deu.
- Perguntas simples ganham de uma a três frases em prosa; títulos, tabelas e listas só quando trazem estrutura de verdade.
- Ressalvas só quando mudam o que você deve fazer em seguida.
- Detalhes completos quando você pede uma explicação.
- Relatos de erro, saída de testes que falham, avisos de segurança e confirmações de ações destrutivas mantêm todo o conteúdo.

Um lembrete curto do estilo é repetido ao agente a cada turno.

### explanatory

Além de fazer o trabalho, o agente te ensina sobre a base de código. Antes e depois de escrever código, ele acrescenta um breve insight sobre as escolhas feitas, neste formato:

```text
★ Insight ─────────────────────────────────────
[dois ou três pontos principais]
─────────────────────────────────────────────────
```

Os insights ficam na conversa, nunca no código, e preferem o que é específico desta base de código ao conhecimento geral de programação.

### learning

O agente te ajuda a aprender fazendo. Quando ele escreveria vinte linhas ou mais envolvendo uma decisão de design, uma regra de negócio com mais de uma abordagem válida, ou um algoritmo ou interface central, ele pede que você escreva um trecho de duas a dez linhas:

1. Acrescenta um marcador `TODO(human)` no código (só um por vez).
2. Pede sob **Learn by doing**, com **Context**, **Your task** (o arquivo e o marcador) e **Guidance** sobre os prós e contras.
3. Para e espera o seu código.

Depois da sua contribuição, compartilha um insight ligando o seu código ao sistema como um todo. Também acrescenta as mesmas notas `★ Insight` do estilo explanatory.

### proactive

O agente trabalha de forma contínua e autônoma:

- Começa a implementar na hora e faz suposições razoáveis em trabalho de baixo risco.
- Prefere uma suposição a uma pergunta em decisões de rotina.
- Não para para planejar, a menos que você peça um plano.
- Trata suas correções como entrada normal.
- Ainda pergunta antes de apagar dados ou mudar sistemas compartilhados ou de produção, e só publica em chats, canais ou tickets, ou compartilha segredos, quando você mandou.

Um lembrete curto do estilo é repetido ao agente a cada turno.

> [!NOTE]
> O estilo molda como o agente fala; o seu [modo de permissão](./permissions) continua decidindo o que ele pode fazer sem perguntar. `proactive` no modo `ask` ainda pergunta antes de edições e comandos.

## Definir o estilo

### No chat

`/output-style` abre um seletor com os estilos embutidos e o atual destacado; `/output-style <nome>` define um direto:

```text
/output-style
/output-style explanatory
/output-style default
```

O chat responde `Output style: explanatory, from the next message.` A escolha vale a partir da próxima mensagem e fica guardada para os próximos chats. O `/config` também lista **Output style** com o valor atual.

### Nas configurações

A configuração `outputStyle` escolhe um estilo para toda sessão que lê essas configurações, inclusive `{{COMMAND}} -p`:

```json
{
  "outputStyle": "concise"
}
```

Coloque em `~/{{DIR}}/settings.json` (suas), `{{DIR}}/settings.json` (do projeto, versionadas) ou `{{DIR}}/settings.local.json` (suas, para este projeto). Ou defina pelo shell:

```bash
{{COMMAND}} config set outputStyle concise              # ~/{{DIR}}/settings.json
{{COMMAND}} config set outputStyle concise --project    # {{DIR}}/settings.local.json
{{COMMAND}} config unset outputStyle
```

Veja [Configurações](./settings).

### Qual vale

1. Um estilo embutido escolhido no chat (com `/output-style`) vale para aquele chat. `/output-style default` desliga os estilos nele, diga o que disser a configuração.
2. Senão, vale a configuração `outputStyle`.
3. Um nome que o chat não conhece como embutido deixa a escolha para a configuração.

Os nomes embutidos não diferenciam maiúsculas (`Concise` funciona).

## Estilos personalizados

Escreva o seu estilo como um arquivo Markdown com o nome do estilo numa pasta `output-styles`:

```text
~/{{DIR}}/output-styles/review.md          seus estilos
<projeto>/{{DIR}}/output-styles/review.md  estilos do projeto
```

O {{NAME}} procura em `~/{{DIR}}` e na pasta `{{DIR}}` do projeto e de cada pasta acima dele; vale o mais próximo do projeto.

O corpo é a instrução acrescentada ao prompt de sistema. Um frontmatter opcional ajusta:

```markdown
---
keep-coding-instructions: true
turn-reminder: Report findings as a numbered list, most severe first.
---
Review code instead of writing it. For each problem, give the file and line,
what is wrong, and a suggested fix. Do not edit files unless asked.
```

| Chave | Efeito |
|---|---|
| `keep-coding-instructions` | `true` mantém as instruções padrão de entrega do trabalho e de escrita das respostas junto com o seu estilo. Qualquer outro valor, ou omitir a chave, as remove, e o estilo substitui por completo a forma de o agente relatar. |
| `turn-reminder` | Um texto curto repetido ao agente a cada turno para ele não esquecer o estilo. |

Escolha-o pela configuração:

```json
{
  "outputStyle": "review"
}
```

> [!IMPORTANT]
> O nome de um estilo personalizado só pode ter letras, dígitos, `-` e `_`. Um nome com outros caracteres, ou um arquivo de estilo que não é encontrado, é um erro nas configurações (`invalid outputStyle name`, `output style review was not found`).

O seletor do `/output-style` lista só os estilos embutidos; escolha um estilo personalizado pela `outputStyle`.

## O que um estilo não muda

- Os prompts de sistema dos sub-agentes não incluem as instruções do estilo; elas são escritas para o agente principal.
- O comando da linha de status recebe o estilo atual do chat como `output_style` no JSON de entrada; veja [Linha de status](./statusline).
