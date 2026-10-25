# Domain docs

## Layout

Single-context: one GLOSSARY.md at the repo root plus docs/adr/. There is no GLOSSARY-MAP.md and no per-context src/*/docs/adr/.

## Before exploring

Read GLOSSARY.md, then the ADRs in docs/adr/ that touch the area you are about to work in. ADRs are named NNNN-kebab-title.md and are cited as ADR-0001.

If either is missing, proceed silently. domain-modeling creates them lazily when a term or decision is actually resolved, using its own GLOSSARY-FORMAT.md and ADR-FORMAT.md.

## Vocabulary

GLOSSARY.md is written in Russian, and so are issue titles and bodies. Use the glossary terms verbatim in that language — do not translate them into English, and do not drift to the synonyms the glossary marks as avoided.

If a concept you need is missing, that is a signal: either you are inventing language the project does not use (reconsider), or there is a real gap (note it for domain-modeling).

## Decisions

Surface conflicts with an existing ADR rather than silently overriding it, citing the ADR number and the reason for reopening it. docs/agents/review-standards.md applies the same rule to code review.
