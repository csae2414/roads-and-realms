#!/usr/bin/env bash
# The manual as a PDF: screenshots (headless), window crops, the building catalog, pdfLaTeX.
#   manual/build.sh de            everything
#   manual/build.sh de --no-shots  keep the screenshots there are
set -euo pipefail
cd "$(dirname "$0")"
lang="${1:-de}"; shift || true
src=$([ "$lang" = de ] && echo handbuch || echo manual)
if [ "${1:-}" != "--no-shots" ] || [ ! -d "build/$lang/img" ]; then ./shots.sh "$lang"; fi
uv run -q --no-project --with pillow python crop.py "$lang" >/dev/null
python3 gen.py "$lang" >/dev/null
latexmk -pdf -interaction=nonstopmode -halt-on-error -output-directory="build/$lang" "$src.tex" >"build/$lang/latex.log" 2>&1 \
  || { tail -40 "build/$lang/$src.log"; exit 1; }
echo "manual/build/$lang/$src.pdf"
