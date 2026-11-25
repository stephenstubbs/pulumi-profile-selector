use ../shell/pulumi.nu *

def check [condition: bool, message: string] {
    if not $condition {
        error make {msg: $message}
    }
}

def --env main [binary: string] {
    let binary = ($binary | path expand)
    let root = (mktemp --directory)
    $env.PATH = ($env.PATH | prepend ($binary | path dirname))
    $env.PULUMI_HOME = ([$root ".pulumi"] | path join)
    hide-env --ignore-errors PULUMI_BACKEND_URL PULUMI_BACKEND_URL_CURRENT_SHELL

    try {
        mkdir $env.PULUMI_HOME
        let profiles_file = ([$env.PULUMI_HOME "profiles.json"] | path join)
        let current_file = ([$env.PULUMI_HOME "current_profile"] | path join)
        let backend = r#'file:///tmp/a'"\$()`;state'#
        [
            {name: dev, backend: "s3://state-dev"}
            {name: prod, backend: "s3://state-prod"}
            {name: quoted, backend: $backend}
            {name: h, backend: "s3://state-h"}
            {name: c, backend: "s3://state-c"}
        ] | save $profiles_file

        pulumips -a dev
        check ($env.PULUMI_BACKEND_URL == "s3://state-dev") "Persistent activation failed"
        for marker in ["1" "yes"] {
            $env.PULUMI_BACKEND_URL_CURRENT_SHELL = $marker
            $env.PULUMI_BACKEND_URL = "s3://snapshot"
            load_pulumi_profile
            check ($env.PULUMI_BACKEND_URL == "s3://snapshot") "Truthy marker was not respected"
        }
        $env.PULUMI_BACKEND_URL_CURRENT_SHELL = "not-a-boolean"
        load_pulumi_profile
        check ($env.PULUMI_BACKEND_URL == "s3://state-dev") "Invalid marker prevented loading"

        pulumips -c -a quoted
        check ($env.PULUMI_BACKEND_URL == $backend) "JSON backend was not preserved"
        pulumips -c --json -a quoted
        check ($env.PULUMI_BACKEND_URL == $backend) "Explicit JSON failed through the wrapper"
        pulumips -c --shell nu -a quoted
        check ($env.PULUMI_BACKEND_URL == $backend) "Shell option failed through the wrapper"
        pulumips -cah
        check ($env.PULUMI_BACKEND_URL == "s3://state-h") "An attached profile value was parsed as help"
        pulumips -c -a quoted
        check ((open --raw $current_file) == "dev") "Current mode changed the saved profile"
        load_pulumi_profile
        check ($env.PULUMI_BACKEND_URL == $backend) "Prompt hook overwrote a session override"
        pulumips --list
        check ($env.PULUMI_BACKEND_URL == $backend) "Listing cleared a session override"
        pulumips -c --help
        check ($env.PULUMI_BACKEND_URL == $backend) "Help cleared a session override"
        for flag in ["-lh" "-lV" "-ch" "-cV"] {
            pulumips $flag
            check ($env.PULUMI_BACKEND_URL == $backend) "Combined help/version changed the session"
            check ($env.PULUMI_BACKEND_URL_CURRENT_SHELL == true) "Combined help/version cleared the marker"
        }
        let failed = (try { pulumips -c -a missing; false } catch { true })
        check $failed "Failed activation did not propagate an error"
        check ($env.LAST_EXIT_CODE != 0) "Failed activation lost its exit status"
        check ($env.PULUMI_BACKEND_URL == $backend) "Failed activation changed the session"

        pulumips -c -d
        load_pulumi_profile
        check ($env.PULUMI_BACKEND_URL? == null) "Prompt hook reactivated a deactivated session"
        check ((open --raw $current_file) == "dev") "Session deactivation changed saved state"

        pulumips -a prod
        check ($env.PULUMI_BACKEND_URL == "s3://state-prod") "Persistent activation did not reset override"
        pulumips -ca dev
        check ($env.PULUMI_BACKEND_URL == "s3://state-dev") "Combined short flags failed"
        check ((open --raw $current_file) == "prod") "Combined flags changed saved state"

        let code = (^pulumi-profile-selector -c -a quoted --shell nu)
        let round_trip = (^nu --no-config-file -c ($code + "; $env.PULUMI_BACKEND_URL") | str trim)
        check ($round_trip == $backend) "Nushell command quoting failed"

        pulumips --delete=quoted
        check ($env.PULUMI_BACKEND_URL == "s3://state-dev") "Equals-form management cleared the override"
        pulumips --delete=dev
        load_pulumi_profile
        check ($env.PULUMI_BACKEND_URL == "s3://state-dev") "Deleting a profile changed an independent session snapshot"

        pulumips -ac
        check ($env.PULUMI_BACKEND_URL == "s3://state-c") "An attached profile value was parsed as --current"
        check ((open --raw $current_file) == "c") "Attached activation was not persisted"

        pulumips -n file:///tmp/custom-state
        check ($env.PULUMI_BACKEND_URL == "file:///tmp/custom-state") "Unsaved backend was not loaded"
        pulumips -d
        check ($env.PULUMI_BACKEND_URL? == null) "Persistent deactivation failed"
        pulumips -d
        pulumips --help
        print "Nushell integration tests passed"
    } catch {|error|
        rm --recursive $root
        error make $error
    }
    rm --recursive $root
}
