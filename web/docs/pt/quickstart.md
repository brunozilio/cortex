---
title: Início rápido
description: Instale o {{NAME}}, conecte um provedor e envie seu primeiro prompt em poucos minutos.
---

Este guia leva você do zero a uma conversa funcionando: compilar e instalar o binário `{{COMMAND}}`, conectar um provedor de modelos e perguntar ao agente sobre o seu projeto.

## Antes de começar

Você precisa de:

- Um toolchain de Rust recente o bastante para o workspace (ele declara `rust-version = "1.97"`).
- Um terminal. O chat é um app de terminal em tela cheia; não há interface gráfica.
- Uma conta num provedor de LLM: uma chave de API ou uma assinatura em que você possa entrar.

Algumas ferramentas são usadas quando estão presentes, e nada quebra sem elas:

| Ferramenta | Usada para |
|---|---|
| `git` | `/diff`, `/diffs`, comparação com a branch e worktrees. |
| `pdftotext` | Ler PDFs. |
| `ripgrep` | Opcional. A ferramenta `Grep` usa o `rg` quando ele está no `PATH` (mais rápido em projetos grandes, mesmos resultados) e a busca embutida caso contrário; o `doctor` informa qual. |
| Language servers (rust-analyzer, clangd…) | A ferramenta `LSP` e os diagnósticos depois de edições. Veja [Inteligência de código](./code-intelligence). |
| `node` / `npx` | Só para servidores MCP que precisam deles. |

## Passo 1: Instalar

Compile e instale o binário a partir de uma cópia do código-fonte:

```bash
cargo install --path crates/cli
```

Isso coloca o `{{COMMAND}}` no seu `PATH` (na pasta bin do Cargo). Confira se roda:

```bash
{{COMMAND}} --version
```

## Passo 2: Conferir a instalação

`{{COMMAND}} doctor` verifica se esta instalação consegue trabalhar na pasta atual e sai com status de falha quando alguma verificação falha:

```bash
cd meu-projeto
{{COMMAND}} doctor
```

O mesmo relatório existe dentro do chat como `/doctor`.

## Passo 3: Conectar um provedor

Você pode conectar pelo shell ou de dentro do chat.

Pelo shell:

```bash
{{COMMAND}} login
```

Ele lista os provedores, numerados, cada um com a forma de conectar (`paste an API key`, `sign in with the browser` ou `sign in with a code`), e pede que você escolha um:

- **Chave de API**: cole a chave no campo oculto.
- **Login pelo navegador**: o navegador abre; se não abrir, o terminal mostra o endereço para visitar.
- **Login com código**: o terminal mostra um código para digitar num endereço, e abre o navegador nele.

Quando dá certo, aparece `Connected <provedor> (<conta>).`

No chat, rode `/login`: abre o seletor **Connect a provider**. Escolha com `↑`/`↓` e `Enter`. Para chave de API, aparece um campo mascarado no painel: cole ou digite a chave, `Enter` conecta, `Esc` cancela.

> [!NOTE]
> Se não houver nenhum modelo disponível quando o chat abrir, ele avisa "No model yet. Connect a provider to start:" e abre o seletor de login para você.

Chaves e tokens ficam lacrados em `secrets.json` na pasta de dados (a chave dele em `secrets.key`, ambos legíveis só por você). Veja [Contas](./accounts) e [Arquivos e pastas](./files-and-directories).

Para ver o que os provedores conectados oferecem:

```bash
{{COMMAND}} models
```

## Passo 4: Abrir o chat

Rode `{{COMMAND}}` na pasta do projeto:

```bash
cd meu-projeto
{{COMMAND}}
```

O chat abre em tela cheia com o nome do produto e a pasta no topo, algumas dicas (`/resume`, `/model`, `@file`, `!cmd`), o prompt e a linha de status mostrando o modelo, o modo de permissão e o esforço.

Se o projeto traz coisas que rodam código ou ampliam o que o agente pode fazer (hooks, servidores MCP, ferramentas de script, permissões em `settings.local.json`…), o chat primeiro pergunta **Do you trust the files in …?**: **Trust this folder**, **Continue without trusting** ou **Exit**. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).

Também dá para abrir o chat já enviando um primeiro prompt:

```bash
{{COMMAND}} "explain this project"
```

> [!TIP]
> Prefere o chat no rodapé do terminal, com a conversa no histórico de rolagem do próprio terminal? Abra com `{{COMMAND}} --inline`. Veja [Modo inline](./chat#modo-inline).

## Passo 5: Enviar o primeiro prompt

Digite um pedido e aperte `Enter`:

```text
explique este projeto
```

O agente lê arquivos e responde. Enquanto trabalha, aparece uma linha como `Working… (12s · esc to interrupt)`; aperte `Esc` para pará-lo.

Se a pasta não for um repositório git, a primeira mensagem oferece criar um (**Yes, initialize git**, **No, not now**, **No, and don't offer again**). Com git você revisa e desfaz alterações e isola agentes em worktrees.

Agora peça uma alteração:

```text
adicione uma flag --verbose na CLI e um teste para ela
```

Quando o agente quer editar um arquivo ou rodar um comando ainda não liberado, o painel pergunta:

| Resposta | Tecla |
|---|---|
| Sim | `y` |
| Sim, e não perguntar de novo (para aquela regra neste projeto, ou nesta sessão) | `a` |
| Não, e dizer ao agente o que fazer em vez disso | `n`, ou escolha a opção e digite uma nota |

## Passo 6: O essencial

| Para | Como |
|---|---|
| Quebrar linha no prompt | `Alt+Enter` ou `Ctrl+J` (ou `Shift+Enter` onde o terminal informa essa tecla; veja `/terminal-setup`) |
| Apontar um arquivo para o agente | Digite `@` e parte do nome, depois `Tab` |
| Rodar você mesmo um comando de shell | Comece o prompt com `!` |
| Anexar uma imagem | `Ctrl+V`, ou arraste o arquivo de imagem para o terminal |
| Trocar o modo de permissão | `Shift+Tab` alterna ask → edits → plan → bypass |
| Trocar de modelo | `/model` |
| Desfazer as edições do último prompt | `Esc` `Esc` com o prompt vazio, ou `/rewind` |
| Ver os comandos | `/help` |
| Sair | `Ctrl+C` duas vezes, `Ctrl+D` com o prompt vazio, ou `/exit` |

Ao sair depois de uma conversa, o terminal mostra `Continue this conversation with: {{COMMAND}} -c`.

## Próximos passos

- [Chat](./chat): tudo o que o prompt faz.
- [Atalhos de teclado](./keyboard-shortcuts) e [Comandos de barra](./slash-commands): as referências completas.
- [Sessões](./sessions): retomar, voltar atrás, fazer fork e exportar conversas.
- [Permissões](./permissions): decida o que roda sem perguntar.
- [Arquivos de instruções](./instructions-files): conte ao agente sobre o seu projeto no {{INSTRUCTIONS}}.
- [Headless](./headless): use `{{COMMAND}} -p` em scripts e CI.
