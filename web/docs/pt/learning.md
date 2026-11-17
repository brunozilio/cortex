---
title: Aprendizado
description: Transforme as suas correções em lições duradouras com /learn, guarde decisões junto com o que foi descartado e confira um branch contra o que o projeto lembra com /check.
---

Quando você corrige o agente, a correção costuma importar além da tarefa atual. O {{NAME}} pode perceber isso, propor uma lição para guardar em todas as sessões futuras do projeto e, depois, conferir mudanças novas contra o que foi aprendido.

## Lições a partir das suas correções

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-learning`. Padrão: ligada.

Durante uma conversa, o {{NAME}} anota cada vez que você:

- recusa uma chamada de ferramenta e diz o que fazer no lugar;
- envia uma mensagem enquanto o agente trabalha, redirecionando-o;
- vê o agente ser parado por repetir a mesma chamada;
- recebe um veredito reprovado do [juiz de verificação](./verification-and-review#verificar-uma-execucao-contra-criterios).

Depois de uma execução, quando já se acumularam pelo menos dois sinais desses, uma chamada à parte lê a conversa e propõe **uma** lição, ou nenhuma se nada duradouro foi aprendido:

```text
Lesson from this run: remember "This project uses pnpm, never npm"
Why: you refused npm install and said to use pnpm, twice.
/learn keep saves it, /learn skip drops it.
```

Os sinais são então zerados, e a próxima lição precisa de novas correções.

| Lição | Proposta quando | O `/learn keep` faz |
|---|---|---|
| Memória | Um fato ou jeito de trabalhar para lembrar | Salva como [memória](./memory) procedural |
| Skill | Um procedimento repetível que vale escrever | Pede ao agente para criar ou atualizar a skill com a skill `skill-creator` |
| Regra | Você permite ou recusa o mesmo comando repetidas vezes | Salva a regra `allow` ou `deny` no `{{DIR}}/settings.local.json` do projeto |
| Instrução | Você declarou uma regra para todo o trabalho no projeto | Acrescenta uma linha sob `## Learned` no arquivo de instruções do projeto |

O `/learn skip` descarta a lição. Sem lição esperando, o `/learn` responde `No lesson is waiting: one is offered after a run where you corrected the agent.`

A proposta precisa se apoiar no que você disse ou fez; o modelo é orientado a nunca inventar uma preferência e a nunca pôr chaves, tokens ou senhas numa lição.

### Linhas de instrução

Uma lição do tipo instrução é acrescentada ao `{{INSTRUCTIONS}}` na raiz do projeto (ou ao `{{DIR}}/{{INSTRUCTIONS}}` quando só ele existe), sob um título `## Learned` que é criado se faltar:

```markdown
## Learned
- Run cargo nextest, never cargo test
```

Uma linha que já está lá não é acrescentada de novo. Uma linha que parece conter um segredo é recusada: `The instruction holds what looks like a secret, so it was not written.`

> [!NOTE]
> Salvar uma regra ou uma instrução passa pela funcionalidade `suggested-rules`. Com essa flag desligada, o `/learn keep` não consegue salvar esses dois tipos.

## Decisões estruturadas

> [!EXPERIMENTAL]
> Controladas pela flag `experimental-structured-learnings`. Padrão: ligada.

Uma decisão vale mais junto com o que foi tentado e abandonado: é isso que poupa a próxima sessão de tentar de novo. Com esta flag ligada, o agente pode salvar uma decisão em três partes em vez de uma frase:

```json
{
  "remember": [{
    "decided": "parse dates with chrono, not time",
    "ruled_out": ["time: no strftime-compatible formats"],
    "watch_out": ["chrono's Local is not available on wasm"]
  }]
}
```

Ela é guardada como uma memória que diz:

```text
Decided: parse dates with chrono, not time. Ruled out: time: no strftime-compatible formats. Watch out: chrono's Local is not available on wasm.
```

Quando uma memória assim é compartilhada com o [`/memory share`](./memory#memoria-da-equipe), o arquivo da equipe é escrito com as seções `## Decided`, `## Tried and ruled out` e `## Watch out`, e com o seu `user.name` do git e a data de hoje no front matter. Arquivos da equipe escritos assim são lidos de volta como decisões.

## Conferir um branch contra a memória

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-learning-check`. Padrão: ligada.

Antes de fazer commit ou abrir um pull request, o `/check` pede ao agente para comparar as suas mudanças com o que a memória do projeto diz sobre o código que elas tocam:

```text
/check
```

O {{NAME}} encontra as mudanças desde que o branch saiu do branch padrão (`origin/HEAD`, senão `main`, senão `master`; as mudanças não commitadas em relação ao `HEAD` quando você está nele), incluindo arquivos novos não rastreados. Ele escolhe até 12 memórias, suas e da equipe: primeiro as ligadas a um arquivo mudado, depois as cujas palavras mais combinam com os arquivos e linhas mudados. Então envia ao agente um prompt pedindo que leia o `git diff`, não mude nada e aponte cada conflito assim:

```text
✗ src/retry.rs:18
  sets the retry count to 10
  [4] Keep retries at 4: the upstream rate limit resets every 5 attempts
· 1 conflicts · fix them, or say why in the commit message
```

ou `✓ No conflicts with <n> memories`. Uma mudança que concorda com uma memória, ou sobre a qual nenhuma memória fala, não é conflito.

O `/check` precisa de um repositório git (`/check needs a git repository.`). Sem mudança ou sem memória relevante, ele responde `Nothing to check: no change, or no memory is about the changed files.` Com a flag desligada, responde `/check is turned off (experimental-learning-check); /features turns it on.`

## Relacionados

- [Memória](./memory): onde as lições e decisões ficam guardadas.
- [Feature flags](./experimental-features#aprendizado-e-memoria): as flags desta página.
- [Arquivos de instruções](./instructions-files): o arquivo que recebe as lições do tipo instrução.
