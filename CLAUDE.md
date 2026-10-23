# CLAUDE.md

## Current project

OpenSCL4 uses Compose in `android/` and Slint in `desktop/`, with shared device protocols and client rules in `lib/`. For architecture, checks and documentation generation, read `docs/development.md`; for packaging and platform release gates, read `docs/build-release.md`.

`gui/` is the legacy libcosmic client, not the current desktop release. See `docs/history/upstream.md` when changing inherited identities, attribution or historical assets.

## Coding standards

Reviews and implementation follow `docs/agents/review-standards.md`; tooling covers formatting and lints, the document covers the rules tooling cannot check.

## Agent skills

### Issue tracker

Tasks and specs use GitHub Issues in SckraT/OpenSCL4. Before working with tickets, read `docs/agents/issue-tracker.md`.

### Triage labels

Use the canonical five-role mapping. Before triage, read `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` and `docs/adr/`. Before exploring the codebase, read `docs/agents/domain.md`.
