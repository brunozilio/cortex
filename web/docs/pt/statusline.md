---
title: Linha de status
description: Troque a linha abaixo do prompt pela saída de um comando seu, que recebe o estado da sessão em JSON.
---

A linha de status é a linha abaixo do prompt. Por padrão ela mostra o título da conversa, o modelo, o modo de permissão e o esforço. Com `/statusline` você pode desenhá-la com um comando seu: o {{NAME}} roda o comando, passa o estado da sessão em JSON pelo stdin e mostra a primeira linha que ele imprimir.

## Começo rápido

No chat:

```text
/statusline jq -r '"\(.model.display_name) · \(.mode) · $\(.cost.total_cost_usd)"'
```

A linha abaixo do prompt passa a mostrar algo como:

```text
Opus · edits · $0.42
```

Para desligar:

```text
/statusline off
```

## O comando `/statusline`

| Comando | O que faz |
|---|---|
| `/statusline` | Mostra o comando atual, ou explica como definir um |
| `/statusline <comando>` | Desenha a linha de status com esse comando |
| `/statusline off` | Volta para a linha de status de sempre |

O comando fica guardado junto com as preferências do próprio chat (como o modelo e as teclas do vim, na pasta de dados do {{NAME}}; veja [Arquivos e diretórios](./files-and-directories)), então ele vale entre chats e pastas. `/config` o mostra em *Status line*.

## Como o comando roda

- Roda com `sh -c` na pasta do projeto.
- Recebe o estado abaixo como um objeto JSON pelo stdin.
- A primeira linha do stdout aparece, esmaecida. As demais linhas são ignoradas, assim como o stderr.
- Roda de novo no máximo a cada 3 segundos enquanto o chat está aberto.
- Até ele imprimir algo, a linha de status de sempre continua.
- Se não puder ser iniciado, a linha mostra `status line: could not run <comando>`.
- Se rodar por mais de 5 segundos, ele é parado e a linha mostra `status line: the command took too long (over 5s) and was stopped`.
- No máximo 4 KB da saída dele são lidos.

## JSON de entrada

```json
{
  "model": { "id": "id-do-modelo-no-provedor", "display_name": "Opus" },
  "workspace": { "current_dir": "/home/ana/code/app" },
  "session_id": "4f1c…",
  "mode": "edits",
  "output_style": "default",
  "cost": { "total_cost_usd": 0.42 },
  "context": { "used": 48210, "window": 200000 },
  "busy": false
}
```

| Campo | Valor |
|---|---|
| `model.id` | O id do modelo |
| `model.display_name` | O nome do modelo como o chat o mostra |
| `workspace.current_dir` | A pasta do projeto |
| `session_id` | O id da conversa, ou `null` antes de ela começar |
| `mode` | `ask`, `edits`, `plan` ou `bypass` |
| `output_style` | O nome do estilo de saída, `default` quando nenhum está definido |
| `cost.total_cost_usd` | Quanto a conversa custou até agora, em dólares americanos |
| `context.used` | Tokens da janela de contexto em uso |
| `context.window` | A janela de contexto do modelo, quando conhecida |
| `busy` | `true` enquanto o agente está trabalhando |

## Exemplos

### Um script com informações do git

`~/bin/{{COMMAND}}-status.sh`:

```bash
#!/bin/sh
input=$(cat)
model=$(printf '%s' "$input" | jq -r '.model.display_name')
dir=$(printf '%s' "$input" | jq -r '.workspace.current_dir')
branch=$(git -C "$dir" branch --show-current 2>/dev/null)
used=$(printf '%s' "$input" | jq -r '.context.used // 0')
window=$(printf '%s' "$input" | jq -r '.context.window // 0')
percent=0
if [ "$window" -gt 0 ]; then percent=$((used * 100 / window)); fi
echo "$model · ${branch:-sem git} · contexto ${percent}%"
```

```text
/statusline ~/bin/{{COMMAND}}-status.sh
```

### Mostrar quando o agente está trabalhando

```text
/statusline jq -r 'if .busy then "trabalhando… \(.mode)" else "pronto · \(.mode)" end'
```

> [!TIP]
> Mantenha o comando rápido: ele roda a cada poucos segundos. Guarde informações lentas, como uma chamada de rede, num arquivo e atualize-o em segundo plano.

## Veja também

- [Chat](./chat) para o resto da tela.
- [Comandos de barra](./slash-commands).
- [Orçamento e custo](./budget-and-cost) para o que `cost` conta.
