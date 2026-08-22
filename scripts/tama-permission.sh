#!/bin/sh
# Hook PermissionRequest do Claude Code: o pedido de permissão vira uma
# pergunta no tama e você decide de lá. Sem resposta (timeout, esc, tama
# fechado) o script não emite decisão nenhuma — o diálogo normal do Claude
# Code aparece. Fail-open por construção.
#
# TAMA_CMD aponta para outro tama, ex: TAMA_CMD="ssh tamafone tama".
TAMA_CMD="${TAMA_CMD:-tama}"

input=$(cat)
# ponytail: sed em JSON aninhado — só para exibição; jq se precisar de fidelidade
field() {
    printf '%s' "$input" | sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" | head -n1
}
tool=$(field tool_name)
detail=$(field command)
[ -n "$detail" ] || detail=$(field file_path)
detail=$(printf '%s' "$detail" | cut -c1-160)

ans=$($TAMA_CMD ask "claude quer usar ${tool:-?}: $detail" \
    --options permitir --options negar --options "decidir no claude" \
    --from claude --timeout 60s --default "decidir no claude")

case "$ans" in
permitir)
    printf '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}\n'
    ;;
negar)
    printf '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"negado pelo usuário via tama"}}}\n'
    ;;
*) ;; # sem decisão -> o prompt aparece no Claude Code
esac
