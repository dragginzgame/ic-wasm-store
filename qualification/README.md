# Installed Store qualification

Run these commands from the repository root after the [development setup](../README.md#development):

```sh
make qualification-setup
make qualification-test
make qualification-predecessor-test
```

Testkit 0.110.0 supplies the upstream PocketIC client types. Tests require
`IC_TESTKIT_POCKET_IC_URL` and never discover, download or start a server.
They create instances on that server and explicitly attempt instance deletion
on normal completion; upstream deletion does not confirm remote cleanup.

The runner owns the external server through the reviewed `ic-testkit-server`
0.110.0 launcher, installed by `make install-tools` through Toko Tooling's shared
Cargo installer with the `cli` feature. Setup and tests resolve its admitted path
through the same installer's offline `--check`; they never install implicitly.
Rerun `make install-tools` when migrating from `.tools/testkit-bootstrap`; the old
copy is left untouched and is no longer selected automatically.
This is a deliberately frozen development bootstrap,
as specified by the supplier's [simulator ownership contract](https://github.com/dragginzgame/ic-tooling/blob/main/docs/simulator-ownership.md),
not an old Testkit dependency in the qualification graph. Set
`TESTKIT_RELEASE_LAUNCHER` to use a separately prepared 0.110.0 executable.
`IC_TESTKIT_SERVER_DIRECTORY` selects the bundle directory, defaulting to
`target/qualification/server-0.110.0`. Setup provisions it explicitly; ordinary tests
only admit prepared assets offline. The launcher outlives the test command and
owns server shutdown on completion or failure. Local spawning requires loopback
permission. Supplying `IC_TESTKIT_POCKET_IC_URL` instead borrows that server;
the caller retains its lifetime and cleanup responsibility, including failures.

If simulator setup fails, `make qualification-setup` prints the evidence paths
and first 4 KiB of each retained download/error diagnostic. Control and non-ASCII
bytes are escaped, every diagnostic line is indented, and truncation is marked.
Only the failed attempt named by that setup invocation is displayed; stale
attempts cannot masquerade as the current cause. The original setup/check status
is preserved even if reporting fails. CI's `setup-<host>-<attempt>` artifact retains
sanitized setup output and available download, preparation and server diagnostics from
that invocation’s evidence directory for seven days. Compiler and repository
tool setup use the same collection; scanner installation has a separate
`scanner-setup-<attempt>` artifact in the security job. Each diagnostic is capped
at 64 KiB plus a truncation marker, with a 4 KiB console excerpt. Collection reads
at most 1 MiB per source, escapes controls, redacts credential fields, known secret
values, URL credentials/queries and private keys, and refuses symlink paths.
Raw logs, archives, server binaries and unrelated runner files are excluded.
CI links the artifact or reports upload failure/missing evidence. Collection and
upload failures cannot hide the original setup status or make the job pass.

`make test-server-diagnostics` exercises the real frozen launcher's setup with a failed
substitute downloader, without network access. It checks useful error text,
bounded escaped rendering, retained files, stale/missing evidence and exact setup
status preservation. This is also part of `make ci`.
`make test-setup-evidence` additionally checks early scanner/compiler failures,
sanitized hidden-log collection, size limits, missing/linked evidence, collector
failures and shared launcher selection. These offline checks do not prove hosted
artifact upload or retrieval.

Testkit 0.110.0's optional `managed-server` capability is not selected in this
external-server workflow. The frozen launcher remains an explicit development
bootstrap; migrating server ownership to that optional API is separate work.

The test script builds the ordinary Store and a distinct executable with
`maintenance-candidate`, then passes their paths to the independent qualification
workspace. It writes build records and Wasm artifacts under `target/qualification/`.
A separate `make build` is unnecessary for this test. The candidate feature is
for qualification only and must not be selected for live deployment.

For a focused run, use `make qualification-test QUALIFICATION_TEST=final_cache_callback`
or `QUALIFICATION_TEST=removal_at_catalog_capacity`. The owned server has a
30-minute hard lifetime to accommodate slower hosts and closes after the command.

Setup also downloads the published `ic-wasm-store` 0.2.2 archive. The predecessor
script pins its SHA-256 (`5ac8eeefd0b49bc36a1e946bc984dcabae974cb6510150296d90ae205d5e1cd0`)
and builds its unchanged manifest and packaged lockfile with Rust 1.100.0.
Its recorded source is `ed89b13cd63d6f4bdf1205f90a83798f3c022d6c`.
The separate predecessor target verifies that the current candidate rejects that older directory and the old executable can still read the partial upload; `make ci` includes it. Generated historical
sources stay under ignored `target/` and are excluded from the product workspace.

## Coverage

The `store` integration target uses ordinary Cargo directory discovery from
`tests/store/main.rs`. Its upload, capacity, callback and maintenance modules
share one client and simulator setup; test-name filters remain available through
`QUALIFICATION_TEST`.

The installed witness exercises:

- Unauthorized requests and oversized upload frames.
- Replacement Runtime guard traps (reject code 5) with the original refusal
  diagnostic, before decoding or state mutation. Upgrade scenarios stop and
  await completion of the stop operation before upgrade, then restart.
- Removal endpoint controller guards and metadata frame limits, plus deletion
  and cancellation rejection of swapped method IDs, response-direction frames
  and conflicting manifests without changing retained counters.
- An 8 MiB upload interrupted by upgrade, persistent reopen and upload resume.
- Immutable version conflicts, shared bytes, quota changes and exact chunk reconstruction.
- Deletion of aliases and last-reference payload reclamation, upgrade with tombstones,
  cancellation, exact restoration, conflicting-identity refusal and pin protection.
- Retained pins, management-cache population and reuse.
- Refusal of cache work while admission is closed, reopening and delayed-close replay.
- Upgrade while admission is closed, retaining published and incomplete releases,
  tombstones and the exact activation owner. Deletion, cancellation, tombstone
  replay and restoration remain blocked until that owner activates the attempt;
  another authorized controller cannot activate it.
- Admission closure and controller revocation interleaved with outstanding cache calls,
  producing typed `Closed` and `Authority` failures respectively.
- Final upload callbacks after controller revocation and after a complete close/reopen
  cycle. A bounded round/ingress-order sweep must find each witness: a single
  initially absent chunk is present in management storage, yet readiness returns
  `Authority` or epoch `Conflict`. Earlier refusals without an uploaded chunk do
  not count. Reversed close/activate ingress ordering does not count either.
- Deletion and cancellation at 4,096 retained labels, including 31 maximum-size reservations
  with distinct chunk hashes, shared payload retention, 32 pins on a published
  artifact, deletion replay without double reclamation, retained label-quota
  refusal after deletion, exact restoration and an 8 MiB cancellation.

Native package tests cover record migration, persistent state, pin limits and
maintenance continuation epochs. They also cover quota-rejected restoration
across reopen and multi-page tombstone enumeration with mixed active and removed
labels. The installed upload payload has a Wasm header
but is not a validated application module: this proves custody and integrity,
not application installation.

## Qualification boundaries

The capacity fixture proves completion under simulator instruction limits for
that catalog shape. Its elapsed time is a local diagnostic, not an instruction
count or production latency guarantee; it does not saturate pins on every
artifact. The released-predecessor test exercises rollback after a refused 0.2.2 upgrade, not the
historical Canic pre-extraction witness.

Canic owns consumer-specific chunked installation, Root control and fleet upgrade
qualification. Local Linux results do not qualify mainnet deployment or native
macOS execution. See the [extraction evidence](../docs/design/extraction.md#initial-extraction-evidence-2026-11-15)
for the historical predecessor test and its provenance.

## Explicit preservation endpoint: pre-release 2026-11-16 evidence

Before 0.2.3 release preparation, local changes over
`231594c009602a62864ea78049a1d2f9b8e57f9d` added method 117
and explicit active/removed entries for
[#2](https://github.com/dragginzgame/ic-wasm-store/issues/2). Package manifests
still said 0.2.2; these historical candidate bytes are neither the published
0.2.2 executable nor the final 0.2.3 release artifacts.
Focused native preservation tests, protocol frame-budget tests, native/Wasm/
qualification Clippy and both workspaces' Rustdoc passed on Linux. The installed
`closed_upgrade_preserves_removal_fences_and_exact_activation_owner` witness
passed: every discriminated page and counter survives the closed upgrade;
wrong callers, wrong method and oversized frames are refused; explicit activation
still requires the captured owner. Historical method 113 and stable layouts are
unchanged. The native regression also distinguishes restoration when counters
are identical and verifies mixed active/removed order across reopen.

Built original SHA-256:
`29b456be6ff4dfbc1fd6fea583133cc2043cf538d7d02c7aa2811a4adebecee9`.
Maintenance candidate SHA-256:
`1af806957827590290c0ecf2b35f8c02e5d6f8281013a74e10ed1b1e3fb10688`.
The installed runner used both pre-release local artifacts and their existing
lockfiles. This historical evidence does not establish publication, mainnet
upgrade or native macOS qualification. Consumer adoption
and end-to-end removal/maintenance are
[Canic #4](https://github.com/dragginzgame/canic/issues/4).

## Replacement Runtime and management codecs (2026-11-16)

The local candidate uses Runtime 0.23.13, Protocol 0.2.5 management-chunks and
Types 0.21.2. All four installed scenarios passed on Linux, including exact
final-callback witnesses and the 4,096-label capacity case. The updated stopped
maintenance upgrade and explicit guard-refusal checks passed; the published
0.2.2 predecessor-to-candidate upload/upgrade witness also passed. Wasm and
qualification Clippy, Rustdoc, and all 23 lifecycle/query/update export identities
were checked. These are local candidate results, not publication or deployment.

Storage 0.74.1 retains Runtime 0.22.5 with only its ic0 feature. The selected Wasm
graph has a single executor/runtime guard at 0.23.13. Complete removal of that
old raw-memory supplier edge remains with Storage; see the
[adoption boundary](../docs/design/extraction.md#platform-replacement-adoption-2026-11-16).
