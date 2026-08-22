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

Programas externos falam com o tama por um named pipe; respostas saem num
arquivo de saída. Mensagem chegando abre o modo assistente sozinha; perguntas
furam a fila **e interrompem qualquer tela** (pomodoro, jogo, menu — ao
responder você volta para onde estava); falas expiram em ~8s (ou `enter`).

### CLI (o jeito simples)

```bash
tama say "deploy concluído!" --de deploy-bot --tipo sucesso
tama ask "subir pra produção?" --opcoes sim,nao   # bloqueia; imprime a escolha
tama ask "qual banco?" --opcoes "Postgres" --opcoes "Sim, o de sempre" \
  --timeout 60s --padrao "Postgres"               # --opcoes repetível aceita vírgula;
                                                  # expirou -> imprime o --padrao (sem ele: exit 124)
tama lembrar "standup" --em 10m
tama timer 25m                                    # regressivo no cabeçalho
tama do comemorar                                 # comemorar · dormir · acordar · alimentar
tama watch cargo test --release                   # roda e reporta sucesso/erro sozinho
tama pomodoro 25m --pausa 5m                      # ciclos de foco; "tama pomodoro parar" encerra
```

`tama watch` avisa quando o comando começa e reporta o resultado pelo exit
code (verde/vermelho), repassando o exit code adiante — dá para usar no meio
de qualquer script. `--de origem` renomeia a fonte; se o app não estiver
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
[ "$(tama ask 'rodar a suite lenta?' --opcoes sim,nao)" = "sim" ] && cargo test --release
```

Se o app não estiver aberto, o CLI falha em 2s com "tama não está rodando".

### Pipe (qualquer linguagem, sem o binário)

Uma linha de JSON flat por mensagem em `~/.local/share/tama/input`:

```bash
echo '{"fala":"backup ok","tipo":"info","de":"cron"}'        > ~/.local/share/tama/input
echo '{"pergunta":"subir?","opcoes":"sim\nnao","id":"rel-1"}' > ~/.local/share/tama/input
echo '{"progresso":62,"de":"backup"}'                        > ~/.local/share/tama/input
echo '{"lembrete":"standup","em":"10m"}'                     > ~/.local/share/tama/input
echo '{"timer":"25m"}'                                       > ~/.local/share/tama/input
echo '{"acao":"comemorar"}'                                  > ~/.local/share/tama/input
echo '{"pomodoro":"25m","pausa":"5m"}'                       > ~/.local/share/tama/input
```

Campos: `fala` · `pergunta`+`opcoes`+`id`+`expira` · `progresso` ·
`lembrete`+`em` · `timer` · `acao` · `pomodoro`+`pausa` (`"pomodoro":"off"`
encerra), mais `tipo` (`info|sucesso|alerta|erro`) e `de` (origem). Durações
em `s|m|h`. Strings aceitam os escapes JSON `\n` `\t` `\r` `\"` `\\`; as
opções são separadas por `\n` (então podem conter vírgula). `expira` é epoch
absoluto: passou dele, a pergunta some da fila sem resposta (o CLI põe
sozinho quando você usa `--timeout`, com contagem regressiva na tela).
Linha inválida é ignorada em silêncio. Progresso é por origem: cada `de` tem
sua própria barra no painel de eventos, então tarefas concorrentes não se
atropelam (aos 100% a barra sai e vira evento de sucesso).

Respostas viram linhas JSON em `~/.local/share/tama/output`:

```json
{"id":"rel-1","resposta":"sim"}
```

Pergunta descartada (esc, limpar fila ou sair do app) responde `"ignorada"` —
nenhum script fica pendurado. Lembretes e timer valem enquanto o app está
aberto.

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
    "Stop": [{"hooks": [{"type": "command", "command": "tama say 'claude terminou' --de claude --tipo sucesso"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "tama say 'claude precisa de você' --de claude --tipo alerta"}]}]
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
  opções; a escolha volta para o Claude como feedback e ele continua.
- Claude no desktop, tama no celular (Termux): os scripts leem `TAMA_CMD`,
  então `"command": "TAMA_CMD='ssh tamafone tama' /caminho/scripts/tama-permission.sh"`
  manda as perguntas para o telefone (alias `tamafone` do
  `scripts/termux-setup.sh`).

**git** — comemore cada commit:

```bash
printf '#!/bin/sh\ntama say "commit: $(git log -1 --pretty=%%s)" --de git --tipo sucesso\n' \
  > .git/hooks/post-commit && chmod +x .git/hooks/post-commit
```

**Qualquer build/CI local** — embrulhe no `watch`:

```bash
tama watch --de deploy ./deploy.sh
```

## Arquivos

| Caminho | Conteúdo |
|---|---|
| `~/.local/share/tama/state` | save do pet (texto `chave=valor`) |
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
| `assistant.rs` | contrato do pipe: parser JSON flat, leitor, respostas |
| `cli.rs` | subcomandos `say/ask/lembrar/timer/do/watch/pomodoro` |
| `i18n.rs` | todo texto visível (pt-BR; outro idioma entra só aqui) |

Sem dependências além de `crossterm`. O renderer nunca limpa a tela (repinta
todas as células por frame, num flush único com synchronized update), então
não pisca — nem dentro de tmux.

Os mockups que guiaram a interface estão em `design/` (canvases do Claude
Design com os quadros da interface 2.0 e do modo assistente).
