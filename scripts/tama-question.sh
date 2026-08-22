#!/bin/sh
# Hook PreToolUse (matcher AskUserQuestion) do Claude Code: as perguntas de
# múltipla escolha do Claude aparecem no tama e a escolha volta como
# deny+reason — o Claude lê o reason como a resposta do usuário e segue.
# multiSelect vira escolha única (limitação aceita).
#
# Requer jq (as opções vêm em arrays aninhados). Sem jq, timeout, esc ou
# "responder no claude": nenhum output -> a pergunta aparece no harness.
#
# TAMA_CMD aponta para outro tama, ex: TAMA_CMD="ssh tamafone tama".
TAMA_CMD="${TAMA_CMD:-tama}"
FALLBACK="responder no claude"

command -v jq >/dev/null 2>&1 || exit 0
input=$(cat)
n=$(printf '%s' "$input" | jq '.tool_input.questions | length' 2>/dev/null)
[ "$n" -gt 0 ] 2>/dev/null || exit 0

answers=""
i=0
while [ "$i" -lt "$n" ]; do
    q=$(printf '%s' "$input" | jq -r ".tool_input.questions[$i].question")
    header=$(printf '%s' "$input" | jq -r ".tool_input.questions[$i].header // \"pergunta\"")
    ans=$(printf '%s' "$input" | jq -r ".tool_input.questions[$i].options[].label" | {
        set --
        while IFS= read -r label; do
            set -- "$@" --opcoes "$label"
        done
        $TAMA_CMD ask "$q" "$@" --opcoes "$FALLBACK" \
            --de claude --timeout 120s --padrao "$FALLBACK"
    })
    case "$ans" in
    "$FALLBACK" | ignorada | "") exit 0 ;;
    esac
    answers="$answers$header: $ans; "
    i=$((i + 1))
done

# jq -Rs escapa o texto como string JSON com segurança
reason=$(printf '%s' "usuário respondeu via tama — $answers" | jq -Rs .)
printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":%s}}\n' "$reason"
