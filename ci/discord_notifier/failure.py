"""The message sent after a build fails: a mention, the first errors, and the full logs."""

from __future__ import annotations

import re
from dataclasses import dataclass

from discord import Attachment
from jenkins import BuildInfo, BuildRef
from stages import excerpt, format_duration

# Discord's limits: characters in a message, and files on one.
MESSAGE_LIMIT = 2000
ATTACHMENT_LIMIT = 10
UNSAFE_FILE_CHARACTERS = re.compile(r"[^a-z0-9]+")


@dataclass(frozen=True)
class FailedStage:
    """A stage that failed, where it sits, and every line of its log."""

    path: tuple[str, ...]
    log: list[str]

    @property
    def label(self) -> str:
        """Where the stage sits, as people read it: "Linux › all › Clippy"."""
        return " › ".join(self.path)

    @property
    def file_name(self) -> str:
        """A file name for its log: "linux-all-clippy.txt"."""
        return UNSAFE_FILE_CHARACTERS.sub("-", "-".join(self.path).lower()).strip("-") + ".txt"


@dataclass(frozen=True)
class Recipient:
    """Who the failure message is for, and the build message it answers."""

    mention_user_id: str
    build_message_link: str


def failure_message(
    job: str, ref: BuildRef, info: BuildInfo, failed: list[FailedStage], recipient: Recipient
) -> tuple[dict, list[Attachment]]:
    """The message and its log files: who to tell, what failed, and the first errors."""
    mention_user_id = recipient.mention_user_id
    mention = f"<@{mention_user_id}> " if mention_user_id else ""
    headline = f"{mention}❌ **[{job} · {ref.branch} #{info.number}]({ref.url})** failed"
    lines = [f"{headline} in {format_duration(info.duration_ms)}"]
    if recipient.build_message_link:
        lines.append(f"↪️ Re: [the build's message]({recipient.build_message_link})")
    if failed:
        lines.append("Failed: " + ", ".join(f"**{stage.label}**" for stage in failed))
    else:
        lines.append("No stage failed on its own: the pipeline itself stopped. See Jenkins.")
    files = [_log_file(stage) for stage in failed[:ATTACHMENT_LIMIT] if any(stage.log)]
    footer = "Full logs attached." if files else ""
    content = "\n".join(lines)
    if failed and any(failed[0].log):
        content += "\n" + _code_block(failed[0], MESSAGE_LIMIT - len(content) - len(footer) - 2)
    if footer:
        content += "\n" + footer
    allowed = {"users": [mention_user_id]} if mention_user_id else {"parse": []}
    return {"content": content, "allowed_mentions": allowed}, files


def _code_block(stage: FailedStage, room: int) -> str:
    title = f"First errors in {stage.label}:\n"
    fence = "```\n", "\n```"
    body_room = room - len(title) - len(fence[0]) - len(fence[1])
    body = excerpt(stage.log, body_room).replace("```", "'''")
    return f"{title}{fence[0]}{body}{fence[1]}"


def _log_file(stage: FailedStage) -> Attachment:
    return Attachment(stage.file_name, "\n".join(stage.log).encode(), "text/plain; charset=utf-8")
