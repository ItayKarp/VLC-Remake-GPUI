"""Posting to a Discord forum channel through a webhook: one post, then messages inside it."""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass

REQUEST_TIMEOUT_SECONDS = 30
# Discord's edge refuses Python's default user agent; any honest name will do.
USER_AGENT = "jenkins-discord-notifier/1.0"
TOO_MANY_REQUESTS = 429
MAX_RETRIES = 3


@dataclass(frozen=True)
class Attachment:
    """A file sent with a message, which an embed can show as `attachment://<name>`."""

    name: str
    content: bytes
    content_type: str


class Webhook:
    """A forum channel's webhook: makes the post, then sends and edits messages in it."""

    def __init__(self, url: str) -> None:
        self._url = url.rstrip("/")

    def create_post(self, title: str, payload: dict) -> str:
        """Starts a forum post called `title` with `payload` as its first message; its ID."""
        message = self._send("POST", self._url, {"wait": "true"}, {**payload, "thread_name": title})
        return message["channel_id"]

    def send(self, thread_id: str, payload: dict, files: list[Attachment] | None = None) -> str:
        """Sends a message into the post; the message's ID."""
        params = {"wait": "true", "thread_id": thread_id}
        return self._send("POST", self._url, params, payload, files)["id"]

    def edit(
        self, thread_id: str, message_id: str, payload: dict, files: list[Attachment] | None = None
    ) -> None:
        """Replaces a message's content; `files` replace its attachments."""
        if files:
            payload = {
                **payload,
                "attachments": [{"id": index, "filename": f.name} for index, f in enumerate(files)],
            }
        url = f"{self._url}/messages/{message_id}"
        self._send("PATCH", url, {"thread_id": thread_id}, payload, files)

    def _send(
        self,
        method: str,
        url: str,
        params: dict[str, str],
        payload: dict,
        files: list[Attachment] | None = None,
    ) -> dict:
        body, content_type = _encode(payload, files or [])
        request = urllib.request.Request(
            f"{url}?{urllib.parse.urlencode(params)}",
            data=body,
            method=method,
            headers={"Content-Type": content_type, "User-Agent": USER_AGENT},
        )
        for attempt in range(MAX_RETRIES + 1):
            try:
                with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
                    text = response.read()
                    return json.loads(text) if text else {}
            except urllib.error.HTTPError as error:
                if error.code != TOO_MANY_REQUESTS or attempt == MAX_RETRIES:
                    detail = error.read().decode("utf-8", "replace")
                    raise RuntimeError(f"Discord {method} failed: {error.code} {detail}") from error
                time.sleep(_retry_after(error))
        raise AssertionError("the loop either returns or raises")


def _retry_after(error: urllib.error.HTTPError) -> float:
    """How long Discord asked us to wait after a 429."""
    try:
        return float(json.loads(error.read()).get("retry_after", 1.0))
    except (ValueError, AttributeError):
        return 1.0


def _encode(payload: dict, files: list[Attachment]) -> tuple[bytes, str]:
    """The request body: plain JSON, or multipart with `payload_json` when files go along."""
    if not files:
        return json.dumps(payload).encode(), "application/json"
    boundary = uuid.uuid4().hex
    parts = [_part(boundary, 'name="payload_json"', "application/json", json.dumps(payload).encode())]
    for index, attachment in enumerate(files):
        disposition = f'name="files[{index}]"; filename="{attachment.name}"'
        parts.append(_part(boundary, disposition, attachment.content_type, attachment.content))
    body = b"".join(parts) + f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


def _part(boundary: str, disposition: str, content_type: str, content: bytes) -> bytes:
    head = (
        f"--{boundary}\r\nContent-Disposition: form-data; {disposition}\r\n"
        f"Content-Type: {content_type}\r\n\r\n"
    )
    return head.encode() + content + b"\r\n"
