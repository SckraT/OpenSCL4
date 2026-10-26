import fnmatch
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
EVENTS = ("push", "pull_request", "workflow_dispatch")
BRANCHES = ("feature/example", "master")


def concurrency_group(group: str, context: dict[str, str]) -> str:
    def evaluate(match: re.Match[str]) -> str:
        values = (context[name.strip()] for name in match[1].split("||"))
        return next((value for value in values if value), "")

    return re.sub(r"\$\{\{(.+?)\}\}", evaluate, group)


def run_context(workflow: str, event: str, branch: str) -> dict[str, str]:
    # A pull request run checks out the merge ref; push and dispatch runs check out the branch itself.
    pull_request = event == "pull_request"
    return {
        "github.workflow": workflow,
        "github.event_name": event,
        "github.head_ref": branch if pull_request else "",
        "github.ref_name": "1/merge" if pull_request else branch,
    }


def pushes_to(text: str, branch: str) -> bool:
    push = re.search(r"\n  push:\n((?:    .*\n)*)", text)
    branches = re.search(r"(?m)^    branches: \[(.*)\]$", push[1])
    return branches is None or any(
        fnmatch.fnmatchcase(branch, pattern.strip(" \"'")) for pattern in branches[1].split(",")
    )


class WorkflowTest(unittest.TestCase):
    def test_runs_of_one_branch_do_not_cancel_each_other(self):
        checked = []
        for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
            text = path.read_text(encoding="utf-8")
            if "cancel-in-progress: true" not in text:
                continue
            with self.subTest(workflow=path.name):
                group = re.search(r"(?m)^  group: (.+)$", text)
                self.assertIsNotNone(group, "Expected a top-level concurrency group")
                events = [event for event in EVENTS if re.search(rf"(?m)^  {event}:", text)]
                self.assertTrue(events, "Expected triggers as a mapping under `on:`")
                workflow = re.search(r"(?m)^name: (.+)$", text)[1]
                checked.append(path.name)
                for branch in BRANCHES:
                    runs = {
                        event: concurrency_group(group[1], run_context(workflow, event, branch))
                        for event in events
                        if event != "push" or pushes_to(text, branch)
                    }
                    self.assertEqual(
                        len(set(runs.values())),
                        len(runs),
                        f"Runs for {branch} share a cancelling concurrency group: {runs}",
                    )
        self.assertTrue(checked, "No workflow with a cancelling concurrency group was checked")


if __name__ == "__main__":
    unittest.main()
