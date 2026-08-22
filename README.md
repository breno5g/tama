# tama

Um tamagotchi de terminal que também é o rosto do seu assistente: além de pet
virtual com sprites em pixel-art, ele fala mensagens enviadas por programas
externos, faz perguntas cuja resposta volta para o script, mostra progresso de
tarefas longas e dispara lembretes e timers.

## Rodando

```bash
cargo run              # abre o pet (primeira vez: escolha a espécie e o nome)
cargo run -- --gallery # imprime a arte de todos os pets e sai
cargo test             # 59 testes
```

Requer um terminal com suporte a 256 cores. Para ter o binário no PATH:
`cargo install --path .`.

## O pet

- **10 espécies** — gato, cachorro, coelho, dragão, fantasma, sapo, coruja,
  raposa, pinguim e polvo — em sprites de
  blocos estilo LCD, com versões grande, compacta e de uma linha.
- **Stats**: fome, felicidade, energia e higiene decaem com o tempo (inclusive
  offline, com teto de 24h); humor derivado das stats muda a cara e a cor.
- **XP e nível**: toda interação dá XP; a barra e o nível ficam na tela.
- **Cardápio** com 4 comidas de efeitos e tradeoffs (bolinho enche mas suja).
- **Jokenpô** contra o pet (perder deixa *ele* mais feliz).
- **Ciclo dia/noite** no cabeçalho (☀/☾, dia N e relógio) e cenário com grama.
- **Log de eventos** com timestamps e um balão de fala com o som da espécie.
- **Modo zen** (`z`): desliga todo o decay e esconde as barras — o pet só
  existe e anima.

### Teclas

| Tecla | Ação |
|---|---|
| `espaço` | menu de ações (↑↓ ou número, `enter` usa, `esc` volta) |
| `a` | alterna modo pet ↔ modo assistente |
| `q` | sair |

O menu reúne comer, brincar, dormir, banho, jokenpô, assistente, pomodoro,
zen e trocar de pet. Os atalhos diretos continuam funcionando escondidos para quem já
decorou: `f p s b m z c`. O seletor de pet é uma grade com as 10 espécies
visíveis (setas navegam, `enter` confirma, `esc` cancela) e prévia animada.

### Layout responsivo

O layout se adapta a largura **e** altura a cada frame: painéis completos
(cabeçalho, cena do pet, status, humor, eventos) → arte compacta nos painéis →
layout empilhado → mini painel de uma linha (cabe num pane de tmux de 26×8).
Sobra de altura vira céu na cena e mais linhas de eventos. Regra de ouro:
**conteúdo dinâmico nunca redimensiona o layout** — balão, "z Z z" de dormir,
ticker, progresso e eventos têm espaço reservado, então nada pula na tela.

## Modo assistente

Programas externos falam com o tama por um named pipe **ou por HTTP**;
respostas saem num arquivo de saída (pipe/CLI) ou na própria resposta HTTP.
Pergunta se responde com `↑↓` + `enter`, como o menu de ações, ou direto
pelo número (`1-9`). Lista maior que o espaço rola sozinha atrás do cursor,
com `↑N`/`↓N` indicando o que ficou fora de vista. Quando a pergunta aceita
texto livre (`input`), a última opção é **outra (escrever)** — como o "Other"
dos prompts do Claude: escolher ela (ou apertar `t`) abre um campo de várias
linhas — o texto quebra sozinho e acompanha o cursor (`↑` marca o que rolou
para cima), `alt+enter` força uma quebra, `enter` envia e `esc` volta para a
lista. Cabem 1000 caracteres, o bastante para responder um LLM com um
parágrafo inteiro. É assim que um LLM recebe prosa de volta,
e não só uma das opções que ele imaginou.
A API — comandos, flags, chaves e valores do protocolo — é em inglês; só o
que aparece na tela segue em português. Mensagem chegando abre o modo
assistente sozinha; perguntas furam a fila **e interrompem qualquer tela**
(pomodoro, jogo, menu — ao responder você volta para onde estava); falas
expiram em ~8s (ou `enter`).

### CLI (o jeito simples)

```bash
tama say "deploy concluído!" --from deploy-bot --type success
tama ask "subir pra produção?" --options sim,nao  # bloqueia; imprime a escolha
tama ask "qual banco?" --options "Postgres" --options "Sim, o de sempre" \
  --timeout 60s --default "Postgres"              # --options repetível aceita vírgula;
                                                  # expirou -> imprime o --default (sem ele: exit 124)
tama ask "resuma o que fazer" --input             # só texto; com --options, vira
                                                  # "outra (escrever)" no fim da lista
tama remind "standup" --in 10m
tama timer 25m                                    # regressivo no cabeçalho
tama do celebrate                                 # celebrate · sleep · wake · feed
tama watch cargo test --release                   # roda e reporta sucesso/erro sozinho
tama pomodoro 25m --break 5m                      # ciclos de foco; "tama pomodoro off" encerra
```

`tama watch` avisa quando o comando começa e reporta o resultado pelo exit
code (verde/vermelho), repassando o exit code adiante — dá para usar no meio
de qualquer script. `--from origem` renomeia a fonte; se o app não estiver
aberto, o comando roda mesmo assim. No pomodoro o cabeçalho mostra a fase
(`foco`/`pausa`) com o regressivo, e o pet dorme junto nas pausas.

O pomodoro é um **modo** com tela própria (menu de ações → pomodoro): um
relógio LCD gigante rodando ao lado do pet na cena — dourado no foco, azul
na pausa, quando o pet cochila junto até a pausa acabar —, barra de
progresso da fase, contador de ciclos e as tarefas em andamento embaixo.
Iniciar (presets de 25/50/15 min, ou via CLI/pipe com o app na home) abre a
tela e **fica** nela; as viradas de fase acontecem ali mesmo, sem pular para
o assistente. `enter` para o ciclo, `esc` volta pra home com o regressivo no
cabeçalho. Responsiva como a home: sem largura para o pet, fica só o
relógio; em painéis minúsculos, vira uma linha de status.

Uso real num script:

```bash
[ "$(tama ask 'rodar a suite lenta?' --options sim,nao)" = "sim" ] && cargo test --release
```

Se o app não estiver aberto, o CLI falha em 2s com "tama não está rodando".

### HTTP — mande de qualquer projeto ou máquina

Com o app aberto, o tama ouve em `0.0.0.0:8262` (desligue com
`TAMA_HTTP=off`, mude com `TAMA_HTTP=addr:porta`). Um POST com um JSON flat
entrega a mensagem — de qualquer linguagem, inclusive de outra máquina da
rede (PC → tablet sem ssh):

```bash
curl -s http://tablet:8262/ -d '{"from":"ci","message":"build ok","type":"success"}'
curl -s http://tablet:8262/ -d '{"from":"deploy","message":"subir?","actions":["sim","não"]}'
curl -s http://tablet:8262/ -d '{"command":"celebrate","message":"merge!"}'
```

```js
await fetch("http://tablet:8262/", { method: "POST",
  body: JSON.stringify({ from: "app", message: "ok?", actions: ["sim", "não"] }) })
  .then(r => r.json()); // { answer: "sim" }
```

Um POST com `actions` (ou `"input":true`) **segura a conexão até você
responder na TUI** e devolve `{"answer":"sim"}`; sem resposta até `expires`
(default: 5 min) devolve `408` + `{"answer":null}`. Os demais respondem `{"ok":true}` na
hora; corpo inválido dá `400`. `GET /` responde `{"ok":true,"pet":"nome"}`
(bom de health-check). Se `TAMA_TOKEN` estiver setado ao abrir o app, todo
request precisa de `Authorization: Bearer <token>` (senão: aberto — pense
LAN de casa; o HTTP não responde perguntas nem lê nada do pet).

### Esquema de mensagem (HTTP e pipe, o mesmo)

| chave | tipo | efeito |
|---|---|---|
| `message` | string | fala — ou pergunta, se houver `actions` |
| `from` | string | origem exibida |
| `type` | `info\|success\|warn\|error` | cor/expressão da fala |
| `actions` | array de strings (ou string com `\n`) | opções de resposta (máx. 9) |
| `input` | `true` | adiciona "outra (escrever)" à lista; sem `actions`, abre o campo direto |
| `command` | `celebrate\|sleep\|wake\|feed` | ação no pet; pode vir junto de `message` |
| `id` | string | identifica a resposta (HTTP gera sozinho) |
| `expires` | epoch | pergunta some sem resposta depois disso |
| `progress` | 0-100 | barra por origem (100 encerra) |
| `remind` + `in` | string + `30s\|10m\|1h` | lembrete |
| `timer` | duração | regressivo no cabeçalho |
| `pomodoro` + `break` | duração (`"off"` encerra) | ciclos de foco |

Strings aceitam os escapes JSON `\n` `\t` `\r` `\"` `\\` (opção pode conter
vírgula). Durações em `s|m|h`. Progresso é por origem: cada `from` tem sua
própria barra, então tarefas concorrentes não se atropelam.

### Pipe (mesmo esquema, sem rede)

Uma linha de JSON flat por mensagem em `~/.local/share/tama/input`; linha
inválida é ignorada em silêncio:

```bash
echo '{"message":"backup ok","type":"info","from":"cron"}' > ~/.local/share/tama/input
echo '{"pomodoro":"25m","break":"5m"}'                     > ~/.local/share/tama/input
```

Respostas de perguntas viram linhas JSON em `~/.local/share/tama/output`
(é onde o CLI e o HTTP esperam por elas):

```json
{"id":"rel-1","answer":"sim"}
```

Pergunta descartada (esc, limpar fila ou sair do app) responde `"ignored"` —
nenhum script fica pendurado.

Pergunta chegando **toca o bell do terminal**; no Termux ela também vira uma
notificação do Android (uma só, substituída em lugar, que some quando não há
mais nada esperando) — dá para deixar o tablet na mesa e só olhar quando
apitar. Requer o pacote `termux-api`; fora do Termux o aviso é só o bell.

Lembretes, timer e pomodoro **sobrevivem ao fechamento do app**: ficam em
`~/.local/share/tama/schedule` e voltam ao abrir, com o pomodoro retomando de
onde parou. O que venceu enquanto o app estava fechado há mais de uma hora é
descartado (lembrete de dois dias atrás disparando no boot é ruído, não
lembrete). Os saves são atômicos, então um kill no meio da escrita — o
Android faz isso com o Termux — não corrompe nada.

### Expressões por tipo

O pet reage ao que está falando: **info** cara neutra piscando (balão ciano),
**sucesso** cara feliz dando pulinho (verde), **alerta** olhos arregalados sem
piscar (amarelo), **erro** cara triste tremendo (vermelho) — dá pra saber que
é erro sem ler. Progresso ocupa as primeiras linhas do painel de eventos (uma
barra por origem) e aos 100% vira evento de sucesso.

### Integrações prontas

O tama vira o rosto de qualquer ferramenta que consiga rodar um comando.

**Claude Code** — em `~/.claude/settings.json`, avisa quando o Claude termina
ou quer sua atenção:

```json
{
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "tama say 'claude terminou' --from claude --type success"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "tama say 'claude precisa de você' --from claude --type warn"}]}]
  }
}
```

**Claude Code respondido pelo tama** — os scripts em `scripts/` vão além do
aviso: os prompts de permissão e as perguntas de múltipla escolha do Claude
aparecem no tama e você responde de lá (teclas 1-9), sem voltar ao terminal
do Claude. Sem resposta em 60s/120s (ou com o tama fechado), o prompt
aparece no Claude Code normalmente — nada trava.

```json
{
  "hooks": {
    "PermissionRequest": [{"hooks": [{"type": "command", "command": "/caminho/tama/scripts/tama-permission.sh", "timeout": 90}]}],
    "PreToolUse": [{"matcher": "AskUserQuestion", "hooks": [{"type": "command", "command": "/caminho/tama/scripts/tama-question.sh", "timeout": 180}]}]
  }
}
```

- `tama-permission.sh` (hook `PermissionRequest`): dispara só quando um
  prompt interativo apareceria; `permitir`/`negar` decidem na hora,
  `decidir no claude` (ou timeout) devolve o prompt ao harness.
- `tama-question.sh` (hook `PreToolUse` com matcher `AskUserQuestion`,
  requer `jq`): cada pergunta do Claude vira um `tama ask` com as mesmas
  opções mais `--input`, então a lista termina em "outra (escrever)" para
  responder algo que o Claude não ofereceu; a escolha volta para ele como
  feedback e ele continua.
- Claude no desktop, tama no celular (Termux): os scripts leem `TAMA_CMD`,
  então `"command": "TAMA_CMD='ssh tamafone tama' /caminho/scripts/tama-permission.sh"`
  manda as perguntas para o telefone (alias `tamafone` do
  `scripts/termux-setup.sh`).

**git** — comemore cada commit:

```bash
printf '#!/bin/sh\ntama say "commit: $(git log -1 --pretty=%%s)" --from git --type success\n' \
  > .git/hooks/post-commit && chmod +x .git/hooks/post-commit
```

**Qualquer build/CI local** — embrulhe no `watch`:

```bash
tama watch --from deploy ./deploy.sh
```

## Arquivos

| Caminho | Conteúdo |
|---|---|
| `~/.local/share/tama/state` | save do pet (texto `chave=valor`) |
| `~/.local/share/tama/schedule` | lembretes, timer e pomodoro em andamento |
| `~/.local/share/tama/input` | named pipe de entrada do assistente |
| `~/.local/share/tama/output` | respostas das perguntas (JSON por linha) |

## Código

| Módulo | Responsabilidade |
|---|---|
| `pet.rs` | regras do bicho: stats, decay, XP, comidas, humor |
| `species.rs` | espécies e renderização dos sprites |
| `state.rs` | persistência |
| `ui.rs` | renderização: painéis, tiers responsivos, telas |
| `app.rs` | loop principal, telas interativas, fila do assistente |
| `assistant.rs` | contrato das mensagens: parser JSON flat, leitor do pipe, respostas |
| `http.rs` | servidor HTTP mínimo: POST → canal do app, long-poll de perguntas |
| `cli.rs` | subcomandos `say/ask/remind/timer/do/watch/pomodoro` |
| `i18n.rs` | todo texto visível (pt-BR; outro idioma entra só aqui) |

Sem dependências além de `crossterm`. O renderer nunca limpa a tela (repinta
todas as células por frame, num flush único com synchronized update), então
não pisca — nem dentro de tmux.

Os mockups que guiaram a interface estão em `design/` (canvases do Claude
Design com os quadros da interface 2.0 e do modo assistente).
