---
title: Arquivos de instruções
description: Dê ao {{NAME}} instruções permanentes para você, para o projeto e para partes do projeto com {{INSTRUCTIONS}}, {{LOCAL_INSTRUCTIONS}}, AGENTS.md e arquivos de regras.
---

Arquivos de instruções são Markdown simples que o {{NAME}} lê no início de cada prompt e coloca no prompt de sistema do agente. Use-os para o que o agente sempre precisa saber: como compilar e testar o projeto, convenções, pastas a evitar, o jeito como você gosta de trabalhar.

Há três tipos:

- **Arquivos de instruções** (`{{INSTRUCTIONS}}`, `{{DIR}}/{{INSTRUCTIONS}}`, `AGENTS.md`, `{{LOCAL_INSTRUCTIONS}}`): sempre carregados.
- **Arquivos de regras** (`{{DIR}}/rules/**/*.md`): carregados sempre, ou só quando o agente mexe em caminhos correspondentes.
- **Arquivos de instruções aninhados** em subpastas: carregados quando o agente trabalha num arquivo dentro daquela subpasta.

## Começo rápido

Crie `{{INSTRUCTIONS}}` na raiz do repositório:

```markdown
# Notas do projeto

- Compile com `cargo build`; teste com `cargo nextest run --workspace`.
- Nunca edite arquivos em `generated/`; rode `make codegen`.
- Use português do Brasil nos textos para o usuário.

@docs/arquitetura.md
```

A próxima mensagem que você enviar já usa o arquivo; não é preciso reiniciar nada. A última linha importa outro arquivo (veja a seção Importações, mais abaixo).

> [!TIP]
> A skill embutida `/init` analisa o repositório e escreve um primeiro `{{INSTRUCTIONS}}` para você. Se o projeto já tem um `AGENTS.md`, ela só acrescenta o que falta nele.

Para instruções só suas, valendo em todos os projetos, escreva `~/{{DIR}}/{{INSTRUCTIONS}}`. Para notas privadas sobre um projeto, que não devem ir para o repositório, escreva `{{LOCAL_INSTRUCTIONS}}` ao lado do `{{INSTRUCTIONS}}` do projeto e coloque-o no `.gitignore`.

## Quais arquivos são lidos

| Arquivo | Escopo | Compartilhado com a equipe |
|---|---|---|
| `~/{{DIR}}/{{INSTRUCTIONS}}` | Você, em todos os projetos | Não |
| `{{INSTRUCTIONS}}` | O projeto (e cada pasta acima da pasta de trabalho) | Sim, quando versionado |
| `{{DIR}}/{{INSTRUCTIONS}}` | Igual a `{{INSTRUCTIONS}}`, guardado dentro da pasta `{{DIR}}` | Sim, quando versionado |
| `AGENTS.md` | Igual a `{{INSTRUCTIONS}}` | Sim, quando versionado |
| `{{LOCAL_INSTRUCTIONS}}` | Você, neste projeto | Não (deixe fora do git) |
| `~/{{DIR}}/rules/**/*.md` | Suas regras, em todos os projetos | Não |
| `{{DIR}}/rules/**/*.md` | As regras do projeto | Sim, quando versionado |
| `{{INSTRUCTIONS}}`, `{{LOCAL_INSTRUCTIONS}}` ou `AGENTS.md` numa subpasta | Aquela subpasta | Sim, quando versionado |

### Ordem de busca

O {{NAME}} monta a lista nesta ordem, e o agente vê os arquivos na mesma ordem:

1. Seu arquivo de usuário, `~/{{DIR}}/{{INSTRUCTIONS}}`.
2. Cada pasta, do topo do sistema de arquivos até a pasta de trabalho. Em cada pasta ele procura, nesta ordem:
   1. `{{INSTRUCTIONS}}`
   2. `{{DIR}}/{{INSTRUCTIONS}}`
   3. `AGENTS.md`
   4. `{{LOCAL_INSTRUCTIONS}}`

Assim, um monorepo pode ter notas gerais na raiz e notas de um pacote em `packages/api/{{INSTRUCTIONS}}`: abrir o {{NAME}} em `packages/api` carrega os dois, a raiz primeiro. Arquivos mais perto da pasta de trabalho vêm depois, o que permite refinar o que a raiz diz.

Cada arquivo aparece no prompt de sistema sob um título com o caminho e quem o escreveu:

| Escopo | Como o agente é informado |
|---|---|
| user | as instruções privadas do usuário para todos os projetos |
| project | as instruções do projeto, mantidas no repositório |
| local | as instruções privadas do usuário para este projeto |

O agente é avisado de que essas instruções têm precedência sobre o comportamento padrão dele e devem ser seguidas exatamente como escritas.

Um arquivo vazio (ou que fica vazio depois de removidos os comentários) é ignorado.

## Compatibilidade com AGENTS.md e CLAUDE.md

O {{NAME}} lê `AGENTS.md` em cada pasta que verifica, logo depois de `{{INSTRUCTIONS}}`, então um projeto que já tem um não precisa de cópia. Quando os dois existem, os dois são carregados: deixe as notas específicas do {{NAME}} em `{{INSTRUCTIONS}}` e as compartilhadas em `AGENTS.md`.

O {{NAME}} **não** lê os arquivos de outra ferramenta. Nada disto chega a uma execução:

- `CLAUDE.md`, `CLAUDE.local.md`, `.claude/CLAUDE.md` e `~/.claude/CLAUDE.md`
- `.claude/rules/`, `.claude/skills/`, `.claude/agents/`, `.claude/commands/`
- hooks em `.claude/settings.json`
- o `.mcp.json` da raiz do projeto

Para aproveitar um `CLAUDE.md`, copie o que precisar para `{{INSTRUCTIONS}}` (a skill `/init` usa ele como fonte) ou importe-o a partir de `{{INSTRUCTIONS}}`:

```markdown
@CLAUDE.md
```

## Importações

Uma linha pode trazer outro arquivo com `@caminho`:

```markdown
Veja as notas de build: @docs/build.md
@"docs/processo de release.md"
```

- O `@` precisa começar a linha ou vir depois de um espaço.
- Um caminho relativo é resolvido a partir da pasta do arquivo que o contém.
- `@~/…` começa na sua pasta pessoal. Isso é útil sobretudo em `~/{{DIR}}/{{INSTRUCTIONS}}`, por exemplo `@~/{{DIR}}/trechos/estilo.md`, já que as importações precisam ficar dentro de uma pasta confiável (veja abaixo).
- Coloque entre aspas duplas um caminho com espaços.
- Um `,`, `;` ou `)` logo depois do caminho é ignorado, então `(veja @docs/a.md)` funciona.
- O texto importado é inserido logo depois da linha, sob `Imported instructions from <caminho>:`.
- Importações podem importar, até cinco níveis de profundidade.
- Cada arquivo entra uma vez por prompt, mesmo que vários arquivos o importem.

O que **não** é importação:

- `@` dentro de código inline (`` `@nao-importado.md` ``) ou de um bloco de código.
- `@` dentro de um comentário HTML (`<!-- … -->`).

Comentários HTML são removidos do texto que o agente vê, então você pode deixar notas para quem mantém o arquivo sem que elas cheguem ao modelo. Comentários dentro de blocos de código e de código inline ficam como estão.

### Para onde as importações podem apontar

As importações ficam presas a uma pasta confiável:

| Arquivo que importa | Pode importar arquivos dentro de |
|---|---|
| `~/{{DIR}}/{{INSTRUCTIONS}}` | `~/{{DIR}}` |
| Um arquivo do projeto dentro de um repositório git | O repositório (a pasta mais próxima acima da pasta de trabalho que tem `.git`) |
| Um arquivo numa pasta acima do repositório | Aquela pasta |

Um arquivo fora da sua pasta confiável, um caminho de rede (`//servidor/…`, `\\servidor\…`, `/net/…`, `/Network/…`) ou um arquivo que não pode ser lido não é carregado. O agente vê uma nota curta no lugar, como `[Instruction import unavailable: ../privado.md: instruction import is outside the trusted directory]`.

### Limites de tamanho

| Limite | Valor |
|---|---|
| Um arquivo | 100.000 bytes |
| Todos os arquivos de instruções de um prompt, importações incluídas | 250.000 bytes |
| Profundidade de importação | 5 níveis |

Um arquivo acima de um limite, ou que não é UTF-8, é trocado por uma nota de uma linha como `[Not loaded: instruction context exceeds its size budget. Tell the user so they can shorten it.]`, para que o agente possa avisar você em vez de simplesmente não ver o arquivo.

## Arquivos de regras

Arquivos de regras ficam numa pasta `rules` e podem valer para o projeto inteiro ou só para alguns caminhos.

```text
~/{{DIR}}/rules/            suas regras, em todos os projetos
<projeto>/{{DIR}}/rules/    as regras do projeto (subpastas permitidas)
```

Todo arquivo `.md` dentro dessas pastas é uma regra, até oito pastas de profundidade. Como nos arquivos de instruções, `{{DIR}}/rules` é procurado em cada pasta do topo do sistema de arquivos até a pasta de trabalho.

### Regras para qualquer tarefa

Uma regra sem frontmatter vale sempre:

```markdown
Prefira commits pequenos e focados, com assunto no padrão conventional commits.
```

Ela é acrescentada ao lembrete que o agente recebe a cada turno, sob `# Project rule: <caminho>`.

### Regras para alguns caminhos

Adicione `paths` ao frontmatter para limitar a regra a arquivos que casam com padrões glob:

```markdown
---
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
---
Use aritmética verificada. Nunca chame `unwrap()` fora dos testes.
```

`paths` também pode ser uma única string separada por vírgulas; vírgulas dentro de chaves continuam no padrão:

```markdown
---
paths: src/**/*.{ts,tsx}, lib/**/*.ts
---
Componentes são componentes de função com props tipadas.
```

Os padrões são comparados com o caminho relativo à pasta que contém a pasta `{{DIR}}` (para suas regras em `~/{{DIR}}/rules`, relativo à pasta de trabalho). Caminhos com `..` e links simbólicos são resolvidos antes, então `alias/novo.rs` casa com `src/**` quando `alias` aponta para `src`.

Uma regra com caminhos é carregada na primeira vez que o agente chama uma ferramenta cujo `file_path` ou `path` casa: essa chamada volta com a mensagem *New project instructions apply. Read these rules and issue this tool call again*, seguida da regra. O agente então repete a chamada já sabendo da regra, e a regra fica no lembrete de cada turno pelo resto da conversa.

> [!WARNING]
> Uma regra cujo frontmatter não é YAML válido, ou cujo `paths` não é uma lista de strings, falha de forma fechada: a próxima chamada de ferramenta encerra a execução com um erro que cita o arquivo. Corrija o arquivo e envie o prompt de novo.

## Arquivos de instruções aninhados

Um `{{INSTRUCTIONS}}`, `{{LOCAL_INSTRUCTIONS}}` ou `AGENTS.md` numa subpasta da pasta de trabalho (também dentro da pasta `{{DIR}}` de uma subpasta) funciona como uma regra limitada àquela subpasta: é carregado na primeira vez que o agente mexe num arquivo dentro dela.

```text
repo/
├── {{INSTRUCTIONS}}            carregado no início
└── web/
    ├── {{INSTRUCTIONS}}        carregado quando o agente mexe em web/**
    └── src/
```

O {{NAME}} procura até 32 pastas de profundidade e pula `.git`, `node_modules` e `target`. Importações dentro de arquivos aninhados ficam presas ao repositório.

## Subagentes e arquivos de instruções

Subagentes recebem os mesmos arquivos de instruções que o agente principal, exceto:

- os agentes embutidos somente leitura (`Explore`, `Plan`) começam sem eles;
- um agente próprio com `omitClaudeMd: true` no frontmatter começa sem eles (veja [Agentes](./agents)).

As regras são compartilhadas com os subagentes, e um agente que trabalha na própria worktree lê os arquivos da worktree.

## Mudar instruções com um hook

Um hook `Brief` vê todos os arquivos de instruções e regras antes de o prompt ser montado e pode substituí-los, por exemplo para acrescentar uma regra da equipe vinda de um servidor. Veja [Hooks](./hooks#brief).

## Solução de problemas

| Sintoma | Causa |
|---|---|
| Um `CLAUDE.md` é ignorado | O {{NAME}} lê só `{{INSTRUCTIONS}}` e `AGENTS.md`. Importe com `@CLAUDE.md`. |
| Uma importação mostra *outside the trusted directory* | O arquivo importado está fora do repositório (ou fora de `~/{{DIR}}`, no seu arquivo de usuário). Mova-o para dentro. |
| *Not loaded: … size budget* | Um arquivo passa de 100.000 bytes, ou todos juntos passam de 250.000. Divida ou encurte. |
| Uma regra com caminhos nunca se aplica | Os padrões são relativos à pasta que contém `{{DIR}}`, não a `{{DIR}}/rules`. |
| A execução para na primeira chamada de ferramenta citando uma regra | O frontmatter da regra é inválido. |

## Veja também

- [Configurações](./settings) para os arquivos que mudam o comportamento do {{NAME}}.
- [Memória](./memory) para o que o agente aprende sozinho.
- [Arquivos e diretórios](./files-and-directories) para tudo o que fica em `{{DIR}}`.
