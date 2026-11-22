"""Jenkins pipeline stages as the notifier shows them: names, states, durations and logs."""

from __future__ import annotations

import re
from collections.abc import Iterator
from dataclasses import dataclass, field

# Matrix cells are named "Matrix - BACKENDS = 'x11'"; only the axis values say anything.
MATRIX_PREFIX = re.compile(r"^Matrix - ")
AXIS_VALUE = re.compile(r"\w+ = '([^']*)'")

# Console notes Jenkins hides in the raw log, then terminal colours, then the Timestamper
# prefix on each line.
CONSOLE_NOTE = re.compile(r"\x1b\[8m.*?\x1b\[0m")
ANSI_CODE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
TIMESTAMP_PREFIX = re.compile(r"^\[\d{4}-\d\d-\d\dT[\d:.]+Z\] ")
# Where rustc and clippy diagnostics begin: what to show first from a failed stage.
DIAGNOSTIC_START = re.compile(r"^(error|warning)(\[[\w-]+\])?:")

LOG_LINE_WIDTH = 110

STATE_EMOJI = {
    "success": "✅",
    "failure": "❌",
    "unstable": "⚠️",
    "aborted": "⏹️",
    "running": "🔄",
    "queued": "⏳",
    "paused": "⏸️",
    "skipped": "⏭️",
    "not_built": "⬜",
}
UNKNOWN_EMOJI = "❔"

# How a finished build's result reads in a sentence ("Failed in 3m 02s").
RESULT_WORD = {
    "SUCCESS": "Passed",
    "FAILURE": "Failed",
    "UNSTABLE": "Unstable",
    "ABORTED": "Aborted",
    "NOT_BUILT": "Not built",
}


@dataclass
class Stage:
    """One stage, matrix cell or parallel branch, with the ones nested inside it."""

    id: str
    name: str
    state: str
    start_ms: int
    duration_ms: int | None
    children: list[Stage] = field(default_factory=list)

    @property
    def emoji(self) -> str:
        """The emoji for this stage's state."""
        return STATE_EMOJI.get(self.state, UNKNOWN_EMOJI)

    @property
    def has_run(self) -> bool:
        """Whether the stage started: skipped and waiting stages show no duration."""
        return self.state not in ("skipped", "not_built", "queued") and self.start_ms > 0


def parse_tree(document: dict) -> list[Stage]:
    """The top-level stages from the Pipeline Graph View plugin's `stages/tree` response."""
    graph = document.get("data", document)
    return [_parse_stage(node) for node in graph.get("stages") or []]


def _parse_stage(node: dict) -> Stage:
    return Stage(
        id=str(node.get("id", "")),
        name=tidy_name(node.get("name") or node.get("title") or "?"),
        state=str(node.get("state") or "unknown").lower(),
        start_ms=int(node.get("startTimeMillis") or 0),
        duration_ms=node.get("totalDurationMillis"),
        children=[_parse_stage(child) for child in node.get("children") or []],
    )


def tidy_name(name: str) -> str:
    """A stage's display name: matrix cells become their axis values ("x11", "all · arm64")."""
    if not MATRIX_PREFIX.match(name):
        return name
    values = AXIS_VALUE.findall(name)
    return " · ".join(values) if values else MATRIX_PREFIX.sub("", name)


def format_duration(milliseconds: int | None) -> str:
    """A short duration: "12s", "3m 02s" or "1h 04m"."""
    seconds = max(0, (milliseconds or 0) // 1000)
    hours, seconds = divmod(seconds, 3600)
    minutes, seconds = divmod(seconds, 60)
    if hours:
        return f"{hours}h {minutes:02d}m"
    if minutes:
        return f"{minutes}m {seconds:02d}s"
    return f"{seconds}s"


def walk(stages: list[Stage], path: tuple[str, ...] = ()) -> Iterator[tuple[tuple[str, ...], Stage]]:
    """Every stage depth first, with the names of the stages it sits in."""
    for stage in stages:
        here = (*path, stage.name)
        yield here, stage
        yield from walk(stage.children, here)


def leaves(stages: list[Stage]) -> list[tuple[tuple[str, ...], Stage]]:
    """The stages that run steps themselves: the ones with no stages inside."""
    return [(path, stage) for path, stage in walk(stages) if not stage.children]


def log_focus(stages: list[Stage]) -> tuple[tuple[str, ...], Stage] | None:
    """The stage whose log is worth showing: the latest running one, else the first failure."""
    running = [entry for entry in leaves(stages) if entry[1].state == "running"]
    if running:
        return max(running, key=lambda entry: entry[1].start_ms)
    return next((entry for entry in leaves(stages) if entry[1].state == "failure"), None)


def failed_leaves(stages: list[Stage]) -> list[tuple[tuple[str, ...], Stage]]:
    """The stages that failed themselves, rather than because a stage inside them did."""
    return [(path, stage) for path, stage in leaves(stages) if stage.state == "failure"]


def plain_log(text: str) -> list[str]:
    """Every line of a raw Jenkins log as it reads on screen: no colours, notes or timestamps."""
    return [
        TIMESTAMP_PREFIX.sub("", ANSI_CODE.sub("", CONSOLE_NOTE.sub("", raw))).rstrip()
        for raw in text.splitlines()
    ]


def clean_log(text: str, line_count: int) -> list[str]:
    """The last `line_count` non-blank lines of a raw Jenkins log, shortened to fit a message."""
    lines = [line for line in plain_log(text) if line]
    shortened = [
        line if len(line) <= LOG_LINE_WIDTH else line[: LOG_LINE_WIDTH - 1] + "…" for line in lines
    ]
    return shortened[-line_count:]


def excerpt(lines: list[str], limit: int) -> str:
    """At most `limit` characters of a failed log: from the first diagnostic on, else its end."""
    start = next((i for i, line in enumerate(lines) if DIAGNOSTIC_START.match(line)), None)
    if start is None:
        text = "\n".join(lines).strip("\n")
        return text if len(text) <= limit else "…" + text[-(limit - 1) :]
    text = "\n".join(lines[start:]).strip("\n")
    return text if len(text) <= limit else text[: limit - 1] + "…"
