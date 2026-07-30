#!/usr/bin/env bash
# media-install.sh — Linux/CUDA-Installer fuer Medien-Stack des Nutzers (bild/video/tts/stt).
# Ziel: Nobara (Fedora-based, dnf), NVIDIA RTX 4070 SUPER (12 GB VRAM), CUDA 13 / Treiber 595.71.
# IDEMPOTENT: mehrfach laufbar. Legt pro Tool ein eigenes venv an, laedt Gewichte nach
# ~/AI/models-linux (getrennt von den Mac-Gewichten in ~/AI/models).
#
# STATUS: UNGETESTET auf diesem Mac (kein NVIDIA). Erst auf Nobara verifizieren.
#
# Ablauf:
#   1. System-Deps via dnf (ffmpeg, python-devel, git-lfs, Build-Tools, espeak-ng fuer Kokoro)
#   2. NVIDIA-Treiber + CUDA pruefen (klar failen, wenn nicht vorhanden)
#   3. pro-Tool-venvs: diffusers | ltx | tts | stt (torch cu130 passend zu CUDA 13)
#   4. Gewichte nach ~/AI/models-linux (hf download), LoRAs sind portabel (~/AI/loras)
#   5. Wrapper nach ~/.local/bin verlinken (bild/video/tts/stt + _*_gen.py)
set -euo pipefail

MODELS="$HOME/AI/models-linux"
MEDIA="$HOME/AI/media-linux"
VENVS="$MEDIA/venvs"
LORAS="$HOME/AI/loras"
BINSRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/../bundle/bin-linux" && pwd)"
LOCALBIN="$HOME/.local/bin"
TORCH_INDEX="https://download.pytorch.org/whl/cu130"

log() { printf '\n=== %s ===\n' "$*"; }

# --- 1. System-Deps ---------------------------------------------------------
log "System-Deps (dnf)"
SYS_PKGS=(ffmpeg-free python3-devel python3-pip git git-lfs gcc gcc-c++ make cmake espeak-ng)
if command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y "${SYS_PKGS[@]}" || {
    echo "Hinweis: ffmpeg-free evtl. nicht verfuegbar -> RPM Fusion 'ffmpeg' probieren." >&2
    sudo dnf install -y ffmpeg python3-devel python3-pip git git-lfs gcc gcc-c++ make cmake espeak-ng
  }
  git lfs install || true
else
  echo "Fehler: dnf nicht gefunden — dieses Script ist fuer Nobara/Fedora." >&2
  exit 1
fi

# --- 2. NVIDIA/CUDA-Check ---------------------------------------------------
log "NVIDIA/CUDA-Check"
if ! command -v nvidia-smi >/dev/null 2>&1; then
  echo "Fehler: nvidia-smi fehlt. NVIDIA-Treiber (>=595) installieren (Nobara-Treiber-Manager" >&2
  echo "        oder 'sudo dnf install akmod-nvidia'), neu starten, dann erneut laufen." >&2
  exit 1
fi
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader || true
VRAM_MB=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits | head -1 | tr -d ' ')
if [ "${VRAM_MB:-0}" -lt 11000 ]; then
  echo "WARNUNG: <12 GB VRAM erkannt (${VRAM_MB} MB) — die 12-GB-Presets koennen OOM gehen." >&2
fi

# --- 3. venvs ---------------------------------------------------------------
mkdir -p "$VENVS" "$MODELS" "$LORAS"

mk_venv() { # name
  local v="$VENVS/$1"
  if [ ! -x "$v/bin/python" ]; then
    python3 -m venv "$v"
  fi
  "$v/bin/pip" install --upgrade pip wheel >/dev/null
  echo "$v"
}

install_torch() { # venv-path
  # torch fuer CUDA 13. --extra-index-url wegen bekanntem cu130-Index-Bug (cuda-bindings).
  "$1/bin/pip" install torch torchvision torchaudio --extra-index-url "$TORCH_INDEX"
}

log "venv: diffusers (bild)"
V=$(mk_venv diffusers)
install_torch "$V"
# diffusers from git (FLUX.2 klein + Qwen-Image GGUF brauchen aktuelle diffusers)
"$V/bin/pip" install "git+https://github.com/huggingface/diffusers.git" \
  transformers accelerate safetensors sentencepiece protobuf \
  bitsandbytes gguf pillow "huggingface_hub[cli]"

log "venv: ltx (video)"
V=$(mk_venv ltx)
install_torch "$V"
"$V/bin/pip" install "git+https://github.com/huggingface/diffusers.git" \
  transformers accelerate safetensors imageio imageio-ffmpeg "huggingface_hub[cli]"
# fp8-Kernels optional (q8_kernels) — bei Fehlschlag laeuft der bf16-Fallback:
"$V/bin/pip" install q8_kernels 2>/dev/null || echo "Hinweis: q8_kernels nicht installiert (fp8 optional)."

log "venv: tts"
V=$(mk_venv tts)
install_torch "$V"
"$V/bin/pip" install kokoro soundfile numpy "huggingface_hub[cli]"
"$V/bin/pip" install chatterbox-tts 2>/dev/null || echo "Hinweis: chatterbox-tts Installation pruefen (--expressive)."
"$V/bin/pip" install qwen-tts 2>/dev/null || echo "Hinweis: qwen-tts Paketname auf Nobara verifizieren (--de)."

log "venv: stt"
V=$(mk_venv stt)
install_torch "$V"
"$V/bin/pip" install faster-whisper "huggingface_hub[cli]"
# NeMo (parakeet) ist gross; bei Problemen ist --whisper der Fallback.
"$V/bin/pip" install "nemo_toolkit[asr]" 2>/dev/null || echo "Hinweis: NeMo/parakeet nicht installiert — stt --whisper nutzen."

# --- 4. Gewichte ------------------------------------------------------------
log "Gewichte nach $MODELS (hf download)"
HF="$VENVS/diffusers/bin/hf"
dl() { # repo  zielordner  [extra-args...]
  local repo="$1" dst="$2"; shift 2
  if [ -e "$MODELS/$dst" ] && [ -n "$(ls -A "$MODELS/$dst" 2>/dev/null)" ]; then
    echo "vorhanden: $dst"; return 0
  fi
  "$HF" download "$repo" --local-dir "$MODELS/$dst" "$@" || \
    echo "WARNUNG: Download $repo fehlgeschlagen — spaeter nachholen." >&2
}
dl black-forest-labs/FLUX.2-klein-9B  FLUX.2-klein-9B
dl black-forest-labs/FLUX.2-klein-4B  FLUX.2-klein-4B
dl Qwen/Qwen-Image                    Qwen-Image
# Qwen-Image GGUF (nur die Q4_K_S-Datei, ~12 GB):
dl city96/Qwen-Image-gguf             Qwen-Image-gguf  --include "*Q4_K_S*"
dl Lightricks/LTX-2                   LTX-2
echo "LoRAs: flux-klein9b-nsfw-v2.safetensors + qwen-image-nsfw-lora.safetensors nach $LORAS"
echo "       (portabel — per Syncthing vom Mac oder manuell; Basis muss passen)."

# --- 5. Wrapper verlinken ---------------------------------------------------
log "Wrapper nach $LOCALBIN"
mkdir -p "$LOCALBIN"
for f in bild video tts stt _bild_gen.py _video_gen.py _tts_gen.py _stt_gen.py; do
  ln -sf "$BINSRC/$f" "$LOCALBIN/$f"
done

log "VRAM-Budget (12 GB harte Grenze)"
cat <<'EOF'
  bild --schnell : FLUX.2-klein-4B nf4        ~ 6-8 GB   schnell
  bild (quality) : FLUX.2-klein-9B nf4+offload ~11-12 GB  langsamer (Offload)
  bild --text    : Qwen-Image GGUF Q4_K_S      ~12 GB    knapp, sequential-Offload
  video          : LTX-2 fp8 + seq-offload     ~12 GB    nur kurze Clips, 768x512
  tts            : Kokoro <1 GB / Chatterbox ~4-6 GB
  stt            : parakeet ~2-3 GB / faster-whisper large-v3 int8 ~2.5 GB
  NIE zwei grosse Modelle gleichzeitig. Vor 'video' Ollama stoppen (ollama stop <m>).
EOF
log "Fertig (UNGETESTET — auf Nobara verifizieren)."
