---
title: Skills e comandos personalizados
description: Empacote instruções, scripts e arquivos de referência como skills que o agente carrega quando a tarefa pede, ou como comandos que você digita com uma barra.
---

Uma **skill** é uma pasta com um arquivo `SKILL.md`: instruções para um tipo de tarefa, mais os scripts, modelos ou arquivos de referência de que ela precisa. O agente vê a lista de skills com suas descrições e carrega uma quando a tarefa combina, ou quando você digita `/<nome-da-skill>`.

Um **comando personalizado** é um único arquivo Markdown que você roda digitando o nome dele depois de uma barra. Skills e comandos dividem a mesma lista e funcionam do mesmo jeito depois de carregados.

## Começo rápido

Crie `{{DIR}}/skills/release/SKILL.md` no seu projeto:

```markdown
---
name: release
description: Gera uma release deste projeto. Use quando o usuário pedir para lançar, publicar ou criar a tag de uma versão.
---

# Release

1. Garanta que `main` está limpa e atualizada.
2. Suba a versão no `Cargo.toml` para $ARGUMENTS.
3. Atualize o `CHANGELOG.md` com os commits desde a última tag.
4. Faça o commit, crie a tag `v$ARGUMENTS` e envie a tag.
```

Depois, peça com suas palavras ("lança a 1.4.0", e o agente carrega a skill porque a descrição combina) ou digite:

```text
/release 1.4.0
```

`/skills` lista todas as skills e comandos encontrados para esta pasta.

> [!TIP]
> `/skills new <o que ela deve fazer>` pede ao agente que escreva uma skill para você. Ele carrega a skill embutida `skill-creator` e escreve `{{DIR}}/skills/<nome>/SKILL.md`.

## Onde ficam as skills

| Local | Escopo |
|---|---|
| `{{DIR}}/skills/<nome>/SKILL.md` na pasta de trabalho ou em qualquer pasta acima | O projeto |
| `~/{{DIR}}/skills/<nome>/SKILL.md` | Você, em todos os projetos |
| `<plugin>/skills/<nome>/SKILL.md` | Um plugin instalado, com o nome `<plugin>:<nome>` |
| Skills embutidas | Todos os projetos |

Comandos personalizados ficam ao lado da pasta de skills:

| Local | Escopo |
|---|---|
| `{{DIR}}/commands/**/*.md` na pasta de trabalho ou em qualquer pasta acima | O projeto |
| `~/{{DIR}}/commands/**/*.md` | Você, em todos os projetos |
| `<plugin>/commands/**/*.md` | Um plugin instalado, com o nome `<plugin>:<nome>` |

Quando dois têm o mesmo nome, vale o primeiro encontrado, nesta ordem: a pasta de trabalho, depois cada pasta acima dela, depois as suas, depois os plugins, depois as embutidas. Assim, um projeto pode substituir uma skill sua, e você pode substituir uma embutida, usando o mesmo nome.

> [!NOTE]
> Skills e comandos nas pastas de outra ferramenta, como `.claude/skills/` ou `.claude/commands/`, não são lidos.

As skills são procuradas de novo no início de cada prompt; `/reload` mostra a nova contagem na hora.

## Formato do SKILL.md

```markdown
---
name: pdf-forms
description: Preenche formulários PDF a partir de dados. Use quando o usuário pedir para preencher ou completar um formulário PDF.
disable-model-invocation: false
---

# Preenchendo formulários PDF

Rode `scripts/fill.py <formulario.pdf> <dados.json>` a partir da pasta desta skill…
```

| Campo do frontmatter | Significado |
|---|---|
| `name` | O nome da skill, e o `/nome` que você digita. Por padrão, o nome da pasta. |
| `description` | O que a skill faz e quando usá-la. É por este texto que o agente decide se carrega a skill. Por padrão, a primeira linha do corpo que não está vazia e não é um título. |
| `disable-model-invocation` | `true` deixa a skill fora da lista do agente: só você pode rodá-la, digitando `/nome`. |

Outros campos do frontmatter são ignorados. Uma descrição de várias linhas pode usar `|` ou `>` do YAML seguido de linhas recuadas.

O corpo é o que o agente lê quando carrega a skill. Nele:

- `$ARGUMENTS` é trocado pelo que vem depois do comando (`/release 1.4.0` dá `1.4.0`).
- Quando há argumentos, `Arguments: <argumentos>` também é acrescentado no fim.
- O agente fica sabendo qual é a pasta da skill (`Skill release (base directory for its files: …)`), então o corpo pode citar arquivos ao lado do `SKILL.md` por caminho relativo.

### Arquivos de apoio

Mantenha o corpo curto e ponha os detalhes em arquivos que o agente só lê quando precisa:

```text
{{DIR}}/skills/pdf-forms/
├── SKILL.md
├── reference/fields.md
└── scripts/fill.py
```

## Comandos personalizados

Um comando é um arquivo Markdown; o nome dele é o caminho dentro de `commands/` sem a extensão, com `:` entre as pastas:

| Arquivo | Comando |
|---|---|
| `{{DIR}}/commands/audit.md` | `/audit` |
| `{{DIR}}/commands/db/migrate.md` | `/db:migrate` |
| `<plugin acme>/commands/hello.md` | `/acme:hello` |

```markdown
---
description: Audita as dependências em busca de vulnerabilidades conhecidas
---
Rode `cargo audit` e resuma cada achado para $ARGUMENTS com uma correção sugerida.
```

Comandos aceitam `description` e `disable-model-invocation` como as skills; a descrição, por padrão, é a primeira linha do corpo. Comandos também são oferecidos ao agente, a menos que tenham `disable-model-invocation: true`.

## Como as skills são invocadas

### Por você

Digite `/<nome>` seguido de argumentos, se houver. O corpo da skill (ou do comando) é carregado e enviado com a sua mensagem, antes que hooks como `UserPromptSubmit` a vejam. Isso funciona para qualquer skill ou comando, inclusive os que têm `disable-model-invocation: true`.

### Pelo agente

O prompt de sistema lista as skills que o agente pode usar, com as descrições, e o instrui a carregar uma com a ferramenta `Skill` quando você digita `/<nome>` ou quando uma skill da lista cobre a tarefa. O agente chama:

```json
{ "skill": "release", "args": "1.4.0" }
```

A lista tem um orçamento de cerca de 8.000 caracteres: cada descrição é cortada em no máximo 1.536 caracteres, menos quando há muitas skills, e com skills demais só os nomes aparecem. Ponha no começo da descrição as palavras que dizem quando usar a skill.

Uma skill com `disable-model-invocation: true` não aparece na lista, e o agente recebe *requires explicit user invocation* se tentar carregá-la.

### Skills pré-carregadas num agente

Um agente próprio pode listar skills no campo `skills` do frontmatter: elas são carregadas no prompt de sistema dele quando ele começa. Veja [Agentes](./agents).

## Skills em JavaScript

Uma pasta de skill pode ter `SKILL.js` no lugar de `SKILL.md`. Ele roda no runtime de scripts embutido do {{NAME}} e retorna o texto que o agente recebe:

```javascript
export const name = "changelog";
export const description = "Mostra os commits desde a última tag. Use antes de escrever notas de release.";
export const modelInvocation = true;
export const capabilities = ["exec"];

export default function (args, ctx) {
  const tag = ctx.exec("git describe --tags --abbrev=0").stdout.trim();
  return ctx.exec(`git log --oneline ${tag}..HEAD`).stdout;
}
```

| Export | Significado |
|---|---|
| `name` | O nome; por padrão, o da pasta |
| `description` | Como `description` no `SKILL.md` |
| `modelInvocation` | `false` funciona como `disable-model-invocation: true` |
| `capabilities` | As funções do host que o script chama: `readFile`, `writeFile`, `exec`, `fetch`. Vazia por padrão. |
| `default` | Chamada com os argumentos (uma string) e um contexto; retorna o texto. Tem 60 segundos. |

Um arquivo `.js` numa pasta `commands/` funciona do mesmo jeito, como comando. Uma chamada a uma função do host que o script não listou em `capabilities` falha; uma listada é julgada pelas suas permissões como a ferramenta correspondente (`ctx.exec` como `Bash`), perguntando a você quando elas mandam (veja "Capacidades" em [Sandbox e segurança](./sandbox-and-safety)). A configuração `scripts` continua sendo o limite de fora (veja [Configurações](./settings)).

> [!WARNING]
> Mudança incompatível: uma skill ou comando em script que chama `ctx.readFile`, `ctx.writeFile`, `ctx.exec` ou `ctx.fetch` sem listar em `export const capabilities` agora falha com um erro dizendo o que acrescentar. Um `SKILL.js` do projeto é substituído por um seu ou de um plugin com o mesmo nome.

> [!IMPORTANT]
> O `SKILL.js` e os comandos `.js` de um projeto só são oferecidos depois que você confia na pasta do projeto; as skills e os comandos em Markdown carregam de qualquer forma. O mesmo vale para as ferramentas em script do projeto em `{{DIR}}/tools/`. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).

## Ferramentas em script

Uma ferramenta em script é uma ferramenta nova que o agente pode chamar, escrita em JavaScript. Coloque-a em `~/{{DIR}}/tools/` (sua, em todo projeto) ou no `{{DIR}}/tools/` da pasta de trabalho (do projeto, depois que você confia na pasta), como `<nome>.js` ou `<nome>/tool.js`:

```javascript
export const spec = {
  name: "Release",
  description: "Tags the release. Use when the user asks to tag a version.",
  input: { type: "object", properties: { version: { type: "string" } }, required: ["version"] },
  access: "execute",
  capabilities: ["exec"],
};

export function run(input, ctx) {
  return ctx.exec(`git tag v${input.version}`).stdout;
}
```

| Campo do `spec` | Significado |
|---|---|
| `name` | O nome da ferramenta. Obrigatório. |
| `description` | O que o agente lê para decidir quando chamá-la |
| `input` | O JSON Schema da entrada; qualquer objeto quando omitido |
| `access` | O que uma chamada faz, para as permissões: `read`, `edit` ou `execute` (o padrão). Só as suas ferramentas definem isso; as de um projeto sempre contam como `execute`. Veja [Permissões](./permissions). |
| `capabilities` | As funções do host que ela chama: `readFile`, `writeFile`, `exec`, `fetch`. Vazia por padrão. |

A função `run` (ou `execute`) recebe a entrada e um contexto, e devolve um texto ou um valor, enviado como JSON; um erro lançado é uma chamada que falhou. Uma chamada a uma função do host que não está em `capabilities` falha; uma listada é julgada pelas suas permissões como a ferramenta correspondente (veja [Capacidades](./sandbox-and-safety#capacidades)). Uma ferramenta em script pode rodar por 60 segundos.

Uma ferramenta sua com o nome de uma ferramenta embutida a substitui; uma ferramenta do projeto com um nome já usado fica de fora. Um arquivo que não pode ser lido, ou cujo `spec` não tem `name`, fica de fora com um aviso no log.

## Skills embutidas

O {{NAME}} vem com estas skills, que você pode substituir por uma sua de mesmo nome:

| Skill | O que faz |
|---|---|
| `code-review` | Revisa o diff atual, um PR, uma branch ou um caminho em busca de bugs, no nível de esforço escolhido |
| `review` | Revisa as mudanças atuais com esforço médio |
| `security-review` | Revisa as mudanças pendentes em busca de problemas de segurança |
| `simplify` | Procura reaproveitamento, simplificação e eficiência no código alterado e aplica as melhorias |
| `verify` | Confere que uma mudança funciona rodando o fluxo afetado (só quando você digita `/verify`) |
| `run` | Abre e usa o app do projeto para ver uma mudança funcionando |
| `init` | Escreve um primeiro `{{INSTRUCTIONS}}` para o repositório |
| `configure` | Muda as configurações, servidores MCP e hooks do próprio {{NAME}} |
| `debug` | Ajuda a diagnosticar um problema do próprio {{NAME}} (só quando você digita `/debug`) |
| `skill-creator` | Cria e melhora skills |
| `follow-pr` | Acompanha um pull request até o merge, acordando com checks que falham e revisões novas |

### Skills atrás de uma feature flag

`follow-pr` fica disponível enquanto a feature `experimental-pr-follow` estiver ligada (o padrão). Desligá-la nas suas configurações remove a skill:

```json
{ "features": { "experimental-pr-follow": false } }
```

Veja [Recursos experimentais](./experimental-features).

## Mudar skills com um hook

Um hook `Brief` recebe todas as skills encontradas (nome, descrição, caminho, corpo e se o modelo pode invocá-la) e pode acrescentar, reescrever, esconder ou remover skills antes de o prompt ser montado. Veja [Hooks](./hooks#brief).

## Solução de problemas

| Sintoma | O que verificar |
|---|---|
| O agente nunca usa uma skill | A descrição não diz quando usá-la, ou `disable-model-invocation` é `true`. |
| `/nome` é enviado como texto comum | Nenhuma skill ou comando tem esse nome aqui; confira com `/skills`. |
| Uma skill parece ignorada | Outra com o mesmo nome é encontrada antes (a pasta de trabalho vem primeiro). |
| Falta uma skill de `.claude/skills` | Mova-a para `{{DIR}}/skills`. |

## Veja também

- [Comandos de barra](./slash-commands) para os comandos do próprio chat.
- [Agentes](./agents) para pré-carregar skills num agente.
- [Plugins](./plugins) para compartilhar skills e comandos.
