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
#   ./run.sh --resume ID    # reprendre une session par id
#                           # (un préfixe unique suffit) ; /status dans
#                           # le TUI affiche l'id de la session
#
# Plusieurs terminaux = plusieurs ./run.sh : ports auto-attribués,
# sessions indépendantes, chaque REPL meurt avec son terminal.

set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

# rebuild when the binary is missing OR stale (a source file is newer
# than it — a stale debug binary once showed a model the runtime no
# longer used)
if [ ! -x rust/target/debug/bend-harness ] \
   || [ -n "$(find rust/harness/src rust/tui/src -newer rust/target/debug/bend-harness -print -quit 2>/dev/null)" ]; then
  echo "bend-harness absent ou périmé — build cargo..." >&2
  (cd rust && cargo build -p bend-harness)
fi

if [ ! -x rust/jsrt/target/debug/bend-jsrt ]; then
  echo "bend-jsrt absent — build du moteur V8 (premiere fois: quelques minutes)...">&2
  (cd rust/jsrt && cargo build)
fi

if [ ! -x repl-live ]; then
  echo "repl-live absent — compilation avec bend (1-2 min)..." >&2
  export PATH="$HOME/.bend/bin:$PATH"
  bend runtime/repl-live.bend -o repl-live
fi
if [ ! -x repl-scripted ]; then
  echo "repl-scripted absent — compilation avec bend..." >&2
  export PATH="$HOME/.bend/bin:$PATH"
  bend runtime/repl.bend -o repl-scripted
fi

exec ./rust/target/debug/bend-harness "$@"
