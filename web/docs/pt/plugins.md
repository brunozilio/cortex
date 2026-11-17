---
title: Plugins
description: Instale, escreva e compartilhe plugins que trazem skills, comandos, agentes, hooks, servidores MCP e servidores de linguagem para o {{NAME}} numa só pasta.
---

Um **plugin** é uma pasta que reúne extensões para o {{NAME}}: skills, comandos personalizados, agentes, hooks, servidores MCP e servidores de linguagem. Instale um a partir de uma URL git e tudo o que ele contém fica disponível em todos os projetos, sob o nome do plugin.

## Começo rápido

Instale um plugin pelo git:

```bash
{{COMMAND}} plugin install https://github.com/acme/{{COMMAND}}-acme-tools.git
```

```text
Installed {{COMMAND}}-acme-tools. A new chat or /reload picks it up.
```

Num chat aberto, `/reload` mostra o que foi encontrado:

```text
Reloaded ~/{{DIR}} and the project:
  plugins  acme
  skills   14
  agents   5
  MCP      3 ready
```

Depois, use o que ele traz: `/acme:triage`, o agente `acme:reviewer` ou as ferramentas MCP dele.

## Gerenciar plugins

Os plugins ficam em `~/{{DIR}}/plugins/`, uma pasta para cada.

| Comando | O que faz |
|---|---|
| `{{COMMAND}} plugin` ou `{{COMMAND}} plugin list` | Lista os plugins instalados (nome da pasta e descrição) |
| `{{COMMAND}} plugin install <url-git-ou-caminho> [--name <pasta>]` | Clona em `~/{{DIR}}/plugins/<pasta>` (um clone raso) |
| `{{COMMAND}} plugin update <pasta>` | Puxa os commits mais recentes (`git pull --ff-only`) |
| `{{COMMAND}} plugin remove <pasta>` | Apaga a pasta |

- O nome da pasta vem da origem (`…/acme-tools.git` dá `acme-tools`), a menos que você passe `--name`. Ele pode usar letras, dígitos, `-`, `_` e `.`, e não pode começar com `.`.
- Instalar por cima de uma pasta existente é recusado: atualize ou remova antes.
- `update` só funciona num plugin clonado com git.
- Esses comandos precisam do `git` no seu `PATH`.

Você também pode copiar uma pasta, ou criar um link simbólico, em `~/{{DIR}}/plugins/` à mão: toda pasta ali é um plugin.

As mudanças entram no próximo prompt, num chat novo ou com `/reload`.

> [!WARNING]
> Os hooks e servidores MCP de um plugin rodam programas no seu computador, e os servidores MCP dele iniciam sem pedir aprovação. Instale plugins só de fontes em que você confia e leia o `hooks/hooks.json` e o `.mcp.json` deles.

## Estrutura de um plugin

```text
acme-tools/
├── {{PLUGIN_DIR}}/
│   └── plugin.json          o manifesto
├── skills/
│   └── triage/SKILL.md      → /acme:triage
├── commands/
│   └── hello.md             → /acme:hello
├── agents/
│   └── reviewer.md          → acme:reviewer
├── hooks/
│   └── hooks.json           hooks
├── .mcp.json                servidores MCP
├── .lsp.json                servidores de linguagem
└── bin/                     o que mais o plugin precisar
```

Todas as partes são opcionais.

### O manifesto

`{{PLUGIN_DIR}}/plugin.json`:

```json
{
  "name": "acme",
  "description": "Skills de triagem, agente revisor e servidor do Jira da ACME"
}
```

| Campo | Significado |
|---|---|
| `name` | O nome do plugin, usado como namespace de tudo o que ele traz. Por padrão, o nome da pasta. |
| `description` | Mostrada por `{{COMMAND}} plugin list` |
| `mcpServers` | Servidores MCP, usados quando o plugin não tem `.mcp.json` |
| `lspServers` | Servidores de linguagem, usados quando o plugin não tem `.lsp.json` |

Quando dois plugins têm o mesmo nome, só o primeiro é usado.

## O que um plugin pode trazer

| Parte | Onde | Nome |
|---|---|---|
| Skills | `skills/<nome>/SKILL.md` (ou `SKILL.js`) | `<plugin>:<nome>` |
| Comandos personalizados | `commands/**/*.md` (ou `.js`) | `<plugin>:<caminho>`, como `acme:db:migrate` |
| Agentes | `agents/*.md` | `<plugin>:<nome>` |
| Hooks | `hooks/hooks.json` | — |
| Servidores MCP | `.mcp.json`, ou `mcpServers` no manifesto | `plugin_<plugin>_<servidor>` |
| Servidores de linguagem | `.lsp.json`, ou `lspServers` no manifesto | — |

Skills, comandos e agentes seguem os mesmos formatos dos seus (veja [Skills](./skills) e [Agentes](./agents)). Os seus e os do projeto têm precedência sobre os de um plugin quando os nomes coincidem, mas o namespace evita que isso aconteça por acaso.

### Hooks

O `hooks/hooks.json` tem os mesmos eventos de `"hooks"` nas configurações, no nível de cima ou dentro de uma chave `"hooks"`:

```json
{
  "description": "Verificações da ACME",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [{ "type": "command", "command": "${{{ENV}}_PLUGIN_ROOT}/bin/check.sh" }]
      }
    ]
  }
}
```

Um `hooks.json` quebrado (JSON inválido, evento desconhecido…) é deixado de fora, com um aviso no log; o resto do plugin continua funcionando. Veja [Hooks](./hooks).

### Servidores MCP

```json
{
  "mcpServers": {
    "jira": {
      "command": "${{{ENV}}_PLUGIN_ROOT}/bin/jira-mcp",
      "args": ["--root", "${{{ENV}}_PLUGIN_ROOT}"]
    }
  }
}
```

O servidor acima se chama `plugin_acme_jira` (caracteres que não são letras, dígitos ou `-` viram `_`), então as ferramentas dele são `mcp__plugin_acme_jira__<ferramenta>`. Um servidor de plugin chamado `{{COMMAND}}_…` é ignorado. Servidores de plugins ficam `ready` assim que o plugin é instalado. Diferente do seu `mcp.json`, o arquivo de um plugin não lê outras variáveis de ambiente: só `${{{ENV}}_PLUGIN_ROOT}` é preenchida. Veja [Servidores MCP](./mcp).

### Servidores de linguagem

O `.lsp.json` liga o nome de um servidor à configuração dele, no mesmo formato de `lspServers` nas configurações:

```json
{
  "zls": {
    "command": "zls",
    "extensionToLanguage": { ".zig": "zig" }
  }
}
```

Um conjunto quebrado é deixado de fora, com um aviso. Veja [Inteligência de código](./code-intelligence).

### `${{{ENV}}_PLUGIN_ROOT}`

Escreva `${{{ENV}}_PLUGIN_ROOT}` em qualquer lugar de `hooks/hooks.json`, `.mcp.json`, `.lsp.json` ou de `mcpServers` e `lspServers` no manifesto: ele é trocado pela pasta do plugin antes do uso, então o plugin funciona onde quer que esteja instalado.

## Escrever um plugin

1. Crie uma pasta com `{{PLUGIN_DIR}}/plugin.json` e um `name`.
2. Acrescente as partes de que precisar.
3. Crie um link para ela em `~/{{DIR}}/plugins/` para testar:

   ```bash
   ln -s "$PWD" ~/{{DIR}}/plugins/acme-tools
   ```

4. Rode `/reload` num chat e confira `/skills`, `/agents`, `/mcp` e `/hooks`.
5. Envie para um repositório git; outras pessoas instalam com `{{COMMAND}} plugin install <url>`.

## Veja também

- [Skills](./skills), [Agentes](./agents), [Hooks](./hooks), [Servidores MCP](./mcp).
- [Arquivos e diretórios](./files-and-directories).
