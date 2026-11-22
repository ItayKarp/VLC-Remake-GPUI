"""Mirrors Jenkins builds into one Discord forum post, a live-edited message per build.

Run as a service on the Jenkins machine. It asks the local Jenkins every few seconds which
builds exist; when one starts it sends a message into the forum post, edits it as stages start
and finish (with a tail of the running stage's log), and when the build ends adds a picture of
the whole pipeline. Settings come from the environment; see `notifier.env.example`.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from discord import Attachment, Webhook
from jenkins import BuildRef, Jenkins
from message import IMAGE_NAME, LogTail, build_payload
from overview import render
from stages import RESULT_WORD, format_duration, log_focus

# Discord allows a webhook about five requests per two seconds; one edit per build this often
# keeps well inside that with a few builds running.
MIN_EDIT_INTERVAL_SECONDS = 4.0
# Finished builds remembered, so a restart doesn't announce them again.
REMEMBERED_BUILDS = 200
POST_OPENING = "Builds of {job} appear here, one message each, updated while they run."

log = logging.getLogger("notifier")


@dataclass(frozen=True)
class Settings:
    """What the notifier talks to and how, from environment variables."""

    jenkins_url: str
    jenkins_user: str
    jenkins_token: str
    job: str
    webhook_url: str
    post_title: str
    thread_id: str
    state_file: Path
    poll_seconds: float
    log_lines: int

    @classmethod
    def from_environment(cls) -> Settings:
        """Reads the settings; a missing required one stops the program with its name."""
        env = os.environ
        state_dir = env.get("STATE_DIRECTORY", "/var/lib/jenkins-discord-notifier")
        try:
            return cls(
                jenkins_url=env.get("JENKINS_URL", "http://localhost:8080"),
                jenkins_user=env["JENKINS_USER"],
                jenkins_token=env["JENKINS_TOKEN"],
                job=env.get("JENKINS_JOB", "RedesignedVLC"),
                webhook_url=env["DISCORD_WEBHOOK_URL"],
                post_title=env.get("DISCORD_POST_TITLE", "RedesignedVLC"),
                thread_id=env.get("DISCORD_THREAD_ID", ""),
                state_file=Path(env.get("STATE_FILE", f"{state_dir}/state.json")),
                poll_seconds=float(env.get("POLL_SECONDS", "5")),
                log_lines=int(env.get("LOG_LINES", "15")),
            )
        except KeyError as missing:
            sys.exit(f"Missing setting {missing.args[0]}: see notifier.env.example")


class Notifier:
    """Keeps one Discord message per build in step with Jenkins."""

    def __init__(self, settings: Settings) -> None:
        self._settings = settings
        self._jenkins = Jenkins(settings.jenkins_url, settings.jenkins_user, settings.jenkins_token)
        self._webhook = Webhook(settings.webhook_url)
        self._state = _load(settings.state_file)
        self._last_edit: dict[str, float] = {}

    def run_forever(self) -> None:
        """Checks Jenkins every few seconds until stopped; errors are logged and retried."""
        while True:
            try:
                self.check()
            except Exception:
                log.exception("Round failed; trying again")
            time.sleep(self._settings.poll_seconds)

    def check(self) -> None:
        """One round: announce new builds, update running ones, finish ended ones."""
        refs = self._jenkins.recent_builds(self._settings.job)
        builds = self._state.setdefault("builds", {})
        if not self._state.get("seeded"):
            # First run: builds that ended before the notifier existed stay unannounced.
            for ref in refs:
                if not ref.building:
                    builds[ref.path] = {"done": True}
            self._state["seeded"] = True
        for ref in sorted(refs, key=lambda r: r.number):
            if not builds.get(ref.path, {}).get("done"):
                self._update(ref, builds.setdefault(ref.path, {}))
        _forget_old(builds)
        _save(self._settings.state_file, self._state)

    def _update(self, ref: BuildRef, record: dict) -> None:
        info = self._jenkins.build(ref.path)
        stages = self._jenkins.stages(ref.path)
        payload = build_payload(self._settings.job, ref, info, stages, self._log_tail(ref, stages))
        digest = hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()
        thread_id = self._thread_id()
        if "message_id" not in record:
            record["message_id"] = self._webhook.send(thread_id, _without_image(payload))
            log.info("Announced %s #%s", ref.branch, ref.number)
        elif info.building:
            due = time.monotonic() - self._last_edit.get(ref.path, 0.0) >= MIN_EDIT_INTERVAL_SECONDS
            if digest != record.get("digest") and due:
                self._webhook.edit(thread_id, record["message_id"], payload)
                self._last_edit[ref.path] = time.monotonic()
        if not info.building:
            result = RESULT_WORD.get(info.result or "", info.result or "Finished")
            title = f"{self._settings.job} · {ref.branch} #{info.number}"
            picture = render(title, f"{result} in {format_duration(info.duration_ms)}", stages)
            image = Attachment(IMAGE_NAME, picture, "image/png")
            self._webhook.edit(thread_id, record["message_id"], payload, [image])
            record["done"] = True
            log.info("Finished %s #%s: %s", ref.branch, ref.number, info.result)
        record["digest"] = digest

    def _log_tail(self, ref: BuildRef, stages: list) -> LogTail | None:
        focus = log_focus(stages)
        if focus is None:
            return None
        path, stage = focus
        lines = self._jenkins.log_tail(ref.path, stage, self._settings.log_lines)
        return LogTail(path=path, lines=lines, running=stage.state == "running")

    def _thread_id(self) -> str:
        """The forum post's ID: from the settings, from the last run, or a new post."""
        thread_id = self._settings.thread_id or self._state.get("thread_id")
        if not thread_id:
            opening = {"content": POST_OPENING.format(job=self._settings.job)}
            thread_id = self._webhook.create_post(self._settings.post_title, opening)
            log.info("Created the forum post %s", thread_id)
        self._state["thread_id"] = thread_id
        return thread_id


def _without_image(payload: dict) -> dict:
    """The payload without its picture: a new message has no attachment to point at yet."""
    embeds = [{k: v for k, v in embed.items() if k != "image"} for embed in payload["embeds"]]
    return {**payload, "embeds": embeds}


def _forget_old(builds: dict) -> None:
    done = [path for path, record in builds.items() if record.get("done")]
    for path in done[: max(0, len(done) - REMEMBERED_BUILDS)]:
        del builds[path]


def _load(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return {}


def _save(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(state, indent=1))
    temporary.replace(path)


def main() -> None:
    """Starts the notifier with settings from the environment."""
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    Notifier(Settings.from_environment()).run_forever()


if __name__ == "__main__":
    main()
