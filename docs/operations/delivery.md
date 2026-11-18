# Delivery qualification and recovery

The coordinated product pair is one qualification scope: protocol changes affect
Store and its installed consumers. `PACKAGE` selects registry uploads, not a
smaller product proof. No version projection or alternate source checkout is used.

| Flow / command | Guarantee and execution owner |
| --- | --- |
| Development: focused `test-*`, `clippy-*`, `doc`, `qualification-test` | Changed behavior, selected by the developer; no delivery effects |
| Changed tooling: `make test-tooling` | Consumer publication, setup, path, diagnostics, dispatch and evidence fixtures |
| Repository CI: `make ci` | Product release verification plus tooling fixtures; hosted security job owns the one history scan |
| Release/publication: `make release-verify` | Early native/Wasm/qualification Clippy; snapshot admission; formatting, native tests and docs; current and predecessor installed stages |
| `make publish-check` / `make publish` | Shared adapter invokes release verification, then fresh clean-source, Wasm archive and pushed-source provenance admission; Cargo verifies native archives and performs its selected dry-run/upload |
| Recovery: repeat the owning command after diagnosis | Fresh compilation/admission; only valid original installed execution proofs can be reused |

Use `make publish` as the qualification owner during authorized delivery rather
than stacking `ci`, `publish-check` and `publish`. Tooling fixtures remain required
when their callers change and in CI, but are absent from ordinary publication.
All release prerequisites must succeed before installed execution. The installed
runner builds original/candidate Wasm once, builds the frozen predecessor, and
uses Cargo's executable artifact record for the installed test binary.

For authorized upload of prepared, committed and pushed source, run only
`make publish`. `make release-verify` is a standalone readiness check and
`make publish-check` is an optional registry dry run. Both run the product gate,
so running either before upload repeats qualification except for installed stages
whose exact-input proofs remain admissible. Neither is a required upload step.
Fresh source, archive and registry admission still runs on every publication.

## Exact installed execution evidence

`scripts/release-installed.sh` selects two stages through the published shared
`run-validation-stage.pl`: the complete current installed suite, then the published
0.2.2 predecessor upgrade refusal and rollback witness. Each has an independent immutable execution
proof and a fresh admission. The state is `target/installed-evidence/stages`;
each invocation retains its environments and admissions in a unique `selection.*`.
The source label hashes the complete Git-visible working file inventory, including
untracked files. It is a content label, not an assertion of a clean Git commit.
Publication independently requires clean committed/pushed source.

Reuse is qualified for Linux x86_64 only. The observer binds consumer source and
script inventories, both workspace manifests/locks, toolchain declaration,
snapshot receipt, actual test/launcher/server executables, selected original and
candidate Wasm, their dynamically linked libraries, Perl runtime modules and
loader/network/time configuration. The shared runner additionally binds its own
bytes, argv, working directory, host/kernel and exact bounded environment. Cargo
compilation always runs before observation: compiler/configuration/dependency
changes reach the actual binaries, rather than being inferred from a source diff.
No compiler result, archive, registry result or whole-gate receipt is cached.

The execution environment selects C locale, UTC, system PATH and explicit binary
and Wasm paths. The launcher starts a disposable local simulator; test instances
are fresh. Explicit simulator, Wasm, loader and test-runtime overrides take the
ordinary execution path with a printed reason, as do other hosts. This is a
bounded local execution contract, not remote-server evidence reuse. New test
external inputs require extending and qualifying this observer before reuse.
The current tests consume only the selected Wasm and launcher-provided local URL.

Changed source, file inventory, artifacts, host, tools, libraries, command or
environment means execution. Missing, stale or tampered original proofs/logs
cannot authorize reuse. A failed/interrupted stage never becomes a pass; source
or artifact drift between observation and admission refuses. The runner prints
executed/reused and retains monotonic timing and original proof identity.
Raw logs and cache hits alone are not evidence.

## Recovery boundaries

- Compilation/fixture preparation failure: repair the named failure and repeat
  release verification. No installed execution is inferred from partial builds.
- Current-stage failure: it has no successful proof. Retry executes it; no
  predecessor pass is manufactured.
- Predecessor-stage failure: the completed current proof remains available. Retry
  freshly builds/admit artifacts, reuses current only on an exact match, and
  executes the failed stage.
- Archive/provenance failure after installed qualification: repair that admission
  and retry publication. Original installed proofs remain usable if their inputs
  match; source and archives are always admitted afresh. Never edit proof origins.
- Evidence-output failure: retain the attempt. A complete original proof can be
  readmitted by the shared runner; absent or damaged proof/log content executes
  again. Never mark a failed overall command successful from its log text.
- Uncertain upload: inspect `target/publication/*/intent`, `dispatch`, statuses
  and Cargo output, then independently query registry version/checksum and fetch
  published archive source identity. Never replay merely because a response was
  lost. After reconciling exact published versions, select only the unpublished
  package through the existing `PACKAGE` interface. Preserve uncertain attempts;
  an occupied shared publication lock requires owner/state inspection before
  deliberate recovery. No automatic lock deletion or blind upload retry is added.

The installed preparation lock serializes these release callers. A stale lock
requires checking that its process is gone before removing that empty lock
explicitly; never remove another caller's lock or evidence. Ordinary development
builds still require coordination with release inputs. Keep all inputs stable
through validation and delivery.

## Qualification and cost

`make test-delivery-flow` qualifies the actual Make dispatch under parallel Make,
early lint/product failure, shared builds, and installed/predecessor failure
ordering with substituted effects. `make test-installed-evidence` exercises the
real observer and shared runner against disposable inputs: reuse, artifact/source/
environment invalidation, missing/tampered evidence, failed-command recovery,
drift during execution and unsafe state paths. These fixtures do not publish or
establish installed behavior. Real current and predecessor executions qualify
the public installed-test path separately.

Lint, native tests, docs, Cargo preparation and archive/registry admission still
execute. Stage admissions retain timing for actual execution versus reuse; compare
matching inputs on the same host. No total deployment speedup or macOS reuse is
claimed from substituted effects or removed stage counts.

Linux x86_64 qualification on 2026-11-17 passed all four current scenarios and
the predecessor witness. With prepared tools and warm Cargo builds, the two
shared execution/admission stages took 43.32 s and 6.63 s. The matching repeat
reused those original proofs; its admission timings are retained in
`target/qualification/issue-4-final-reuse.log` and the referenced admission JSON.
This comparison measures the installed stages, not a cold setup, complete
release gate or live registry publication. The corresponding execution log is
`target/qualification/issue-4-final-execute.log`; consumer failure fixtures passed
in `target/qualification/issue-4-final-fixtures.log`.
