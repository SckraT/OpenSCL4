# Issue tracker: GitHub

Issues and specs live in SckraT/OpenSCL4. Use gh with --repo SckraT/OpenSCL4 for issue commands.

## Conventions

- Create: gh issue create --repo SckraT/OpenSCL4 --title "..." --body-file BODY_FILE
- Read: gh issue view NUMBER --repo SckraT/OpenSCL4 --json number,title,body,labels,comments
- List: gh issue list --repo SckraT/OpenSCL4 --state open --json number,title,body,labels,assignees
- Comment: gh issue comment NUMBER --repo SckraT/OpenSCL4 --body-file BODY_FILE
- Labels: gh issue edit NUMBER --repo SckraT/OpenSCL4 --add-label LABEL or --remove-label LABEL
- Close: gh issue close NUMBER --repo SckraT/OpenSCL4
- Use UTF-8 files with actual newlines for multiline bodies.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## Skill operations

"Publish to the issue tracker" means create a GitHub issue.
"Fetch the relevant ticket" means read its body, labels and comments.

## Wayfinding operations

- Map: one issue labelled wayfinder:map.
- Children: GitHub sub-issues, labelled wayfinder:research, wayfinder:prototype, wayfinder:grilling or wayfinder:task.
- Create a child with gh issue create --parent NUMBER. If unavailable, add "Part of #NUMBER" to the child and link it in the map's task list.
- Blocking: use native issue dependencies through gh api. POST repos/SckraT/OpenSCL4/issues/CHILD/dependencies/blocked_by with issue_id=BLOCKER_DATABASE_ID. Obtain that ID from gh api repos/SckraT/OpenSCL4/issues/NUMBER --jq .id.
- If dependencies are unavailable, record "Blocked by: #NUMBER" in the child.
- Frontier: open children, in map order, with no assignee and no open blockers.
- Claim: gh issue edit NUMBER --repo SckraT/OpenSCL4 --add-assignee @me.
- Resolve: comment with the answer, close the ticket, then add a named link and a short decision summary to the map.
