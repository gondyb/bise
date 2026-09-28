#!/usr/bin/env bash
# Wrapper de commodité : tout se lance via l'exécutable unique
# rust/target/debug/bend-harness (REPL Bend enfant + TUI). Le provider
# (HTTPS vers api.mistral.ai) et le tool bash tournent DANS le REPL Bend
# (packages hub HTTP + snap) — plus aucun bridge.
#
#   ./run.sh                 # session live (opus-5.5 + bash)
#   ./run.sh --scripted      # session scriptée
#   ./run.sh --model NOM     # modèle du provider
#   ./run.sh --port N        # forcer le port du REPL
#   ./run.sh --debug        # afficher les annotations (tours, idle)
#   ./run.sh --continue     # reprendre la session la plus récente
#                           # (par dernière activité, pas un fichier fixe)
#   ./run.sh switchboard    # Switchboard : main + tâches, dans le dossier courant
#   ./run.sh switchboard --stop  # arrêter le hub du dossier courant
#   ./run.sh switchboard --dev   # Switchboard de test à côté du live : build
#                                # isolé dans /tmp/sb-dev, hub séparé
#                                # (voir sb-dev.sh : --no-tui, --stop, --status, --reset)
#   ./run.sh --resume ID    # reprendre une session par id
#                           # (un préfixe unique suffit) ; /status dans
#                           # le TUI affiche l'id de la session
#
# Plusieurs terminaux = plusieurs ./run.sh : ports auto-attribués,
# sessions indépendantes, chaque REPL meurt avec son terminal.

set -euo pipefail
# the dev switchboard builds elsewhere: never the live tree's binaries
if [ "${1:-}" = "switchboard" ] && [[ " $* " == *" --dev "* ]]; then
  shift
  exec "$(dirname "$0")/sb-dev.sh" "$@"
fi
# switchboard: the workspace is where the user launched from
export SB_LAUNCH_DIR="${SB_LAUNCH_DIR:-$PWD}"
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

# rebuild when the binary is missing OR stale (a source file is newer
# than it — a stale debug binary once showed a model the runtime no
# longer used)
if [ ! -x rust/target/debug/bend-harness ] \
   || [ -n "$(find rust/harness/src rust/tui/src rust/switchboard/src -newer rust/target/debug/bend-harness -print -quit 2>/dev/null)" ]; then
  echo "bend-harness absent ou périmé — build cargo..." >&2
  (cd rust && cargo build -p bend-harness)
fi

if [ ! -x rust/jsrt/target/debug/bend-jsrt ]; then
  echo "bend-jsrt absent — build du moteur V8 (premiere fois: quelques minutes)...">&2
  (cd rust/jsrt && cargo build)
fi

# the Bend REPLs: rebuilt when absent OR older than any Bend source
# (runtime/, core/, the tool descriptions). A failed rebuild keeps the
# existing binary when there is one (no toolchain: still runnable).
export PATH="$HOME/.bend/bin:$PATH"
bend_stale() {
  [ ! -x "$1" ] || [ -n "$(find runtime core tool-desc-*.txt -newer "$1" -print -quit 2>/dev/null)" ]
}
build_repl() {
  local out="$1" src="$2"
  if bend_stale "$out"; then
    echo "$out absent ou périmé — compilation avec bend (1-2 min)..." >&2
    if ! bend "$src" -o "$out" >/dev/null; then
      if [ -x "$out" ]; then
        echo "compilation de $out échouée — binaire existant conservé" >&2
      else
        echo "compilation de $out échouée" >&2
        exit 1
      fi
    fi
  fi
}
build_repl repl-live runtime/repl-live.bend
build_repl repl-scripted runtime/repl.bend

exec ./rust/target/debug/bend-harness "$@"
