#!/usr/bin/env python3
"""Checks a running AsterOpsAI service's cross-layer correlation verdict
against a scenario's expected-verdict.json (unit U21).

Talks to the service's real Unix domain socket directly, stdlib only —
this environment doesn't have `curl` installed (only libcurl as a
library), so this avoids adding a new system dependency for something
Python's own http.client can already do.
"""

import argparse
import http.client
import json
import socket
import sys
import time

ENDPOINT = "/api/v1/analysis/correlation"


class UnixSocketHTTPConnection(http.client.HTTPConnection):
    def __init__(self, socket_path, timeout=5.0):
        super().__init__("localhost", timeout=timeout)
        self._socket_path = socket_path

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self._socket_path)


def fetch_verdict(socket_path):
    """Returns (data, None) on success or (None, error_message)."""
    conn = UnixSocketHTTPConnection(socket_path)
    try:
        conn.request("GET", ENDPOINT)
        resp = conn.getresponse()
        body = resp.read()
        if resp.status != 200:
            return None, f"HTTP {resp.status}: {body.decode('utf-8', 'replace')}"
        envelope = json.loads(body)
        if not envelope.get("success"):
            return None, f"envelope reported failure: {envelope.get('error')}"
        return envelope.get("data"), None
    except (OSError, http.client.HTTPException) as exc:
        return None, f"{type(exc).__name__}: {exc}"
    finally:
        conn.close()


def diff_against_expected(data, expected):
    ranked = data.get("ranked", [])
    ruled_out = {r["cause"] for r in data.get("ruled_out", [])}
    top_cause = ranked[0]["cause"] if ranked else None

    problems = []
    if top_cause != expected["expected_top_cause"]:
        problems.append(
            f"top ranked cause is {top_cause!r}, expected "
            f"{expected['expected_top_cause']!r}"
        )
    missing = set(expected["expected_ruled_out"]) - ruled_out
    if missing:
        problems.append(f"expected these ruled out but they weren't: {sorted(missing)}")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", required=True, help="path to the service's core.sock")
    parser.add_argument("--expected", required=True, help="path to expected-verdict.json")
    parser.add_argument("--timeout", type=float, default=90.0, help="seconds to keep polling")
    parser.add_argument("--poll-interval", type=float, default=2.0)
    args = parser.parse_args()

    with open(args.expected, encoding="utf-8") as f:
        expected = json.load(f)

    deadline = time.monotonic() + args.timeout
    last_problems = None
    last_data = None
    attempt = 0
    while True:
        attempt += 1
        data, error = fetch_verdict(args.socket)
        if error is None:
            problems = diff_against_expected(data, expected)
            last_problems, last_data = problems, data
            if not problems:
                print(f"PASS (attempt {attempt})")
                print(json.dumps(data, indent=2))
                return 0
        else:
            last_problems = [error]
        if time.monotonic() >= deadline:
            break
        time.sleep(args.poll_interval)

    print(f"FAIL after {attempt} attempt(s)")
    for problem in last_problems or ["no response received"]:
        print(f"  - {problem}")
    if last_data is not None:
        print(json.dumps(last_data, indent=2))
    return 1


if __name__ == "__main__":
    sys.exit(main())
