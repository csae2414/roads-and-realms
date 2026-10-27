#!/usr/bin/env bash
# usage: gen_codex.sh key "description" — the same prompt as gen.sh, through Codex's image tool
# (OpenAI image model), while agy's image quota is used up
cd "$(dirname "$0")"
key="$1"; desc="$2"
timeout 600 codex exec --skip-git-repo-check --sandbox workspace-write -C "$PWD" "Use your image generation tool to create exactly one image and save it as $key.png in the current directory. Image: a single small building for a strategy game, isolated: $desc. Camera: three-quarter frontal view from about 30 degrees above, the front door facing the viewer slightly to the left. Style: soft pre-rendered 1990s 3D look, smooth shading, no black outlines, no pixel art, light from the upper left, warm natural colors, realistic proportions. The building stands directly on the background: no ground plate, no grass patch, no shadow on the ground, no other objects, no text. Background: one solid flat pure green color #00FF00 everywhere around the building." > "$key.codex.log" 2>&1
echo "$(date +%T) $key $( [ -f "$key.png" ] && echo ok || echo FAIL)" >> progress.txt
