import fnmatch
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / ".github/workflows/build.yml"
APPIMAGE = "build-output/openscl4-1.2.3-linux-x86_64.AppImage"
APPIMAGE_REPORT = "build-output/build-linux-appimage.json"
DEB = "build-output/openscl4_1.2.3_amd64.deb"
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


def needs(job_text: str) -> list[str]:
    found = re.search(r"(?m)^    needs: (?:\[(.*)\]|(\S+))$", job_text)
    return [name.strip() for name in (found[1] or found[2]).split(",")] if found else []


def matrix(job_text: str, key: str) -> list[str]:
    values = re.search(rf"(?m)^        {key}: \[(.*)\]$", job_text)[1]
    return [value.strip(" \"'") for value in values.split(",")]


def steps(job_text: str) -> list[str]:
    """Each step of a job, from the key after its `- ` to the next step."""
    return re.split(r"(?m)^      - ", job_text)[1:]


def step_key(step: str, key: str) -> str | None:
    found = re.search(rf"(?m)^(?:        )?{key}: (.+)$", step)
    return found[1] if found else None


def fetched_artifacts(job_text: str) -> list[str]:
    """The artifacts of this run that a job looks up by name through the Actions API."""
    return re.findall(r"item\[\"name\"\] == \"([^\"]+)\"", job_text)


def bundle_pattern(text: str) -> str:
    """The artifact names whose files the bundle collects into one set."""
    return re.search(r"--pattern '([^']+)'", job(text, "bundle"))[1]


def artifact_uploads(job_text: str, condition: str | None = None) -> dict[str, list[str]]:
    """Each actions/upload-artifact step of a job, or only those run on `if: condition`: artifact name and paths."""
    uploads = {}
    for step in steps(job_text):
        if not step.startswith("uses: actions/upload-artifact@"):
            continue
        if condition is not None and step_key(step, "if") != condition:
            continue
        name = re.search(r"(?m)^          name: (.+)$", step)[1]
        path = re.search(r"(?m)^          path: (.+)$((?:\n            .+)*)", step)
        uploads[name] = [line.strip() for line in path[2].splitlines() if line.strip()] if path[1] == "|" else [path[1]]
    return uploads


def holders(uploads: dict[str, list[str]], file: str) -> list[str]:
    """The uploaded artifacts whose path patterns take this file."""
    def uploaded(patterns: list[str]) -> bool:
        excluded = any(fnmatch.fnmatchcase(file, pattern[1:]) for pattern in patterns if pattern.startswith("!"))
        return not excluded and any(fnmatch.fnmatchcase(file, pattern) for pattern in patterns if not pattern.startswith("!"))

    return [name for name, patterns in uploads.items() if uploaded(patterns)]


class WorkflowTest(unittest.TestCase):
    def test_linux_job_uploads_each_package_once_and_the_appimage_with_its_report_on_their_own(self):
        # The bundle copies every downloaded file into one folder, where two files of one name collide, and a later
        # check fetches the AppImage without the other packages. A set carries the AppImage and its report together.
        text = BUILD.read_text(encoding="utf-8")
        uploads = artifact_uploads(job(text, "linux"))
        files = [
            APPIMAGE,
            APPIMAGE_REPORT,
            "build-output/openscl4-1.2.3-linux-x86_64.tar.gz",
            DEB,
            "build-output/build-linux.json",
        ]
        held = {file: holders(uploads, file) for file in files}
        self.assertEqual({file: 1 for file in files}, {file: len(names) for file, names in held.items()}, held)
        self.assertEqual([APPIMAGE, APPIMAGE_REPORT], [file for file in files if held[file] == held[APPIMAGE]])
        self.assertTrue(all(fnmatch.fnmatchcase(name, bundle_pattern(text)) for name in uploads), uploads)

    def test_a_failed_appimage_build_leaves_the_linux_packages_to_their_checks_and_the_bundle(self):
        # The AppImage is not a release gate: its build may fail without failing the linux job, which the .deb checks
        # and the bundle wait for. Its artifact is then missing, so the AppImage check fails and the run is red. The
        # AppImage is uploaded before the job ends, so the bundle finds it or not by the job's result, not by timing.
        text = BUILD.read_text(encoding="utf-8")
        linux = job(text, "linux")
        builds = {step_key(step, "run"): step for step in steps(linux) if "scripts/release.py build" in step}
        appimage = builds.pop("python3 scripts/release.py build linux-appimage")
        self.assertEqual(["python3 scripts/release.py build linux"], list(builds))
        self.assertEqual("true", step_key(appimage, "continue-on-error"))
        self.assertIsNone(step_key(builds["python3 scripts/release.py build linux"], "continue-on-error"))
        on_success = artifact_uploads(linux, f"steps.{step_key(appimage, 'id')}.outcome == 'success'")
        self.assertEqual(list(on_success), holders(artifact_uploads(linux), APPIMAGE))
        self.assertEqual([], holders(on_success, DEB))
        self.assertIn("linux", needs(job(text, "bundle")))

    def test_appimage_check_starts_the_built_appimage_on_each_ubuntu_release(self):
        # The AppImage itself is started on both checked releases; the application's own behaviour stays with the
        # .deb's package checks, which do not fetch the AppImage.
        text = BUILD.read_text(encoding="utf-8")
        appimage = holders(artifact_uploads(job(text, "linux")), APPIMAGE)
        check = job(text, "linux-appimage-check")
        self.assertIn("linux", needs(check))
        self.assertEqual(["24.04", "26.04"], matrix(check, "ubuntu"))
        self.assertRegex(check, r"(?m)^    container: ubuntu:\$\{\{ matrix\.ubuntu \}\}$")
        self.assertEqual(appimage, fetched_artifacts(check))
        self.assertRegex(check, r"chmod \+x \S+\.AppImage")
        self.assertRegex(check, r"(?m)^      - run: bash scripts/test-linux-appimage\.sh ")
        self.assertNotIn("test-linux-package.sh", check)
        self.assertFalse(set(appimage) & set(fetched_artifacts(job(text, "linux-package-check"))))

    def test_appimage_check_keeps_the_logs_of_a_failed_start(self):
        # Each Ubuntu release keeps its own logs, apart from the packages the bundle collects.
        text = BUILD.read_text(encoding="utf-8")
        check = job(text, "linux-appimage-check")
        logs = re.search(r"(?m)^      - run: bash scripts/test-linux-appimage\.sh \S+ (\S+)$", check)[1]
        kept = artifact_uploads(check, "failure()")
        self.assertEqual([[logs]], list(kept.values()), kept)
        name = next(iter(kept))
        self.assertIn("${{ matrix.ubuntu }}", name)
        self.assertFalse(fnmatch.fnmatchcase(name, bundle_pattern(text)), name)

    def test_appimage_check_gates_no_other_job(self):
        # The AppImage is not a release gate of the first release (docs/cross-platform-release-plan.md), so a failed
        # check marks the run without holding back the bundle.
        text = BUILD.read_text(encoding="utf-8")
        jobs = re.findall(r"(?m)^  ([\w-]+):$", text[text.index("\njobs:\n"):])
        self.assertIn("linux-appimage-check", jobs)
        self.assertEqual([], [name for name in jobs if "linux-appimage-check" in needs(job(text, name))])


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
