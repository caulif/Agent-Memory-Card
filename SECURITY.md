# Security Policy

## Supported Versions

Agent Memory Kernel is currently pre-1.0. Security fixes are made on the main
development branch and included in the next tagged release.

## Reporting a Vulnerability

Please report security issues privately through GitHub Security Advisories if
the repository is hosted on GitHub. If advisories are unavailable, contact the
maintainer privately before opening a public issue.

Do not include real API keys, local conversation logs, or private project files
in public reports. A minimal reproduction with redacted data is preferred.

## Local Data

Agent Memory Kernel is local-first and may read local agent history, project
rules, and generated memory-card artifacts when the user asks it to. Review all
generated Drafts and Memory Cards before approving or syncing them into agent
configuration files.
