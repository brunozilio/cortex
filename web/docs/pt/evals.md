---
title: Evals
description: Escreva casos de teste repetíveis para o agente em {{DIR}}/evals e pontue-os com {{COMMAND}} eval.
---

Um eval é uma tarefa que você dá ao agente várias vezes, com um jeito de saber se ele fez certo. Os evals mostram se uma mudança nas suas instruções, skills, configurações ou modelo ainda resolve o trabalho, e quanto custa.

## Escrever o primeiro eval

Cada eval é uma pasta dentro de `{{DIR}}/evals/` no seu projeto, com um `case.json` e, se a tarefa precisar de arquivos, uma pasta `fixture/`:

```text
{{DIR}}/evals/
└── rename/
    ├── case.json
    └── fixture/
        └── src/
            └── lib.rs
```

O `case.json` diz o que pedir e como conferir o trabalho:

```json
{
  "prompt": "Rename greet to welcome",
  "mode": "bypass",
  "checks": [
    { "file_contains": { "path": "src/lib.rs", "text": "fn welcome" } },
    { "file_lacks": { "path": "src/lib.rs", "text": "fn greet" } },
    { "answer_contains": "welcome" },
    { "command": "cargo check -q" }
  ],
  "rubric": "Only the function and its callers changed."
}
```

Rode:

```bash
{{COMMAND}} eval
```

```text
Running rename…
rename  pass    5/5  $0.14
1 of 1 cases passed, $0.14
```

## Como um caso roda

1. A pasta `fixture/` é copiada para uma pasta de rascunho nova e vazia. Sem `fixture/`, a pasta de rascunho fica vazia. A fixture em si nunca é alterada.
2. O agente roda ali uma vez, no modo print, com o prompt e o `mode` do caso: `{{COMMAND}} -p --output-format json -C <rascunho> [--mode <modo>] [--model <modelo>] [--trust] -- <prompt>`, com `--trust` quando a pasta do seu projeto é confiável.
3. Cada verificação é pontuada com base na resposta do agente e nos arquivos que ficaram na pasta de rascunho.
4. Se o caso tiver uma `rubric`, uma execução juíza avalia o trabalho segundo ela.

A pasta de rascunho é apagada quando o caso termina.

> [!IMPORTANT]
> O agente roda na pasta de rascunho, não no seu projeto, então as configurações em `{{DIR}}` e o `{{INSTRUCTIONS}}` do projeto não são encontrados ali. Coloque o que o caso precisa (instruções, `{{DIR}}/settings.json`, skills) dentro de `fixture/`. As suas configurações em `~/{{DIR}}` continuam valendo. A pasta de rascunho é tratada como confiável quando a pasta do seu projeto é confiável; caso contrário, hooks, scripts, servidores MCP e o `settings.local.json` da fixture são ignorados (veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta)).

> [!WARNING]
> O modo print nega todo pedido de permissão. Sem `mode`, a execução usa `edits`: o agente pode alterar arquivos, mas qualquer comando que peça permissão é recusado. Use `"mode": "bypass"` quando a tarefa precisar rodar comandos, já que a pasta de rascunho é uma cópia descartável.

## Referência do case.json

| Campo | Tipo | Obrigatório | Significado |
|---|---|---|---|
| `prompt` | string | sim | O que pedir ao agente. Não pode ser vazio. |
| `mode` | string | não | O modo de permissão da execução: `ask`, `edits`, `plan` ou `bypass` |
| `checks` | lista | `checks` ou `rubric` | Verificações pontuadas depois da execução |
| `rubric` | string | `checks` ou `rubric` | Critérios pelos quais um juiz avalia o trabalho |

Um caso sem verificações nem rubrica é recusado com `case.json needs checks or a rubric`. Outros campos são ignorados.

### Verificações

Cada verificação é um objeto com uma chave:

| Verificação | Passa quando |
|---|---|
| `{"file_contains": {"path": "...", "text": "..."}}` | O arquivo existe e contém o texto |
| `{"file_lacks": {"path": "...", "text": "..."}}` | O arquivo não contém o texto, ou não existe |
| `{"answer_contains": "..."}` | A resposta do agente contém o texto, sem diferenciar maiúsculas de minúsculas |
| `{"command": "..."}` | O comando, rodado com `sh -c` na pasta de rascunho, sai com 0 |

Os caminhos são relativos à pasta de rascunho. A saída de uma verificação `command` é descartada e ela não recebe entrada, então só conta o código de saída.

### Rubrica

Quando um caso tem `rubric`, uma segunda execução do {{NAME}} faz o papel de juiz, no modo plan e na mesma pasta de rascunho, então pode ler os arquivos mas não alterá-los. O juiz recebe a rubrica, a tarefa, a resposta do agente e a lista de arquivos alterados (`A` adicionado, `M` modificado, `D` apagado, em relação à fixture), e precisa responder com um objeto JSON:

```json
{ "passed": true, "why": "Only lib.rs changed." }
```

A rubrica conta como mais uma verificação. Se o juiz reprovar, aparece `rubric: <motivo>`; se a resposta dele não tiver esse objeto, `rubric: the judge answered out of form`; se a própria execução do juiz falhou, `rubric: the judge failed: <erro>`.

## Pontuação

Um caso ganha um ponto por verificação que passa, mais um pela rubrica. Ele passa quando ganha todos os pontos. O custo da execução do agente e o da execução do juiz são somados.

Quando algo dá errado antes das verificações, o caso fica com 0/1 e uma nota:

| Nota | Causa |
|---|---|
| `case.json: ...` | O arquivo não é JSON válido ou não segue o formato |
| `case.json needs a prompt` | `prompt` está vazio |
| `no scratch folder: ...` | Não foi possível criar a pasta de rascunho |
| `could not copy the fixture: ...` | Não foi possível copiar a fixture |
| `the agent failed: ...` | A execução do agente falhou, por exemplo porque nenhum modelo está conectado |

Uma verificação que falhou aparece como `failed: <verificação>`, por exemplo `failed: src/lib.rs contains "fn welcome"` ou ``failed: `cargo check -q` succeeds``.

## Rodar evals

| Comando | O que faz |
|---|---|
| `{{COMMAND}} eval` | Roda todos os casos, em ordem de nome |
| `{{COMMAND}} eval rename fix-bug` | Roda só estes casos |
| `{{COMMAND}} eval --json` | Imprime as pontuações em JSON |
| `{{COMMAND}} --model <modelo> eval` | Roda todos os casos, e o juiz, com esse modelo |
| `{{COMMAND}} -C <caminho> eval` | Roda os evals de outro projeto |

Uma pasta só é um caso se tiver um `case.json`. Durante a execução, cada caso é anunciado na saída de erro como `Running <nome>…`.

Se `{{DIR}}/evals` não existir, o comando para com `No evals in <pasta>: add <name>/case.json and an optional <name>/fixture/`; se nenhum caso corresponder aos nomes passados, com `No eval case matched in <pasta>`.

### Saída em tabela

```text
rename   pass    4/4  $0.12
fix-bug  fail    1/3  $0.30
           failed: a.txt contains "fixed"
           rubric: touched the tests
1 of 2 cases passed, $0.42
```

Cada caso mostra o nome, `pass` ou `fail`, os pontos ganhos e o custo, seguido das notas. A última linha conta os casos que passaram e o custo total.

### Saída em JSON

`--json` imprime uma lista com um objeto por caso:

```json
[
  {
    "name": "rename",
    "passed": 4,
    "total": 4,
    "cost_usd": 0.12,
    "notes": []
  }
]
```

| Campo | Significado |
|---|---|
| `name` | O nome da pasta do caso |
| `passed` | Pontos ganhos |
| `total` | Pontos possíveis: um por verificação, mais um pela rubrica |
| `cost_usd` | O custo do agente e do juiz somados |
| `notes` | Por que pontos foram perdidos |

### Código de saída

`{{COMMAND}} eval` sai com 0 quando todos os casos passaram e com 1 caso contrário, então pode barrar um job de CI:

```bash
{{COMMAND}} eval --json > eval-results.json
```

> [!TIP]
> Rode os mesmos evals com modelos diferentes para compará-los: `{{COMMAND}} --model <a> eval` e `{{COMMAND}} --model <b> eval`. Veja [Modelos e provedores](./models-and-providers).

Veja também o [Modo headless](./headless), que cada caso usa por baixo.
