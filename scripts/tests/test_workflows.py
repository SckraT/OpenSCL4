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


def job(text: str, name: str) -> str:
    return re.search(rf"(?ms)^  {name}:\n(.*?)(?=^  \S|\Z)", text)[1]


def artifact_uploads(job_text: str) -> dict[str, list[str]]:
    """Each actions/upload-artifact step of a job: its artifact name and path patterns."""
    uploads = {}
    for step in re.split(r"(?m)^      - ", job_text):
        if not step.startswith("uses: actions/upload-artifact@"):
            continue
        name = re.search(r"(?m)^          name: (.+)$", step)[1]
        path = re.search(r"(?m)^          path: (.+)$((?:\n            .+)*)", step)
        uploads[name] = [line.strip() for line in path[2].splitlines() if line.strip()] if path[1] == "|" else [path[1]]
    return uploads


def uploaded(file: str, patterns: list[str]) -> bool:
    excluded = any(fnmatch.fnmatchcase(file, pattern[1:]) for pattern in patterns if pattern.startswith("!"))
    return not excluded and any(fnmatch.fnmatchcase(file, pattern) for pattern in patterns if not pattern.startswith("!"))


class WorkflowTest(unittest.TestCase):
    def test_linux_job_uploads_each_package_once_and_the_appimage_on_its_own(self):
        # The bundle copies every downloaded file into one folder, where two files of one name collide, and a later
        # check fetches the AppImage without the other packages.
        text = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")
        uploads = artifact_uploads(job(text, "linux"))
        appimage = "build-output/openscl4-1.2.3-linux-x86_64.AppImage"
        files = [
            appimage,
            "build-output/openscl4-1.2.3-linux-x86_64.tar.gz",
            "build-output/openscl4_1.2.3_amd64.deb",
            "build-output/build-linux.json",
        ]
        holders = {file: [name for name, patterns in uploads.items() if uploaded(file, patterns)] for file in files}
        self.assertEqual({file: 1 for file in files}, {file: len(names) for file, names in holders.items()}, holders)
        self.assertEqual([appimage], [file for file in files if holders[file] == holders[appimage]])
        collected = re.search(r"--pattern '([^']+)'", job(text, "bundle"))[1]
        self.assertTrue(all(fnmatch.fnmatchcase(name, collected) for name in uploads), uploads)


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
