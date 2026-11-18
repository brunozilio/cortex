---
title: Introdução
description: O que é o {{NAME}}, o que ele faz e como esta documentação está organizada.
---

O {{NAME}} é um único agente de programação no terminal para vários provedores de modelos, com uma fronteira de confiança que se mantém quando a entrada é hostil: o mesmo chat, as mesmas regras e as mesmas sessões, qualquer que seja o modelo, e uma linha clara do que o agente pode fazer sem perguntar. É para um desenvolvedor, na sua própria máquina e no seu próprio celular; não é um serviço hospedado ou multiusuário, nem um SDK embutível, nem um editor. Você roda `{{COMMAND}}` na pasta de um projeto, descreve o que quer em palavras simples, e o agente lê o código, edita arquivos, roda comandos e te conta o que fez, pedindo sua autorização antes de qualquer coisa que você ainda não liberou.

```bash
cd meu-projeto
{{COMMAND}}
```

Você trabalha num chat no terminal: não há interface gráfica. Uma página local no navegador só mostra o [trace](./trace-console) e esta documentação, e pode conter um terminal ligado a uma sessão. O mesmo agente também pode responder uma única vez e imprimir o resultado (`{{COMMAND}} -p "..."`), para scripts e CI.

## O que dá para fazer

- **Trabalhar no código conversando.** Pergunte sobre uma base de código, peça para corrigir um bug, criar uma funcionalidade ou escrever testes, e acompanhe enquanto ele lê, edita e roda comandos. Veja [Chat](./chat).
- **Manter o controle do que ele faz.** Quatro modos de permissão (ask, edits, plan, bypass), regras de allow/ask/deny e um sandbox opcional decidem o que roda sem perguntar. Veja [Permissões](./permissions) e [Sandbox e segurança](./sandbox-and-safety).
- **Planejar antes de codar.** No modo plan o agente só lê e propõe um plano; você aprova antes de qualquer edição. Veja [Comandos de barra](./slash-commands#mode).
- **Voltar no tempo.** Cada prompt é um checkpoint: volte arquivos e conversa, repita um prompt, ramifique ou faça um fork da conversa, retome uma anterior. Veja [Sessões](./sessions).
- **Escolher modelos e provedores.** Conecte um provedor com chave de API ou assinatura, troque de modelo no meio da conversa, defina um modelo por papel e uma cadeia de fallback. Veja [Modelos e provedores](./models-and-providers) e [Contas](./accounts).
- **Dar conhecimento do projeto.** Arquivos de instruções ([{{INSTRUCTIONS}}](./instructions-files)), [memória](./memory) que persiste entre sessões e [inteligência de código](./code-intelligence) vinda de language servers.
- **Estender.** Adicione [skills](./skills) e comandos personalizados (rodam como `/nome`), [sub-agentes](./agents), [hooks](./hooks), [servidores MCP](./mcp), [canais](./channels) e [plugins](./plugins), ou desenhe sua própria [linha de status](./statusline).
- **Conferir o trabalho.** [Verificação e revisão](./verification-and-review) julga as execuções contra critérios; [orçamentos](./budget-and-cost) limitam o gasto.
- **Automatizar.** Rode sem interface com `-p` ([Headless](./headless)), agende prompts ([Agendamentos](./schedules)) e avalie o agente nos seus próprios casos ([Evals](./evals)).
- **Ver o agente trabalhando.** O [console de trace](./trace-console) mostra ao vivo, no navegador, as execuções, spans, custo e esperas de cada sessão.
- **Levar com você.** O [controle remoto](./remote-control) coloca o console e um terminal no seu celular, através de um relay seu, com criptografia ponta a ponta.

## Como o chat é

A tela tem quatro partes, de cima para baixo:

| Parte | O que mostra |
|---|---|
| Conversa | Seus prompts, as respostas do agente (Markdown renderizado, com código destacado), as chamadas de ferramentas, diffs, planos e avisos. |
| Painel | O que precisa de você agora: uma pergunta ou pedido de permissão, um seletor, sugestões de completar, ou a lista de tarefas do agente. |
| Prompt | Onde você digita. Uma marca `>`, ou um `!` rosa no modo shell. |
| Linha de status | Título da conversa, modelo, modo de permissão, esforço, contexto restante, custo e marcadores de recursos. |

## Como esta documentação está organizada

| Seção | Páginas |
|---|---|
| Primeiros passos | Esta página e o [Início rápido](./quickstart). |
| Usando o chat | [Chat](./chat), [Atalhos de teclado](./keyboard-shortcuts), [Comandos de barra](./slash-commands), [Sessões](./sessions), [Estilos de saída](./output-styles). |
| Modelos e contas | [Modelos e provedores](./models-and-providers), [Contas](./accounts). |
| Configuração | [Configurações](./settings), [Permissões](./permissions), [Sandbox e segurança](./sandbox-and-safety), [Variáveis de ambiente](./environment-variables), [Arquivos e pastas](./files-and-directories), [Arquivos de instruções](./instructions-files). |
| Extensões | [Skills](./skills), [Agentes](./agents), [Hooks](./hooks), [MCP](./mcp), [Canais](./channels), [Plugins](./plugins), [Linha de status](./statusline). |
| Recursos | [Recursos experimentais](./experimental-features), [Memória](./memory), [Orçamento e custo](./budget-and-cost), [Branches e git](./branches-and-git), [Verificação e revisão](./verification-and-review), [Aprendizado](./learning), [Agendamentos](./schedules), [Inteligência de código](./code-intelligence), [Console de trace](./trace-console), [Controle remoto](./remote-control). |
| Automação | [Headless](./headless), [Evals](./evals). |
| Referência | [Referência da CLI](./cli-reference), [Solução de problemas](./troubleshooting). |

> [!TIP]
> Dentro do chat, `/help` lista todos os comandos e as teclas principais, `/help <comando>` explica um deles, e `/docs` abre esta documentação no navegador (`/docs <assunto>` busca nela).

## Por onde seguir

- Primeira vez? Siga o [Início rápido](./quickstart).
- Já está rodando? Conheça o [chat](./chat) e os [atalhos de teclado](./keyboard-shortcuts).
- Procurando um comando? Veja a [referência de comandos de barra](./slash-commands).
