use ../shell/pulumi.nu *

def check [condition: bool, message: string] {
    if not $condition {
        error make {msg: $message}
    }
}

def failed-exit [] {
    (($env.LAST_EXIT_CODE? | default 0) | into int) != 0
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
        let adversarial = "team-'$(touch injected)`id` \"spaced\""
        let adversarial_backend = "file:///tmp/a'b\"c $(touch injected) `id` ${HOME}"
        [
            {name: dev, backend: "s3://state-dev"}
            {name: prod, backend: "s3://state-prod"}
            {name: quoted, backend: $backend}
            {name: h, backend: "s3://state-h"}
            {name: c, backend: "s3://state-c"}
            {name: company-a, backend: "https://api.pulumi.com"}
            {name: company-b, backend: "https://api.pulumi.com"}
            {name: $adversarial, backend: $adversarial_backend}
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
        pulumips -c --context -a quoted
        check ($env.PULUMI_BACKEND_URL == $backend) "Context without explicit JSON failed"
        let terminated_failed = (try {pulumips -c -- --list; false} catch {true})
        check $terminated_failed "A literal after the terminator was interpreted as an option"
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

        # Context: identity stays distinct even when two backends are identical.
        pulumips -a company-a
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-a") "Activation lost profile identity"
        pulumips -a company-b
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Identical backend changed the URL"
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-b") "Identical backend lost identity"
        let shown = (^pulumi-profile-selector --show-current --context --json | from json)
        check ($shown.profile == "company-b") "Context JSON lost identity"
        check ($shown.backend == "https://api.pulumi.com") "Context JSON lost backend"
        let legacy = (^pulumi-profile-selector --show-current --json | from json)
        check (($legacy | columns) == ["backend"]) "Legacy JSON gained identity"

        pulumips -n https://api.pulumi.com
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Raw backend was not loaded"
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Raw backend claimed an identity"

        # A current-shell snapshot is independent of later profile edits.
        pulumips -c -a company-a
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-a") "Current override lost identity"
        check ($env.PULUMI_BACKEND_URL_CURRENT_SHELL == true) "Current override lost its marker"
        pulumips --delete=prod
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-a") "Management cleared snapshot identity"
        pulumips --list
        load_pulumi_profile
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-a") "Prompt hook replaced snapshot identity"
        pulumips -c -d
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Session deactivation kept identity"
        check ($env.PULUMI_BACKEND_URL? == null) "Session deactivation kept the backend"
        load_pulumi_profile
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Prompt hook restored a deactivated identity"

        # Adversarial identity is applied literally, never evaluated.
        pulumips -c -a $adversarial
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == $adversarial) "Adversarial identity was altered"
        check ($env.PULUMI_BACKEND_URL == $adversarial_backend) "Adversarial backend was altered"
        pulumips -c -d
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Adversarial deactivation kept identity"

        # A failed persistent activation keeps the whole current-shell snapshot.
        pulumips -a company-a
        pulumips -c -a company-b
        pulumips -l
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-b") "Short list replaced snapshot identity"
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Short list replaced snapshot backend"
        check ($env.PULUMI_BACKEND_URL_CURRENT_SHELL == true) "Short list cleared snapshot marker"
        check ((open --raw $current_file) == "company-a") "Short list changed persisted state"
        let snapshot_failed = (try { pulumips -a missing; false } catch { true })
        check $snapshot_failed "Failed persistent activation reported success"
        check (failed-exit) "Failed persistent activation lost its exit status"
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-b") "Failed activation replaced the snapshot identity"
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Failed activation changed the snapshot backend"
        check ($env.PULUMI_BACKEND_URL_CURRENT_SHELL == true) "Failed activation cleared the marker"

        # Without an override, a failed activation leaves the persisted identity.
        pulumips -d
        pulumips -a company-a
        let plain_failed = (try { pulumips -a missing; false } catch { true })
        check $plain_failed "Failed plain activation reported success"
        check (failed-exit) "Failed plain activation lost its exit status"
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-a") "Failed plain activation changed the identity"
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Failed plain activation changed the backend"

        # A broken store clears both variables when there is no override.
        let good = (open --raw $profiles_file)
        "{broken" | save --force $profiles_file
        load_pulumi_profile
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Broken store kept a stale identity"
        check ($env.PULUMI_BACKEND_URL? == null) "Broken store kept a stale backend"

        # An explicit override survives the same broken store.
        $good | save --force $profiles_file
        pulumips -c -a company-b
        "{broken" | save --force $profiles_file
        load_pulumi_profile
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE == "company-b") "Broken store replaced override identity"
        check ($env.PULUMI_BACKEND_URL == "https://api.pulumi.com") "Broken store replaced override backend"
        check ($env.PULUMI_BACKEND_URL_CURRENT_SHELL == true) "Broken store cleared the override marker"

        # Valid JSON on stdout with a failing exit status is still a failure.
        $good | save --force $profiles_file
        let stub_dir = ([$env.PULUMI_HOME "stub"] | path join)
        mkdir $stub_dir
        let stub_bin = ([$stub_dir "pulumi-profile-selector"] | path join)
        "#!/bin/sh\nprintf '%s' '{\"profile\":\"trap\",\"backend\":\"s3://trap\"}'\nexit 1" | save --force $stub_bin
        ^chmod +x $stub_bin
        $env.PATH = ($env.PATH | prepend $stub_dir)
        let trap_failed = (try { pulumips -c -a company-a; false } catch { true })
        check $trap_failed "Valid JSON with a nonzero exit was accepted"
        check (failed-exit) "Valid JSON with a nonzero exit lost its exit status"
        hide-env --ignore-errors PULUMI_BACKEND_URL_CURRENT_SHELL
        $env.PULUMI_PROFILE_SELECTOR_PROFILE = "stale"
        $env.PULUMI_BACKEND_URL = "s3://stale"
        load_pulumi_profile
        check ($env.PULUMI_PROFILE_SELECTOR_PROFILE? == null) "Failing selector kept a stale identity"
        check ($env.PULUMI_BACKEND_URL? == null) "Failing selector kept a stale backend"
        $env.PATH = ($env.PATH | skip 1)
        pulumips --help
        print "Nushell integration tests passed"
    } catch {|error|
        rm --recursive $root
        error make $error
    }
    rm --recursive $root
}
