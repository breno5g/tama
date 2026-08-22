# tama

Um tamagotchi de terminal que também é o rosto do seu assistente: além de pet
virtual com sprites em pixel-art, ele fala mensagens enviadas por programas
externos, faz perguntas cuja resposta volta para o script, mostra progresso de
tarefas longas e dispara lembretes e timers.

## Rodando

```bash
cargo run              # abre o pet (primeira vez: escolha a espécie e o nome)
cargo run -- --gallery # imprime a arte de todos os pets e sai
cargo test             # 50 testes
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

O menu reúne comer, brincar, dormir, banho, jokenpô, assistente, zen e trocar
de pet. Os atalhos diretos continuam funcionando escondidos para quem já
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
furam a fila; falas expiram em ~8s (ou `enter`).

### CLI (o jeito simples)

```bash
tama say "deploy concluído!" --de deploy-bot --tipo sucesso
tama ask "subir pra produção?" --opcoes sim,nao   # bloqueia; imprime a escolha
tama lembrar "standup" --em 10m
tama timer 25m                                    # regressivo no cabeçalho
tama do comemorar                                 # comemorar · dormir · acordar · alimentar
```

Uso real num script:

```bash
[ "$(tama ask 'rodar a suite lenta?' --opcoes sim,nao)" = "sim" ] && cargo test --release
```

Se o app não estiver aberto, o CLI falha em 2s com "tama não está rodando".

### Pipe (qualquer linguagem, sem o binário)

Uma linha de JSON flat por mensagem em `~/.local/share/tama/input`:

```bash
echo '{"fala":"backup ok","tipo":"info","de":"cron"}'        > ~/.local/share/tama/input
echo '{"pergunta":"subir?","opcoes":"sim,nao","id":"rel-1"}' > ~/.local/share/tama/input
echo '{"progresso":62,"de":"backup"}'                        > ~/.local/share/tama/input
echo '{"lembrete":"standup","em":"10m"}'                     > ~/.local/share/tama/input
echo '{"timer":"25m"}'                                       > ~/.local/share/tama/input
echo '{"acao":"comemorar"}'                                  > ~/.local/share/tama/input
```

Campos: `fala` · `pergunta`+`opcoes`+`id` · `progresso` · `lembrete`+`em` ·
`timer` · `acao`, mais `tipo` (`info|sucesso|alerta|erro`) e `de` (origem).
Durações em `s|m|h`. Linha inválida é ignorada em silêncio.

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
é erro sem ler. Progresso ocupa a primeira linha do painel de eventos e aos
100% vira evento de sucesso.

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
| `cli.rs` | subcomandos `say/ask/lembrar/timer/do` |
| `i18n.rs` | todo texto visível (pt-BR; outro idioma entra só aqui) |

Sem dependências além de `crossterm`. O renderer nunca limpa a tela (repinta
todas as células por frame, num flush único com synchronized update), então
não pisca — nem dentro de tmux.

Os mockups que guiaram a interface estão em `design/` (canvases do Claude
Design com os quadros da interface 2.0 e do modo assistente).
