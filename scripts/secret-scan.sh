#!/bin/sh
# Cheap repo-owned secret scan for source, nonignored worktree files, commit
# metadata, and GitHub event payloads. Findings print redacted matches only.
set -eu

python3 - "$@" <<'PY'
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

MAX_BYTES = 1_000_000
OVERLAP_BYTES = 4096
EXCLUDED_PREFIXES = (
    ".git/",
    "target/",
    "node_modules/",
)
GENERATED_ROOT = Path(".allie")
GENERATED_EXCLUDED_DIRS = {
    ".git",
    "bundle",
    "ms-playwright",
    "node_modules",
    "target",
    "tooling",
}

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


def scan_text(label, text, line_base=0, minimum_match_end=0):
    findings = []
    for rule, pattern in PATTERNS:
        for match in pattern.finditer(text):
            if match.end() <= minimum_match_end:
                continue
            findings.append(
                {
                    "label": label,
                    "line": line_base + line_number(text, match.start()),
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


def generated_evidence_paths(base=Path(".")):
    root = base / GENERATED_ROOT
    if not root.is_dir():
        return
    for current, dirs, files in os.walk(
        root, followlinks=False, onerror=raise_scan_error
    ):
        current_path = Path(current)
        dirs[:] = [
            name
            for name in dirs
            if name not in GENERATED_EXCLUDED_DIRS
            and not (current_path / name).is_symlink()
        ]
        for name in files:
            path = current_path / name
            if path.is_symlink():
                continue
            yield path.relative_to(base).as_posix()


def raise_scan_error(error):
    raise error


def scan_file(path, label=None):
    file_path = Path(path)
    if not file_path.is_file():
        raise OSError("secret scan input is not a readable file")
    findings = []
    overlap = b""
    line_base = 0
    first_chunk = True
    with file_path.open("rb") as handle:
        while True:
            chunk = handle.read(MAX_BYTES)
            if not chunk:
                break
            if first_chunk and looks_binary(chunk):
                return []
            first_chunk = False
            data = overlap + chunk
            text = data.decode("utf-8", "replace")
            overlap_text = overlap.decode("utf-8", "replace")
            findings.extend(
                scan_text(label or path, text, line_base, len(overlap_text))
            )
            keep = min(len(data), OVERLAP_BYTES)
            consumed = data[:-keep] if keep else data
            line_base += consumed.count(b"\n")
            overlap = data[-keep:] if keep else b""
    return findings


def metadata_findings():
    findings = []
    commit = run_git(["log", "-1", "--pretty=%B"]).decode("utf-8", "replace")
    if commit.strip():
        findings.extend(scan_text("git:HEAD-message", commit))

    event_path = os.environ.get("GITHUB_EVENT_PATH")
    if event_path:
        findings.extend(scan_file(event_path, "github:event-payload"))
    return findings


def scan_repo():
    findings = []
    seen = set()
    for path in (*worktree_paths(), *generated_evidence_paths()):
        if path in seen:
            continue
        seen.add(path)
        findings.extend(scan_file(path))
    findings.extend(metadata_findings())
    return findings


def self_test():
    token = "sk-" + ("a" * 40)
    findings = scan_text("self-test", f"OPENAI_API_KEY={token}\n")
    if len(findings) != 1:
        raise SystemExit("secret scan self-test failed: expected one synthetic OpenAI key finding")
    rendered = format_findings(findings)
    if token in rendered or "aaaaaaaaaa" in rendered:
        raise SystemExit("secret scan self-test failed: finding output was not redacted")
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        artifacts = [
            ".allie/verify/latest/run/artifacts/dom-account.html",
            ".allie/verify/latest/run/artifacts/console-account.json",
            ".allie/verify/latest/run/artifacts/network-account.json",
            ".allie/verify/latest/run/artifacts/axe-account.html",
            ".allie/verify/latest/run/evidence.json",
        ]
        for relative in artifacts:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"synthetic={token}\n", encoding="utf-8")
        boundary = root / artifacts[2]
        boundary.write_text(
            "x" * (MAX_BYTES - 5) + " " + token + "\n", encoding="utf-8"
        )
        generated = list(generated_evidence_paths(root))
        if sorted(generated) != sorted(artifacts):
            raise SystemExit(
                "secret scan self-test failed: generated .allie evidence families were not enumerated"
            )
        findings = []
        old_cwd = Path.cwd()
        try:
            os.chdir(root)
            for path in generated:
                findings.extend(scan_file(path))
        finally:
            os.chdir(old_cwd)
        if len(findings) != len(artifacts):
            raise SystemExit(
                "secret scan self-test failed: expected a finding in every textual evidence family"
            )
        rendered = format_findings(findings)
        if token in rendered or "aaaaaaaaaa" in rendered:
            raise SystemExit(
                "secret scan self-test failed: generated-evidence output was not redacted"
            )
        vanished = root / ".allie/verify/latest/run/artifacts/vanished.html"
        vanished.write_text("evidence", encoding="utf-8")
        vanished.unlink()
        try:
            scan_file(vanished)
        except OSError:
            pass
        else:
            raise SystemExit(
                "secret scan self-test failed: unreadable evidence did not fail closed"
            )
        old_event_path = os.environ.get("GITHUB_EVENT_PATH")
        event = root / "large-event.json"
        event.write_text(
            "x" * (MAX_BYTES - 5) + " " + token + "\n", encoding="utf-8"
        )
        os.environ["GITHUB_EVENT_PATH"] = str(event)
        event_findings = metadata_findings()
        if not any(
            finding["label"] == "github:event-payload"
            for finding in event_findings
        ):
            raise SystemExit(
                "secret scan self-test failed: large event payload bypassed streaming scan"
            )
        event.unlink()
        try:
            metadata_findings()
        except OSError:
            pass
        else:
            raise SystemExit(
                "secret scan self-test failed: unreadable event payload did not fail closed"
            )
        finally:
            if old_event_path is None:
                os.environ.pop("GITHUB_EVENT_PATH", None)
            else:
                os.environ["GITHUB_EVENT_PATH"] = old_event_path
    print("secret scan self-test passed: source metadata and generated .allie evidence")


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
    try:
        findings = scan_repo()
    except OSError:
        print("secret scan failed: an input could not be read", file=sys.stderr)
        raise SystemExit(1)
    if findings:
        print(format_findings(findings), file=sys.stderr)
        raise SystemExit(1)
    print("secret scan ok: source, generated .allie evidence, commit metadata, and GitHub event payload are clean")


if __name__ == "__main__":
    main()
PY
