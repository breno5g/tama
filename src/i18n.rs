//! All user-facing text lives here. Currently pt-BR only; adding a locale
//! means dispatching inside these functions, with no changes elsewhere.

use crate::pet::Mood;
use crate::species::Species;

pub fn species_name(s: Species) -> &'static str {
    match s {
        Species::Cat => "gato",
        Species::Dog => "cachorro",
        Species::Bunny => "coelho",
        Species::Dragon => "dragão",
        Species::Ghost => "fantasma",
        Species::Frog => "sapo",
        Species::Owl => "coruja",
        Species::Fox => "raposa",
        Species::Penguin => "pinguim",
        Species::Octopus => "polvo",
    }
}

pub fn species_trait(s: Species) -> &'static str {
    match s {
        Species::Cat => "independente",
        Species::Dog => "brincalhão",
        Species::Bunny => "tímido",
        Species::Dragon => "orgulhoso",
        Species::Ghost => "misterioso",
        Species::Frog => "tranquilão",
        Species::Owl => "sábio",
        Species::Fox => "esperto",
        Species::Penguin => "elegante",
        Species::Octopus => "curioso",
    }
}

pub fn species_sound(s: Species) -> &'static str {
    match s {
        Species::Cat => "miau!",
        Species::Dog => "au au!",
        Species::Bunny => "hop hop",
        Species::Dragon => "rawr!",
        Species::Ghost => "buu!",
        Species::Frog => "ribbit!",
        Species::Owl => "huu huu!",
        Species::Fox => "yip yip!",
        Species::Penguin => "noot noot!",
        Species::Octopus => "blub blub",
    }
}

pub fn species_tastes(s: Species) -> (&'static str, &'static str) {
    match s {
        Species::Cat => ("peixe grelhado", "banho"),
        Species::Dog => ("bolinho", "ficar sozinho"),
        Species::Bunny => ("chá de camomila", "barulho"),
        Species::Dragon => ("peixe grelhado", "banho frio"),
        Species::Ghost => ("chá de camomila", "sol do meio-dia"),
        Species::Frog => ("dia de chuva", "sol do meio-dia"),
        Species::Owl => ("madrugada", "acordar cedo"),
        Species::Fox => ("bolinho", "coleira"),
        Species::Penguin => ("peixe grelhado", "calor"),
        Species::Octopus => ("peixe grelhado", "aquário apertado"),
    }
}

pub fn mood_label(m: Mood) -> &'static str {
    match m {
        Mood::Happy => "feliz",
        Mood::Hungry => "com fome!",
        Mood::Dirty => "precisa de banho",
        Mood::Sleepy => "com sono...",
        Mood::Sad => "triste :(",
        Mood::Sleeping => "dormindo",
    }
}

// Index-aligned with pet::FOODS.
pub const FOOD_NAMES: [&str; 4] = ["ração", "peixe grelhado", "bolinho", "chá de camomila"];

pub const STAT_LABELS: [&str; 4] = ["fome", "felicidade", "energia", "higiene"];
pub const STAT_SHORT: [char; 4] = ['F', 'A', 'E', 'H'];
pub const XP_LABEL: &str = "xp";
pub const APP_TITLE: &str = "~ tama ~";
pub const ZEN_TAG: &str = "zen";
pub const ZEN_MODE: &str = "modo zen";
pub const DAY: &str = "dia";
pub const LEVEL_SHORT: &str = "nv";
pub const LIKES: &str = "gosta de";
pub const HATES: &str = "detesta";
pub const PANEL_STATUS: &str = "status";
pub const PANEL_MOOD: &str = "humor";
pub const PANEL_EVENTS: &str = "eventos";
pub const LOG_EMPTY: &str = "sem eventos por enquanto";
pub const MENU_TITLE: &str = "cardápio";
pub const GAME_TITLE: &str = "jokenpô";
pub const PICKER_TITLE: &str = "escolha seu pet";
pub const NAME_PROMPT: &str = "qual o nome do seu pet?";
pub const DEFAULT_NAME: &str = "tama";
pub const GALLERY_SMALL: &str = "(compacto)";
pub const GALLERY_MINI: &str = "(mini)";
pub const HANDS: [&str; 3] = ["pedra", "papel", "tesoura"];

pub const FOOTER_HOME: [&str; 3] = [
    "[espaço] ações  [a] assistente  [q] sair",
    "espaço:ações a:assist q:sair",
    "esp a q",
];
pub const ACTIONS_TITLE: &str = "ações";
pub const FOOTER_ACTIONS: [&str; 2] = ["[↑↓] ou número  [enter] usar  [esc] voltar", "↑↓ 1-9 enter esc"];
// Index-aligned with app::Action and ui::ACTION_GLYPHS.
pub const ACTION_LABELS: [&str; 9] =
    ["comer", "brincar", "dormir", "banho", "jokenpô", "assistente", "pomodoro", "zen", "trocar pet"];
pub const FOOTER_ASSISTANT: [&str; 3] = [
    "[enter] próxima da fila  [x] limpar fila  [a] modo pet  [q] sair",
    "enter:próxima x:limpar a:pet q:sair",
    "enter x a q",
];
pub const FOOTER_ASK: [&str; 2] = ["[1-9] responder  [esc] ignorar  [a] modo pet", "1-9 esc a"];
pub const ASSISTANT_TAG: &str = "modo assistente";
pub const PANEL_QUEUE: &str = "fila";
pub const NO_MESSAGES: &str = "sem mensagens — esperando programas...";
pub const ANSWER_IGNORED: &str = "ignorada";
pub const TIMER_LABEL: &str = "timer";
pub const TYPE_LABEL: &str = "tipo";
pub const FROM_LABEL: &str = "de";
pub const ASKS_VERB: &str = "pergunta";

pub fn kind_label(k: crate::assistant::Kind) -> &'static str {
    match k {
        crate::assistant::Kind::Info => "info",
        crate::assistant::Kind::Success => "sucesso",
        crate::assistant::Kind::Warn => "alerta",
        crate::assistant::Kind::Error => "erro",
    }
}

pub fn msg_reminder(text: &str) -> String {
    format!("lembrete: {text}")
}
pub const MSG_TIMER_DONE: &str = "tempo esgotado!";
pub const PROGRESS_DEFAULT: &str = "progresso";
pub fn msg_progress_done(from: &str) -> String {
    let from = if from.is_empty() { "tarefa" } else { from };
    format!("{from} concluído!")
}
pub fn msg_answered(text: &str, answer: &str) -> String {
    format!("você respondeu \"{answer}\" para: {text}")
}
pub fn msg_ask_expired(from: &str) -> String {
    format!("pergunta de {from} expirou")
}
pub const EXPIRES_LABEL: &str = "expira";
pub fn msg_celebrate(name: &str) -> String {
    format!("\\o/ {name} comemorou!")
}
pub fn msg_action_fed(name: &str) -> String {
    format!("{name} ganhou ração de um programa")
}
pub const UNKNOWN_SENDER: &str = "?";

fn fmt_secs(secs: u64) -> String {
    if secs >= 60 { format!("{}m{:02}s", secs / 60, secs % 60) } else { format!("{secs}s") }
}
pub fn msg_watch_start(cmd: &str) -> String {
    format!("rodando: {cmd}")
}
pub fn msg_watch_ok(cmd: &str, secs: u64) -> String {
    format!("{cmd} — ok ({})", fmt_secs(secs))
}
pub fn msg_watch_fail(cmd: &str, code: i32, secs: u64) -> String {
    format!("{cmd} — falhou (exit {code}, {})", fmt_secs(secs))
}

pub const POMO_FOCUS: &str = "foco";
pub const POMO_BREAK: &str = "pausa";
pub const POMO_FROM: &str = "pomodoro";
pub const MSG_POMO_START: &str = "pomodoro começou — foco!";
pub const MSG_POMO_BREAK: &str = "pausa! hora de descansar";
pub const MSG_POMO_FOCUS: &str = "pausa acabou — foco!";
pub const MSG_POMO_STOPPED: &str = "pomodoro encerrado";
pub const POMO_TITLE: &str = "pomodoro";
pub const POMO_TASKS: &str = "tarefas em andamento";
pub const POMO_NO_TASKS: &str = "nenhuma tarefa agora";
pub const POMO_CYCLE: &str = "ciclo";
// Index-aligned with app::POMO_PRESETS.
pub const POMO_PRESET_LABELS: [&str; 3] =
    ["25m foco · 5m pausa", "50m foco · 10m pausa", "15m foco · 3m pausa"];
pub const FOOTER_POMO_IDLE: [&str; 2] = ["[↑↓] ou número  [enter] começar  [esc] voltar", "↑↓ 1-3 enter esc"];
pub const FOOTER_POMO_ACTIVE: [&str; 2] = ["[enter] parar  [esc] voltar", "enter esc"];

pub const CLI_NOT_RUNNING: &str = "tama não está rodando — abra o app primeiro";
pub const CLI_PIPE_ERROR: &str = "não consegui escrever no pipe do tama";
pub const CLI_USAGE_SAY: &str = "uso: tama say \"texto\" [--de origem] [--tipo info|sucesso|alerta|erro]";
pub const CLI_USAGE_ASK: &str =
    "uso: tama ask \"pergunta\" [--opcoes a,b,c | --opcoes a --opcoes b ...] [--de origem] [--id id] [--timeout 60s] [--padrao resposta]";
pub const CLI_ASK_TIMEOUT: &str = "sem resposta do tama (timeout)";
pub const CLI_USAGE_REMIND: &str = "uso: tama lembrar \"texto\" --em 10m";
pub const CLI_USAGE_TIMER: &str = "uso: tama timer 25m";
pub const CLI_USAGE_DO: &str = "uso: tama do comemorar|dormir|acordar|alimentar";
pub const CLI_USAGE_WATCH: &str = "uso: tama watch [--de origem] comando [args...]";
pub const CLI_USAGE_POMODORO: &str = "uso: tama pomodoro [25m] [--pausa 5m] | tama pomodoro parar";
pub const FOOTER_MENU: [&str; 2] = ["[↑↓] escolher  [enter] dar  [esc] voltar", "↑↓ enter esc"];
pub const FOOTER_GAME: [&str; 2] = ["[1] pedra  [2] papel  [3] tesoura  [esc] voltar", "1 2 3 esc"];
pub const FOOTER_PICKER: [&str; 2] = ["[←↑↓→] navegar  [enter] confirmar  [esc] voltar", "setas · enter · esc"];
pub const FOOTER_NAME: [&str; 1] = ["[enter] confirmar"];

pub fn msg_played(name: &str) -> String {
    format!("você brincou com {name}")
}

pub fn msg_fed(food: &str, name: &str) -> String {
    format!("você deu {food} para {name}")
}

pub fn msg_bathed(name: &str) -> String {
    format!("{name} tomou banho")
}

pub const BATH_SUFFIX: &str = "(higiene 100)";

pub fn msg_sleep(name: &str, sleeping: bool) -> String {
    if sleeping { format!("{name} foi dormir, bons sonhos...") } else { format!("{name} acordou!") }
}

pub fn msg_zen(on: bool) -> String {
    format!("modo zen {}", if on { "ligado" } else { "desligado" })
}

pub fn msg_became(name: &str, species: Species) -> String {
    format!("{name} virou um {}!", species_name(species))
}

pub fn msg_level_up(name: &str, level: u32) -> String {
    format!("{name} subiu para o nível {level}!")
}

pub fn msg_game(player: &str, pet: &str, outcome: &str) -> String {
    format!("jokenpô: {player} × {pet} — {outcome}")
}

pub fn msg_game_waiting(name: &str) -> String {
    format!("{name} está esperando sua jogada...")
}

pub const GAME_DRAW: &str = "empate";
pub const GAME_WIN: &str = "você venceu";
pub const GAME_LOSS: &str = "você perdeu";

pub fn msg_warning(mood: Mood, name: &str) -> Option<String> {
    match mood {
        Mood::Hungry => Some(format!("{name} está com fome!")),
        Mood::Dirty => Some(format!("{name} está precisando de banho")),
        Mood::Sleepy => Some(format!("{name} está com sono...")),
        Mood::Sad => Some(format!("{name} está triste")),
        _ => None,
    }
}
