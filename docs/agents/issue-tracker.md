# Issue tracker: GitHub

Issues and specs live in SckraT/OpenSCL4 (origin). Always pass --repo SckraT/OpenSCL4: the upstream remote is the inherited OpenSCQ30 project and is never the tracker.

Issue titles and bodies are Russian; use the vocabulary from GLOSSARY.md as described in docs/agents/domain.md.

## Conventions

Write multi-line bodies to a UTF-8 file with real newlines and pass --body-file; avoid shell quoting on Windows.

- Create: gh issue create --repo SckraT/OpenSCL4 --title "..." --body-file BODY_FILE (with --label / --assignee @me as needed)
- Read: gh issue view NUMBER --repo SckraT/OpenSCL4 --json number,title,body,labels,comments
- List: gh issue list --repo SckraT/OpenSCL4 --state open --json number,title,body,labels,assignees
- Across states: --state all, optionally with --search "..." or --label LABEL
- Comment: gh issue comment NUMBER --repo SckraT/OpenSCL4 --body-file BODY_FILE
- Labels: gh issue edit NUMBER --repo SckraT/OpenSCL4 --add-label LABEL or --remove-label LABEL
- Close: gh issue close NUMBER --repo SckraT/OpenSCL4

Category labels are bug and enhancement; state labels are the five triage roles in docs/agents/triage-labels.md.

## Pull requests as a triage surface

**PRs as a request surface: no.** Triage covers issues only.

## Skill operations

"Publish to the issue tracker" means create a GitHub issue.
"Fetch the relevant ticket" means read its body, labels and comments.

## Wayfinding operations

Used by wayfinder. The map is one issue; its children are the tickets.

- Map: one issue labelled wayfinder:map, whose body holds notes, decisions so far and fog.
- Children: sub-issues labelled wayfinder:research, wayfinder:prototype, wayfinder:grilling or wayfinder:task.
- Create a child: gh issue create --repo SckraT/OpenSCL4 --parent NUMBER ... (gh 2.94+), or gh issue edit PARENT --add-sub-issue CHILD. Without sub-issues, link the child from a task list in the map body and put "Part of #NUMBER" at the top of the child.
- Blocking: native issue dependencies. gh api --method POST repos/SckraT/OpenSCL4/issues/CHILD/dependencies/blocked_by -F issue_id=BLOCKER_DATABASE_ID, where the id comes from gh api repos/SckraT/OpenSCL4/issues/NUMBER --jq .id (database id, not #number). Without dependencies, record "Blocked by: #NUMBER" in the child.
- Frontier: open children, in map order, with no assignee and no open blockers.
- Claim: gh issue edit NUMBER --repo SckraT/OpenSCL4 --add-assignee @me — the first write.
- Resolve: comment the answer, close the ticket, then append a named link and a one-line decision summary to the map.
