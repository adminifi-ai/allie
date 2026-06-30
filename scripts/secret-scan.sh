#!/bin/sh
# Cheap repo-owned secret scan for source, nonignored worktree files, commit
# metadata, and GitHub event payloads. Findings print redacted matches only.
set -eu

python3 - "$@" <<'PY'
import os
import re
import subprocess
import sys
from pathlib import Path

MAX_BYTES = 1_000_000
EXCLUDED_PREFIXES = (
    ".git/",
    ".allie/",
    "target/",
    "node_modules/",
)

PATTERNS = [
    ("private-key", re.compile(r"-----BEGIN (?:RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----")),
    ("github-token", re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,}\b")),
    ("github-fine-grained-token", re.compile(r"\bgithub_pat_[A-Za-z0-9_]{22}_[A-Za-z0-9_]{59}\b")),
    ("openai-key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}\b")),
    ("stripe-secret-key", re.compile(r"\bsk_(?:live|test)_[A-Za-z0-9]{20,}\b")),
    ("slack-token", re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b")),
    ("aws-access-key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
]


def run_git(args):
    return subprocess.run(
        ["git", *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    ).stdout


def redact(value):
    if len(value) <= 10:
        return "<redacted>"
    return f"{value[:4]}...{value[-4:]}"


def line_number(text, offset):
    return text.count("\n", 0, offset) + 1


def scan_text(label, text):
    findings = []
    for rule, pattern in PATTERNS:
        for match in pattern.finditer(text):
            findings.append(
                {
                    "label": label,
                    "line": line_number(text, match.start()),
                    "rule": rule,
                    "match": redact(match.group(0)),
                }
            )
    return findings


def looks_binary(data):
    return b"\0" in data[:4096]


def should_skip(path):
    normalized = path.replace(os.sep, "/")
    return normalized.startswith(EXCLUDED_PREFIXES)


def worktree_paths():
    seen = set()
    for args in (["ls-files", "-z"], ["ls-files", "--others", "--exclude-standard", "-z"]):
        for raw in run_git(args).split(b"\0"):
            if not raw:
                continue
            path = raw.decode("utf-8", "surrogateescape")
            if path in seen or should_skip(path):
                continue
            seen.add(path)
            yield path


def scan_file(path):
    file_path = Path(path)
    if not file_path.is_file():
        return []
    try:
        data = file_path.read_bytes()
    except OSError:
        return []
    if len(data) > MAX_BYTES or looks_binary(data):
        return []
    text = data.decode("utf-8", "replace")
    return scan_text(path, text)


def metadata_texts():
    commit = run_git(["log", "-1", "--pretty=%B"]).decode("utf-8", "replace")
    if commit.strip():
        yield "git:HEAD-message", commit

    event_path = os.environ.get("GITHUB_EVENT_PATH")
    if event_path:
        try:
            data = Path(event_path).read_bytes()
        except OSError:
            data = b""
        if data and len(data) <= MAX_BYTES and not looks_binary(data):
            yield "github:event-payload", data.decode("utf-8", "replace")


def scan_repo():
    findings = []
    for path in worktree_paths():
        findings.extend(scan_file(path))
    for label, text in metadata_texts():
        findings.extend(scan_text(label, text))
    return findings


def self_test():
    token = "sk-" + ("a" * 40)
    findings = scan_text("self-test", f"OPENAI_API_KEY={token}\n")
    if len(findings) != 1:
        raise SystemExit("secret scan self-test failed: expected one synthetic OpenAI key finding")
    rendered = format_findings(findings)
    if token in rendered or "aaaaaaaaaa" in rendered:
        raise SystemExit("secret scan self-test failed: finding output was not redacted")
    print("secret scan self-test passed")


def format_findings(findings):
    lines = ["secret scan failed:"]
    for finding in findings:
        lines.append(
            f"- {finding['label']}:{finding['line']} {finding['rule']} {finding['match']}"
        )
    return "\n".join(lines)


def main():
    args = set(sys.argv[1:])
    if "--self-test" in args:
        self_test()
        return
    findings = scan_repo()
    if findings:
        print(format_findings(findings), file=sys.stderr)
        raise SystemExit(1)
    print("secret scan ok: source, nonignored worktree files, commit metadata, and GitHub event payload are clean")


if __name__ == "__main__":
    main()
PY
