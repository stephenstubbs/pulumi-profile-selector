# Pulumi Profile Selector

An inline, fuzzy-searchable CLI for selecting and managing Pulumi backend profiles.
Supports Linux and macOS through Nix, with shell output for Nushell, Fish, and
POSIX shells.

Nix targets are `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`.
Current nixpkgs no longer provides an Intel macOS (`x86_64-darwin`) stdenv.

## Installation

```sh
nix profile add github:stephenstubbs/pulumi-profile-selector
pulumi-profile-selector --help
```

Or run without installing:

```sh
nix run github:stephenstubbs/pulumi-profile-selector -- --help
```

To include it in another flake:

```nix
{
  inputs.pulumi-profile-selector = {
    url = "github:stephenstubbs/pulumi-profile-selector";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  # In your package list, for the appropriate system:
  # inputs.pulumi-profile-selector.packages.${system}.default
}
```

Without Nix, use Rust 1.89 or newer:

```sh
cargo install --path . --locked
```

Packages expose `pulumiProfileContextVersion = 1` as non-secret capability
metadata. Consumers can use the native context protocol and shell modules
without reapplying a legacy compatibility patch.

## Usage

```sh
pulumi-profile-selector                       # Interactive selection
pulumi-profile-selector --activate dev        # Activate a saved profile
pulumi-profile-selector --new s3://other-state # Use an unsaved backend
pulumi-profile-selector --deactivate          # Clear the persisted selection
pulumi-profile-selector --show-current        # Print the persisted backend URL

pulumi-profile-selector --add                 # Create a profile interactively
pulumi-profile-selector --edit dev            # Change its backend URL
pulumi-profile-selector --delete dev          # Also clears its persisted selection
pulumi-profile-selector --list
```

Short options: `-a` activate, `-n` new, `-d` deactivate, `-c` current, `-l` list.
Actions are mutually exclusive. Use arrow keys to navigate, type to filter,
Enter to select, or Esc/Ctrl-C to cancel. Cancellation exits unsuccessfully
without changing the selection.

### Profile identity context

`--context` adds the saved profile name to backend output, so two profiles that
point at the same backend stay distinguishable. It is opt-in: without it every
existing output keeps its previous shape.

```sh
pulumi-profile-selector --show-current --context --json
# {"profile":"company-a","backend":"https://api.pulumi.com"}
pulumi-profile-selector --show-current --context --json   # raw backend selection
# {"profile":null,"backend":"https://api.pulumi.com"}
pulumi-profile-selector --show-current --context --json   # nothing active
# {"profile":null,"backend":null}
```

`profile` is a saved profile name or `null`. A raw backend stays unnamed even
when its URL equals a saved profile's backend; identity is never inferred from
the URL. `--context` works with `--current`, `--show-current`, explicit
selection, interactive selection, `--new`, and current-shell deactivation.

### Current shell only

`--current` prints a shell command without changing persisted state. A child
process cannot directly change its parent shell's environment.

```sh
# Bash / Zsh / POSIX shell
eval "$(pulumi-profile-selector --current --activate dev --shell posix)"
eval "$(pulumi-profile-selector --current --deactivate --shell posix)"
```

Shell syntax is detected from the `SHELL` environment variable; use
`--shell posix`, `--shell fish`, or `--shell nu` to override the login shell.
Backend values are quoted literally, including shell metacharacters. JSON
output takes precedence over `--shell`.

With `--context`, shell output also carries the non-secret profile identity. A
raw backend unsets it:

```sh
eval "$(pulumi-profile-selector --current --context --activate dev --shell posix)"
# export PULUMI_BACKEND_URL='s3://pulumi-state-dev'
# export PULUMI_PROFILE_SELECTOR_PROFILE='dev'

eval "$(pulumi-profile-selector --current --context --new s3://other --shell posix)"
# export PULUMI_BACKEND_URL='s3://other'
# unset PULUMI_PROFILE_SELECTOR_PROFILE

eval "$(pulumi-profile-selector --current --context --deactivate --shell posix)"
# unset PULUMI_BACKEND_URL
# unset PULUMI_PROFILE_SELECTOR_PROFILE
```

For integrations, prefer structured output instead of parsing shell commands:

```sh
pulumi-profile-selector --current --activate dev --json
# {"backend":"s3://pulumi-state-dev"}

pulumi-profile-selector --current --activate dev --context --json
# {"profile":"dev","backend":"s3://pulumi-state-dev"}

pulumi-profile-selector --current --deactivate --json
# {"backend":null}

pulumi-profile-selector --show-current --json
# {"backend":"s3://pulumi-state-dev"} (or null when inactive)
```

### Integrations

`shell/pulumi.nu` (Nushell) and `shell/pulumi.bash` (Bash) both provide
`pulumips` and `load_pulumi_profile`. They export only the
non-secret backend URL and profile identity, keep a current-shell override as a
snapshot of both values, and set `PULUMI_BACKEND_URL_CURRENT_SHELL=true` while an
override is active. Deactivation through `pulumips -c -d` unsets both variables
in the current shell; a persistent `pulumips -d` clears the saved selection.
The current-shell marker stays true after `pulumips -c -d`, preventing a prompt
hook from silently restoring the persisted company's selection.
Listing, editing or deleting profiles leaves a shell snapshot untouched, so the
`PULUMI_PROFILE_SELECTOR_PROFILE` name survives until the snapshot is replaced.

Bash sources the module and calls `load_pulumi_profile` at startup or from
`PROMPT_COMMAND`:

```sh
source /path/to/pulumi-profile-selector/shell/pulumi.bash
PROMPT_COMMAND="load_pulumi_profile${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
```

### Nushell integration

Keep this checkout available and add the module to `config.nu` using its
absolute path:

```nu
use /path/to/pulumi-profile-selector/shell/pulumi.nu *

$env.config.hooks.pre_prompt = (
    $env.config.hooks.pre_prompt | append {|| load_pulumi_profile}
)
```

This appends a hook without replacing other hooks. The single pre-prompt hook
also covers directory changes, so a separate `PWD` hook is unnecessary.

```nu
pulumips -a dev          # Persist and immediately load dev
pulumips -c -a prod      # Override only this shell
pulumips -c -d           # Unset only this shell, including subsequent prompts
pulumips -n file://./state
pulumips -d
pulumips --help
```

The wrapper and hook use JSON to preserve backend values exactly. A current-shell
override is a snapshot of the backend URL and profile identity, independent of
saved profiles. Listing, editing, or deleting profiles does not change that
snapshot. It survives prompt reloads until `pulumips -c -d` unsets it or a
persistent activation/deactivation through `pulumips` resumes saved-state
loading. Both hooks export metadata only.

## Profile files

Files live in `$PULUMI_HOME`, or `~/.pulumi` when it is unset or empty.
`profiles.json` is an array:

```json
[
  {"name": "dev", "backend": "s3://pulumi-state-dev"},
  {"name": "prod", "backend": "s3://pulumi-state-prod"},
  {"name": "local", "backend": "file://./state"}
]
```

`current_profile` contains the selected profile name as plain text. `--new`
can also store a raw backend URL or unsaved name; the integration resolves a
saved name to its backend and otherwise uses the stored value literally.
Both files remain compatible with earlier versions.

Names must be unique. Names and backends must be nonempty, with no control
characters or leading/trailing whitespace. Read-only commands do not create
configuration files. Writes replace files atomically rather than truncating
them; invalid JSON is reported instead of being silently replaced.

The files hold names and backend URLs only; the schema is unchanged and stays
compatible with earlier versions. The persistent selection stores a profile name
(so a later backend edit is followed) or a raw backend value; deleting the active
profile clears it. A current-shell snapshot differs by design: it keeps the
identity and backend it was given, even if that profile is later edited or
deleted, until another selection replaces it.

These profiles are managed by this tool, not Pulumi's credentials file.
Persistent selection takes effect through shell integration, not by modifying
the Pulumi CLI's login state.

## Secrets

This tool is a non-secret backend/profile selector. It never stores, requests,
prints, or reads Pulumi access tokens, stack-encryption passphrases, or
`credentials.json`; there is no secret or 1Password integration in it. Both
secret types belong to external consumers that select a profile here and then
load the matching credential themselves. Only the backend URL and the saved
profile name are ever exported.

## Development

From a checkout:

```sh
direnv allow
direnv reload
cargo build --locked
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo deny --locked check
cargo machete
nu --no-config-file tests/nushell.nu target/debug/pulumi-profile-selector
bash tests/bash.sh target/debug/pulumi-profile-selector
python3 tests/interactive.py target/debug/pulumi-profile-selector
nix flake check
```

The interactive check uses private PTYs and fictional profiles for both shells.
It verifies a visible menu, current and persistent selection, and Escape/Ctrl-C
cancellation without changing the previous selection. Every case has a deadline.

The flake uses the same latest stable Rust toolchain for development and
packaging. Its lockfile pins that toolchain and Nix packages reproducibly.
To refresh dependencies deliberately:

```sh
nix flake update
direnv reload
cargo upgrade --incompatible
cargo update
```

## License

MIT
