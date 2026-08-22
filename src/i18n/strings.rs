//! The full set of user-facing strings, as one struct. Every locale is one
//! `static Strings` (see `pt_br.rs`, `en.rs`), so adding a language cannot
//! silently skip a string: a missing field is a compile error in one place.
//!
//! Fields holding `{placeholder}` templates are filled by `msg.rs`. Arrays
//! indexed by an enum keep the enum's declaration order — the accessors in
//! `mod.rs` index them with `as usize`.

pub struct Strings {
    // --- species, indexed by species::Species ---
    pub species_names: [&'static str; 10],
    pub species_traits: [&'static str; 10],
    pub species_sounds: [&'static str; 10],
    pub species_tastes: [(&'static str, &'static str); 10], // (likes, hates)

    // --- pet state ---
    pub mood_labels: [&'static str; 6], // indexed by pet::Mood
    pub food_names: [&'static str; 4],  // index-aligned with pet::FOODS
    pub stat_labels: [&'static str; 4],
    pub stat_short: [char; 4],
    pub xp_label: &'static str,

    // --- chrome ---
    pub app_title: &'static str,
    pub day: &'static str,
    pub level_short: &'static str,
    pub zen_tag: &'static str,
    pub zen_mode: &'static str,

    // --- panels ---
    pub panel_status: &'static str,
    pub panel_mood: &'static str,
    pub panel_events: &'static str,
    pub panel_queue: &'static str,
    pub likes: &'static str,
    pub hates: &'static str,
    pub log_empty: &'static str,

    // --- menus and pickers ---
    pub actions_title: &'static str,
    pub action_labels: [&'static str; 9], // index-aligned with app::Action
    pub menu_title: &'static str,
    pub game_title: &'static str,
    pub picker_title: &'static str,
    pub name_prompt: &'static str,
    pub default_name: &'static str,
    pub hands: [&'static str; 3],
    pub game_draw: &'static str,
    pub game_win: &'static str,
    pub game_loss: &'static str,
    pub bath_suffix: &'static str,

    // --- gallery (--gallery) ---
    pub gallery_small: &'static str,
    pub gallery_mini: &'static str,

    // --- assistant ---
    pub assistant_tag: &'static str,
    pub no_messages: &'static str,
    pub kind_labels: [&'static str; 4], // indexed by assistant::Kind
    pub from_label: &'static str,
    pub type_label: &'static str,
    pub asks_verb: &'static str,
    pub expires_label: &'static str,
    pub option_write: &'static str, // the "Other" of the harness prompts
    /// Fallback options for an ask that named none. Shown as labels AND
    /// returned as the answer, so they are text, not protocol.
    pub default_yes: &'static str,
    pub default_no: &'static str,
    pub unknown_sender: &'static str,
    pub progress_default: &'static str,
    pub progress_fallback: &'static str, // unnamed source in msg_progress_done
    pub timer_label: &'static str,

    // --- pomodoro ---
    pub pomo_title: &'static str,
    pub pomo_focus: &'static str,
    pub pomo_break: &'static str,
    pub pomo_from: &'static str,
    pub pomo_tasks: &'static str,
    pub pomo_no_tasks: &'static str,
    pub pomo_cycle: &'static str,
    pub pomo_preset_labels: [&'static str; 3], // index-aligned with app::POMO_PRESETS

    // --- footers, widest candidate first ---
    pub footer_home: [&'static str; 3],
    pub footer_actions: [&'static str; 2],
    pub footer_menu: [&'static str; 2],
    pub footer_game: [&'static str; 2],
    pub footer_picker: [&'static str; 2],
    pub footer_name: [&'static str; 1],
    pub footer_assistant: [&'static str; 3],
    pub footer_ask: [&'static str; 3],
    pub footer_input: [&'static str; 3],
    pub footer_pomo_idle: [&'static str; 2],
    pub footer_pomo_active: [&'static str; 2],

    // --- log messages ({placeholder} templates) ---
    pub msg_played: &'static str,
    pub msg_fed: &'static str,
    pub msg_bathed: &'static str,
    pub msg_sleep: &'static str,
    pub msg_wake: &'static str,
    pub msg_zen_on: &'static str,
    pub msg_zen_off: &'static str,
    pub msg_became: &'static str,
    pub msg_level_up: &'static str,
    pub msg_game: &'static str,
    pub msg_game_waiting: &'static str,
    pub msg_celebrate: &'static str,
    pub msg_action_fed: &'static str,
    pub msg_reminder: &'static str,
    pub msg_timer_done: &'static str,
    pub msg_progress_done: &'static str,
    pub msg_answered: &'static str,
    pub msg_ask_expired: &'static str,
    /// Indexed by pet::Mood; `""` means this mood raises no warning.
    pub msg_warnings: [&'static str; 6],

    // --- pomodoro messages ---
    pub msg_pomo_start: &'static str,
    pub msg_pomo_break: &'static str,
    pub msg_pomo_focus: &'static str,
    pub msg_pomo_stopped: &'static str,

    // --- CLI (flags and wire keys stay English; only these are translated) ---
    pub cli_not_running: &'static str,
    pub cli_pipe_error: &'static str,
    pub cli_ask_timeout: &'static str,
    pub cli_usage_say: &'static str,
    pub cli_usage_ask: &'static str,
    pub cli_usage_remind: &'static str,
    pub cli_usage_timer: &'static str,
    pub cli_usage_do: &'static str,
    pub cli_usage_watch: &'static str,
    pub cli_usage_pomodoro: &'static str,
    pub msg_watch_start: &'static str,
    pub msg_watch_ok: &'static str,
    pub msg_watch_fail: &'static str,

    // --- HTTP (the protocol is English; these are displayed) ---
    pub msg_http_on: &'static str,
    pub msg_http_off: &'static str,
    pub msg_http_fail: &'static str,
    pub http_err_bad: &'static str,
    pub http_err_token: &'static str,
    pub http_err_not_found: &'static str,
}
