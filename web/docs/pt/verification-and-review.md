---
title: Verificação e revisão
description: Peça a um juiz para conferir cada execução contra os seus critérios, deixe um revisor ler as mudanças, rode os testes afetados e evite que o agente pare cedo ou ande em círculos.
---

O {{NAME}} pode conferir o trabalho do agente antes de uma execução terminar. Algumas checagens você pede (critérios de verificação); outras rodam sozinhas quando a flag delas está ligada (revisão automática, testes afetados, follow-through, escalonamento).

## Verificar uma execução contra critérios

Diga ao {{NAME}} o que significa "pronto", e um agente **juiz** confere cada execução contra isso antes de ela terminar:

```text
/verification `cargo test --workspace` passa e o README documenta a nova flag --verbose
Each run will be judged against these before it finishes (a judge agent runs the checks).
```

Enquanto há critérios definidos, a linha de status mostra `verify (yours)`.

| Comando | Efeito |
|---|---|
| `/verification` | Mostra os critérios em vigor e de onde vêm |
| `/verification <critérios>` | Define os seus critérios para esta conversa |
| `/verification off` | Apaga os seus critérios |

Também aparece como **Verification** no `/config`.

### Como o juiz funciona

No fim de uma execução em que o agente usou pelo menos uma ferramenta, o juiz recebe os critérios, o seu pedido, as chamadas de ferramenta do agente (as 60 últimas) e a mensagem final. Ele trabalha com **Read**, **Glob**, **Grep**, **Bash** e **Skill**, por no máximo 50 turnos, e responde com um veredito: aprovado ou reprovado, uma nota de 0 a 1 e uma checagem por critério com a evidência. O chat mostra o veredito:

```text
✓ verification passed (score 0.90)
```

```text
✗ verification failed, attempt 1/3 (score 0.50) · the agent tries again
  ✗ tests: 1 failed
```

Quando o trabalho é reprovado, as checagens que falharam voltam para o agente, que as corrige e termina de novo. O juiz roda no máximo **3** vezes por execução. Se o juiz não consegue dar um veredito (ele para, ou a resposta não é válida), a execução fica marcada como reprovada com o motivo e o agente não é mandado de volta.

Execuções no modo plano não são julgadas. Subagentes não são julgados separadamente; a execução principal é.

### O que o juiz pode executar

O juiz nunca pergunta nada a você: uma chamada de ferramenta que precisaria da sua aprovação é recusada. Alguns comandos já vêm aprovados para ele:

- todo comando escrito nos critérios entre crases, ou numa linha própria dentro de um bloco de código cercado (um `$ ` no começo é descartado). `` `cargo test` passa `` permite que ele rode `cargo test`;
- as regras listadas em `verification.allow` nos seus próprios arquivos de configurações (`~/{{DIR}}/settings.json`, `settings.local.json`, `managed-settings.json`):

```json
{
  "verification": {
    "allow": ["Bash(make check:*)"]
  }
}
```

Um comando composto só é permitido quando todas as partes dele são.

### Escolher o modelo do juiz

O juiz usa o modelo do papel **review**: `judgeModel` nas configurações, ou `/model review <modelo>` para a sessão. Sem nenhum, usa o modelo da sessão. Veja [Modelos e provedores](./models-and-providers).

### Critérios de um plano

Quando você aprova um plano que tem uma seção **Verification** (um título `## Verification`, ou um item como `- **Verification:** …`), essa seção vira os critérios assim que você sai do modo plano, e a linha de status mostra `verify (plan)`. Os critérios que você define com `/verification` têm precedência; apagar os seus deixa os do plano em vigor.

### Verificação no modo headless

O `--verification` passa critérios para uma execução [headless](./headless). Ele exige `--print`. Os vereditos são impressos como notas, e o comando sai com status 1 quando um veredito reprova:

```bash
{{COMMAND}} -p --verification '`npm test` passa' "corrija o teste do parser de datas que está falhando"
```

## Critérios do projeto

> [!NOTE]
> Controlado pela flag `project-criteria`. Padrão: ligada.

Um projeto pode versionar um `{{DIR}}/verification.md` com os critérios que toda execução nele deve cumprir:

```markdown
- `cargo test --workspace` passa
- `cargo clippy --workspace --all-targets -- -D warnings` não aponta nada
- Toda função pública nova tem um teste
```

O {{NAME}} usa o arquivo mais próximo, da pasta em que você trabalha subindo pelas pastas acima (o seu `~/{{DIR}}/verification.md` não é lido). O arquivo não pode estar vazio e deve ter 64 KB ou menos. Esses critérios valem quando você não definiu nenhum e nenhum plano definiu.

> [!IMPORTANT]
> Os comandos citados no `verification.md` de um projeto **não** vêm aprovados para o juiz: um arquivo versionado não pode conceder permissões a si mesmo. Só valem as suas regras de `verification.allow`; acrescente lá os comandos em que você confia.

## Revisão de código automática

> [!NOTE]
> Controlada pela flag `auto-review`. Padrão: ligada.

Quando uma execução mudou arquivos do projeto com **Edit**, **Write** ou **NotebookEdit**, um revisor independente lê as mudanças antes de a execução terminar. Ele:

- lê cada arquivo mudado em volta das linhas alteradas e procura defeitos reais: lógica errada, um caso que a mudança quebra, um caminho de erro que falta, um teste que não testa mais o que o nome diz;
- lê os arquivos de `docs/` e os arquivos de instruções que descrevem o que mudou, e aponta qualquer afirmação que a mudança tornou falsa ou regra que ela quebra;
- não aponta estilo nem coisas que a mudança não tocou.

O revisor só pode ler (**Read**, **Glob**, **Grep**), usa o modelo da sessão e tem no máximo 30 turnos. Cada achado é **high** (um defeito, ou uma regra documentada quebrada) ou **low** (documentação desatualizada, um risco menor). O chat lista os achados:

```text
Review of the changes:
[fix] src/parse.rs:42: the new branch returns before closing the file
[note] docs/cli.md: still says --verbose is a no-op
```

Os achados high voltam para o agente uma vez, como uma revisão do sistema que você não viu; o agente os corrige ou explica por que um deles não é problema, e a resposta final continua respondendo o que você pediu. Os achados low são só mostrados. A revisão roda uma vez por execução, só para o agente principal. Se o revisor falhar, a execução termina sem achados.

## Testes afetados

> [!EXPERIMENTAL]
> Controlados pela flag `experimental-affected-tests`. Padrão: ligada.

Depois que o agente edita ou escreve um arquivo do projeto, o {{NAME}} inicia em segundo plano os testes que o cobrem e avisa o agente, que recebe o resultado quando eles terminam:

```text
<tests>Started cargo test -q -p parser in crates/parser as bash_3 for this change; its result will reach you when it ends.</tests>
```

| Arquivo mudado | Testes iniciados |
|---|---|
| `.rs` | `cargo test -q -p <crate>` na pasta mais próxima com um `Cargo.toml` que nomeia um pacote, com `CARGO_TARGET_DIR` apontando para `target/agent-tests` no projeto |
| `.go` | `go test .` na pasta do arquivo |
| `.py` | `python -m pytest -q '<arquivo de teste>'`, onde o arquivo de teste é o próprio arquivo quando ele se chama `test_*.py`, senão `test_<nome>.py` ao lado dele, numa pasta `tests/` ao lado dele ou na `tests/` do projeto |
| `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs` | `npm test --silent -- '<arquivo de teste>'` quando o projeto tem um `package.json`, onde o arquivo de teste é o próprio arquivo quando termina em `.test` ou `.spec`, senão `<nome>.test.<ext>` ou `<nome>.spec.<ext>` ao lado dele |

Nada é iniciado quando nenhum teste é encontrado. Quando os mesmos testes já estão rodando por uma edição anterior, essa execução é parada e substituída. Os testes rodam como os comandos em segundo plano do próprio agente, dentro do sandbox quando ele está ligado.

## Follow-through

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-follow-through`. Padrão: ligada.

Às vezes uma resposta encerra o turno prometendo mais ("Agora vou rodar os testes."). Depois de uma execução em que o agente usou uma ferramenta, o {{NAME}} procura uma promessa nos últimos 300 caracteres da resposta final: `I'll`, `I will`, `let me`, `I'm going to`, `I am going to`, `next I`, ou os equivalentes em português e espanhol `vou`, `deixa eu`, `deixe-me`, `em seguida`, `voy a`. Uma resposta que termina com pergunta é deixada em paz.

Quando encontra uma, um modelo pequeno (o `smallModel`, ou o modelo da sessão) lê o seu pedido e a resposta e decide se o trabalho está feito, esperando algo, ou se deve continuar. Se deve continuar, o agente é mandado de volta:

```text
Your answer said there was more to do, but the turn was ending. Run the tests you mentioned.
Carry it out now, or say plainly why it cannot be done.
```

Isso acontece no máximo duas vezes por execução, e só para o agente principal.

## Escalonamento quando o agente empaca

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-escalation`. Padrão: ligada.

O agente conta como empacado quando a mesma chamada de ferramenta (mesma ferramenta, mesma entrada) falha 3 vezes, ou quando a proteção contra chamadas repetidas o parou duas vezes. A ajuda vem então, no máximo duas vezes por execução:

- se há um modelo **Advisor** definido (`advisorModel`) e a ferramenta Advisor está disponível, o agente é orientado a chamá-la contando o que tentou e o que falhou, e a seguir o conselho;
- senão, se o `escalationModel` está definido, os próximos turnos rodam nesse modelo e o agente é orientado a dar um passo atrás e mudar a abordagem. O chat mostra a troca como um fallback.

Sem nenhum dos dois, nada acontece. O `escalationModel` só é lido dos seus próprios arquivos de configurações (`~/{{DIR}}/settings.json`, `settings.local.json`, `managed-settings.json`), nunca de um arquivo versionado do projeto:

```json
{
  "advisorModel": "opus",
  "escalationModel": "opus"
}
```

## Relacionados

- [Feature flags](./experimental-features#revisao): as flags desta página.
- [Modo headless](./headless): `--verification` em scripts e CI.
- [Aprendizado](./learning): uma verificação reprovada conta como correção para a lição oferecida depois de uma execução.
