#!/usr/bin/env bash
# Downloads the Whisper models Order Check uses into models/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p models
BASE=https://huggingface.co/ggerganov/whisper.cpp/resolve/main
for m in ggml-large-v3-turbo-q5_0.bin ggml-base.en.bin; do
  if [ -s "models/$m" ]; then
    echo "models/$m already present"
  else
    echo "downloading $m"
    curl -fL --progress-bar -o "models/$m" "$BASE/$m"
  fi
done
