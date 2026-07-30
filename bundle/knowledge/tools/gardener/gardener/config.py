"""Central configuration and hard safety rules for the gardener."""
from __future__ import annotations

from pathlib import Path

DEFAULT_VAULT = Path.home() / "Knowledge"
TOOL_DIR = Path(__file__).resolve().parent.parent  # _meta/tools/gardener/
STATE_DIR = TOOL_DIR / "state"
LOG_DIR = TOOL_DIR / "logs"

OLLAMA_URL = "http://localhost:11434"
# 120 s was too tight: with another model resident the judge shares the GPU and
# a single call ran into the timeout, which used to kill the whole run.
OLLAMA_TIMEOUT = 180
# embeddinggemma's context is 2048 tokens. Measured 2026-07-12 against the live
# model: appending text past ~8000-9000 chars of German prose does not change the
# vector at all - it is silently dropped. Notes are therefore embedded in chunks
# that safely fit (4000 chars is under 2048 tokens even for token-dense code/URLs)
# and mean-pooled, so a long note's tail still influences its links.
EMBED_MAX_CHARS = 8000        # hard backstop for a single embed call
EMBED_CHUNK_CHARS = 4000
EMBED_CHUNK_OVERLAP = 200     # so a fact on a chunk border is not cut in half
EMBED_MAX_CHUNKS = 8          # 8 x 4000 = 32k chars; beyond that a note is a book
EMBED_VERSION = 2             # bump to invalidate cached (truncated) vectors
EMBED_MODEL = "embeddinggemma:latest"
JUDGE_MODEL = "ornith:9b"
VISION_MODEL = "qwen3-vl:8b"   # local image description; skipped when not pulled
MAX_VISION_BYTES = 12 * 1024**2  # bigger images are left to a human

# Hard exclusions: never read, never embed, never write anything below these.
EXCLUDE_DIRS = {"90-secrets", ".obsidian", ".git", "_meta"}
# Managed / generated top-level files: never part of the linking corpus.
EXCLUDE_FILES = {"HOT.md", "INDEX.md", "CRITICAL-FACTS.md", "review-queue.md",
                 "OPEN-QUESTIONS.md", "LOG.md"}
# Generated files at any depth: never part of the linking corpus.
EXCLUDE_ANY_DEPTH = {"review-queue.md", "MOC.md", "DECISIONS.md"}
EXCLUDE_PREFIXES = ("gardener-report-", "brain-health-")

# Sidecar layer (Brain 3.x): every non-.md file gets a `<name>.<ext>.md`
# sibling note describing it, so an agent can skip opening the real file
# unless it actually needs to.
SIDECAR_EXTRA_EXCLUDE_DIRS = {"__pycache__"}
SIDECAR_EXCLUDE_FILE_NAMES = {".DS_Store"}
SIDECAR_EXCLUDE_FILE_GLOBS = ("*.lock", "*.pyc", "*.pyo", "*.cache", "*.tmp",
                             "*.swp", "*.swo", "*~")
BRAINIGNORE_FILE = ".brainignore"
SIDECAR_EXTRACT_MAX_CHARS = 6000   # first slice handed to the local judge
SIDECAR_EXTERNAL_MB = 50           # matches the git-lfs commit cap (INDEX.md)

ASSET_DIR = "_assets"
# Raw, unreviewed material. Nothing in here may claim a structural relation to
# the canonical vault: a human still has to promote it (see linking.is_staging).
STAGING_DIR = "00-sources"
DROP_DIR = "00-sources/drop"
# Written by the B4 read-tracking hook; may not exist. Vault-relative.
HEAT_LOG = "_meta/tools/state/read-heat.log"
TRANSCRIPT_DIR = Path.home() / ".claude" / "projects"
MINED_DIR = "00-sources/mined"

MAX_NEW_LINKS_PER_NOTE = 5
NEIGHBOR_TOP_K = 5
LINK_MIN_SIMILARITY = 0.55
LINK_MIN_CONFIDENCE = 0.70   # an unsure "yes" from the judge is not a link
MERGE_MIN_SIMILARITY = 0.90
MERGE_MIN_CONFIDENCE = 0.85
STALE_MARKER_MONTHS = 6

# Topic hubs
TOPIC_MEMBER_SIM = 0.60      # note counts as hub member from this similarity
TOPIC_MAX_AUTO_MEMBERS = 12
CLUSTER_MIN_SIM = 0.70       # pairwise similarity inside a hub-suggestion cluster
CLUSTER_MIN_SIZE = 4         # muss zu SYNTH_MIN_SOURCES passen: ein Cluster,
                             # aus dem keine Seite entstehen darf, wurde bei 3
                             # gefunden und bei 4 verworfen - Lauf fuer Lauf
MAX_HUB_SUGGESTIONS = 3      # per run; clusters are disjoint and biggest-first

# Read-heat / resurfacing
COLD_MONTHS = 6              # no read in this many months -> review-queue hint
COLD_QUEUE_MAX = 5           # at most this many cold-note hints per run
RESURFACE_COUNT = 2          # "Vergessene Schaetze" per HOT.md run
HOT_HEAT_DAYS = 30

# Note size lint: rough token estimate = words * TOKENS_PER_WORD
NOTE_MAX_TOKENS = 800
TOKENS_PER_WORD = 1.3

# Transcript mining (local only, nothing ever leaves the machine)
TRANSCRIPT_DAYS = 7
TRANSCRIPT_MAX_FILES = 15
TRANSCRIPT_MAX_CHARS = 12000     # per transcript, handed to the local judge
MINE_MAX_PER_TRANSCRIPT = 3      # a transcript rarely holds more durable facts
MINE_MAX_PER_RUN = 8             # 00-sources is a queue for a human, not a dump

RUN_BUDGET_SECONDS = 45 * 60
LOCK_STALE_SECONDS = 15 * 60
# 48-GB rule: abort if a big model is already loaded in Ollama.
MAX_LOADED_MODEL_BYTES = 15 * 1024**3

GIT_AUTHOR = "<your-github-user> <you@example.com>"

RELATION_TYPES = ("relates-to", "depends-on", "supersedes", "part-of", "contradicts")
# Types that assert more than "these two are related". Only accepted when one
# note demonstrably references the other (see linking.validate_verdict).
STRUCTURAL_TYPES = ("depends-on", "supersedes", "part-of", "contradicts")

PHASES = ("linking", "consolidate", "maintain", "ingest", "sidecar", "mine",
          "lint", "synth", "all")

# Contradiction detection (`brain contradict`). Reuses JUDGE_MODEL, the 48-GB
# rule and the gardener.db embedding cache - see gardener/contradict.py.
CONTRADICT_TOP_K = 5             # semantic neighbors checked per note, not O(n^2)
CONTRADICT_MIN_CONFIDENCE = 0.75  # a missed contradiction is cheaper than a false alarm
# Vault-relative. Distinct from the tool-local, gitignored `_meta/tools/*/state/`
# caches: these three are shared/git-tracked knowledge, like the root review-queue.md.
CONTRADICT_LAST_RUN_FILE = "_meta/tools/state/contradict.json"
CONTRADICTIONS_FILE = "_meta/state/contradictions.json"
CONTRADICT_REVIEW_QUEUE = "review-queue.md"   # Wurzel, siehe queue.py

# Topic synthesis (Brain 4.x, `brain gardener run --phase synth`). Unlike
# topics.py's hand-curated hub + appended "further notes" block, a synth page
# (`30-topics/<t>/MOC.md`, `class: derived`) is written end to end by the
# gardener and treated as disposable/regenerable - see gardener/synth.py.
SYNTH_MIN_SOURCES = 4              # a page over fewer notes is noise
SYNTH_MAX_SOURCE_CHARS = 1200      # per-source excerpt handed to the judge
SYNTH_MAX_SOURCES_IN_PROMPT = 20   # keep the prompt bounded on a big topic
SYNTH_MAX_CLUSTER_CANDIDATES = 5   # new wikilink/embedding-cluster pages per run
