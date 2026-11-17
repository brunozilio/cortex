---
title: Inteligência de código
description: Como o {{NAME}} entende o código além da busca de texto, com servidores de linguagem para definições, referências e diagnósticos, um mapa de código tree-sitter que aponta os prompts para os arquivos certos e o raio de impacto de uma edição.
---

Além de ler arquivos e buscar texto, o {{NAME}} usa duas fontes de conhecimento sobre o código:

- **servidores de linguagem** (os mesmos que o seu editor usa) respondem perguntas sobre definições, referências e tipos, apontam erros depois de cada edição e encontram quem chama o que uma edição mudou;
- um **mapa de código**, um índice tree-sitter das funções, tipos e chamadas do repositório, aponta cada prompt para o código de que ele trata.

## Servidores de linguagem

### Servidores embutidos

| Servidor | Comando | Arquivos | Marcadores de projeto |
|---|---|---|---|
| `rust-analyzer` | `rust-analyzer` | `.rs` | `Cargo.toml` |
| `typescript` | `typescript-language-server --stdio` | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `tsconfig.json`, `jsconfig.json`, `package.json` |
| `pyright` | `pyright-langserver --stdio` | `.py`, `.pyi` | `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt` |
| `gopls` | `gopls` | `.go` | `go.mod`, `go.work` |
| `clangd` | `clangd` | `.c`, `.h`, `.cc`, `.cpp`, `.hpp` | `compile_commands.json`, `CMakeLists.txt` |

Um servidor é usado quando o comando dele está no seu `PATH`, ou quando o {{NAME}} consegue instalá-lo para você.

> Um servidor de linguagem roda código do próprio projeto: o rust-analyzer roda build scripts (`build.rs`) e macros procedurais, o tsserver carrega o TypeScript de `node_modules/typescript` do projeto, e outros servidores leem a configuração do projeto do seu jeito. Por isso, numa pasta em que você não [confiou](./sandbox-and-safety#confianca-na-pasta), nenhum servidor de linguagem é iniciado: a ferramenta `LSP` responde que a pasta não é confiável e que você pode confiar nela com `/trust`, e as edições voltam sem diagnósticos nem chamadores. Numa pasta confiável os servidores rodam como descrito aqui, fora da sandbox.

### Instalados no primeiro uso

Quando falta um servidor embutido, a primeira pergunta da ferramenta **LSP** sobre um arquivo dele o instala na pasta de dados local do {{NAME}}, em `lsp/<servidor>`, e a pergunta espera a instalação:

| Servidor | Instalado com |
|---|---|
| `rust-analyzer` | Uma versão fixa baixada do GitHub e conferida pelo SHA-256 (macOS, Linux e Windows) |
| `clangd` | Uma versão fixa baixada do GitHub e conferida pelo SHA-256 (macOS, Linux x86-64, Windows x86-64) |
| `typescript` | `npm` (`typescript-language-server` e `typescript`); precisa de Node.js |
| `pyright` | `npm`; precisa de Node.js |
| `gopls` | `go install`; precisa de Go |

Só os servidores embutidos, com o comando embutido, são instalados assim. Você é consultado antes de cada instalação, em todo modo, inclusive bypass; o modo plan e as pastas em que você não confiou nunca instalam, e onde ninguém pode ser consultado a instalação é recusada, a menos que uma regra allow como `LSP(install:pyright)` a nomeie (veja [Sandbox e segurança](./sandbox-and-safety)). Depois que uma instalação falha, ela não é tentada de novo por 30 segundos; uma instalação que passa de 10 minutos é interrompida. Até um servidor ser instalado, as edições não são conferidas por ele; enquanto ele instala, o resultado de uma edição avisa:

```text
<diagnostics>
rust-analyzer is still installing, so this file was not checked; later edits will be.
</diagnostics>
```

Um servidor parado há 10 minutos é encerrado; ele volta quando for preciso.

### A ferramenta LSP

Quando há pelo menos um servidor disponível, o agente ganha a ferramenta **LSP**. Ela pergunta ao servidor de um arquivo:

| Operação | Responde |
|---|---|
| `goToDefinition` | Onde o símbolo numa posição é definido |
| `findReferences` | Onde ele é usado |
| `goToImplementation` | As implementações dele |
| `hover` | O tipo e a documentação |
| `incomingCalls` / `outgoingCalls` | Quem chama uma função, e o que ela chama |
| `documentSymbol` | O esboço de um arquivo |
| `workspaceSymbol` | Um símbolo encontrado pelo nome no projeto inteiro (com `query`) |
| `diagnostics` | Os erros e avisos atuais do arquivo |
| `workspaceDiagnostics` | Todos os erros e avisos que o servidor do arquivo já relatou para o projeto |

Linhas e colunas contam a partir de 1, como o **Read** mostra. O agente é orientado a preferi-la ao Grep para achar onde um símbolo é definido ou usado, e a pedir o `documentSymbol` antes de ler um arquivo longo.

O `diagnostics` manda o arquivo como está no disco ao servidor dele e espera até 10 segundos pelo relatório do servidor. O `workspaceDiagnostics` faz o mesmo e depois lista o que o servidor já relatou para cada arquivo (servidores como o rust-analyzer verificam o projeto inteiro; outros só relatam os arquivos que foram abertos). Os dois respondem com uma contagem e uma linha por problema, erros primeiro, dicas de fora, no máximo 50 linhas:

```text
1 error and 1 warning in 2 files
src/parse.rs:42:9 error: mismatched types: expected `u32`, found `i64`
src/lib.rs:7:5 warning: unused import: `std::fs`
```

Quando o servidor ainda não está instalado, a pergunta pede permissão antes, como qualquer primeiro uso, e nada é instalado numa pasta em que você não confiou.

### Erros depois de cada edição

Depois que o **Edit** ou o **Write** muda um arquivo que um servidor disponível atende, o {{NAME}} manda o texto novo ao servidor e acrescenta os erros e avisos dele ao resultado da ferramenta, erros primeiro, até 20:

```text
<diagnostics>
src/parse.rs:42:9 error: mismatched types: expected `u32`, found `i64`
src/parse.rs:57:5 warning: unused variable: `count`
… and 3 more
</diagnostics>
```

Dicas e mensagens informativas ficam de fora.

### Configurar servidores

A configuração `lspServers` muda um servidor embutido campo a campo, acrescenta um servidor ou desliga um:

```json
{
  "lspServers": {
    "rust-analyzer": { "command": "/opt/rust-analyzer", "env": { "RA_LOG": "error" } },
    "gopls": { "disabled": true },
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" } }
  }
}
```

| Campo | Significado |
|---|---|
| `command` | O programa a iniciar |
| `args` | Os argumentos dele |
| `extensionToLanguage` | As extensões de arquivo que ele atende, mapeadas para ids de linguagem |
| `rootMarkers` | Arquivos que marcam a raiz de um projeto |
| `env` | Variáveis de ambiente extras |
| `disabled` | `true` remove o servidor |

Um servidor novo precisa pelo menos de `command` e `extensionToLanguage`. Arquivos de configurações lidos depois prevalecem sobre os anteriores. O `{{DIR}}/settings.json` versionado de um projeto pode mudar quais arquivos um servidor atende, os marcadores de raiz ou desligá-lo, mas o `command`, os `args` e o `env` dele são ignorados: só os seus próprios arquivos de configurações escolhem que programa roda. Plugins também podem acrescentar servidores (veja [Plugins](./plugins)).

## Raio de impacto

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-blast-radius`. Padrão: ligada.

Quando uma edição muda uma função, um tipo ou outro símbolo, o código que o usa pode precisar mudar também. Depois que o **Edit** ou o **Write** muda um arquivo do projeto, o {{NAME}} pergunta ao servidor de linguagem do arquivo a quais símbolos pertencem as linhas mudadas (até 3) e quais outros arquivos os referenciam, e conta ao agente:

```text
<callers>Other files use what this edit changed in src/client.rs; check they still fit before you finish:
- retry (4 places): src/a.rs:7, src/jobs/sync.rs:31,88, src/main.rs:12
</callers>
```

São listados até 8 arquivos por símbolo, com até 3 linhas cada. Referências dentro do próprio arquivo editado não entram. A conferência usa um servidor que já está disponível e nunca espera uma instalação.

## Mapa de código

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-code-map`. Padrão: ligada.

Num repositório git, o {{NAME}} mantém um índice tree-sitter do código do repositório e acrescenta a cada prompt uma lista curta dos arquivos e símbolos que combinam com ele, para o agente abri-los em vez de buscar:

```text
<code-map>
Files and symbols that match this prompt (from a tree-sitter index; open them rather than searching):
- src/memory/dedup.rs: fn merge_near_duplicates (L11, called from 2 places: src/memory/mod.rs, src/book.rs)
- src/memory/mod.rs: struct MemoryBook (L118)
</code-map>
```

### O que é indexado

| Linguagem | Arquivos |
|---|---|
| Rust | `.rs` |
| TypeScript | `.ts`, `.mts`, `.cts`, `.tsx` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Python | `.py`, `.pyi` |
| Go | `.go` |

O índice segue o `.gitignore`, pula as pastas `target` e `node_modules` e os arquivos acima de 512 KB. Ele registra as funções, os tipos e outras declarações de cada arquivo com a linha, e quais nomes cada arquivo chama.

### Como os prompts são casados

O seu prompt é dividido em palavras, identificadores (como `merge_near_duplicates` ou `MemoryBook`) e caminhos. Cada arquivo recebe pontos pelos seus símbolos e pelo seu caminho: um identificador ou um caminho citado exatamente vale mais, depois palavras inteiras; arquivos com menos de um quarto da melhor pontuação saem. São listados até 12 arquivos, cada um com até 4 símbolos e, para cada símbolo, quantos lugares o chamam e de quais arquivos (até 3 nomeados). Um prompt que não combina com nada não ganha mapa.

### Quando ele roda

- Só para prompts que você digita: não para comandos, chamadas à parte ou otimização de memória.
- Só dentro de um repositório git, e nunca quando a pasta do projeto é a sua pasta pessoal.
- O índice fica em cache na pasta de dados local do {{NAME}}, em `code-map/`, um arquivo por projeto, e só os arquivos que mudaram desde o último prompt são lidos de novo.

Um projeto pode desligar o mapa de código para todo mundo versionando `"experimental-code-map": false` sob `features` no seu `{{DIR}}/settings.json`; aí o repositório nunca é varrido.

## Relacionados

- [Feature flags](./experimental-features#inteligencia-de-codigo): as flags desta página.
- [Verificação e revisão](./verification-and-review#testes-afetados): rodar os testes que uma edição afeta.
- [Configurações](./settings): onde fica o `lspServers`.
