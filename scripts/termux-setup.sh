#!/data/data/com.termux/files/usr/bin/bash
# Configuração básica do tama no Termux.
# Fase 1 (sem o código): instala pacotes, configura SSH e mostra como enviar o projeto.
# Fase 2 (com o código em ~/tama): compila e instala o binário.
set -e

REPO_DIR="$HOME/tama"

echo "==> pacotes (rust, git, openssh)"
pkg update -y
# upgrade é obrigatório: rust e o LLVM do sistema precisam estar em versões
# casadas, senão o rustc quebra com "cannot locate symbol LLVMGetNextGlobal"
pkg upgrade -y -o Dpkg::Options::=--force-confnew
pkg install -y rust git openssh

echo "==> ssh"
# senha é necessária para o primeiro acesso (depois use ssh-copy-id e chaves)
if [ ! -s "$HOME/.ssh/authorized_keys" ]; then
  echo "defina uma senha para o acesso ssh (pedida uma vez):"
  passwd
fi
pgrep -x sshd >/dev/null || sshd
# sobe o sshd automaticamente sempre que o Termux abrir
grep -q 'pgrep -x sshd' "$HOME/.bashrc" 2>/dev/null || \
  echo 'pgrep -x sshd >/dev/null || sshd' >> "$HOME/.bashrc"

# ~/.cargo/bin no PATH (onde o cargo install coloca o tama)
grep -q '.cargo/bin' "$HOME/.bashrc" 2>/dev/null || \
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$HOME/.bashrc"

echo "==> impedindo o Android de matar o Termux"
termux-wake-lock || true

USER_NAME=$(whoami)
IP=$(ip -4 addr 2>/dev/null | awk '/inet / && $2 !~ /^127/ {sub(/\/.*/, "", $2); print $2; exit}')
IP=${IP:-"<veja o IP do Wi-Fi em Configurações>"}

if [ -d "$REPO_DIR" ]; then
  echo "==> compilando o tama (primeira vez demora alguns minutos)"
  cd "$REPO_DIR"
  cargo install --path .
  echo
  echo "pronto! rode:  tama"
else
  echo
  echo "==> falta o código. No SEU PC, envie o projeto e rode o script de novo:"
  echo
  echo "    scp -P 8022 -r ~/Development/projects/tama $USER_NAME@$IP:~/tama"
  echo "    ssh -P 8022 $USER_NAME@$IP ~/tama/scripts/termux-setup.sh"
fi

echo
echo "==> conexão a partir do PC:"
echo "    ssh -p 8022 $USER_NAME@$IP"
echo
echo "==> alias sugerido no PC (~/.bashrc ou ~/.zshrc):"
echo "    alias tamafone='ssh -p 8022 $USER_NAME@$IP ~/.cargo/bin/tama'"
echo
echo "    tamafone say \"oi do PC!\" --de pc --tipo info"
echo "    tamafone ask \"deu certo?\" --opcoes sim,nao"
echo
echo "dica: use 'ssh-copy-id -p 8022 $USER_NAME@$IP' no PC para não digitar senha."
