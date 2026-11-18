---
title: Controle remoto
description: Acompanhe e conduza as sessões desta máquina pelo celular, através de um relay seu, com criptografia ponta a ponta e a página instalada na tela inicial.
---

O controle remoto coloca o [console](./trace-console) no seu celular: as sessões, as execuções e um terminal ligado a uma sessão. A página se instala na tela inicial como um app. O celular nunca chega direto ao seu computador: os dois se conectam a um relay que é seu, e tudo entre eles tem criptografia ponta a ponta, então o relay repassa bytes que não consegue ler.

## Configurar

Primeiro você precisa de um relay. O jeito mais rápido é um Worker da Cloudflare na sua própria conta, publicado pelo {{NAME}}:

```bash
{{COMMAND}} remote up
```

Na primeira vez, ele abre o navegador para entrar na Cloudflare: escolha a conta e autorize. Ele publica o relay em `https://{{COMMAND}}-relay.<seu subdomínio>.workers.dev`, faz dele o relay desta máquina e imprime um QR code que pareia o primeiro celular. Cabe no plano gratuito da Cloudflare, e nada fica aberto no seu computador além do chat.

Depois, num chat:

```text
/remote pair
```

Escaneie o QR code (ou abra o link) no celular e deixe a página conectar. Um link de pareamento vale para um celular, uma vez, em até dez minutos.

No iPhone, a página adicionada à tela de início guarda seus dados à parte do Safari, então o pareamento feito no Safari não chega até ela. Adicione a página à tela de início, abra-a por ali e cole o link de pareamento no campo que ela mostra enquanto não está pareada.

A página abre nas **Conversas**: a lista de sessões, cada uma como um chat. Toque numa para ler e responder, aprove ou negue o que o agente pede pelos botões, e use **+** para uma conversa nova na pasta onde o chat da máquina está aberto. **Trace**, no topo, leva ao trace e ao terminal.

A máquina fica alcançável enquanto um chat estiver aberto com o controle remoto ligado. Com um relay definido e um celular pareado, todo chat novo se conecta sozinho; só um chat da máquina segura o relay por vez, e os outros avisam.

## Gerenciar celulares

| Comando | O que faz |
|---|---|
| `/remote` | O relay, o id desta máquina, os celulares pareados e se este chat está conectado |
| `/remote pair` | Convida mais um celular |
| `/remote off` | Desconecta este chat do relay até o próximo `/remote pair` ou o próximo início |
| `{{COMMAND}} remote devices` | Lista os celulares pareados |
| `{{COMMAND}} remote revoke "iPhone"` | Esquece um celular; as conexões abertas dele caem em segundos |
| `{{COMMAND}} remote logout` | Esquece o login da Cloudflare que o `remote up` guarda (o relay continua no ar) |

Rode `{{COMMAND}} remote up` de novo depois de atualizar o {{NAME}}: ele publica a página e o relay novos, e os celulares pareados continuam funcionando. Todos os comandos estão na [referência da CLI](./cli-reference#remote).

## Rodar seu próprio relay

Em vez da Cloudflare, você pode rodar o programa `relay` deste repositório em qualquer servidor, atrás de HTTPS (um celular só instala a página de um endereço HTTPS):

```bash
cargo build --release -p relay
RELAY_LISTEN=127.0.0.1:8080 ./target/release/relay
```

Coloque um proxy reverso com TLS na frente dele (Caddy, nginx, o balanceador da sua plataforma) e informe-o na máquina:

```bash
{{COMMAND}} remote relay https://relay.example.com
```

`relay --listen <endereço:porta>` ou `RELAY_LISTEN` escolhem onde ele escuta; o padrão é `0.0.0.0:8080`. O relay não guarda nada em disco.

## Como fica protegido

- **Criptografia ponta a ponta.** Cada celular fala com a máquina por uma sessão Noise (`IK`, e `IKpsk2` durante o pareamento). O relay só vê bytes selados.
- **Pareamento.** O código de pareamento vai na parte `#` do link, que o navegador nunca envia a um servidor. Um celular só é registrado depois de provar que tem o código. Cada celular tem a própria chave; não existe senha compartilhada.
- **O relay confere a máquina.** Uma máquina só ganha seu lugar no relay depois de provar que tem a chave da qual o id dela é derivado, então ninguém toma o lugar da sua máquina.
- **Uma porta só.** A máquina repassa apenas a API do console para o próprio console em `127.0.0.1`, com as verificações dele; ela não abre nenhuma porta.

## Os riscos

- **O terminal fica ligado.** Um celular pareado pode abrir um terminal que roda comandos como você. É para isso que o recurso existe, e ele fica ligado por padrão. Se perder um celular, revogue: `{{COMMAND}} remote revoke "<nome>"`.
- **O relay serve a página.** A página que o celular roda vem do relay. Um relay comprometido, ou a conta da Cloudflare onde ele roda, poderia servir uma página que lê a chave do celular e age como ele, terminal incluído. A criptografia não protege contra isso: só rodar o relay você mesmo, numa conta ou servidor em que confia.
- **Disponibilidade.** Quem alcança o relay pode tentar sobrecarregá-lo. Isso pode derrubar o controle remoto, mas não dá acesso.
