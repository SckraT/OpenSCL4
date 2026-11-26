---
name: implement-spec
description: "Implement the result of /to-spec and /to-tickets in code."
disable-model-invocation: true
---

You have been provided a spec. This spec should have tickets associated with it, describing how to implement the spec.

The issue tracker should have been provided to you. If not, tell the user to run `/setup-matt-pocock-skills`.

The goal is the entire spec implemented on a single **integration branch**, with every ticket resolved the way the issue tracker closes work.

The tickets are not a list of steps. They are a **task graph** with blocking relationships between them. This means there is always a **frontier** of tickets which are ready to be grabbed.

Communication to and from subagents should be sparse. Communicate primarily through **context pointers**: to the spec, tickets, research notes, and previous commits. Don't duplicate information already available via pointers.

**Implementer subagents** should be run in the background where possible for maximum concurrency.

## Steps

1. Read the spec, its tickets and the repository's completion rules to understand the task graph. [Issue tracker](../../../docs/agents/issue-tracker.md#completing-implementation-tickets)

2. (optional) Use an **exploration subagent** to conduct any exploration required by the tickets - relevant codebase files or external documentation. Ensure the exploration subagent can save files - it should save its markdown notes in a directory outside the repo, accessible by all future subagents. This lets **implementer subagents** focus on implementation rather than exploration.

3. Create or resume the integration branch. When resuming an existing PR, use its current `headRefOid` to identify the published integration tip. For delivery through a PR, open its draft after the first integration merge and reference its tickets with `Refs #NUMBER`.

4. Pin the integration SHA for each ticket and create a separate branch and clean worktree from that commit for its **implementer subagent**. If a worktree is occupied or at a different revision, use a new path and preserve the existing work. Each implementer follows these steps.

    - Verify `git rev-parse HEAD` and `git status --short --untracked-files=all` before editing.
    - Use `tdd` to implement and verify the ticket.
    - Merge the current integration tip into the ticket branch. Run the relevant checks and report their results with the resulting commit SHA.

5. Once an **implementer subagent** completes, merge its work to the integration branch with a **merger subagent**.

6. Start **implementer subagents** for newly unblocked tickets. Within the same integration run, a prerequisite satisfies its dependency once its code is integrated and the relevant checks pass there. GitHub ticket completion follows the tracker rules.

7. Once all ticket changes are integrated, call `code-review` on the integration branch. Fix the confirmed findings in a single **implementer subagent**.

8. If a draft PR exists, publish acceptance evidence and mark it ready for review. Apply the tracker completion rules per ticket when choosing `Closes` or `Refs`. For branch-only delivery, report the integration branch, its commit and the status of acceptance for each ticket.

9. Remove a worktree created for this run only after `git status --short --untracked-files=all` confirms it is clean and its committed result is present on the integration branch.
