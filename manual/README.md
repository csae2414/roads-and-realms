# The manual

`manual/build.sh de` makes the German manual, `manual/build/de/handbuch.pdf`:
1. `shots.sh`: screenshots, headless (`tools/shot.sh` / `tools/uitest.sh`, `RR_OPEN` opens the windows).
2. `crop.py`: each window cut out of its screenshot (the windows' panel colour).
3. `gen.py`: the building catalog from `data/rules/classic.toml`, the game's German names (`i18n.rs`) and
   `assets/art/buildings`; the descriptions are in `gen.py`.
4. pdfLaTeX (`handbuch.tex`, look in `style.tex`; TeX Gyre Pagella and Heros).

`--no-shots` keeps the screenshots there are.
