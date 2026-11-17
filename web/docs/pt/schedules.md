---
title: Agendamentos
description: Repita uma tarefa num intervalo com /loop, deixe o agente agendar prompts com expressões cron ou despertares únicos, e liste ou apague o que está agendado.
---

Um prompt agendado é enviado à sua conversa mais tarde, como se você o tivesse digitado: a cada dez minutos, todo dia útil às 9:00 ou uma vez daqui a meia hora. Use isso para ficar de olho num build, num deploy ou num pull request sem conferir na mão.

## Repetir uma tarefa com /loop

```text
/loop 5m confira se o deploy terminou e me avise se falhou
```

O `/loop` roda a tarefa agora e depois pede ao agente para agendá-la de novo no intervalo que você deu:

| Intervalo | Agenda |
|---|---|
| `<n>s` | A cada `⌈n/60⌉` minutos (segundos arredondam para cima em minutos inteiros, de 1 a 59) |
| `<n>m` | A cada n minutos; 60 ou mais vira horas inteiras, arredondadas para baixo (`90m` é a cada hora) |
| `<n>h` | A cada n horas; 24 ou mais vira dias inteiros, arredondados para baixo |
| `<n>d` | A cada n dias, até 31; um valor maior não é lido como intervalo, e o texto inteiro vira a tarefa |

O agente cria o agendamento com **CronCreate** e diz como pará-lo.

Sem intervalo, o agente define o próprio ritmo:

```text
/loop acompanhe o PR até o CI ficar verde
```

Ele faz a tarefa e, ao terminar, a menos que a tarefa esteja concluída de vez, agenda o próprio despertar (entre 60 e 3600 segundos) para voltar a ela.

O `/loop` sem tarefa explica o uso: `/loop [5m|2h|1d] <task>`.

## Pedir ao agente para agendar algo

Você também pode simplesmente pedir: "todo dia útil às 9, resuma os commits de ontem", "me lembre daqui a 30 minutos de conferir a migração". O agente tem quatro ferramentas para isso:

| Ferramenta | Faz |
|---|---|
| **CronCreate** | Agenda um prompt numa expressão cron, recorrente (o padrão) ou só na próxima ocorrência |
| **ScheduleWakeup** | Envia um prompt uma vez, depois de um intervalo de 60 a 3600 segundos |
| **CronList** | Lista os prompts agendados para esta sessão |
| **CronDelete** | Remove um pelo id |

O agente é orientado de que cada disparo é um turno completo do modelo: para esperar uma condição fixa (uma issue chegar a um status, um arquivo mudar), ele deve usar o **Watch**, que confere sem gastar um turno; para acompanhar a saída de um comando, o **Monitor**.

### Expressões cron

Uma expressão cron tem cinco campos, no seu horário **local**:

```text
┌ minuto (0-59)
│ ┌ hora (0-23)
│ │ ┌ dia do mês (1-31)
│ │ │ ┌ mês (1-12)
│ │ │ │ ┌ dia da semana (0-7, domingo é 0 ou 7)
│ │ │ │ │
*/10 * * * *
```

Cada campo aceita `*`, um número, um intervalo (`1-5`), uma lista (`15,45`) e um passo (`*/5`, `8-18/2`). Como no cron padrão, quando o dia do mês e o dia da semana estão ambos restritos, um dia que bate com qualquer um dos dois dispara.

| Expressão | Dispara |
|---|---|
| `*/10 * * * *` | A cada 10 minutos |
| `0 9 * * 1-5` | Às 9:00 nos dias úteis |
| `15,45 8-10 * * *` | Aos :15 e :45 das 8, 9 e 10 horas |
| `0 0 1 * *` | À meia-noite do dia 1 de cada mês |

## Listar e apagar prompts agendados

```text
/schedules
Scheduled (/schedules delete <id> removes one):
  cron_3    every `*/10 * * * *`, in 4m: confira se o deploy terminou
  cron_4    once, in 27m: confira a migração
```

Cada linha mostra o id, como ele se repete, os minutos até o próximo disparo e o prompt. O `/schedules delete <id>` remove um.

## Quando um prompt agendado dispara

O {{NAME}} confere a cada segundo se há prompts vencidos. Um prompt vencido só é enviado enquanto o chat está ocioso: nenhuma execução em andamento e nenhuma pergunta esperando você. Ele aparece no chat como `Scheduled (<id>): <prompt>` e inicia uma execução como uma mensagem que você digitou.

> [!IMPORTANT]
> Os agendamentos só existem enquanto o processo do {{NAME}} que os criou estiver aberto. Eles não são salvos: sair do chat os descarta, e eles só disparam com ele aberto.

Limites:

- no máximo **50** prompts agendados ao mesmo tempo;
- um prompt recorrente para depois de **7 dias**;
- uma expressão cron precisa bater com algum momento dentro do próximo ano.

## Desligar os agendamentos

Defina `"schedules": false` sob `features` nas suas configurações (veja [Feature flags](./experimental-features#desligar-uma-funcionalidade-interna-inteira)). O `/schedules` passa a falhar e os prompts vencidos deixam de ser entregues.

## Relacionados

- [Comandos de barra](./slash-commands): `/loop` e `/schedules` junto com os outros comandos.
- [Branches e git](./branches-and-git#acompanhar-um-pull-request-ate-o-merge): acompanhar um pull request com um monitor em vez de um agendamento.
