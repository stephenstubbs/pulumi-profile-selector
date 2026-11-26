#!/usr/bin/env python3
import errno
import json
import os
import pty
import re
import select
import shlex
import shutil
import signal
import sys
import tempfile
import termios
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
BACKEND = "https://api.pulumi.com"
QUESTION = b"Select Pulumi Profile:"
LIMIT = 15


def nu_quote(value):
    return json.dumps(str(value))


def script(shell, action):
    command = "pulumips" if action == "persist" else "pulumips --current --"
    if shell == "bash":
        return f"""
source {shlex.quote(str(ROOT / "shell/pulumi.bash"))}
pulumips --activate company-a
pulumips --current --activate company-a
code=0
{command} || code=$?
printf '\\nRESULT|%s|%s|%s|%s\\n' "$code" "${{PULUMI_BACKEND_URL-}}" \
"${{PULUMI_PROFILE_SELECTOR_PROFILE-}}" "${{PULUMI_BACKEND_URL_CURRENT_SHELL-}}"
"""
    attempt = (
        f"{command}\nlet code = 0"
        if action in ("current", "persist")
        else f"let failed = (try {{{command}; false}} catch {{true}})\n"
        'if not $failed {error make {msg:"Cancellation reported success"}}\nlet code = 1'
    )
    return f"""
source {nu_quote(ROOT / "shell/pulumi.nu")}
pulumips --activate company-a
pulumips --current --activate company-a
{attempt}
print $"RESULT|($code)|($env.PULUMI_BACKEND_URL? | default '')|\
($env.PULUMI_PROFILE_SELECTOR_PROFILE? | default '')|\
($env.PULUMI_BACKEND_URL_CURRENT_SHELL? | default '')"
"""


def stop(pid):
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass


def drive(argv, env, keys):
    pid, master = pty.fork()
    if pid == 0:
        termios.tcsetwinsize(0, (30, 110))
        os.execvpe(argv[0], argv, env)
    data = bytearray()
    cursor = 0
    sent = False
    status = None
    deadline = time.monotonic() + LIMIT
    try:
        while time.monotonic() < deadline:
            readable, _, _ = select.select(
                [master], [], [], min(0.1, max(0, deadline - time.monotonic()))
            )
            if readable:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                data.extend(chunk)
                while True:
                    query = data.find(b"\x1b[6n", cursor)
                    if query == -1:
                        break
                    os.write(master, b"\x1b[1;1R")
                    cursor = query + 4
                if not sent and QUESTION in data:
                    os.write(master, keys)
                    sent = True
            waited, result = os.waitpid(pid, os.WNOHANG)
            if waited:
                status = result
                break
        if status is None:
            raise AssertionError("Interactive selector exceeded its 15-second deadline")
        if not sent:
            raise AssertionError("Interactive prompt was not visible before input")
        if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
            raise AssertionError("Shell did not finish successfully")
        return bytes(data)
    except BaseException as error:
        raise AssertionError(f"{error}\n{bytes(data[-8192:]).decode(errors='replace')}") from error
    finally:
        if status is None:
            stop(pid)
        os.close(master)


def run_case(binary, shell, action):
    with tempfile.TemporaryDirectory(prefix="pulumi-interactive-") as directory:
        base = Path(directory)
        home = base / "home"
        state = base / "pulumi"
        commands = base / "bin"
        for path in (home, state, commands):
            path.mkdir()
        (commands / "pulumi-profile-selector").symlink_to(binary)
        profiles = [
            {"name": "company-a", "backend": BACKEND},
            {"name": "company-b", "backend": BACKEND},
        ]
        profile_bytes = json.dumps(profiles).encode()
        (state / "profiles.json").write_bytes(profile_bytes)
        current = state / "current_profile"
        current.write_text("company-a")
        source = base / ("case.sh" if shell == "bash" else "case.nu")
        source.write_text(script(shell, action))
        executable = shutil.which(shell)
        if executable is None:
            raise AssertionError(f"Required shell is missing: {shell}")
        argv = (
            [executable, "--noprofile", "--norc", str(source)]
            if shell == "bash"
            else [executable, "--no-config-file", str(source)]
        )
        env = {
            "PATH": str(commands) + os.pathsep + os.environ["PATH"],
            "HOME": str(home),
            "PULUMI_HOME": str(state),
            "XDG_CONFIG_HOME": str(home / "config"),
            "XDG_DATA_HOME": str(home / "data"),
            "XDG_CACHE_HOME": str(home / "cache"),
            "TERM": "xterm-256color",
            "SHELL": executable,
            "LANG": "C.UTF-8",
        }
        keys = {"escape": b"\x1b", "interrupt": b"\x03"}.get(action, b"company-b\r")
        data = drive(argv, env, keys)
        match = re.search(
            rb"RESULT\|(\d+)\|([^|\r\n]*)\|([^|\r\n]*)\|([^|\r\n]*)", data
        )
        if match is None:
            raise AssertionError(f"No result record: {data.decode(errors='replace')}")
        code, backend, profile, marker = (
            item.decode() for item in match.groups()
        )
        cancelled = action in ("escape", "interrupt")
        expected = (
            ("1", BACKEND, "company-a", "true")
            if cancelled
            else ("0", BACKEND, "company-b", "" if action == "persist" else "true")
        )
        if (code, backend, profile, marker) != expected:
            raise AssertionError(f"{shell}/{action}: incorrect shell snapshot")
        saved = "company-b" if action == "persist" else "company-a"
        if current.read_text() != saved:
            raise AssertionError(f"{shell}/{action}: incorrect persisted selection")
        if (state / "profiles.json").read_bytes() != profile_bytes:
            raise AssertionError(f"{shell}/{action}: profile metadata was modified")
        print(f"ok - {shell}: visible prompt, {action}, identity and persistence")


def main():
    binary = Path(sys.argv[1]).resolve(strict=True)
    for shell in ("bash", "nu"):
        for action in ("current", "escape", "interrupt", "persist"):
            run_case(binary, shell, action)
    print("8 real PTY Bash/Nushell selection and cancellation checks passed")


if __name__ == "__main__":
    main()
