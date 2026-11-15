# Installed Store qualification

Run these commands from the repository root after the [development setup](../README.md#development):

```sh
make qualification-setup
make qualification-test
```

IC Testkit owns server preparation and admission. `IC_TESTKIT_SERVER_DIRECTORY`
selects a different bundle directory for both commands; the default is
`target/qualification/server`. Setup provisions the bundle explicitly. Tests
require an already prepared bundle and permission to start a local server
listening on loopback; the harness starts and closes that server.

The test script builds the ordinary Store and a distinct executable with
`maintenance-candidate`, then passes their paths to the independent qualification
workspace. It writes build records and Wasm artifacts under `target/qualification/`.
A separate `make build` is unnecessary for this test. The candidate feature is
for qualification only and must not be selected for live deployment.

## Coverage

The installed witness exercises:

- Unauthorized requests and oversized upload frames.
- An 8 MiB upload interrupted by upgrade, persistent reopen and upload resume.
- Immutable version conflicts, shared bytes, quota changes and exact chunk reconstruction.
- Deletion of aliases and last-reference payload reclamation, upgrade with tombstones,
  cancellation, exact restoration, conflicting-identity refusal and pin protection.
- Retained pins, management-cache population and reuse.
- Refusal of cache work while admission is closed, reopening and delayed-close replay.
- Admission closure and controller revocation interleaved with outstanding cache calls,
  producing typed `Closed` and `Authority` failures respectively.

Native package tests cover record migration, persistent state, pin limits and
maintenance continuation epochs. The installed upload payload has a Wasm header
but is not a validated application module: this proves custody and integrity,
not application installation.

## Qualification boundaries

The interleaving cases assert that the cache call is still outstanding before
changing admission or controllers. They do not isolate the final upload callback,
or a complete close/reopen cycle before that callback; those schedules still need
installed coverage.
The ordinary test builds both executables from the current checkout; it does not
reproduce the historical pre-extraction upgrade witness.

Canic owns consumer-specific chunked installation, Root control and fleet upgrade
qualification. Local Linux results do not qualify mainnet deployment or native
macOS execution. See the [extraction evidence](../docs/design/extraction.md#initial-extraction-evidence-2026-11-15)
for the historical predecessor test and its provenance.
