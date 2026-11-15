# T-ENG-01 on the HOI4 Agent Tools 4.18.0 pin

Evidence kind: tool execution in throwaway containers without network. `scripts/t-eng-01.sh hf-s04ui-server:latest` was run on 2026-11-15 on the Windows PC against the server image built from branch `s04-finish` at `db01c97`, and against an image built by the script with `ENGINE_BINARY=omit`. Exit code 0.

- `image.txt` names the image.
- `run.log` holds the script's eight checks, all ok.
- With the engine: `engine-status` answers available for `hoi4-engine` 0.1.0 at revision `fb78624c7c76c993268c33fe1c50a62632547b98`, and an engine-layout `hoi4.focus_rewrite` on F-FOCUS-SMALL answers `FOCUS_CHANGES_APPLIED`.
- Without the engine binary: `engine-status` answers `binary_not_found` with exit 1, and the same rewrite answers `FOCUS_ENGINE_UNAVAILABLE` and leaves the source unchanged.

The run on the earlier pin is in [t-eng-01](../t-eng-01/run.log). Both images were removed after the run.
