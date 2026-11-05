export PATH="$HOME/.local/bin:$PATH"
export STARSHIP_LOG=error
nix-fetch

# The spinning flake pauses during a command and resumes at the prompt.
autoload -Uz add-zsh-hook
nix_spin_file="${XDG_RUNTIME_DIR:-/tmp}/nix-spin-$$"
nix_spin_pause() {
  [[ -r $nix_spin_file ]] || return 0
  local pid rest
  read -r pid rest < "$nix_spin_file"
  kill -USR1 "$pid" 2>/dev/null
  sleep 0.06
}
nix_spin_resume() {
  [[ -r $nix_spin_file ]] && nix-fetch --resume
}
add-zsh-hook preexec nix_spin_pause
add-zsh-hook precmd nix_spin_resume
