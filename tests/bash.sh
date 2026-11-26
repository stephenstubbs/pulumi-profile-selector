#!/usr/bin/env bash
# Shell-level checks for shell/pulumi.bash.
# Usage: bash tests/bash.sh <selector-binary>
set -euo pipefail

selector=${1:?usage: bash tests/bash.sh <selector-binary>}
selectordir=$(cd "$(dirname "$0")/.." && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home" "$work/pulumi"
ln -s "$(cd "$(dirname "$selector")" && pwd)/$(basename "$selector")" \
    "$work/bin/pulumi-profile-selector"
PATH=$work/bin:$PATH
export PATH HOME=$work/home PULUMI_HOME=$work/pulumi

cat >"$PULUMI_HOME/profiles.json" <<'JSON'
[
  {"name": "company-a", "backend": "https://api.pulumi.com"},
  {"name": "company-b", "backend": "https://api.pulumi.com"},
  {"name": "team-'$(touch injected)`id` \"spaced\"", "backend": "file:///tmp/a'b\"c $(touch injected) `id` ${HOME}"}
]
JSON

adversarial_name="team-'\$(touch injected)\`id\` \"spaced\""
adversarial_backend="file:///tmp/a'b\"c \$(touch injected) \`id\` \${HOME}"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

check_eq() {
    [[ $1 == "$2" ]] || fail "$3 (expected '$2', got '$1')"
}

cd "$selectordir"
# shellcheck source=shell/pulumi.bash
source shell/pulumi.bash
cd "$work"

pulumips -a company-a
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "persistent activation identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "persistent activation backend"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" "" "persistent activation kept no marker"

pulumips -a company-b
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-b "identical backend kept identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "identical backend kept the URL"

check_eq "$(pulumi-profile-selector --show-current --json)" \
    '{"backend":"https://api.pulumi.com"}' "legacy JSON output"
check_eq "$(pulumi-profile-selector --show-current --context --json)" \
    '{"profile":"company-b","backend":"https://api.pulumi.com"}' "context JSON output"

pulumips -a company-a
pulumips -c -a company-b
pulumips -l >/dev/null
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-b "short list replaced snapshot identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "short list replaced snapshot backend"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "short list cleared snapshot marker"
check_eq "$(cat "$PULUMI_HOME/current_profile")" company-a "short list changed persisted selection"

pulumips -n https://api.pulumi.com
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "raw backend loaded"
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "" "raw backend stayed unnamed"

pulumips -c -a company-a
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "session override identity"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "session override marker"

# A failed persistent activation keeps snapshot B while A stays persisted.
pulumips -c -a company-b
if pulumips -a missing 2>/dev/null; then
    fail "failed persistent activation succeeded"
fi
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-b "failed activation replaced snapshot identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "failed activation changed snapshot backend"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "failed activation cleared the marker"

pulumips -c -a company-a
pulumips --delete=company-b
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "delete preserved snapshot identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "delete preserved snapshot backend"
pulumips --list >/dev/null
load_pulumi_profile
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "hook preserved snapshot identity"

pulumips -c -a company-a --shell fish --json
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "caller output flags were normalised"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "normalised override kept marker"
if pulumips -c -- --list 2>/dev/null; then
    fail "a literal argument after the terminator was interpreted as an option"
fi
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "invalid terminated args changed identity"

pulumips -c -a "$adversarial_name"
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "$adversarial_name" "adversarial identity was literal"
check_eq "${PULUMI_BACKEND_URL-}" "$adversarial_backend" "adversarial backend was literal"
[[ ! -e injected ]] || fail "context commands evaluated metacharacters"

pulumips -c -d
check_eq "${PULUMI_BACKEND_URL-}" "" "session deactivation cleared the backend"
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "" "session deactivation cleared identity"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "session deactivation kept the marker"
load_pulumi_profile
check_eq "${PULUMI_BACKEND_URL-}" "" "hook did not revive a deactivated session"

pulumips -d
pulumips -a company-a
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "reactivation identity"
if pulumips -c -a missing 2>/dev/null; then
    fail "missing profile selection succeeded"
fi
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "failed selection kept identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "failed selection kept the backend"

# Without an override, a failed persistent activation keeps the identity.
if pulumips -a missing 2>/dev/null; then
    fail "failed plain activation succeeded"
fi
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "failed plain activation changed identity"
check_eq "${PULUMI_BACKEND_URL-}" https://api.pulumi.com "failed plain activation changed the backend"

# A broken store clears both variables when there is no override.
cp "$PULUMI_HOME/profiles.json" "$work/good.json"
printf '{broken' >"$PULUMI_HOME/profiles.json"
load_pulumi_profile 2>/dev/null
check_eq "${PULUMI_BACKEND_URL-}" "" "broken store kept the backend"
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "" "broken store kept identity"

# An explicit override survives the same broken store.
cp "$work/good.json" "$PULUMI_HOME/profiles.json"
pulumips -c -a "$adversarial_name"
printf '{broken' >"$PULUMI_HOME/profiles.json"
load_pulumi_profile 2>/dev/null
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "$adversarial_name" "broken store replaced override identity"
check_eq "${PULUMI_BACKEND_URL-}" "$adversarial_backend" "broken store replaced override backend"
check_eq "${PULUMI_BACKEND_URL_CURRENT_SHELL-}" true "broken store cleared the override marker"
cp "$work/good.json" "$PULUMI_HOME/profiles.json"

pulumips -ca company-a
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "combined short flags set identity"
pulumips -ch >/dev/null
load_pulumi_profile
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" company-a "help kept the session override"

pulumips -d
check_eq "${PULUMI_BACKEND_URL-}" "" "persistent deactivation cleared the backend"
check_eq "${PULUMI_PROFILE_SELECTOR_PROFILE-}" "" "persistent deactivation cleared identity"

printf 'Bash integration tests passed\n'
