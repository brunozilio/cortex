---
title: Sandbox e segurança
description: A sandbox, a ocultação de segredos, os snapshots antes de comandos destrutivos, a proteção de dependências e as outras verificações que mantêm as ações do agente seguras.
---

As [permissões](./permissions) decidem o que o agente pode fazer. Além delas, o {{NAME}} tem um conjunto de mecanismos de segurança que limitam o estrago que uma ação equivocada ou manipulada pode causar. A maioria vem ligada por padrão; esta página explica cada um e como ligá-lo ou desligá-lo.

## Visão geral

| Mecanismo | Ligado por padrão | Desligado com | Onde |
|---|---|---|---|
| Sandbox | Sim | `features.experimental-sandbox` | Só macOS |
| Ocultação de segredos | Sim | `features.redact-secrets` | Todos os sistemas |
| Snapshots antes de comandos destrutivos | Sim | `features.destructive-snapshot` | Todos os sistemas |
| Proteção de dependências | Sim | `features.experimental-dependency-guard` | Todos os sistemas |
| Comandos que expõem segredos | Sempre | não pode ser desligado | Todos os sistemas |
| Ações que sempre perguntam | Sempre | não pode ser desligado | Todos os sistemas |
| Verificação de segurança no modo bypass | Sempre no bypass | use outro modo | Todos os sistemas |
| Conteúdo não confiável | Sempre | não pode ser desligado | Todos os sistemas |

Os recursos são alternados com `/features <nome> on|off` (salvo em `~/{{DIR}}/settings.json`) ou pela chave `features` das suas configurações; veja [Configurações](./settings#features).

> [!IMPORTANT]
> A sandbox, a ocultação de segredos e a proteção de dependências são **recursos de segurança**. O `{{DIR}}/settings.json` versionado de um projeto pode ligá-los, mas nunca desligá-los; só os seus próprios arquivos de configuração podem desligá-los.

## Sandbox

> [!EXPERIMENTAL]
> A sandbox é o recurso `experimental-sandbox`. Ela só funciona no macOS; em outros sistemas `/features` a mostra como "not on this system" e os comandos rodam sem confinamento.

No macOS, os comandos de shell que o agente executa (as ferramentas `Bash` e `Monitor`), os hooks de comando e as execuções de testes em segundo plano rodam dentro da sandbox do sistema (`/usr/bin/sandbox-exec`). Dentro dela:

- **Leitura** não tem restrição.
- **Gravação** só é permitida em:
  - a pasta do projeto;
  - as [pastas adicionais](./permissions);
  - o repositório, quando a sessão trabalha na própria worktree;
  - as pastas temporárias (a pasta temporária do sistema e `/tmp`);
  - caches de pacotes na sua pasta pessoal: `Library/Caches`, `.cache`, `.npm`, `.cargo/registry`, `.cargo/git`, `.cargo/.package-cache`, `.cargo/.package-cache-mutate`, `.cargo/.global-cache`, `go/pkg`, `.gradle`, `.rustup`, `.pnpm-store`, `.yarn`;
  - as pastas em `sandbox.allowWrite`;
  - dispositivos de terminal como `/dev/null` e `/dev/tty`.
- `{{DIR}}/settings.json` e `{{DIR}}/settings.local.json` no projeto e nas pastas adicionais **nunca** podem ser gravados por um comando na sandbox, para que um comando não consiga se dar permissões.
- O acesso à **rede** é permitido, a menos que `sandbox.network` seja `false`.

Quando um comando falha com "operation not permitted", o {{NAME}} acrescenta à saída uma dica dizendo ao agente para não contornar a sandbox e avisar você, para que você possa adicionar uma pasta a `sandbox.allowWrite`.

### Configurando a sandbox

```json
{
  "sandbox": {
    "allowWrite": ["~/.m2", "/opt/shared-data"],
    "network": false
  }
}
```

| Chave | Padrão | Descrição |
|---|---|---|
| `sandbox.allowWrite` | `[]` | Pastas extras com gravação liberada: absolutas ou começando com `~/`. Caminhos relativos são ignorados. Lida só dos seus próprios arquivos de configuração. |
| `sandbox.network` | `true` | `false` bloqueia todo acesso à rede para comandos na sandbox. As configurações versionadas de um projeto podem desligá-la, mas não religá-la. |

### Rodando um comando fora da sandbox

Quando um comando realmente precisa gravar em outro lugar, o agente pode pedir para rodá-lo sem sandbox. Você é consultado toda vez, com "Run OUTSIDE the sandbox (it may write anywhere)", e a resposta não pode ser lembrada. Em execuções `-p`, onde ninguém pode responder, o pedido é recusado.

### Desligando a sandbox

```bash
/features experimental-sandbox off
```

ou `{{COMMAND}} config set features.experimental-sandbox false`.

## Ocultação de segredos

Com `redact-secrets` ligado (o padrão), a saída das ferramentas é limpa antes que o modelo, a transcrição e os logs a vejam. O mesmo vale para as entradas de ferramentas mostradas na tela, a saída ao vivo dos comandos e as saídas completas guardadas em arquivos. Cada segredo é trocado por `[redacted]`:

- tokens com prefixos conhecidos: `sk-`, `sk_`, `ghp_`, `gho_`, `ghs_`, `ghu_`, `github_pat_`, `glpat-` (20 caracteres ou mais), `xoxb-`, `xoxp-`, `xoxa-`, `xoxs-`;
- ids de chave de acesso da AWS (`AKIA…`, `ASIA…`);
- JSON Web Tokens (`eyJ…` com três partes);
- o valor em `chave=valor` ou `chave: valor` quando a chave termina em `password`, `passwd`, `secret`, `token`, `api_key`, `apikey` ou `api-key`;
- a palavra depois de `Bearer`;
- todo valor de 8 caracteres ou mais nos arquivos `.env` e `.env.*` do projeto (exceto `.env.example` e `.env.sample`).

```text
config:
    API_KEY=[redacted]
    auth: Bearer [redacted]
```

Desligue com `/features redact-secrets off`.

## Snapshots antes de comandos destrutivos

Com `destructive-snapshot` ligado (o padrão), o {{NAME}} guarda uma cópia dos arquivos que um comando está para destruir, para que o `/rewind` consiga trazê-los de volta mesmo que nunca tenham ido para um commit. Ele procura por:

- `rm` com caminhos (todo arquivo dentro de uma pasta removida é guardado);
- `git clean` (arquivos não versionados; com `-x` ou `-X`, também os ignorados);
- `git reset --hard`;
- `git checkout .`, `git checkout -- …`, `git restore .`, `git restore -- …`;
- `git stash -u`, `--include-untracked`, `-a`, `--all`.

São guardados no máximo 5.000 arquivos e 200 MB por comando; quando seria preciso mais, a saída informa quantos ficaram de fora. Veja [Sessões](./sessions) para o `/rewind`.

## Proteção de dependências

> [!EXPERIMENTAL]
> A proteção de dependências é o recurso `experimental-dependency-guard`.

Antes que um comando instale um pacote, o {{NAME}} consulta o pacote no registro dele e pergunta a você antes quando:

- ele **não está publicado** (o nome pode ter sido inventado);
- ele **parece um pacote popular com erro de digitação** (uma letra a mais, a menos ou trocada, ou duas letras vizinhas invertidas, por exemplo `expres` no lugar de `express`);
- ele foi **publicado pela primeira vez há menos de 30 dias**.

| Ecossistema | Comandos verificados | Registro |
|---|---|---|
| npm | `npm`/`pnpm`/`bun` `add`, `install`, `i`; `yarn add` | registry.npmjs.org |
| Rust | `cargo add` | crates.io |
| Python | `pip`/`pip3`/`pipx install`, `uv pip install`, `uv add`, `poetry add`, `python -m pip install` | pypi.org |

A pergunta mostra "Check the package first: …" com o motivo. Ela é feita em qualquer modo, inclusive bypass; em execuções `-p` o comando é recusado.

## Comandos que expõem segredos

Alguns comandos de shell são recusados de imediato, em qualquer modo, porque imprimiriam segredos na conversa:

- imprimir o ambiente: `env`, `set`, `export` ou `export -p` sozinhos, e `printenv`;
- ler um arquivo secreto com um leitor de conteúdo (`cat`, `head`, `grep`, `base64`, `cp`, `curl`, `jq`, `openssl`…): `.env` e `.env.*` (exceto as variantes example, sample, template e dist), chaves SSH (`id_rsa`, `id_ed25519`, `id_ecdsa`, `id_dsa`), `.npmrc`, `.netrc`, `.pypirc`, `.git-credentials`, `credentials`, `secrets.json` e arquivos terminados em `.pem`, `.key`, `.p12`, `.pfx`.

O agente é orientado a deixar você rodar esse comando por conta própria com `! <comando>`, se precisar.

## Ações que sempre perguntam

Estas são perguntadas toda vez, mesmo no modo bypass e mesmo com uma regra allow, e não podem ser lembradas:

- `git push` para o branch padrão, ou com `--force`, `--force-with-lease`, `--mirror`, `--delete`, `--prune`, ou refspecs começando com `+` ou `:`;
- `publish` do `cargo`, `npm`, `pnpm` ou `yarn`;
- `gh pr merge`, `gh release create|delete|edit`, `gh api`;
- `docker push`, `podman push`;
- buscar um endereço que nunca apareceu na conversa, depois que a sessão leu conteúdo da web ou de um servidor MCP (esse conteúdo pode mandar um agente enviar dados para um endereço inventado).

Uma regra deny continua recusando-as. Veja [Permissões](./permissions).

## Verificação de segurança no modo bypass

No modo bypass nada pergunta a você, então o {{NAME}} confere cada ação que não seja uma simples leitura com uma chamada de modelo separada antes de executá-la. A verificação vê a conversa, a ação e seus arquivos de instruções; uma instrução que autoriza explicitamente aquela ação específica conta, um incentivo genérico ("seja autônomo") não. Ações avaliadas como arriscadas demais são seguradas, e o agente é orientado a pedir a você ou buscar outro caminho, e a dizer o que foi segurado. Se a verificação não conseguir rodar, a ação também é segurada.

## Conteúdo não confiável

A saída das ferramentas é tratada como dado, não como instrução. Caracteres invisíveis são removidos e tudo o que imita as marcações do próprio {{NAME}} (`system-reminder`, `persisted-output`, `untrusted-content`) é escapado, para que uma página web ou arquivo não se passe por uma mensagem do {{NAME}}. O `WebFetch` recusa endereços deste computador ou da rede privada (`localhost`, `127.0.0.1`, `10.x`, `192.168.x`, `169.254.169.254`, `*.local`, `*.internal`…).

## Outras proteções

- **Ler fora do projeto pergunta**, exceto no modo bypass ou nas [pastas adicionais](./permissions).
- **O modo plan** recusa toda edição e todo comando até você aprovar o plano.
- **As credenciais** ficam seladas em `secrets.json`, cifrado com `secrets.key`, ambos legíveis só por você. Veja [Arquivos e pastas](./files-and-directories).
- **Scripts** (ferramentas, skills, hooks e provedores em JavaScript) podem ter arquivos, processos ou rede negados com as configurações [`scripts`](./settings#scripts).
- **Servidores MCP do projeto** só iniciam depois que você os aprova. Veja [MCP](./mcp).
