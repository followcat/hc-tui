# Contributing to hc-tui

Thanks for your interest! Issues and pull requests are welcome. Before a large pull request or a new feature, open an
issue first so we can agree on the scope.

## What belongs in hc-tui

hc-tui is a plain terminal client for `hc` (herdr-core). A change fits one of these:

1. **Herdr TUI parity.** Defaults, keys, layout and look follow herdr 0.9.5 ([parity](docs/parity.md)). Existing
   behaviour does not change by default.
2. **hc concepts the herdr TUI never had**, such as terminals started with a parent, machines, keep and documents.
   These follow the official Herdr app, so the same session looks and behaves the same on every client.
3. **hc-tui's own additions.** General terminal-client features that neither reference has. They are opt-in: off by
   default, or with no effect until you use them. They are documented in `docs/` and marked "hc-tui only".

Multi-agent coordination, studios and orchestration do not belong here. They are built as a separate project.

## It runs on official hc

Every change must work on official `hc`. Facts that only the followcat fork of herdr-core publishes render blank on
official `hc`, never as an error. See [core compatibility](docs/compatibility.md).

## One purpose per pull request

- A pull request does one thing that can be reviewed, merged and reverted on its own.
- Fixes, refactors and features are separate pull requests. So is a fix that changes behaviour while new settings are
  off: it must not hide inside a feature.
- When pull requests depend on each other, say the order in their descriptions.
- There is no size limit, but a pull request that mixes purposes will be asked to split.
- Keep commits small. Each commit builds and passes its tests ([AGENTS.md](AGENTS.md)).
- Tests move with the change. Never delete tests to make a pull request smaller.

## Before you open it

- These must pass: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. CI also runs
  macOS and the integration suite against a real `hc` built from source.
- New logic has tests. **A fixed bug comes with a test that fails before the fix and passes after it.** Test what
  the user sees (cells on screen, a request sent to `hc`, a stored value), not the wiring, at the fastest level that
  can catch it. Never add sleeps or retries to hide a race.
- Never touch real `hc` sessions. Use a scratch session (see `docs/plan.md`, Testing rules).
- Code ported from herdr or herdr-core starts with its source file and the Apache-2.0 notice, and the project is
  listed in `NOTICE`.
- User-visible behaviour is documented in `docs/*.md`, with a `CHANGELOG.md` entry under `## Unreleased`. Put a
  new section where it belongs in its page, not at the end of the file, where parallel pull requests collide.
- Source files stay under about 400 lines. Split them by responsibility.

## Language and public history

- English everywhere developers look: code, comments, test names, commit messages, pull requests, issues and docs.
- The repository and its whole history are public. A private value committed and then removed is still published.
  Fixtures use neutral data only (`host-a`, `alice`, `/home/user`, `acme-shop`). Screens captured from a real
  terminal get their host, user, folder, branch and session names replaced before they are committed.

## In the description

- Which of the three kinds above the change is. For parity and hc concepts, name the reference behaviour you
  matched. For an addition, name the setting that turns it on.
- For input, layout, mouse or multi-client changes, say where you tried it: OS, terminal, local or remote, and
  official `hc` or the fork. Also say what you did not try.
- For visible changes, include screens. Text captures are fine.

The [pull request template](.github/pull_request_template.md) asks for the same things.

## Security issues

Please do not open a public issue for a vulnerability. Report it privately through GitHub's
[private vulnerability reporting](https://github.com/followcat/hc-tui/security/advisories/new).
