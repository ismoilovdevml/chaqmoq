#!/usr/bin/env python3
"""Run a command attached to a pseudo-terminal and echo its raw output.

Chaqmoq only emits colour when it is talking to a terminal, so the screenshots
have to be captured through a PTY. `script(1)` needs a controlling terminal of
its own and is spelled differently on BSD and util-linux; doing it here keeps
capture.sh portable and CI-friendly.

Anything on stdin is forwarded to the child, which is how the REPL screenshot
is driven.

Usage:
    pty_run.py chaqmoq run app.ts
    printf '1 + 1\\n.exit\\n' | pty_run.py chaqmoq repl
"""

from __future__ import annotations

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

# A fixed window size keeps captures reproducible across machines.
ROWS = 50
COLUMNS = 110

# How long to let the child set up its terminal before sending the first line,
# and how far apart to space the lines after that.
STARTUP_GRACE = 0.5
LINE_INTERVAL = 0.15

# A capture must never wedge the docs build.
DEFAULT_TIMEOUT = 60.0


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: pty_run.py [--timeout SECONDS] COMMAND [ARGS...]", file=sys.stderr)
        return 2

    timeout = DEFAULT_TIMEOUT
    if argv[0] == "--timeout":
        timeout = float(argv[1])
        argv = argv[2:]

    stdin_data = b"" if sys.stdin.isatty() else sys.stdin.buffer.read()

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))

    # Turn off echo so piped-in REPL commands do not appear twice, and disable
    # output post-processing so we get exactly what the program wrote.
    attrs = termios.tcgetattr(slave)
    attrs[3] &= ~termios.ECHO  # lflag
    attrs[1] &= ~termios.OPOST  # oflag
    termios.tcsetattr(slave, termios.TCSANOW, attrs)

    process = subprocess.Popen(
        argv,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
        env={**os.environ, "TERM": "xterm-256color", "COLUMNS": str(COLUMNS)},
    )
    os.close(slave)

    # Feed input one line at a time rather than in a single burst. A line editor
    # such as rustyline puts the terminal into raw mode on startup with
    # TCSAFLUSH, which discards anything already queued -- so input written
    # before the prompt appears is simply lost.
    pending = stdin_data.splitlines(keepends=True)
    next_write = time.monotonic() + STARTUP_GRACE

    deadline = time.monotonic() + timeout
    chunks: list[bytes] = []

    while True:
        if time.monotonic() > deadline:
            process.kill()
            print(
                f"pty_run: {argv[0]} did not finish within {timeout}s",
                file=sys.stderr,
            )
            break

        if pending and time.monotonic() >= next_write:
            os.write(master, pending.pop(0))
            next_write = time.monotonic() + LINE_INTERVAL

        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                data = os.read(master, 65536)
            except OSError:
                # The child closed its end of the PTY; we are done.
                break
            if not data:
                break
            chunks.append(data)
        elif process.poll() is not None:
            # Drain whatever is still buffered after the child exited.
            try:
                while True:
                    data = os.read(master, 65536)
                    if not data:
                        break
                    chunks.append(data)
            except OSError:
                pass
            break

    os.close(master)
    process.wait()

    sys.stdout.buffer.write(b"".join(chunks))
    sys.stdout.buffer.flush()
    return process.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
