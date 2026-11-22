"""Tests for the notifier's parsing, message and picture, on a stage tree like the real one.

Run from this folder: `python3 -m unittest test_notifier`.
"""

from __future__ import annotations

import unittest

from failure import MESSAGE_LIMIT, FailedStage, Recipient, failure_message
from jenkins import BuildInfo, BuildRef, Commit
from message import FIELD_LIMIT, LogTail, build_payload
from overview import render
from stages import clean_log, format_duration, log_focus, parse_tree, tidy_name

START = 1_763_810_000_000


def _stage(node_id: str, name: str, state: str, seconds: int = 0, children=None) -> dict:
    node = {"id": node_id, "name": name, "state": state, "type": "STAGE", "children": children or []}
    if state not in ("not_built", "skipped", "queued"):
        node |= {"startTimeMillis": START + int(node_id) * 1000, "totalDurationMillis": seconds * 1000}
    return node


def _cell(base: int, backends: str, clippy: str) -> dict:
    after = "not_built" if clippy == "running" else "skipped"
    steps = [
        _stage(str(base + 1), "Toolchain", "success", 4),
        _stage(str(base + 2), "Libraries", "success", 1),
        _stage(str(base + 3), "Clippy", clippy, 190),
        _stage(str(base + 4), "Test", after),
        _stage(str(base + 5), "Release build", after),
    ]
    state = "failure" if clippy == "failure" else clippy
    return _stage(str(base), f"Matrix - BACKENDS = '{backends}'", state, 200, steps)


TREE = {
    "status": "ok",
    "data": {
        "complete": False,
        "stages": [
            _stage("6", "Format", "success", 12),
            _stage("15", "Linux", "running", 400, [_cell(20, "all", "failure"), _cell(30, "x11", "running")]),
            _stage("50", "SonarQube", "not_built"),
            _stage("60", "Quality gate", "not_built"),
        ],
    },
}
REF = BuildRef("/job/RedesignedVLC/job/main/14/", "http://pi.local:8080/job/RedesignedVLC/job/main/14/", "main", 14, True)
COMMIT = Commit("9fb1c51", "Give the Linux 'all' build its features by name\n\nBody.", "ItayKarp")


def _info(building: bool, result: str | None) -> BuildInfo:
    return BuildInfo(14, building, result, START, 3_840_000, 4_000_000, [COMMIT])


class StagesTest(unittest.TestCase):
    def test_matrix_cells_are_named_by_their_axis_values(self) -> None:
        self.assertEqual(tidy_name("Matrix - BACKENDS = 'x11'"), "x11")
        self.assertEqual(tidy_name("Matrix - OS = 'linux', ARCH = 'arm64'"), "linux · arm64")
        self.assertEqual(tidy_name("Clippy"), "Clippy")

    def test_the_tree_keeps_cells_and_their_steps(self) -> None:
        stages = parse_tree(TREE)
        self.assertEqual([s.name for s in stages], ["Format", "Linux", "SonarQube", "Quality gate"])
        self.assertEqual([c.name for c in stages[1].children], ["all", "x11"])
        self.assertEqual(stages[1].children[0].children[2].state, "failure")

    def test_the_log_follows_the_running_stage_before_a_failure(self) -> None:
        path, stage = log_focus(parse_tree(TREE))
        self.assertEqual(path, ("Linux", "x11", "Clippy"))
        self.assertEqual(stage.state, "running")

    def test_logs_lose_colours_notes_timestamps_and_blank_lines(self) -> None:
        raw = (
            "[2026-11-22T11:23:20.003Z] \x1b[8mha:////abc\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m gpui\n"
            "\n[2026-11-22T11:23:21.000Z] error: unexpected argument 'null' found\n"
        )
        self.assertEqual(clean_log(raw, 5), ["   Compiling gpui", "error: unexpected argument 'null' found"])

    def test_durations_read_naturally(self) -> None:
        self.assertEqual(format_duration(12_000), "12s")
        self.assertEqual(format_duration(182_000), "3m 02s")
        self.assertEqual(format_duration(3_840_000), "1h 04m")


class MessageTest(unittest.TestCase):
    def test_a_running_build_lists_stages_and_the_live_log(self) -> None:
        log = LogTail(("Linux", "x11", "Clippy"), ["   Compiling gpui v0.2.2"] * 40, running=True)
        embed = build_payload("RedesignedVLC", REF, _info(True, None), parse_tree(TREE), log)["embeds"][0]
        self.assertEqual(embed["title"], "RedesignedVLC · main #14")
        self.assertIn("❌ **all**", embed["description"])
        self.assertIn("🔄 Clippy", embed["description"])
        self.assertIn("`9fb1c51` Give the Linux 'all' build its features by name — ItayKarp", embed["description"])
        self.assertEqual(embed["fields"][0]["name"], "Live log · Linux › x11 › Clippy")
        self.assertLessEqual(len(embed["fields"][0]["value"]), FIELD_LIMIT)
        self.assertNotIn("image", embed)

    def test_a_finished_build_shows_its_result_and_picture(self) -> None:
        embed = build_payload("RedesignedVLC", REF, _info(False, "FAILURE"), parse_tree(TREE), None)["embeds"][0]
        self.assertEqual(embed["footer"]["text"], "Failed in 1h 04m")
        self.assertEqual(embed["image"]["url"], "attachment://pipeline.png")


CLIPPY_LOG = [
    "+ cargo clippy --all-targets --no-default-features --features x11,wayland -- -D warnings",
    "   Compiling gpui v0.2.2",
    "    Checking redesigned_vlc v0.1.0",
    "error: this `if` has identical blocks",
    "  --> src/video_window.rs:42:9",
    "   |",
    "42 |         if wide { 1 } else { 1 }",
    "",
    "error: could not compile `RedesignedVLC` (bin \"RedesignedVLC\") due to 1 previous error",
    "script returned exit code 101",
]


BUILD_MESSAGE_LINK = "https://discord.com/channels/1/2/3"


class FailureTest(unittest.TestCase):
    def _message(self, failed, mention="123456789012345678", link=BUILD_MESSAGE_LINK):
        recipient = Recipient(mention, link)
        return failure_message("RedesignedVLC", REF, _info(False, "FAILURE"), failed, recipient)

    def test_it_mentions_the_user_and_starts_at_the_first_error(self) -> None:
        payload, files = self._message([FailedStage(("Linux", "all", "Clippy"), CLIPPY_LOG)])
        content = payload["content"]
        self.assertTrue(content.startswith("<@123456789012345678> ❌"))
        self.assertEqual(payload["allowed_mentions"], {"users": ["123456789012345678"]})
        self.assertIn(f"↪️ Re: [the build's message]({BUILD_MESSAGE_LINK})", content)
        self.assertIn("Failed: **Linux › all › Clippy**", content)
        self.assertIn("```\nerror: this `if` has identical blocks", content)
        self.assertNotIn("Compiling gpui", content)
        self.assertLessEqual(len(content), MESSAGE_LIMIT)
        self.assertEqual(files[0].name, "linux-all-clippy.txt")
        self.assertIn(b"Compiling gpui", files[0].content)

    def test_a_huge_log_still_fits_one_message(self) -> None:
        log = ["error: first"] + [f"warning: number {n}" for n in range(5000)]
        payload, _ = self._message([FailedStage(("Linux", "x11", "Test"), log)])
        self.assertLessEqual(len(payload["content"]), MESSAGE_LIMIT)

    def test_without_a_user_id_nobody_is_mentioned(self) -> None:
        payload, files = self._message([], mention="", link="")
        self.assertTrue(payload["content"].startswith("❌"))
        self.assertEqual(payload["allowed_mentions"], {"parse": []})
        self.assertEqual(files, [])


class OverviewTest(unittest.TestCase):
    def test_the_picture_is_a_png(self) -> None:
        picture = render("RedesignedVLC · main #14", "Failed in 1h 04m", parse_tree(TREE))
        self.assertTrue(picture.startswith(b"\x89PNG"))


if __name__ == "__main__":
    unittest.main()
