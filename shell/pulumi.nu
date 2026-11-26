# Non-secret saved-profile identity, carried beside the backend URL.
export const PROFILE_VAR = "PULUMI_PROFILE_SELECTOR_PROFILE"

# Capture failures without capturing the interactive menu on stderr.
def --wrapped selector-record [...args: string] {
    do --capture-errors { ^pulumi-profile-selector ...$args } | from json
}

# Apply a {profile, backend} selection to the current shell.
export def --env apply_pulumi_selection [selection: record] {
    if $selection.backend == null {
        hide-env --ignore-errors PULUMI_BACKEND_URL $PROFILE_VAR
    } else {
        $env.PULUMI_BACKEND_URL = $selection.backend
        if $selection.profile == null {
            hide-env --ignore-errors $PROFILE_VAR
        } else {
            $env.PULUMI_PROFILE_SELECTOR_PROFILE = $selection.profile
        }
    }
}

export def --env load_pulumi_profile [] {
    if ($env.PULUMI_BACKEND_URL_CURRENT_SHELL? in [true "true" 1 "1" "yes"]) {
        return
    }
    # A broken or missing store must clear the metadata instead of leaving a
    # stale identity that would claim a fresh company.
    let selection = (try {
        selector-record --show-current --context --json
    } catch {
        null
    })
    if $selection == null {
        hide-env --ignore-errors PULUMI_BACKEND_URL $PROFILE_VAR
        return
    }
    apply_pulumi_selection $selection
}

def option-flags [args: list<string>] {
    mut flags = []
    for arg in $args {
        if $arg == "--" {
            break
        }
        if ($arg | str starts-with "--") {
            $flags = ($flags | append ($arg | split row "=" | get 0))
        } else if ($arg | str starts-with "-") {
            for flag in ($arg | str substring 1.. | split chars) {
                let name = match $flag {
                    "c" => "--current"
                    "l" => "--list"
                    "h" => "--help"
                    "V" => "--version"
                    _ => $"-($flag)"
                }
                $flags = ($flags | append $name)
                if $flag in ["a" "n"] {
                    break
                }
            }
        }
    }
    $flags
}

export def --env --wrapped pulumips [
    ...args: string
] {
    let flags = (option-flags $args)
    let current = "--current" in $flags
    let informational = ("--help" in $flags) or ("--version" in $flags)
    if $current and not $informational {
        let extra = (
            (if "--context" in $flags { [] } else { ["--context"] })
            ++ (if "--json" in $flags { [] } else { ["--json"] })
        )
        let selection = (selector-record ...($extra ++ $args))
        apply_pulumi_selection $selection
        $env.PULUMI_BACKEND_URL_CURRENT_SHELL = true
    } else {
        do --capture-errors { ^pulumi-profile-selector ...$args }
        if $informational {
            return
        }
        let management = ($flags | any {|flag|
            $flag in ["--add" "--edit" "--delete" "--list" "--show-current"]
        })
        if not $management {
            hide-env --ignore-errors PULUMI_BACKEND_URL_CURRENT_SHELL
        }
        load_pulumi_profile
    }
}
