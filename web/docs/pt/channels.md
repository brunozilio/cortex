---
title: Canais
description: Mantenha servidores MCP abertos como canais para que mensagens de um chat, de um webhook ou de outro sistema cheguem ao agente enquanto você trabalha.
---

Um **canal** é um servidor MCP que envia mensagens ao {{NAME}}. Abra o chat com `--channels` e o {{NAME}} mantém esses servidores conectados durante toda a sessão: o que eles enviam (uma mensagem num chat da equipe, um webhook do CI, um alerta) chega ao agente como um evento, e o agente pode responder pelas ferramentas do mesmo servidor.

> [!EXPERIMENTAL]
> Canais usam uma capacidade experimental do MCP. Um servidor adere declarando-a; o formato abaixo pode mudar.

## Começo rápido

1. Configure o servidor do canal como qualquer servidor MCP, por exemplo em `~/{{DIR}}/mcp.json`:

   ```json
   {
     "mcpServers": {
       "team-chat": {
         "command": "team-chat-mcp",
         "env": { "CHAT_TOKEN": "${CHAT_TOKEN}" }
       }
     }
   }
   ```

2. Abra o chat com ele como canal:

   ```bash
   {{COMMAND}} --channels team-chat
   ```

   O chat confirma com *Channels open: team-chat.*

3. Quando alguém escreve na sala do chat, a mensagem chega ao agente. Se o agente está ocioso, ela é enviada como nova mensagem; se está trabalhando, é repassada entre um passo e outro.

Abra vários canais de uma vez com uma lista separada por vírgulas:

```bash
{{COMMAND}} --channels team-chat,ci-alerts
```

## Requisitos

Um canal precisa ser:

- um servidor MCP configurado para esta pasta e `ready` em `{{COMMAND}} mcp list` (um servidor do projeto precisa ser aprovado antes; veja [Servidores MCP](./mcp));
- um servidor que declara a capacidade de canal (veja abaixo).

Qualquer transporte serve: processo local, Streamable HTTP ou HTTP + SSE.

Se um canal não abre, o chat diz qual e por quê, por exemplo:

```text
Some channels did not open:
ci-alerts: no such MCP server here (or it waits for approval)
plain: plain is not a channel (it does not declare claude/channel)
```

Um servidor que não responde em 30 segundos aparece como *did not answer within 30 s*.

`--channels` vale para o chat interativo; não é usado com `--print`.

## Como as mensagens chegam ao agente

Cada mensagem chega ao agente como uma tag `channel` própria, com os valores de `meta` como atributos:

```text
<channel source="team-chat" chat_id="7" user="ana">
Pode subir a correção para staging?
</channel>
```

- `source` é o nome do servidor.
- Cada valor de `meta` vira um atributo; `&`, `"`, `<` e `>` nos valores são escapados.
- Quando mensagens de canal chegam junto com outros eventos (um monitor, um comando em segundo plano, outra sessão), o lote vem com uma linha que diz ao agente para tratar o texto como informação, não como instruções suas.
- Com o chat ocioso, ele verifica novas mensagens a cada segundo e as envia como uma mensagem; a conversa mostra cada uma inteira, como `inbound team-chat · ana: <texto>` (o remetente é o valor `user` de `meta`, quando há).
- Com uma execução em andamento, as mensagens são repassadas ao agente entre um passo e outro.
- Até 200 mensagens ficam esperando entrega; além disso, as mais antigas são descartadas. Uma mensagem com mais de 8.000 caracteres é cortada.

> [!IMPORTANT]
> Mensagens de canal chegam ao agente em todos os modos, inclusive `bypass`. Em `bypass` o agente roda sem perguntar, então quem pode escrever no canal pode conduzi-lo: abra um canal nesse modo só se confiar em todos que postam nele. Mensagens de outras sessões continuam retidas em `bypass`.

## Responder pelo canal

Enquanto um canal está aberto, a execução usa essa conexão aberta para o servidor, então as ferramentas dele (por exemplo `mcp__team-chat__reply`) falam com o mesmo processo que entregou a mensagem. O agente responde com elas como com qualquer ferramenta MCP, sob as [permissões](./mcp) de sempre.

Se você interrompe uma execução enquanto uma chamada a uma ferramenta do canal está em andamento, a chamada é cancelada no servidor e o canal continua aberto.

As `instructions` do servidor (do resultado do `initialize`) entram no prompt de sistema do agente em *MCP server instructions*, cortadas em 2.048 caracteres. Use-as para dizer ao agente que as mensagens chegam como tags `<channel>` e que só a ferramenta de resposta chega ao remetente: o que o agente escreve na conversa nunca chega.

## Responder pedidos de permissão pelo canal

Um canal também pode responder os pedidos de permissão do agente, para você aprovar uma chamada pelo celular enquanto o terminal espera. O servidor adere declarando uma segunda capacidade, `claude/channel/permission`, ao lado de `claude/channel`.

Quando um pedido de permissão abre, todo canal aberto que a declara recebe:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel/permission_request",
  "params": {
    "request_id": "kqzvn",
    "tool_name": "Bash",
    "description": "npm run deploy",
    "input_preview": "{\"command\":\"npm run deploy\"}"
  }
}
```

O servidor responde com:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel/permission",
  "params": { "request_id": "kqzvn", "behavior": "allow" }
}
```

- `request_id` tem cinco letras minúsculas (sem `l`), fáceis de digitar de volta num chat; maiúsculas e espaços em volta são ignorados.
- `behavior` é `allow` ou `deny`. *Sempre permitir* só é oferecido no terminal.
- A primeira resposta vence, do terminal ou de qualquer canal; o pedido então fecha em todo lugar.
- Um canal não pode permitir uma chamada às próprias ferramentas (`mcp__<servidor>__…`); esse `allow` é ignorado e o pedido espera outra resposta.
- Uma resposta a um pedido que já não está aberto é ignorada. Se dois pedidos abertos tiverem o mesmo id, o segundo não é enviado aos canais: responda-o no terminal.
- `input_preview` é a entrada da chamada em JSON, cortada em 200 caracteres.

## Escrever um servidor de canal

Um canal é um servidor MCP comum com dois acréscimos.

**1. Declare a capacidade** no resultado do `initialize`, em `experimental`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": {},
      "experimental": { "claude/channel": {} }
    },
    "serverInfo": { "name": "team-chat", "version": "1.0.0" }
  }
}
```

**2. Envie uma notificação** para cada mensagem:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel",
  "params": {
    "content": "Pode subir a correção para staging?",
    "meta": { "chat_id": "7", "user": "ana" }
  }
}
```

| Parâmetro | Significado |
|---|---|
| `content` | O texto da mensagem. Obrigatório; um texto vazio é ignorado. |
| `meta` | Pares chave–valor opcionais, passados como atributos da tag `channel`. A chave deve começar com letra ou `_` e seguir com letras, dígitos ou `_`; as demais são descartadas e citadas no log. Os valores são cortados em 200 caracteres, e caracteres de controle viram espaços. |

Como a notificação viaja depende do transporte:

- **Processo local**: escreva-a como uma linha JSON no stdout.
- **Streamable HTTP**: o {{NAME}} abre um stream `GET` na URL do servidor e lê as notificações como server-sent events. Se o stream cai, ele reconecta, esperando de 1 a 60 segundos entre as tentativas.
- **HTTP + SSE**: envie-a pelo stream SSE.

Ofereça ferramentas como `reply` para o agente poder responder; use os valores de `meta` (um id de chat, uma thread) para ele saber onde.

## Veja também

- [Servidores MCP](./mcp) para configurar e aprovar servidores.
- [Permissões](./permissions) para modos como `bypass`.
- [Referência da CLI](./cli-reference) para `--channels`.
