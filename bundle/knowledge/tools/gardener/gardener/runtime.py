"""Runtime plumbing: lock file (self-healing), deadline, git snapshots, logging."""
from __future__ import annotations

import datetime as dt
import json
import logging
import os
import subprocess
import time
from pathlib import Path

from . import config

log = logging.getLogger("gardener")

LAST_RUN_FILE = "last-run.json"


class LockHeldError(Exception):
    pass


class Deadline:
    def __init__(self, budget_seconds: float = config.RUN_BUDGET_SECONDS):
        self.end = time.monotonic() + budget_seconds

    def expired(self) -> bool:
        return time.monotonic() >= self.end


class Lock:
    """PID+timestamp lock file. A lock whose holder PID is dead, or whose age
    exceeds the stale threshold when the PID is unreadable, is taken over.
    A live holder keeps the lock regardless of age (runs may exceed 15 min)."""

    def __init__(self, path: Path, stale_seconds: float = config.LOCK_STALE_SECONDS):
        self.path = Path(path)
        self.stale_seconds = stale_seconds

    def _stale(self) -> bool:
        try:
            pid = int(self.path.read_text().split()[0])
        except (OSError, ValueError, IndexError):
            pid = None
        if pid is not None:
            try:
                os.kill(pid, 0)
                return False            # holder alive
            except ProcessLookupError:
                return True             # holder dead
            except PermissionError:
                return False            # alive, other user
        age = time.time() - self.path.stat().st_mtime
        return age >= self.stale_seconds

    def acquire(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        for attempt in (1, 2):
            try:
                fd = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            except FileExistsError:
                if attempt == 2 or not self._stale():
                    raise LockHeldError(
                        f"lock held ({self.path}) - another run active")
                log.warning("taking over stale lock %s", self.path)
                try:
                    self.path.unlink()
                except FileNotFoundError:
                    pass
                continue
            with os.fdopen(fd, "w") as f:
                f.write(f"{os.getpid()} {dt.datetime.now().isoformat()}\n")
            return

    def release(self) -> None:
        try:
            if self.path.read_text().split()[0] == str(os.getpid()):
                self.path.unlink()
        except (OSError, IndexError):
            pass


def git_commit(vault: Path, message: str, dry_run: bool = False) -> bool:
    """Commit everything in the vault as <your-github-user>. Never pushes."""
    if dry_run:
        log.info("dry-run: skipping git commit (%s)", message)
        return False
    try:
        subprocess.run(["git", "-C", str(vault), "add", "-A"],
                       check=True, capture_output=True, timeout=60)
        r = subprocess.run(
            ["git", "-C", str(vault), "commit",
             "--author", config.GIT_AUTHOR, "-m", message],
            capture_output=True, text=True, timeout=60)
        if r.returncode == 0:
            log.info("git commit: %s", message)
            return True
        if "nothing to commit" in (r.stdout + r.stderr):
            log.info("git: nothing to commit (%s)", message)
            return False
        log.error("git commit failed: %s%s", r.stdout, r.stderr)
        return False
    except Exception as e:
        log.error("git commit error: %s", e)
        return False


def write_last_run(state_dir: Path, data: dict) -> Path | None:
    """Record the outcome of a real run so Brain.app can show it without
    parsing logs. Never written by a dry-run (the caller decides)."""
    path = Path(state_dir) / LAST_RUN_FILE
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        return path
    except OSError as e:
        log.warning("could not write %s: %s", path, e)
        return None


def read_last_run(state_dir: Path) -> dict | None:
    path = Path(state_dir) / LAST_RUN_FILE
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def setup_logging(log_dir: Path, verbose: bool = False) -> Path:
    log_dir.mkdir(parents=True, exist_ok=True)
    logfile = log_dir / f"gardener-{dt.date.today().strftime('%Y%m%d')}.log"
    handlers = [logging.FileHandler(logfile), logging.StreamHandler()]
    logging.basicConfig(
        level=logging.DEBUG if verbose else logging.INFO,
        format="%(asctime)s %(levelname)s %(message)s",
        handlers=handlers, force=True)
    # date-based rotation: prune logs older than 30 days
    cutoff = time.time() - 30 * 86400
    for f in log_dir.glob("gardener-*.log"):
        try:
            if f.stat().st_mtime < cutoff:
                f.unlink()
        except OSError:
            pass          # vanished under us: nothing to prune
    return logfile
