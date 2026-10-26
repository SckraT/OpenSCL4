import fnmatch
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BRANCH = "feature/example"


def concurrency_group(group: str, context: dict[str, str]) -> str:
    def evaluate(match: re.Match[str]) -> str:
        values = (context[name.strip()] for name in match[1].split("||"))
        return next((value for value in values if value), "")

    return re.sub(r"\$\{\{(.+?)\}\}", evaluate, group)


class WorkflowTest(unittest.TestCase):
    def test_branch_push_and_its_pull_request_do_not_cancel_each_other(self):
        for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
            text = path.read_text(encoding="utf-8")
            group = re.search(r"(?m)^  group: (.+)$", text)
            if group is None or "cancel-in-progress: true" not in text or "\n  pull_request:" not in text:
                continue
            push = re.search(r"\n  push:\n((?:    .*\n)*)", text)
            if push is None:
                continue
            branches = re.search(r"(?m)^    branches: \[(.*)\]$", push[1])
            if branches and not any(
                fnmatch.fnmatchcase(BRANCH, pattern.strip(" \"'")) for pattern in branches[1].split(",")
            ):
                continue
            workflow = re.search(r"(?m)^name: (.+)$", text)[1]
            with self.subTest(workflow=path.name):
                push_run = {
                    "github.workflow": workflow,
                    "github.event_name": "push",
                    "github.head_ref": "",
                    "github.ref_name": BRANCH,
                }
                pull_request_run = {
                    "github.workflow": workflow,
                    "github.event_name": "pull_request",
                    "github.head_ref": BRANCH,
                    "github.ref_name": "1/merge",
                }
                self.assertNotEqual(
                    concurrency_group(group[1], push_run),
                    concurrency_group(group[1], pull_request_run),
                    "A push to a pull request branch cancels the pull request run of the same commit",
                )


if __name__ == "__main__":
    unittest.main()
