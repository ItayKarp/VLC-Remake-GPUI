"""The Discord message for one build: an embed with its stages, commits and a log tail."""

from __future__ import annotations

from dataclasses import dataclass

from jenkins import BuildInfo, BuildRef
from stages import RESULT_WORD, Stage, format_duration

# Discord's embed limits: description and field value.
DESCRIPTION_LIMIT = 4096
FIELD_LIMIT = 1024
COMMITS_SHOWN = 3
COMMIT_MESSAGE_WIDTH = 72

COLOUR_RUNNING = 0x5865F2
RESULT_COLOUR = {
    "SUCCESS": 0x23A55A,
    "FAILURE": 0xF23F43,
    "UNSTABLE": 0xF0B232,
    "ABORTED": 0x80848E,
    "NOT_BUILT": 0x80848E,
}
IMAGE_NAME = "pipeline.png"


@dataclass(frozen=True)
class LogTail:
    """The last lines of the stage worth watching, and where that stage sits."""

    path: tuple[str, ...]
    lines: list[str]
    running: bool


def build_payload(
    job: str, ref: BuildRef, info: BuildInfo, stages: list[Stage], log: LogTail | None
) -> dict:
    """The whole message for the build as it stands now."""
    embed = {
        "title": f"{job} · {ref.branch} #{info.number}",
        "url": ref.url,
        "color": RESULT_COLOUR.get(info.result or "", COLOUR_RUNNING),
        "description": _clip(_description(info, stages), DESCRIPTION_LIMIT),
        "footer": {"text": _status_line(info)},
    }
    if log and log.lines:
        embed["fields"] = [_log_field(log)]
    if not info.building:
        embed["image"] = {"url": f"attachment://{IMAGE_NAME}"}
    return {"embeds": [embed], "allowed_mentions": {"parse": []}}


def _description(info: BuildInfo, stages: list[Stage]) -> str:
    started = f"Started <t:{info.started_ms // 1000}:R>"
    return "\n".join([*_commit_lines(info), started, "", *_stage_lines(stages)])


def _commit_lines(info: BuildInfo) -> list[str]:
    if not info.commits:
        return ["No new commits (started by hand or by a branch scan)"]
    lines = []
    for commit in info.commits[:COMMITS_SHOWN]:
        title = commit.message.splitlines()[0] if commit.message else ""
        if len(title) > COMMIT_MESSAGE_WIDTH:
            title = title[: COMMIT_MESSAGE_WIDTH - 1] + "…"
        lines.append(f"`{commit.id}` {title} — {commit.author}")
    hidden = len(info.commits) - COMMITS_SHOWN
    if hidden > 0:
        lines.append(f"…and {hidden} more")
    return lines


def _stage_lines(stages: list[Stage]) -> list[str]:
    """One line per stage; a stage of matrix cells gets a line per cell listing its steps."""
    lines = []
    for stage in stages:
        lines.append(f"{stage.emoji} **{stage.name}**{_duration(stage)}")
        for cell in stage.children:
            steps = "  ".join(f"{step.emoji} {step.name}" for step in cell.children)
            lines.append(f" {cell.emoji} **{cell.name}**{_duration(cell)}")
            if steps:
                lines.append(f"  {steps}")
    return lines


def _duration(stage: Stage) -> str:
    if not stage.has_run:
        return " · skipped" if stage.state == "skipped" else ""
    return f" · {format_duration(stage.duration_ms)}"


def _log_field(log: LogTail) -> dict:
    title = "Live log" if log.running else "Failure log"
    body = "\n".join(log.lines).replace("```", "'''")
    room = FIELD_LIMIT - len("```\n\n```")
    if len(body) > room:
        body = "…" + body[-(room - 1) :]
    return {"name": f"{title} · {' › '.join(log.path)}", "value": f"```\n{body}\n```"}


def _status_line(info: BuildInfo) -> str:
    if info.building:
        estimate = f" · usually takes {format_duration(info.estimated_ms)}" if info.estimated_ms > 0 else ""
        return f"Running{estimate}"
    word = RESULT_WORD.get(info.result or "", info.result or "Finished")
    return f"{word} in {format_duration(info.duration_ms)}"


def _clip(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[: limit - 1] + "…"
