---
title: Contas
description: Conecte várias contas de um mesmo provedor, alterne entre elas e deixe uma assumir quando outra estiver ocupada.
---

Um provedor pode ter mais de uma conta: duas chaves de API, ou duas assinaturas com login feito uma depois da outra. Uma conta fica em uso por vez, e você escolhe se as outras ficam paradas, entram quando ela está ocupada ou se revezam.

## Adicionar outra conta

Conecte o provedor de novo com `{{COMMAND}} login` ou `/login`, usando outra chave ou fazendo login com outro usuário. A conta nova entra ao lado das que você já tem e passa a ser a conta em uso.

Cada conta recebe o nome do que a identifica:

| Conta | Nome |
|---|---|
| Chave de API | `…` e os quatro últimos caracteres da chave, por exemplo `…bbbb` (chaves com menos de 12 caracteres mostram só `…`) |
| Assinatura | O e-mail da conta; senão o id da conta; senão o nome do provedor e os oito primeiros caracteres do id do usuário; senão o nome do provedor com um número, como `Claude Subscription #2` |

Fazer login de novo com uma conta que você já tem substitui os tokens dela em vez de criar uma segunda cópia.

## Trocar de conta

```text
/accounts
```

Abre um seletor com todas as contas de todos os provedores conectados, no formato `<provedor> · <conta>`, com a conta em uso marcada como `in use`. O cursor começa na primeira conta que não está em uso. Escolha uma e ela passa a ser usada a partir da próxima mensagem: o chat avisa `Using <conta> from the next message.`

Se nenhuma conta estiver conectada, aparece `No account is connected: /login connects one.`

## Remover uma conta

```text
/accounts remove
```

Abre a mesma lista, com o título *Forget which account?*. A conta escolhida é esquecida junto com a chave ou os tokens guardados: `Forgot <conta>.` Se era a conta em uso, a primeira conta restante do provedor assume o lugar dela.

Para esquecer todas as contas de um provedor de uma vez, use `/logout` ou `{{COMMAND}} logout`:

```bash
{{COMMAND}} logout
```

```text
  1. Anthropic API
  2. Grok Subscription
Forget which? 1
Forgot Anthropic API.
```

## Dividir o trabalho entre contas

`/accounts balance` define como as contas do provedor do modelo atual dividem o trabalho. Sem valor, ele lista as opções.

```text
/accounts balance failover
```

| Balance | O que acontece |
|---|---|
| `off` | Só a conta em uso. É o padrão. |
| `failover` | A conta em uso, e depois as outras quando ela está ocupada ou deslogada |
| `round-robin` | Cada execução começa na próxima conta, e as outras assumem quando uma está ocupada |

O chat confirma com `Accounts balance: <balance> (<o que faz>).` O balance é guardado por provedor, então cada provedor pode ter o seu.

### Quando outra conta assume

Com `failover` ou `round-robin`, uma conta passa o turno para a próxima quando o provedor responde que está sobrecarregado ou que a conta está deslogada. O chat mostra:

```text
Account …aaaa is busy or signed out; going on with …bbbb.
```

Uma conta só assume se a que falhou ainda não tinha começado a responder; um turno que já transmitiu texto não é repetido em outra conta. Erros de rede e outros erros do provedor não passam o turno. Quando todas as contas falham, a execução falha com o último erro, e os [modelos de fallback](./models-and-providers#modelos-de-fallback) são tentados em seguida.

> [!TIP]
> As contas distribuem o trabalho dentro de um provedor; os modelos de fallback trocam para outro modelo, possivelmente de outro provedor. Os dois funcionam juntos: primeiro as contas, depois os fallbacks.

## Comandos

| Comando | O que faz |
|---|---|
| `/accounts` | Escolhe a conta em uso |
| `/accounts remove` | Esquece uma conta |
| `/accounts balance` | Lista as opções de balance |
| `/accounts balance off\|failover\|round-robin` | Define como as contas do provedor atual dividem o trabalho |
| `/login` | Conecta um provedor, ou adiciona uma conta a ele |
| `/logout` | Esquece um provedor com todas as contas dele |
| `{{COMMAND}} login` | Conecta pelo terminal, fora do chat |
| `{{COMMAND}} logout` | Esquece um provedor pelo terminal |

Veja também [Modelos e provedores](./models-and-providers) e [Comandos de barra](./slash-commands).
