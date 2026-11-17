---
title: Modelos e provedores
description: Conecte um provedor de LLM com chave de API ou assinatura, escolha um modelo e ajuste papéis, fallbacks, esforço e contexto.
---

O {{NAME}} não vem com um modelo próprio: ele conversa com os provedores de LLM que você conecta. Você pode conectar vários provedores ao mesmo tempo, e todos os modelos que eles oferecem aparecem numa lista só. Esta página mostra como conectar provedores, escolher modelos, dar modelos a papéis, definir fallbacks e ajustar o esforço de raciocínio.

## Conectar um provedor

Rode o comando de login num terminal:

```bash
{{COMMAND}} login
```

Ele mostra os provedores disponíveis, cada um com a forma de conectar, e pede um número:

```text
Connect a provider:
  1. Anthropic API (paste an API key)
  2. Claude Subscription (sign in with the browser)
  3. OpenAI API (paste an API key)
  ...
Choose 1-7:
```

- **Provedores com chave de API** pedem a chave. O que você digita fica oculto e aparece como `•`; colar funciona. Esc ou Ctrl+C cancela.
- **Provedores com login no navegador** abrem a página de login no navegador. Se ele não abrir, a URL é impressa para você visitar.
- **Provedores com login por código** imprimem um código e uma URL (`Enter the code AB12-CD34 at https://github.com/login/device`), abrem o navegador e esperam até você aprovar.

Quando dá certo, aparece `Connected <provedor> (<conta>).`

Dentro do chat, `/login` abre a mesma lista como um seletor.

> [!TIP]
> Rode `{{COMMAND}} doctor` depois de conectar: as linhas `providers` e `models` confirmam que o provedor está conectado e que oferece modelos. Veja [Solução de problemas](./troubleshooting).

### Chaves de API versus assinaturas

| Tipo | Como conecta | O que você paga |
|---|---|---|
| Chave de API | Você cola uma chave do console do provedor | O provedor cobra a chave por token |
| Assinatura | Você faz login no navegador ou com um código de dispositivo | O plano de assinatura que você já tem |

Chaves e tokens de login ficam guardados na sua pasta de dados, selados em `secrets.json` (legível só por você), cuja chave fica em `secrets.key` ao lado dele, ou no chaveiro do sistema se você definir `{{ENV}}_SECRETS=keychain`. Os tokens de assinatura são renovados sozinhos quando expiram. Veja [Arquivos e diretórios](./files-and-directories).

> [!NOTE]
> As chaves vêm só do que você conecta com `{{COMMAND}} login` ou `/login`. O {{NAME}} não lê chaves de provedores de variáveis de ambiente.

Espaços em volta da chave são removidos; uma chave vazia, ou com espaços no meio, é recusada.

## Provedores suportados

Os provedores aparecem nesta ordem em `{{COMMAND}} login` e `/login`:

| Provedor | Id | Conecta com | Janela de contexto padrão |
|---|---|---|---|
| Anthropic API | `anthropic` | Chave de API | 200.000 tokens |
| Claude Subscription | `claude-subscription` | Login no navegador | 200.000 tokens |
| OpenAI API | `openai` | Chave de API | 400.000 tokens |
| ChatGPT Subscription | `openai-subscription` | Login no navegador | 400.000 tokens |
| GitHub Copilot Subscription | `copilot-subscription` | Código de dispositivo | 131.072 tokens |
| Kiro Subscription | `kiro-subscription` | Login no navegador | 200.000 tokens |
| Grok Subscription | `grok-subscription` | Código de dispositivo | 500.000 tokens |
| Antigravity Subscription | `antigravity-subscription` | Login no navegador | 200.000 tokens |
| Provedores declarados | você escolhe | Chave de API | 128.000 tokens, salvo se declarado |

Quando o catálogo do provedor informa o tamanho da janela de um modelo, esse número é usado; senão vale o padrão desta tabela.

### Anthropic API

Cole uma chave do console da Anthropic. Os turnos vão para a Messages API em `api.anthropic.com`, e a lista de modelos vem do catálogo de modelos da API. Preços, nomes curtos de modelos e regras de raciocínio vêm da família de modelos Anthropic embutida.

### Claude Subscription

Faz login pelo navegador; a página de login devolve você para `http://localhost:<porta livre>/callback`. Os turnos vão para a Messages API com a sua assinatura. Quando o catálogo da conta vem vazio, é oferecida uma lista fixa de modelos atuais.

### OpenAI API

Cole uma chave da plataforma da OpenAI. Os turnos vão para a Responses API em `api.openai.com/v1`; só os modelos de chat do catálogo aparecem.

### ChatGPT Subscription

Faz login pelo navegador com o callback `http://localhost:1455/auth/callback`, então a porta 1455 precisa estar livre durante o login. Os turnos vão para o backend do Codex em `chatgpt.com/backend-api/codex`, sempre com um esforço de raciocínio. Os tokens são renovados antes de expirar, e mais uma vez depois de um 401.

### GitHub Copilot Subscription

Faz login com o fluxo de dispositivo do GitHub: você digita o código impresso em `github.com/login/device`. Tokens clássicos `ghp_` são recusados. O seletor mostra o que o `/models` do Copilot lista, e cada modelo roda na melhor API que oferece (`/v1/messages`, depois `/responses`, depois `/chat/completions`). As idas e voltas de ferramentas não são enviadas como requisições premium: só os turnos que começam com uma mensagem sua.

### Kiro Subscription

Faz login pelo navegador via `app.kiro.dev/signin`, com o callback `http://localhost:49153/oauth/callback`. O Kiro aceita só essa porta, então ela precisa estar livre durante o login. Logins com Google, GitHub, Builder ID e de organização (IAM Identity Center) funcionam; um login de organização continua na página da sua organização e precisa de um perfil Kiro para a sua conta, senão falha com `signed in, but this account has no Kiro profile`. Os tokens são renovados sozinhos, também depois de um 401 ou 403.

### Grok Subscription

Usa uma assinatura SuperGrok ou X Premium. Faz login com o fluxo de dispositivo da xAI: aparecem um código e um link para aprová-lo, e o {{NAME}} espera até você aprovar. Os turnos vão para a Responses API da xAI em `api.x.ai/v1`. Os tokens duram cerca de seis horas e são renovados antes de expirar e uma vez depois de um 401. Só os modelos de chat do Grok aparecem (nada de modelos de imagem, vídeo, fala ou embeddings).

Só alguns modelos Grok aceitam um nível de esforço (`grok-3-mini`, `grok-4.20-multi-agent`, `grok-4.3`, `grok-4.5`, `grok-4.6`, ou o que o catálogo disser); o `grok-4.6` aceita `xhigh`, os outros param em `high`. Os demais modelos Grok raciocinam por conta própria.

### Antigravity Subscription

Usa a conta Google que você usa no Antigravity. Faz login pelo Google no navegador com o callback `http://localhost:51121/oauth-callback`, então a porta 51121 precisa estar livre durante o login. Depois do login o seu projeto do Cloud Code é procurado (uma conta sem projeto é configurada no plano gratuito). Os turnos vão para o backend Cloud Code do Google (`daily-cloudcode-pa.googleapis.com`), que serve os modelos Gemini, Claude e GPT-OSS que o seletor lista; o nível de raciocínio faz parte do nome do modelo (`-low`, `-medium`, `-high`). Os tokens são renovados sozinhos, também depois de um 401.

Este não é um cliente oficial: o Google pode restringir contas que usam o login do Antigravity fora do Antigravity.

### Provedores declarados

Qualquer outra API que fale um dos protocolos suportados pode ser adicionada com um arquivo JavaScript em `~/{{DIR}}/providers/` (seus) ou em `{{DIR}}/providers/` num projeto. O export default do arquivo declara o provedor:

```javascript
export default {
  id: "groq",
  name: "Groq",
  wire: "chat",
  base: "https://api.groq.com/openai/v1",
  headers: { "x-team": "core" },
};
```

| Campo | Obrigatório | Significado |
|---|---|---|
| `id` | sim | Letras minúsculas, dígitos e hífens. Não pode ser o id de um provedor embutido. |
| `name` | sim | O nome mostrado na lista de login e nos rótulos dos modelos. |
| `wire` | sim | `chat` (`/chat/completions`), `responses` (`/responses`) ou `messages` (`/messages`). |
| `base` | sim | A URL base da API: `https://`, ou `http://` só em `localhost` ou `127.0.0.1`. |
| `authHeader` | não | O cabeçalho que leva a chave. Padrão `authorization`. |
| `authPrefix` | não | Texto colocado antes da chave. Padrão `Bearer `. |
| `headers` | não | Cabeçalhos extras enviados em toda requisição. |
| `models` | não | Ids de modelos a oferecer. Sem ele, os modelos vêm de `GET <base>/models`. |
| `window` | não | A janela de contexto em tokens. Padrão 128.000. |

Campos desconhecidos são recusados. Um provedor declarado conecta com chave de API, como os provedores de chave embutidos, e aparece por último na lista de login. Os modelos dele se chamam `<id>:<modelo>`, por exemplo `groq:llama-4`.

O arquivo também pode exportar uma função `shape`, que recebe o corpo de cada requisição e devolve o corpo a enviar (ou nada, para mantê-lo como está). Ela roda sem acesso a arquivos, processos ou rede e tem dois segundos:

```javascript
export default { id: "local", name: "Local", wire: "chat", base: "http://localhost:8080/v1" };

export function shape(body) {
  return { ...body, temperature: 0.2 };
}
```

> [!IMPORTANT]
> Os provedores declarados de um projeto só carregam quando você confia na pasta do projeto (veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta)) e os aprovou: adicione os ids deles em `enabledProviders` nas suas próprias configurações (`~/{{DIR}}/settings.json` ou o `settings.local.json` do projeto confiável). Até lá, o `{{COMMAND}} doctor` avisa que o provedor foi ignorado ou está esperando.

```json
{
  "enabledProviders": ["groq"]
}
```

O `{{COMMAND}} doctor` informa cada arquivo: quantos carregaram, ou por que um ficou de fora.

## Listar os modelos

```bash
{{COMMAND}} models
```

Imprime todos os modelos que os provedores conectados oferecem, um por linha: o id e depois o rótulo.

```text
anthropic:claude-sonnet-5-5              <rótulo vindo do provedor>
groq:llama-4                             llama-4
```

Os ids de modelo são `<provedor>:<modelo>`. Onde quer que você indique um modelo (`--model`, `/model`, papéis, fallbacks), o {{NAME}} aceita o id completo, a parte depois dos dois-pontos ou o rótulo, sem diferenciar maiúsculas de minúsculas.

Quando um provedor conectado não responde com a lista de modelos, o chat avisa uma vez para aquele provedor (`Could not list <provider>'s models: <reason>`) e segue com os modelos dos outros.

## Escolher um modelo

| Onde | Como |
|---|---|
| Ao abrir o chat | `{{COMMAND}} --model <modelo>` (ou `-m`) |
| Numa resposta impressa | `{{COMMAND}} -p --model <modelo> "..."` |
| No chat | `/model` abre um seletor; `/model <modelo>` troca direto |

O modelo escolhido no chat é lembrado e usado da próxima vez. Sem `--model`, o {{NAME}} usa o último modelo escolhido, ou o primeiro oferecido. No modo print, um nome de modelo que não corresponde a nenhum encerra o comando com `no model named <nome>`.

`/status` mostra o modelo em uso, com o id.

## Papéis de modelo

Alguns trabalhos não precisam do modelo da sessão. Os papéis permitem dar outro modelo a eles:

| Papel | Para que serve |
|---|---|
| `plan` | O modo plan e o agente Plan |
| `explore` | O agente Explore, que busca no código (usa o modelo `small` se não tiver um) |
| `small` | Tarefas de fundo: títulos, escolha de esforço, leitura da web |
| `review` | O juiz que confere o trabalho terminado |
| `advisor` | O conselheiro que o agente consulta |

Um papel sem modelo usa o modelo da sessão.

| Comando | O que faz |
|---|---|
| `/model roles` | Lista cada papel, o modelo dele e o que faz |
| `/model <papel>` | Abre um seletor para o papel; a primeira opção, *Session model*, limpa o papel |
| `/model <papel> <modelo>` | Dá esse modelo ao papel |
| `/model <papel> default` | Limpa o papel (`none` e `off` também funcionam) |

```text
/model plan claude-opus-5-5
/model small claude-haiku-5-5
/model review off
```

Os papéis definidos no chat são lembrados. Os seus arquivos de configuração também podem definir três deles, e o que você define no chat tem prioridade:

| Configuração | Papel |
|---|---|
| `smallModel` | `small` |
| `judgeModel` | `review` |
| `advisorModel` | `advisor` |

Veja em [Configurações](./settings) onde ficam os arquivos de configuração.

## Modelos de fallback

Quando a chamada ao modelo falha porque ele está sobrecarregado, porque a sessão do login caiu ou porque a rede falhou, o {{NAME}} pode seguir com outro modelo. O chat mostra `The model call failed; going on with <modelo>.`

| Comando | O que faz |
|---|---|
| `/model fallbacks` | Mostra a cadeia |
| `/model fallbacks <modelo> [modelo…]` | Define a cadeia, tentada em ordem |
| `/model fallbacks off` | Limpa a cadeia (`default` e `none` também funcionam) |

```text
/model fallbacks claude-sonnet-5-5 claude-haiku-5-5
```

Os fallbacks vêm de um destes três lugares, o primeiro que estiver definido:

1. `--fallback-model <modelo>`: um modelo, para esta execução.
2. A cadeia definida com `/model fallbacks` (lembrada entre chats).
3. `fallbackModel` nas suas configurações: um nome de modelo ou uma lista deles.

```json
{
  "fallbackModel": ["claude-sonnet-5-5", "claude-haiku-5-5"]
}
```

Uma execução tenta cada modelo no máximo uma vez e troca no máximo oito vezes. Outras falhas, como um erro do provedor ou um prompt longo demais, não descem pela cadeia.

> [!TIP]
> Para dividir a carga entre várias contas do mesmo provedor, veja [Contas](./accounts).

## Esforço

O esforço define quanto o modelo raciocina antes de responder.

| Nível | Significado |
|---|---|
| `low` | Respostas mais rápidas, com menos raciocínio |
| `medium` | Equilibrado (o padrão) |
| `high` | Mais raciocínio para trabalhos difíceis |
| `xhigh` | Raciocínio estendido |
| `max` | Todo o raciocínio que o modelo permitir |
| `auto` | Cada pedido escolhe o modelo e o esforço de que precisa |

`/effort` abre um seletor; `/effort <nível>` define direto; `/effort auto` liga a escolha automática. A escolha é lembrada.

Cada provedor traduz o nível para o que a API dele entende. Por exemplo, os modelos Claude atuais recebem raciocínio adaptativo com o esforço, e os modelos que raciocinam com um orçamento de tokens recebem uma fração do orçamento de saída. Um modelo que não aceita o nível escolhido recusa o pedido com `model <modelo> does not support effort <nível>`.

### Esforço automático

Com `/effort auto`, antes de cada pedido uma chamada rápida ao modelo small decide se o pedido precisa do modelo da sessão ou do modelo small, e se precisa de esforço low, medium ou high. O chat mostra a escolha como `Auto: <modelo>, <esforço> effort`. Confirmações sozinhas (`ok`, `okay`, `yes`, `y`, `go`, `go on`, `continue`, `proceed`, `sim`, `continua`, `pode`, `segue`, com ou sem `.` ou `!` no fim) pulam essa chamada e usam o modelo da sessão com esforço low. `/effort <nível>` volta para um nível fixo.

## Janela de contexto

Cada modelo tem uma janela de contexto: quantos tokens de conversa ele consegue ler. O {{NAME}} usa a janela que o catálogo do provedor informa para o modelo, ou o padrão do provedor na [tabela acima](#provedores-suportados).

- `/context` mostra uma barra de quanto a janela está cheia, quanto falta antes de compactar e o que a ocupa (prompt de sistema, ferramentas, ferramentas MCP, agentes, memória, skills, mensagens).
- Quando a conversa se aproxima do limite da janela, ela é compactada; `/compact [o que manter]` faz isso na hora. Veja [Chat](./chat).

## Cache de prompt

O {{NAME}} marca o que não muda entre os turnos para que os provedores possam guardar em cache: no protocolo Messages, as ferramentas, o prompt de sistema e a conversa ficam em cache separadamente; no protocolo Responses, os turnos de uma conversa compartilham uma chave de cache.

Os caches dos provedores expiram depois de uns cinco minutos sem chamadas. Com o recurso `cache-warning` ligado, a linha de status faz a contagem regressiva dos últimos dois minutos (`cache 1:45`) depois de uma chamada que guardou pelo menos 10.000 tokens em cache, e depois que expira mostra `cache cold: next turn rewrites 48k tokens`. Veja [Recursos experimentais](./experimental-features) e [Orçamento e custo](./budget-and-cost).

## Esquecer um provedor

```bash
{{COMMAND}} logout
```

Lista os provedores conectados e pergunta qual esquecer. `/logout` no chat faz o mesmo com um seletor. Esquecer um provedor remove todas as contas dele; para remover uma conta só, veja [Contas](./accounts#remover-uma-conta).
