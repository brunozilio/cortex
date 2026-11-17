---
title: Servidores MCP
description: Conecte o {{NAME}} a ferramentas e dados externos por servidores do Model Context Protocol, via HTTP, SSE ou um processo local.
---

O [Model Context Protocol](https://modelcontextprotocol.io) (MCP) é um padrão aberto para dar ferramentas e dados a agentes de IA. Conecte um servidor MCP ao {{NAME}} e as ferramentas dele viram ferramentas que o agente pode chamar: buscar no seu gerenciador de issues, consultar um banco de dados, ler um arquivo de design, postar no chat.

O {{NAME}} suporta:

- servidores remotos via **Streamable HTTP** e o transporte mais antigo **HTTP + SSE**, com login OAuth;
- servidores locais iniciados como um **processo** que conversa por stdin e stdout;
- **servidores em JavaScript** executados pelo runtime de scripts embutido do {{NAME}};
- servidores trazidos por [plugins](./plugins).

## Começo rápido

Adicione um servidor remoto:

```bash
{{COMMAND}} mcp add tracker https://mcp.example.com/mcp
```

Adicione um servidor local iniciado com `npx`, passando um token do seu ambiente:

```bash
{{COMMAND}} mcp add -e GITHUB_TOKEN='${GITHUB_TOKEN}' github -- npx -y @modelcontextprotocol/server-github
```

Veja o que uma sessão nesta pasta vai usar:

```bash
{{COMMAND}} mcp list
```

```text
tracker  user           ready                 https://mcp.example.com/mcp
github   user           ready                 npx -y @modelcontextprotocol/server-github
```

Servidores novos entram no próximo prompt. Peça ao agente para usá-los ("ache os bugs abertos atribuídos a mim no tracker") ou cite uma ferramenta diretamente.

> [!TIP]
> Você também pode pedir ao agente que configure um servidor, por exemplo "configure o MCP do Jira". A skill embutida `configure` edita o `mcp.json` do próprio {{NAME}} e diz o que escreveu.

## Onde os servidores são configurados

| Arquivo | Escopo | Inicia |
|---|---|---|
| `~/{{DIR}}/mcp.json` | Seus servidores, em todos os projetos | Na hora |
| `{{DIR}}/mcp.json` no projeto (o mais próximo encontrado na pasta de trabalho ou numa pasta acima) | Os servidores do projeto, versionados para a equipe | Só depois que você aprovar cada um |
| `~/{{DIR}}/mcp/*.js` | Seus servidores em JavaScript | Na hora |
| `{{DIR}}/mcp/*.js` na pasta de trabalho | Os servidores em JavaScript do projeto | Só depois que você aprovar cada um |
| O `.mcp.json` de um plugin instalado (ou `mcpServers` no manifesto) | Onde o plugin estiver instalado | Na hora |

> [!NOTE]
> O {{NAME}} lê apenas os próprios arquivos `mcp.json`. Um `.mcp.json` na raiz do projeto, ou a configuração de outro app, não é lido.

### Formato do arquivo

O `mcp.json` tem um objeto `mcpServers`. Um servidor com `url` é remoto; um servidor com `command` é um processo local:

```json
{
  "mcpServers": {
    "tracker": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer ${TRACKER_TOKEN}" }
    },
    "legacy": {
      "type": "sse",
      "url": "https://old.example.com/sse"
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "${GITHUB_TOKEN}" }
    },
    "sonar": {
      "type": "stdio",
      "command": "sonar-mcp",
      "env": { "SONAR_HOST": "${SONAR_HOST:-https://sonar.local}" }
    }
  }
}
```

| Campo | Significado |
|---|---|
| `type` | `http` (Streamable HTTP), `sse` (HTTP + SSE) ou `stdio` (processo local). Opcional: um servidor só com `url` é `http`; um com `command` é `stdio`. |
| `url` | O endereço do servidor (`http` e `sse`) |
| `headers` | Cabeçalhos das requisições (`http` e `sse`) |
| `command` | O programa a iniciar (`stdio`) |
| `args` | Os argumentos dele (`stdio`) |
| `env` | Variáveis de ambiente acrescentadas ao processo (`stdio`) |
| `sandbox` | Onde o processo pode gravar e se ele alcança a rede (`stdio`): `{ "writable": [...], "network": false }`, ou `false` para iniciá-lo fora da sandbox. `writable` e `false` só valem no seu próprio `~/{{DIR}}/mcp.json`. Veja "Servidores MCP na sandbox" em [Sandbox e segurança](./sandbox-and-safety). |

Um servidor com um valor do tipo errado ou fora das opções aceitas (outro `type`, um `sandbox` de outro formato, `args` que não é lista) é deixado de fora; os outros servidores do arquivo continuam iniciando. Uma chave que o {{NAME}} não conhece é ignorada. Os dois casos aparecem no `{{COMMAND}} mcp list` com o arquivo e o caminho da chave, por exemplo `warning: /home/eu/{{DIR}}/mcp.json: mcpServers.docs.type: expected one of "stdio", "http", "sse", found "websocket"`.

O arquivo pode indicar o JSON Schema dele, para que o editor complete as chaves e sublinhe os erros; o {{NAME}} ignora essa chave. O schema é copiado para `~/{{DIR}}/schemas/mcp.schema.json`, então o seu próprio arquivo pode usar um caminho relativo, e o de um projeto o caminho completo da sua cópia:

```json
{
  "$schema": "./schemas/mcp.schema.json",
  "mcpServers": {}
}
```

Veja [JSON Schema](./settings#json-schema) para os dois schemas.

### Variáveis de ambiente no arquivo

Em `~/{{DIR}}/mcp.json` e no `{{DIR}}/mcp.json` do projeto, qualquer string pode usar:

| Sintaxe | Valor |
|---|---|
| `${NOME}` | A variável `NOME` do ambiente do {{NAME}} quando o servidor inicia |
| `${NOME:-padrão}` | `NOME`, ou `padrão` quando ela não está definida |

Uma variável não definida e sem padrão fica como foi escrita. Use isso para todo segredo: nunca escreva um token no arquivo.

### Nomes de servidor

Um nome de servidor pode usar letras, dígitos, `-` e `_`. Nomes que começam com `{{COMMAND}}_` ou `plugin_` são reservados: um servidor configurado com um desses nomes nunca inicia.

Quando o seu arquivo e o do projeto declaram o mesmo nome, um servidor do projeto aprovado vence; enquanto o do projeto não for aprovado, o seu é usado.

## O comando `{{COMMAND}} mcp`

| Comando | O que faz |
|---|---|
| `{{COMMAND}} mcp` ou `{{COMMAND}} mcp list` | Lista os servidores, de onde vem cada um e se ele inicia |
| `{{COMMAND}} mcp get <nome>` | Mostra um servidor: origem, estado, alvo, arquivo e a entrada guardada, com os segredos ocultos |
| `{{COMMAND}} mcp add <nome> <url-ou-comando> [-- args…]` | Acrescenta um servidor seu em `~/{{DIR}}/mcp.json` |
| `{{COMMAND}} mcp remove <nome>` | Remove um servidor seu |
| `{{COMMAND}} mcp approve <nome>` | Deixa um servidor do projeto iniciar |
| `{{COMMAND}} mcp deny <nome>` | Desliga um servidor do projeto |
| `{{COMMAND}} mcp login <nome>` | Faz login num servidor remoto pelo navegador (OAuth) |
| `{{COMMAND}} mcp logout <nome>` | Esquece o login num servidor remoto |

`add` aceita estas opções, em qualquer lugar antes de `--`:

| Opção | Significado |
|---|---|
| `-e`, `--env CHAVE=VALOR` | Uma variável de ambiente para um servidor local (pode repetir). Use aspas simples em `'${VAR}'` para o shell deixá-la para o {{NAME}} ler ao iniciar. |
| `-H`, `--header "Nome: valor"` | Um cabeçalho para um servidor remoto (pode repetir). Coloque `'${VAR}'` entre aspas simples do mesmo jeito. |
| `--transport sse` | Usa o transporte mais antigo HTTP + SSE para uma URL (`http` por padrão) |
| `--force` | Substitui um servidor seu que já tem esse nome |

Um alvo que começa com `https://` ou `http://` cria um servidor remoto; qualquer outra coisa é o comando de um servidor local, e o que vem depois são os argumentos dele. Escreva `--` antes dos argumentos do servidor quando algum deles começa com `-`: tudo depois de `--` é do servidor. Um argumento que começa com `-` sem `--` antes é recusado, com a dica de acrescentar um.

```bash
{{COMMAND}} mcp add -H 'Authorization: Bearer ${API_TOKEN}' docs https://docs.example.com/mcp
{{COMMAND}} mcp add --transport sse legacy https://old.example.com/sse
{{COMMAND}} mcp add jira jira-mcp -- --stdio
{{COMMAND}} mcp add github -- npx -y @modelcontextprotocol/server-github
```

`add` recusa um nome com outros caracteres, um nome reservado, um servidor sem comando nem URL, ou um nome que você já usa (a não ser com `--force`; o erro mostra a entrada que existe). Também recusa um segredo escrito por extenso: um valor de cabeçalho, de ambiente ou um argumento que parece um token, ou um cabeçalho ou variável de ambiente cujo nome diz que guarda um (`Authorization`, `GITHUB_TOKEN`, `API_KEY`…), a não ser que o valor leia uma `${VAR}`. `remove` só remove servidores do seu próprio `~/{{DIR}}/mcp.json`. `approve`, `deny` e `get` recusam um nome que não está na lista e dizem quais estão.

### A lista

Cada linha mostra o nome, a origem, o estado e o alvo (URL ou linha de comando). Cabeçalhos e valores de ambiente nunca aparecem.

| Origem | Significado |
|---|---|
| `user` | `~/{{DIR}}/mcp.json` ou `~/{{DIR}}/mcp/` |
| `project` | O `{{DIR}}/mcp.json` ou `{{DIR}}/mcp/` do projeto |
| `plugin <nome>` | Um plugin instalado |
| `host` | Fornecido por um app que embute o {{NAME}} |

| Estado | Significado |
|---|---|
| `ready` | Inicia no próximo prompt |
| `waiting for approval` | Um servidor do projeto que você ainda não aprovou |
| `off` | Um servidor do projeto que você desligou |
| `skipped: folder not trusted` | Um servidor de um projeto cuja pasta você não marcou como confiável: aparece na lista, mas nada do projeto inicia |

Um arquivo de configuração que não é JSON válido aparece com o erro dele. Quando é o seu `~/{{DIR}}/mcp.json`, `{{COMMAND}} mcp list` falha (código de saída 1); um `{{DIR}}/mcp.json` quebrado do projeto vira uma linha `warning:` e a lista segue. Um valor deixado de fora de qualquer um dos dois (veja [Formato do arquivo](#formato-do-arquivo)) também vira uma linha `warning:`.

## No chat: `/mcp`

| Comando | O que faz |
|---|---|
| `/mcp` | Lista os servidores, como `{{COMMAND}} mcp list` |
| `/mcp approve <nome>` | Aprova um servidor do projeto |
| `/mcp deny <nome>` | Desliga um servidor do projeto |
| `/mcp login <nome>` | Faz login num servidor remoto pelo navegador |
| `/mcp logout <nome>` | Esquece esse login |

A mudança vale a partir da próxima mensagem. `/reload` relê `~/{{DIR}}` e o projeto e conta os servidores prontos, aguardando aprovação ou desligados.

## Aprovar servidores do projeto

Um servidor no `{{DIR}}/mcp.json` versionado de um projeto roda um programa ou chama uma URL em seu nome, então duas coisas precisam valer antes de ele iniciar:

1. Você confia na pasta do projeto. Até lá os servidores dele não são iniciados; `{{COMMAND}} mcp list` os mostra como `skipped: folder not trusted`, e `{{COMMAND}} doctor` os conta na linha de confiança. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).
2. Você aprova aquele servidor. Confiar na pasta não aprova os servidores dela.

As aprovações ficam nas suas próprias configurações, nunca no arquivo compartilhado do projeto:

- `{{COMMAND}} mcp approve <nome>` (ou `/mcp approve <nome>`) acrescenta o nome a `enabledMcpServers` no `settings.local.json` ao lado do `mcp.json` do projeto, e o tira de `disabledMcpServers`.
- `{{COMMAND}} mcp deny <nome>` faz o contrário.

Você também pode editar as chaves à mão:

```json
{
  "enabledMcpServers": ["jira"],
  "disabledMcpServers": ["sonar"],
  "enableAllProjectMcpServers": false
}
```

| Chave | Significado |
|---|---|
| `enabledMcpServers` | Servidores do projeto que podem iniciar |
| `disabledMcpServers` | Servidores do projeto desligados; vence as outras duas |
| `enableAllProjectMcpServers` | `true` aprova todos os servidores do projeto |

Essas chaves só contam nos seus próprios arquivos: `~/{{DIR}}/settings.json`, `~/{{DIR}}/managed-settings.json` e o `settings.local.json` de um projeto em cuja pasta você confia. No `{{DIR}}/settings.json` versionado de um projeto elas são ignoradas.

## Login com OAuth

Servidores remotos que usam OAuth precisam de login:

```bash
{{COMMAND}} mcp login tracker
```

O {{NAME}} abre a página de login do servidor no navegador e espera a resposta num endereço local. Ele:

1. descobre o servidor de autorização pelos metadados OAuth do servidor;
2. se registra como cliente (o servidor precisa permitir registro dinâmico de clientes);
3. faz o login com PKCE;
4. guarda os tokens lacrados no seu cofre de segredos e os renova quando expiram.

A partir daí, toda requisição para a URL daquele servidor leva `Authorization: Bearer <token>`, a menos que a configuração do servidor já defina um cabeçalho `Authorization`. Só endereços `https`, ou `http` para este computador, são aceitos para login. Um servidor local (`stdio`) não faz login. `{{COMMAND}} mcp logout <nome>` esquece os tokens.

## Ferramentas, prompts e recursos

### Nomes das ferramentas

Cada ferramenta que um servidor oferece vira uma ferramenta chamada:

```text
mcp__<servidor>__<ferramenta>
```

Por exemplo `mcp__github__create_issue`. O servidor de um plugin se chama `plugin_<plugin>_<servidor>`, então as ferramentas dele ficam como `mcp__plugin_acme_jira__search`.

Quando há mais de 20 ferramentas MCP disponíveis, elas não são todas enviadas ao modelo de saída: o agente recebe uma ferramenta `ToolSearch` e carrega as ferramentas MCP de que precisa buscando por elas.

### Prompts

Um servidor que oferece prompts pode ser usado como comando no chat:

```text
/mcp__<servidor>__<prompt> arg1 arg2 o resto
```

As palavras preenchem os argumentos do prompt em ordem; o último argumento recebe o resto da linha. O texto do prompt é enviado como sua mensagem.

### Recursos

Quando um servidor oferece recursos, o agente ganha mais duas ferramentas: `ListMcpResources` para listá-los e `ReadMcpResource` para ler um deles.

### Resultados

Texto, imagens, recursos de texto embutidos e links para recursos no resultado de uma ferramenta chegam ao agente. Conteúdo estruturado é acrescentado como JSON quando o texto ainda não o contém.

## Permissões para ferramentas MCP

Ferramentas MCP contam como ações. Nos modos `ask` e `edits`, o agente pergunta antes de chamar uma, a menos que uma regra permita; no modo `bypass`, elas rodam. Uma regra pode citar um servidor inteiro ou uma ferramenta:

```json
{
  "permissions": {
    "allow": ["mcp__jira"],
    "deny": ["mcp__github__delete_repo"]
  }
}
```

| Regra | Casa com |
|---|---|
| `mcp__jira` | Todas as ferramentas do servidor `jira` (não as de um servidor chamado `jiraffe`) |
| `mcp__github__delete_repo` | Só essa ferramenta |

Veja [Permissões](./permissions) para como as regras se combinam. Um agente próprio pode ser limitado a alguns servidores com `mcpServers` no frontmatter (veja [Agentes](./agents)), e um hook pode checar chamadas MCP com um matcher como `^mcp__github__` (veja [Hooks](./hooks)).

## Servidores em JavaScript

Um arquivo `.js` em `~/{{DIR}}/mcp/` (ou no `{{DIR}}/mcp/` da pasta de trabalho) é um servidor com o nome do arquivo. O export padrão lista as ferramentas:

```javascript
export default {
  capabilities: ["readFile"],
  tools: {
    add: {
      description: "Soma a e b.",
      input: { type: "object", properties: { a: { type: "number" }, b: { type: "number" } } },
      run: (input) => String(input.a + input.b),
    },
    notes: {
      description: "Lê as notas do projeto.",
      run: (input, ctx) => ctx.readFile("NOTES.md"),
    },
  },
};
```

Um `math.js` com esse conteúdo oferece `mcp__math__add` e `mcp__math__notes`. Uma ferramenta retorna texto (ou um valor, enviado como JSON); um erro lançado é uma chamada que falhou. O servidor lista em `capabilities` cada função do host que suas ferramentas chamam (`readFile`, `writeFile`, `exec`, `fetch`); uma chamada não listada falha. Cada chamada listada é julgada pelas suas regras de permissão e pelo modo como a ferramenta correspondente (`ctx.readFile` como `Read`, `ctx.exec` como `Bash`), e a configuração `scripts` continua sendo o limite de fora (veja "Capacidades" em [Sandbox e segurança](./sandbox-and-safety) e [Configurações](./settings)).

> [!WARNING]
> Mudança incompatível: um servidor em JavaScript que chama `ctx.readFile`, `ctx.writeFile`, `ctx.exec` ou `ctx.fetch` sem listar em `capabilities` agora falha com um erro dizendo o que acrescentar.

## Como os servidores iniciam

No primeiro prompt de uma sessão, o {{NAME}} inicia todo servidor `ready` (versão `2025-06-18` do protocolo MCP). Os servidores continuam conectados nos prompts seguintes da sessão; cada prompt pede de novo as ferramentas deles. Um servidor só é iniciado de novo quando a entrada ou a aprovação dele mudou, ou quando o processo dele parou; um servidor que deixou de estar `ready` é fechado. Quando o {{NAME}} libera a sessão (veja [`SessionEnd`](./hooks#sessionend)), os servidores dela são fechados. Os servidores em JavaScript são carregados de novo a cada prompt.

Um servidor que não consegue iniciar, ou listar suas ferramentas, fica fora daquela execução, com um aviso no log; a execução segue sem ele.

- Um servidor local roda como processo filho com o seu ambiente mais o `env` dele, dentro da sandbox quando ela está ligada (veja "Servidores MCP na sandbox" em [Sandbox e segurança](./sandbox-and-safety)). Quando ele para, o erro cita as últimas linhas que ele escreveu no stderr. Uma chamada pode levar até 10 minutos.
- Um servidor remoto mantém o id de sessão que recebe entre as chamadas.

## Solução de problemas

| Sintoma | O que verificar |
|---|---|
| Um servidor do projeto nunca inicia | `{{COMMAND}} mcp list` mostra `waiting for approval`: rode `{{COMMAND}} mcp approve <nome>`. |
| Um servidor aparece na lista mas as ferramentas não | Ele não conseguiu iniciar ou listar as ferramentas. Rode o comando à mão, confira a URL e veja o log (veja [Solução de problemas](./troubleshooting)). |
| `${TOKEN}` chega ao servidor do jeito que foi escrito | A variável não está definida no ambiente em que você abriu o {{NAME}}. |
| `{{COMMAND}} mcp login` diz que não há metadados OAuth, ou registro | O servidor não suporta login OAuth com registro dinâmico. Use um cabeçalho com token. |
| Um servidor chamado `{{COMMAND}}_…` ou `plugin_…` é ignorado | Esses prefixos são reservados. Mude o nome. |

## Veja também

- [Canais](./channels) para deixar um servidor MCP enviar mensagens ao agente.
- [Plugins](./plugins) para compartilhar servidores.
- [Permissões](./permissions).
