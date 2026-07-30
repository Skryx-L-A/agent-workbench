# Lokale Modelle auf Linux (Nobara) — Ollama + llama.cpp, CUDA, 12 GB VRAM

STATUS: UNGETESTET auf diesem Mac. Auf Nobara + RTX 4070 SUPER (12 GB) verifizieren.
Der Mac-Standard (Q6/Q8, 35B, 128K, alles im 48-GB-Unified-Memory) gilt hier NICHT — 12 GB
dedizierter VRAM ist die harte Grenze. Realistische Auswahl unten.

## Ollama (nativ, systemd) — 128K-Kontext via drop-in

Auf macOS steckt `OLLAMA_CONTEXT_LENGTH` in einem launchd-plist. Auf Nobara laeuft Ollama als
**systemd-Service** — das plist gilt NICHT. Stattdessen ein systemd-Drop-in:

```bash
sudo mkdir -p /etc/systemd/system/ollama.service.d
sudo tee /etc/systemd/system/ollama.service.d/override.conf >/dev/null <<'EOF'
[Service]
Environment="OLLAMA_CONTEXT_LENGTH=131072"
EOF
sudo systemctl daemon-reload
sudo systemctl restart ollama
```

Pruefen: `ollama ps` (CONTEXT-Spalte) nach dem Laden eines Modells. Installation von Ollama auf
Nobara: `curl -fsSL https://ollama.com/install.sh | sh` (installiert den systemd-Service +
NVIDIA-Support automatisch, wenn CUDA-Treiber da sind).

WICHTIG: 131072 als KV-Cache auf 12 GB ist teuer — der Kontext-Cache selbst frisst VRAM. Bei
groesseren Modellen 128K real nur mit KV-Quant (`OLLAMA_KV_CACHE_TYPE=q8_0`) und/oder kleinerem
`num_ctx` pro Aufruf. Auf 12 GB ist volles 128K nur bei kleinen Modellen praktisch.

## Was passt in 12 GB VRAM (realistisch)

Der Mac-Default `ornith:35b` (35B) passt **NICHT** in 12 GB — auch nicht bei Q4 (ein 35B-Q4
ist ~20 GB Gewichte, plus KV-Cache). Ehrliche Auswahl fuer 12 GB, voll auf der GPU:

| Rolle | Modell | Quant | Gewichte ~ | Kontext realistisch |
|---|---|---|---|---|
| Coder (default) | Qwen3-Coder / Qwen2.5-Coder **7B** | Q5_K_M/Q6_K | ~5-6 GB | 32K-64K bequem, 128K mit KV-q8 |
| Coder (staerker) | 14B-Klasse (Qwen2.5-Coder-14B, Codestral-ish) | Q4_K_M | ~9-10 GB | 16K-32K; 128K sprengt VRAM |
| Bulk/schnell | 3B-4B (Qwen3-4B, Llama-3.2-3B) | Q6_K/Q8 | ~3-4 GB | volles 128K moeglich |
| Embeddings | nomic-embed / bge-m3 | — | <1 GB | — |

Faustregel: **7B ist der praktische Coder-Sweetspot** auf 12 GB (voll GPU, brauchbarer Kontext).
14B nur mit reduziertem Kontext. Nichts >14B erwartet 100 % GPU auf 12 GB — Spill auf CPU macht
es unbrauchbar langsam. `ollama ps` muss 100 % GPU zeigen; sobald "CPU" auftaucht: Modell/Quant/
Kontext kleiner.

Vor jedem Medien-Lauf (bild-quality/text, video) das grosse Coder-Modell stoppen
(`ollama stop <modell>`) — Medien + LLM teilen sich dieselben 12 GB und kollidieren sonst.

## llama.cpp (CUDA-Build)

Fuer GGUF-Modelle, die Ollama nicht nativ faehrt, oder fuer Vision/fp8:

```bash
git clone https://github.com/ggml-org/llama.cpp
cd llama.cpp
cmake -B build -DGGML_CUDA=on
cmake --build build --config Release -j
# Server (128K, so viele Layer auf die GPU wie in 12 GB passen — Rest bleibt CPU):
./build/bin/llama-server -m <modell>.gguf -c 131072 -ngl 999 --host 127.0.0.1 --port 8080
```

`-ngl 999` versucht alle Layer auf die GPU; bei OOM `-ngl` senken (weniger Layer, Rest CPU) oder
`-c` reduzieren. `llama-bench` zum Vergleich von Quants/Layer-Splits.

## Offene Punkte (Nobara-Verifikation)
- Exaktes Coder-Modell + Quant, das 100 % GPU haelt und noch nuetzlichen Kontext hat (7B Q5/Q6
  messen; 14B Q4 gegen den Kontextbedarf abwaegen).
- KV-Cache-Quant (`OLLAMA_KV_CACHE_TYPE=q8_0`) real testen — passt 128K damit auf 12 GB?
- Pi-`models.json` `contextWindow` <= tatsaechlich bedientem Wert halten (wie am Mac).
