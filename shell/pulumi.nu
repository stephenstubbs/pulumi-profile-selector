export def --env load_pulumi_profile [] {
    if $env.PULUMI_BACKEND_URL_CURRENT_SHELL? in [true "true" 1 "1" "yes"] {
        return
    }
    let backend = (^pulumi-profile-selector --show-current --json | from json | get backend)
    if $backend == null {
        hide-env --ignore-errors PULUMI_BACKEND_URL
    } else {
        $env.PULUMI_BACKEND_URL = $backend
    }
}

def option-flags [args: list<string>] {
    $args | each {|arg|
        if ($arg | str starts-with "--") {
            [($arg | split row "=" | get 0)]
        } else if ($arg | str starts-with "-") {
            mut flags = []
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
            $flags
        } else {
            []
        }
    } | flatten
}

export def --env --wrapped pulumips [
    ...args: string
] {
    let flags = (option-flags $args)
    let current = "--current" in $flags
    let informational = ("--help" in $flags) or ("--version" in $flags)
    if $current and not $informational {
        let output_args = if "--json" in $flags { $args } else { $args | append "--json" }
        let backend = (^pulumi-profile-selector ...$output_args | from json | get backend)
        if $backend == null {
            hide-env --ignore-errors PULUMI_BACKEND_URL
        } else {
            $env.PULUMI_BACKEND_URL = $backend
        }
        $env.PULUMI_BACKEND_URL_CURRENT_SHELL = true
    } else {
        ^pulumi-profile-selector ...$args
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
