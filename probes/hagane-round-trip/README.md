# Hagane Studio focus round trip probe (S04 gate 10)

Times Hagane Studio's focus editor round trip on F-FOCUS-SMALL, the tree HOI4 Factory's own measurement uses (`measure_preview_and_interaction` in `crates/server/tests/focus.rs`). Hagane Studio is GPL-3.0 and is built here from its public source and only run. No Hagane code enters this repository (A1-1, A1-2).

## Run

From a scratch folder under `.work/` (short paths for Windows):

```sh
git -c core.autocrlf=false clone https://github.com/hagane-works/hagane-studio hs
git -C hs checkout 191c2fe54e1d2ddc76ab2dc4fa699e96f3766c25
printf 'hs/.git\nhs/web/node_modules\n' > .dockerignore
docker build -t hf-hagane-measure:1 -f <repo>/probes/hagane-round-trip/Dockerfile .
docker run --rm --cpus 4 -v "<game folder>:/game:ro" \
  -v "<repo>/crates/server/tests/fixtures/focus-small:/fixture:ro" \
  -v "<repo>/probes/hagane-round-trip/measure.sh:/measure.sh:ro" -v "<out>:/out" \
  hf-hagane-measure:1 sh /measure.sh
```

On Windows set `MSYS_NO_PATHCONV=1` and pass host paths through `cygpath -m`. The image builds only the Go server, with a placeholder page instead of the React interface. The probe drives the same HTTP routes the interface calls. The game folder is mounted read-only because Hagane checks focus trees against the base game. The game is never started.

## What it measures

1. A session cookie from loading the page, as a browser gets it, then a new project and the focus file imported through `POST /api/go/projects/{id}/import/focus-tree`.
2. Five moves of `exp_right`, each timed from `PUT .../node-positions` (what a drag in the focus graph sends) until `GET .../nodes` reads the new position back. Hagane saves this in its own JSON project, not in native mod files.
3. Five moves each followed by `export_project` through the tool API, which writes native mod files into a ZIP. The probe unpacks the last ZIP and prints the exported focus file.

Remove the image afterwards with `docker rmi hf-hagane-measure:1`.
