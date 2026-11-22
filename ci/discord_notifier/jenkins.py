"""A read-only client for the Jenkins JSON API and the Pipeline Graph View plugin."""

from __future__ import annotations

import base64
import json
import urllib.parse
import urllib.request
from dataclasses import dataclass

from stages import Stage, clean_log, parse_tree

REQUEST_TIMEOUT_SECONDS = 20
# Builds per branch to look at each round: enough to catch one that started and ended between
# two rounds.
BUILDS_PER_BRANCH = 3
BUILD_FIELDS = (
    "number,result,building,timestamp,duration,estimatedDuration,"
    "changeSets[items[msg,commitId,author[fullName]]]"
)


@dataclass(frozen=True)
class BuildRef:
    """A build found in the job: where to ask about it, and its public link."""

    path: str
    url: str
    branch: str
    number: int
    building: bool


@dataclass(frozen=True)
class Commit:
    """A commit the build brought in."""

    id: str
    message: str
    author: str


@dataclass(frozen=True)
class BuildInfo:
    """What a build is doing: its result so far, timing and commits."""

    number: int
    building: bool
    result: str | None
    started_ms: int
    duration_ms: int
    estimated_ms: int
    commits: list[Commit]


class Jenkins:
    """Asks the local Jenkins about one job's builds, as a user with an API token."""

    def __init__(self, base_url: str, user: str, token: str) -> None:
        self._base = base_url.rstrip("/")
        credentials = base64.b64encode(f"{user}:{token}".encode()).decode()
        self._headers = {"Authorization": f"Basic {credentials}"}

    def recent_builds(self, job: str) -> list[BuildRef]:
        """The latest builds of `job`: of each branch for a multibranch job, else of the job."""
        limit = f"{{0,{BUILDS_PER_BRANCH}}}"
        builds = f"builds[number,url,building]{limit}"
        document = self._json(f"/job/{_quote(job)}/api/json", {"tree": f"name,{builds},jobs[name,{builds}]"})
        branches = document.get("jobs")
        if branches is None:
            branches = [document]
        return [
            BuildRef(
                path=urllib.parse.urlsplit(build["url"]).path,
                url=build["url"],
                branch=branch.get("name", job),
                number=build["number"],
                building=bool(build.get("building")),
            )
            for branch in branches
            for build in branch.get("builds") or []
        ]

    def build(self, path: str) -> BuildInfo:
        """The state, timing and commits of the build at `path`."""
        document = self._json(f"{path}api/json", {"tree": BUILD_FIELDS})
        commits = [
            Commit(
                id=item.get("commitId", "")[:7],
                message=(item.get("msg") or "").strip(),
                author=(item.get("author") or {}).get("fullName", ""),
            )
            for change_set in document.get("changeSets") or []
            for item in change_set.get("items") or []
        ]
        return BuildInfo(
            number=document["number"],
            building=bool(document.get("building")),
            result=document.get("result"),
            started_ms=int(document.get("timestamp") or 0),
            duration_ms=int(document.get("duration") or 0),
            estimated_ms=int(document.get("estimatedDuration") or 0),
            commits=commits,
        )

    def stages(self, path: str) -> list[Stage]:
        """The build's stage tree, as the pipeline graph shows it."""
        return parse_tree(self._json(f"{path}stages/tree"))

    def log_tail(self, path: str, stage: Stage, line_count: int) -> list[str]:
        """The last lines of one stage's log, cleaned for display."""
        text = self._get(f"{path}stages/log", {"nodeId": stage.id}).decode("utf-8", "replace")
        return clean_log(text, line_count)

    def _json(self, path: str, params: dict[str, str] | None = None) -> dict:
        return json.loads(self._get(path, params))

    def _get(self, path: str, params: dict[str, str] | None = None) -> bytes:
        url = self._base + path
        if params:
            url += "?" + urllib.parse.urlencode(params)
        request = urllib.request.Request(url, headers=self._headers)
        with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            return response.read()


def _quote(name: str) -> str:
    return urllib.parse.quote(name, safe="")
