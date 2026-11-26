# Pulumi profile selector integration for bash.
# Source this file to load saved selections into the current shell; it never
# handles credentials, only the backend URL and the saved profile identity.

_pulumi_selector_current=0
_pulumi_selector_informational=0
_pulumi_selector_management=0

_pulumi_scan_flags() {
    _pulumi_selector_current=0
    _pulumi_selector_informational=0
    _pulumi_selector_management=0
    local arg chars index char
    for arg in "$@"; do
        case $arg in
            --) break ;;
            --current) _pulumi_selector_current=1 ;;
            --help|--version) _pulumi_selector_informational=1 ;;
            --add|--edit|--delete|--list|--show-current) _pulumi_selector_management=1 ;;
            --edit=*|--delete=*) _pulumi_selector_management=1 ;;
            --*) ;;
            -?*)
                chars=${arg#-}
                for ((index = 0; index < ${#chars}; index++)); do
                    char=${chars:index:1}
                    case $char in
                        c) _pulumi_selector_current=1 ;;
                        l) _pulumi_selector_management=1 ;;
                        h|V) _pulumi_selector_informational=1 ;;
                        a|n) break ;;
                    esac
                done
                ;;
        esac
    done
}

# Drop caller-chosen output flags so this shell gets POSIX commands with context.
_pulumi_current_args() {
    local skip=0 literal=0 arg
    for arg in "$@"; do
        if ((literal)); then
            printf '%s\n' "$arg"
            continue
        fi
        if ((skip)); then
            skip=0
            continue
        fi
        case $arg in
            --) literal=1; printf '%s\n' "$arg" ;;
            --shell) skip=1 ;;
            --shell=*|--json|--context) ;;
            *) printf '%s\n' "$arg" ;;
        esac
    done
}

pulumips() {
    local -a args=()
    local arg output status
    _pulumi_scan_flags "$@"

    if ((_pulumi_selector_current)) && ((!_pulumi_selector_informational)); then
        args=(--context --shell posix)
        while IFS= read -r arg; do
            args+=("$arg")
        done < <(_pulumi_current_args "$@")
        if output=$(pulumi-profile-selector "${args[@]}"); then
            status=0
        else
            status=$?
        fi
        if ((status != 0)); then
            return "$status"
        fi
        eval "$output"
        export PULUMI_BACKEND_URL_CURRENT_SHELL=true
        return 0
    fi

    if pulumi-profile-selector "$@"; then
        status=0
    else
        status=$?
    fi
    if ((status != 0)); then
        return "$status"
    fi
    ((_pulumi_selector_informational)) && return 0
    ((_pulumi_selector_management)) || unset PULUMI_BACKEND_URL_CURRENT_SHELL
    load_pulumi_profile
}

# Pre-prompt/startup hook: metadata only, and never over a session override.
load_pulumi_profile() {
    case ${PULUMI_BACKEND_URL_CURRENT_SHELL-} in
        true|1|yes) return 0 ;;
    esac
    local output
    if output=$(pulumi-profile-selector --show-current --context --shell posix); then
        eval "$output"
    else
        # A broken or missing store must not leave stale metadata behind.
        unset PULUMI_BACKEND_URL PULUMI_PROFILE_SELECTOR_PROFILE
    fi
    return 0
}
