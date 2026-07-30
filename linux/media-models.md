# Media-Modelle: Mac (MLX) -> Linux (CUDA, RTX 4070 SUPER 12 GB)

STATUS: UNGETESTET auf diesem Mac. VRAM-Zahlen aus Modellkarten/Community-Guides (Juli 2026),
auf Nobara mit `nvidia-smi` beim ersten Lauf zu bestaetigen. Der Mac faehrt bis 32 GB (8-bit);
auf 12 GB VRAM ist alles kleiner quantisiert + CPU/sequential-Offload (deshalb langsamer).

## bild

| Mac-Modus | Mac-Modell (MLX) | Linux-HF-Repo | Quant (12 GB) | VRAM ~ | Speed-Caveat |
|---|---|---|---|---|---|
| `--schnell` | Z-Image-Turbo | `black-forest-labs/FLUX.2-klein-4B` | bnb NF4, 8 Schritte | 6-8 GB | schnellste Route; passt locker |
| (quality) | flux2-klein-9b-4bit | `black-forest-labs/FLUX.2-klein-9B` | bnb NF4 + `enable_model_cpu_offload` | 11-12 GB | Offload macht es spuerbar langsamer als Mac |
| `--text` | qwen-image-8bit | `Qwen/Qwen-Image` + `city96/Qwen-Image-gguf` (Q4_K_S) | GGUF Q4_K_S + `enable_sequential_cpu_offload` | ~12 GB | knapp; bestes Text-Rendering; nichts parallel |
| `--unzensiert` | klein-9B + NSFW-LoRA | `FLUX.2-klein-9B` + `flux-klein9b-nsfw-v2.safetensors` | NF4 + Offload + LoRA | 11-12 GB | 9B-Basis ist ungefiltert; LoRA portabel |
| `--unzensiert-gross` | qwen-image + NSFW-LoRA | `Qwen/Qwen-Image` GGUF + `qwen-image-nsfw-lora.safetensors` | GGUF Q4_K_S + Offload + LoRA | ~12 GB | langsamste bild-Route |
| `--bearbeiten` beste | flux2-klein-9b-8bit edit | `FLUX.2-klein-9B` (`Flux2KleinPipeline`, Bild-Referenz-Input) | NF4 + Offload | 11-12 GB | FLUX.2 klein vereint Gen+Edit im selben Modell — kein separates Edit-Modell noetig |
| `--bearbeiten` sparsam | flux2-klein-4b-4bit edit | `FLUX.2-klein-4B` | NF4, wenige Schritte | 6-8 GB | schnell; wie am Mac fuer NSFW-Sparmodus ungeeignet -> nur zensiert |

Verworfen fuer 12 GB:
- **FLUX.2-klein-9B fp16/8-bit direkt** (~29 GB / >16 GB) — sprengt 12 GB ohne 4-bit+Offload.
- **Qwen-Image bf16/fp8-full auf GPU** (>16 GB) — nur GGUF Q4 + sequential-Offload passt.
- **Z-Image-Turbo als 1:1-Port** — auf CUDA nicht besser als klein-4B-NF4; klein-4B haelt die
  Modell-Familie einheitlich (weniger Downloads/venvs). Optional nachruestbar.

Backend-Wahl **diffusers (CUDA)**, nicht ComfyUI: der Mac-`bild` ist ein scriptbares CLI ohne
Server. diffusers bildet das 1:1 nach (ein `python`-Aufruf pro Bild, Quant + Offload in Code),
waehrend ComfyUI einen laufenden Server + Workflow-JSON braeuchte. GGUF laedt diffusers via
`GGUFQuantizationConfig`/`from_single_file`, NF4 via `BitsAndBytesConfig`.

## video

| Mac | Linux-HF-Repo | Quant (12 GB) | VRAM ~ | Caveat |
|---|---|---|---|---|
| `video` (distilled) | `Lightricks/LTX-2` (`LTX2Pipeline`), fp8-Variante `ltx-2-19b-dev-fp8` | fp8 + `enable_sequential_cpu_offload` | ~12 GB (Minimum) | nur kurze Clips, 768x512; `num_frames`=8n+1, Kanten durch 32 teilbar |
| `video --hq` | dasselbe Repo, voller Pfad | fp8 + Offload, mehr Schritte | ~12 GB | deutlich langsamer; 12 GB ist LTX-2-Minimum, RAM-Offload bremst stark |

Mac nutzt `ltx-2-mlx` (MLX-Port) — Linux geht zurueck auf die **offizielle Lightricks-CUDA-
Pipeline** (`Lightricks/LTX-Video` / LTX-2 in diffusers).

## tts

| Mac-Flag | Mac (mlx-audio) | Linux-Paket + Repo | VRAM ~ |
|---|---|---|---|
| (default) | `mlx-community/Kokoro-82M-bf16` | `kokoro` (hexgrad) PyTorch, `hexgrad/Kokoro-82M` | <1 GB |
| `--expressive` | `chatterbox-turbo-8bit` | `chatterbox-tts` (resemble-ai) CUDA | ~4-6 GB |
| `--de` | `Qwen3-TTS-12Hz-0.6B-8bit` | `qwen-tts`, `Qwen/Qwen3-TTS-12Hz-0.6B-Base` | ~2-3 GB |

Ausgabe wie am Mac: genau eine Datei `<prefix>_000.wav` (24 kHz), Pfad auf stdout.

VERIFIED 2026-07-19 on Peer-Rechner: Kokoro default works on GPU (torch 2.13.0+cu130, `cuda: True`).
CORRECTION: the tts venv must be **Python 3.12**, NOT the system 3.14 — kokoro pins
`numpy==1.26.4`, which has no cp314 wheel and fails to build from source. Build it with
`uv venv --python 3.12 ~/AI/media-linux/venvs/tts` (uv already manages a 3.12 here). Kokoro's
misaki English G2P installs the spacy model `en_core_web_sm` into the venv on first use via
uv, which needs `VIRTUAL_ENV` set — the `tts` wrapper now exports it. `--expressive`
(chatterbox) and `--de` (qwen-tts) still to verify on the 3.12 venv.

## stt

VERIFIED 2026-07-19 on Peer-Rechner (RTX 4070 SUPER, CUDA 13). CORRECTION: faster-whisper /
CTranslate2 4.8.1 links **CUDA 12** (`libcublas.so.12`) and FAILS on this CUDA-13 box
(GPU) — it only runs on CPU there. This machine already runs **ein fremder Dienst's whisper.cpp**,
a **CUDA-13-native** build (`~/.local/bin/whisper-cli`, links `libcublas.so.13`) with the
ggml models already on disk in `~/.local/share/whisper-models/` (large-v3, large-v3-turbo,
base, tiny, silero-VAD). So the Linux `stt` default now REUSES that — GPU works, zero
download.

| Mac-Flag | Mac (MLX) | Linux-Engine + Modell | VRAM ~ | Status |
|---|---|---|---|---|
| (default) | `mlx-whisper` large-v3-turbo | **whisper.cpp** `whisper-cli` + `~/.local/share/whisper-models/ggml-large-v3-turbo.bin` (CUDA-13 native, reuses ein fremder Dienst) | ~2 GB | VERIFIED GPU |
| `--timestamps` | — | whisper.cpp `-osrt` (SRT to stdout) | ~2 GB | VERIFIED |
| `--whisper` | `mlx-whisper` | `faster-whisper` (CTranslate2) `large-v3` — **CPU only on CUDA-13** (needs CUDA-12 libs for GPU) | ~2.5 GB (CPU) | CPU-only here |
| `--parakeet` | `parakeet-mlx` | NVIDIA NeMo `nvidia/parakeet-tdt-0.6b-v2` (large NeMo install) | ~2-3 GB | not installed |

`--timestamps` erzeugt SRT. Transkript auf stdout. To use faster-whisper on GPU here, install
CUDA-12 runtime libs into its venv (`pip install nvidia-cublas-cu12 nvidia-cudnn-cu12`) and
put them on `LD_LIBRARY_PATH` — but whisper.cpp already covers GPU, so this is rarely needed.

## Nobara-Verifikation — Stand 2026-07-19 (Peer-Rechner)

VERIFIED (klein-Ebene, Wahl des Nutzers "klein jetzt, schwer aufschieben"):
- **stt** — whisper.cpp GPU (large-v3-turbo, reused). Round-trip mit tts bestanden.
- **tts** — kokoro auf py3.12-venv, GPU. Round-trip bestanden.

GLOBALE KORREKTUR (gilt fuer ALLE ML-venvs): System-Python ist **3.14**, zu neu fuer torch/
kokoro/etc. `media-install.sh` muss die venvs mit **`uv venv --python 3.12`** bauen (uv managed
hier schon 3.12), nicht mit `python3 -m venv`. torch 2.13.0+cu130 hat cp312-Wheels.

OFFEN (schwere Ebene, bewusst aufgeschoben — Disk ist mit 196 GB frei kein Blocker mehr):
- **bild**: diffusers-Pipeline-Klasse FLUX.2 klein (`Flux2KleinPipeline` vs `Flux2Pipeline`),
  NF4-Laden aus Basis-Ordner vs `*-bnb-4bit`-Export. venv py3.12.
- **Qwen-Image** GGUF Q4_K_S realer Peak auf 12 GB (~12.3 GB laut Guide — evtl. Q4_K_M zu gross).
- **video/LTX-2** fp8: q8_kernels-Abhaengigkeit; sonst bf16-Fallback + reiner Offload.
- **tts `--expressive`/`--de`** (chatterbox / qwen-tts) auf der 3.12-venv verifizieren.
- **stt `--parakeet`** (NeMo) — gross; whisper.cpp deckt den Default bereits GPU-nativ ab.
