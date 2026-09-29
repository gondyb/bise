#!/bin/sh
# install.sh — PROTOTYPE installer of the harness + Switchboard.
#
#   sh install.sh [--from <tarball|bundle dir>] [--prefix <dir>] [--bin-dir <dir>]
#                 [--no-modify-path] [--keep <n>]
#   sh install.sh --uninstall [--prefix <dir>] [--bin-dir <dir>] [--purge]
#
# Run from an extracted bundle (app/ next to this file), it installs that
# bundle; --from takes a tarball or a bundle dir. (The published form
# would be `curl -fsSL <url>/install.sh | sh`: it downloads the tarball
# of the platform from BISE_DIST_URL - nothing is published yet.)
#
# Layout (the command is `bise` since BISE-165; the prefix moves to
# ~/.local/share/bise with BISE-170):
#   $PREFIX/versions/<id>/   immutable app roots (the versions.sh layout)
#   $PREFIX/current -> versions/<id>
#   $PREFIX/bin/bise         the launcher (sh): --version, init, uninstall,
#                            then exec the current app root's binary
#   $PREFIX/install.sh       a copy of this file (uninstall, reinstall)
#   $BIN_DIR/bise -> $PREFIX/bin/bise
#   $BIN_DIR/bend-harness -> $PREFIX/bin/bise   (the old name, one release)
# Defaults: PREFIX=~/.local/share/bend-harness, BIN_DIR=~/.local/bin.
# User data is never inside $PREFIX: ~/.bend-harness (keys, config,
# sessions) and ~/.local/state/switchboard (hubs) survive an uninstall
# unless --purge.

set -eu

CMD=bise                  # the command name (BISE-165)
OLD_CMD=bend-harness      # its old name: a second link, kept one release
PREFIX="${BISE_PREFIX:-$HOME/.local/share/bend-harness}"
BIN_DIR="${BISE_BIN_DIR:-$HOME/.local/bin}"
FROM=""
MODIFY_PATH=1
KEEP=3
ACTION=install
PURGE=0
MARK="# added by the $CMD installer"
OLD_MARK="# added by the $OLD_CMD installer"   # before BISE-165: same PATH line

say() { printf '%s\n' "$CMD install: $*" >&2; }
die() { say "error: $*"; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --from) FROM="$2"; shift ;;
    --prefix) PREFIX="$2"; shift ;;
    --bin-dir) BIN_DIR="$2"; shift ;;
    --no-modify-path) MODIFY_PATH=0 ;;
    --keep) KEEP="$2"; shift ;;
    --uninstall) ACTION=uninstall ;;
    --purge) PURGE=1 ;;
    -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
  shift
done

# ---- the shell rc files the PATH line goes to ----
rc_files() {
  case "$(basename "${SHELL:-/bin/sh}")" in
    zsh) echo "${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) echo "$HOME/.bashrc"; [ "$(uname -s)" = Darwin ] && echo "$HOME/.bash_profile" ;;
    fish) echo "$HOME/.config/fish/conf.d/$CMD.fish" ;;
    *) echo "$HOME/.profile" ;;
  esac
}

# the hubs running from this prefix (their app root is a version dir here)
running_hubs() {
  ps -axo pid=,command= 2>/dev/null | grep -F "$PREFIX/versions/" | grep -F " sbd " | grep -v grep || true
}

# ---- uninstall ----
if [ "$ACTION" = uninstall ]; then
  hubs="$(running_hubs)"
  if [ -n "$hubs" ]; then
    say "stopping the Switchboard hubs of this install (agents too):"
    printf '%s\n' "$hubs" >&2
    printf '%s\n' "$hubs" | while read -r pid _; do kill "$pid" 2>/dev/null || true; done
  fi
  for c in "$CMD" "$OLD_CMD"; do [ -L "$BIN_DIR/$c" ] && rm -f "$BIN_DIR/$c"; done
  rm -f "$PREFIX/bin/$OLD_CMD"
  rm -rf "$PREFIX"
  for f in $(rc_files); do
    [ -f "$f" ] || continue
    if grep -qF -e "$MARK" -e "$OLD_MARK" "$f"; then
      { grep -vF -e "$MARK" -e "$OLD_MARK" "$f" || true; } > "$f.tmp.$$"
      mv "$f.tmp.$$" "$f"
      say "PATH line removed from $f"
    fi
  done
  if [ "$PURGE" = 1 ]; then
    rm -rf "$HOME/.bend-harness" "${XDG_STATE_HOME:-$HOME/.local/state}/switchboard"
    say "purged ~/.bend-harness (keys, config, sessions) and the Switchboard state"
  else
    say "kept your data: ~/.bend-harness and ~/.local/state/switchboard (--purge removes them)"
  fi
  say "uninstalled"
  exit 0
fi

# ---- install: find the bundle ----
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/$CMD-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT INT TERM
if [ -z "$FROM" ]; then
  if [ -d "$here/app" ]; then
    FROM="$here"
  elif [ -n "${BISE_DIST_URL:-}" ]; then
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"
    [ "$arch" = aarch64 ] && arch=arm64
    say "downloading $BISE_DIST_URL/latest-$os-$arch.tar.gz"
    curl -fsSL "$BISE_DIST_URL/latest-$os-$arch.tar.gz" -o "$tmp/dl.tar.gz" || die "download failed"
    curl -fsSL "$BISE_DIST_URL/latest-$os-$arch.tar.gz.sha256" -o "$tmp/dl.sha" || die "checksum download failed"
    [ "$(shasum -a 256 "$tmp/dl.tar.gz" | cut -d' ' -f1)" = "$(cut -d' ' -f1 "$tmp/dl.sha")" ] || die "checksum mismatch"
    FROM="$tmp/dl.tar.gz"
  else
    die "no bundle: run from an extracted bundle, or pass --from <tarball|dir>"
  fi
fi
if [ -f "$FROM" ]; then
  if [ -f "$FROM.sha256" ]; then
    [ "$(shasum -a 256 "$FROM" | cut -d' ' -f1)" = "$(cut -d' ' -f1 "$FROM.sha256")" ] || die "checksum mismatch: $FROM"
  fi
  tar -C "$tmp" -xzf "$FROM" || die "cannot extract $FROM"
  FROM="$(find "$tmp" -mindepth 2 -maxdepth 2 -type d -name app | head -n 1 | xargs dirname)"
fi
app="$FROM/app"
# the command: bise, or bend-harness in a bundle built before BISE-165
[ -e "$app/bise" ] || [ -e "$app/bend-harness" ] \
  || die "incomplete bundle: app/bise missing in $FROM (a download cut short, or removed by security software)"
for f in repl-live sb-core VERSION; do
  [ -e "$app/$f" ] || die "incomplete bundle: app/$f missing in $FROM (a download cut short, or removed by security software)"
done
# the V8 engine: app/bend-jsrt, or its path before BISE-114
[ -e "$app/bend-jsrt" ] || [ -e "$app/rust/jsrt/target/debug/bend-jsrt" ] \
  || die "incomplete bundle: app/bend-jsrt missing in $FROM (a download cut short, or removed by security software)"

id="$(sed -n 's/^id=//p' "$app/VERSION")"
target="$(sed -n 's/^target=//p' "$app/VERSION")"
os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
[ -z "$target" ] || [ "$target" = "$os-$arch" ] || die "this bundle is for $target, this machine is $os-$arch"
command -v git >/dev/null 2>&1 || say "warning: git not found (Switchboard needs it for worktrees and /version)"

# ---- the version dir: immutable, written once, then the pointer flips ----
mkdir -p "$PREFIX/versions" "$PREFIX/bin"
if [ -x "$PREFIX/versions/$id/bise" ] || [ -x "$PREFIX/versions/$id/bend-harness" ]; then
  say "version $id already installed"
else
  rm -rf "$PREFIX/versions/.$id.tmp"
  cp -R "$app" "$PREFIX/versions/.$id.tmp"
  # a browser download carries the quarantine flag; Gatekeeper would
  # block the unsigned binaries (curl does not set it)
  [ "$os" = darwin ] && xattr -dr com.apple.quarantine "$PREFIX/versions/.$id.tmp" 2>/dev/null || true
  mv "$PREFIX/versions/.$id.tmp" "$PREFIX/versions/$id"
fi
# (BSD mv onto a symlink to a dir moves INTO the dir: no rename trick,
# rm + ln; the launcher resolves 'current' once, at start)
rm -f "$PREFIX/current"
ln -s "versions/$id" "$PREFIX/current"
cp "$0" "$PREFIX/install.sh" 2>/dev/null || true

# ---- the launcher ----
q_prefix="$(printf '%s' "$PREFIX" | sed "s/'/'\\\\''/g")"
cat > "$PREFIX/bin/$CMD.tmp" <<EOF
#!/bin/sh
# $CMD launcher, written by install.sh. Runs the CURRENT version from
# its real (immutable) dir: a hub records that dir, and an update that
# flips 'current' never changes a running hub under it.
PREFIX='$q_prefix'
EOF
cat >> "$PREFIX/bin/$CMD.tmp" <<'EOF'
CMD="$(basename "$0")"
root="$(cd "$PREFIX/current" 2>/dev/null && pwd -P)" || { echo "$CMD: no version installed in $PREFIX" >&2; exit 1; }
ver() { sed -n "s/^$1=//p" "$root/VERSION"; }

has_key() {
  for k in MISTRAL_API_KEY ANTHROPIC_FOUNDRY_API_KEY; do
    eval "v=\${$k:-}"; [ -n "$v" ] && return 0
    for f in "$HOME/.bend-harness/.env" "$HOME/.vibe/.env"; do
      [ -f "$f" ] && grep -Eq "^(export )?$k=.+" "$f" && return 0
    done
  done
  return 1
}

# first run: the API key, in ~/.bend-harness/.env (mode 600), and a
# public default model (the built-in default is a private proxy)
init() {
  mkdir -p "$HOME/.bend-harness"; chmod 700 "$HOME/.bend-harness"
  env_file="$HOME/.bend-harness/.env"
  key="${MISTRAL_API_KEY:-}"
  if [ -z "$key" ]; then
    [ -t 0 ] || { echo "$CMD init: no terminal; run: MISTRAL_API_KEY=... $CMD init" >&2; return 1; }
    printf 'Mistral API key (https://console.mistral.ai/api-keys): ' >&2
    stty -echo 2>/dev/null; read -r key; stty echo 2>/dev/null; echo >&2
  fi
  [ -n "$key" ] || { echo "$CMD init: no key given" >&2; return 1; }
  touch "$env_file"; chmod 600 "$env_file"
  grep -vE '^(export )?MISTRAL_API_KEY=' "$env_file" > "$env_file.tmp" || true
  printf 'MISTRAL_API_KEY=%s\n' "$key" >> "$env_file.tmp"
  mv "$env_file.tmp" "$env_file"; chmod 600 "$env_file"
  cfg="$HOME/.bend-harness/config.toml"
  if [ ! -f "$cfg" ]; then
    printf '# written by %s init\nmodel = "%s"\n' "$CMD" "${BISE_DEFAULT_MODEL:-mistral-medium-latest}" > "$cfg"
  fi
  echo "$CMD: key saved in $env_file; config in $cfg" >&2
}

case "${1:-}" in
  --version|-V|version)
    echo "$CMD $(ver id) ($(ver target), commit $(ver commit | cut -c1-12), built $(ver built))"
    exit 0 ;;
  init) init; exit $? ;;
  uninstall) shift; exec sh "$PREFIX/install.sh" --uninstall --prefix "$PREFIX" "$@" ;;
  update) echo "$CMD update: no release channel is published yet (see docs/packaging.md)" >&2; exit 1 ;;
  sb|sbd|sbswitch|keyprobe|--headless|--scripted) ;;
  *)
    # interactive first run without any key: onboard, then go on
    if [ -t 0 ] && [ -t 1 ] && ! has_key; then
      echo "$CMD: no API key found (env, ~/.bend-harness/.env, ~/.vibe/.env) - first-run setup" >&2
      init || exit 1
    fi ;;
esac
# the user's folder: the workspace of `switchboard`, and the bash/patch
# tools' directory of a single session (the binary moves to its app root)
export SB_LAUNCH_DIR="${SB_LAUNCH_DIR:-$PWD}"
export BEND_WORKDIR="${BEND_WORKDIR:-$PWD}"
# bise; a version installed before BISE-165 has bend-harness only
[ -x "$root/bise" ] && exec "$root/bise" "$@"
exec "$root/bend-harness" "$@"
EOF
chmod 755 "$PREFIX/bin/$CMD.tmp"
mv -f "$PREFIX/bin/$CMD.tmp" "$PREFIX/bin/$CMD"

mkdir -p "$BIN_DIR"
if [ -e "$BIN_DIR/$CMD" ] && [ ! -L "$BIN_DIR/$CMD" ]; then
  die "$BIN_DIR/$CMD exists and is not our link; remove it or pass --bin-dir"
fi
ln -sfn "$PREFIX/bin/$CMD" "$BIN_DIR/$CMD"
# the old name, for one release: the same launcher (it prints the name it
# was called by); an install before BISE-165 left a launcher file there
rm -f "$PREFIX/bin/$OLD_CMD"
if [ ! -e "$BIN_DIR/$OLD_CMD" ] || [ -L "$BIN_DIR/$OLD_CMD" ]; then
  ln -sfn "$PREFIX/bin/$CMD" "$BIN_DIR/$OLD_CMD"
fi

# ---- keep the last $KEEP versions, never the current one nor one a hub runs ----
in_use="$(ps -axo command= 2>/dev/null | grep -F "$PREFIX/versions/" | grep -v grep || true)"
n=0
for d in $(ls -t "$PREFIX/versions"); do
  case "$d" in .*) continue ;; esac
  n=$((n + 1))
  [ "$n" -le "$KEEP" ] && continue
  [ "$d" = "$id" ] && continue
  printf '%s' "$in_use" | grep -qF "$PREFIX/versions/$d/" && continue
  rm -rf "$PREFIX/versions/$d"
done

# ---- PATH ----
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    if [ "$MODIFY_PATH" = 1 ]; then
      for f in $(rc_files); do
        mkdir -p "$(dirname "$f")"
        grep -qF -e "$MARK" -e "$OLD_MARK" "$f" 2>/dev/null && continue
        case "$f" in
          *.fish) printf 'fish_add_path %s %s\n' "$BIN_DIR" "$MARK" >> "$f" ;;
          *) printf 'export PATH="%s:$PATH" %s\n' "$BIN_DIR" "$MARK" >> "$f" ;;
        esac
        say "added $BIN_DIR to PATH in $f (open a new terminal)"
      done
    else
      say "$BIN_DIR is not on your PATH: add it yourself"
    fi ;;
esac

say "installed $CMD $id in $PREFIX ($(du -sh "$PREFIX/versions/$id" | cut -f1))"
say "next: '$CMD init' (API key), then '$CMD' (a session) or '$CMD switchboard' (in a project folder)"
