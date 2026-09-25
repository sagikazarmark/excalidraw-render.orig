# CI execution and input-size work

The cold baseline used Rust 1.97.1, two Cargo build jobs, incremental compilation
off, and new empty Cargo-home and target directories. Dependency download took
3 seconds (252 crates, 23.5 MB); local workspace-test compilation took 8m41s.
An isolated Cloud compilation took 7m58s, versus 21m27s inside the full parallel
matrix. The full matrix therefore inflated build wall time substantially.

The slow unsupported-value test took 13m30s locally. It was an umbrella calling
33 routines, 31 already registered as independent tests. ADR-0018 replaces it
with a checked test-group owner, preserves the 31 tests, and registers the two
remaining routines. The compiled `fast` test list changes only by removing that
umbrella and adding those two tests: 682 becomes 683. All 33 group members passed
as independently scheduled tests in **5m13s** on the same local machine. Their
assertions and rendering expectations are unchanged.

The new Dagger input views measured approximately **8 MB for formatting** and
**216 MB for compilation**, instead of the **1 GiB** full source tree. Full
runtime evidence and Git history are restored for execution/verification.
The `input-boundaries` check prevents those heavy inputs from creeping back into
formatting or compilation; successful Cargo builds establish include completeness.

Compilation layers now share the native workspace test build and release CLI,
execute the built xtask for verification, share package archives between targets,
and compile each target's packaged consumers together. Cargo-chef dependency
layers are keyed by manifest recipes instead of every source/evidence edit.
Browser installation is keyed by its scripts and tool manifest, not the scene
corpus. Compiler versions, optimization profiles and output assertions remain
the same; target artifact and report equality still gates the applicable scope.

## Verification status

The group execution, test-list preservation, input views, scratch-path behavior,
and isolated native build have focused verification. The revised complete build
graph is still being validated; the earlier release-preparation green run does
not certify these subsequent CI changes.

Build phases can be inspected individually, without launching the whole matrix:

```sh
dagger --engine cloud --progress plain api call ci-build --phase native-dependencies
dagger --engine cloud --progress plain api call ci-build --phase native-tests
```

Place `--progress plain` before `api call` with beta.14 to get live command
output. A cached phase can replay an older Cargo log; the Dagger `CACHED` marker
and enclosing phase duration, not the replayed Cargo time, identify cache reuse.
The repeat native-dependency probe returned the compiled layer in 0.3 seconds
after workspace/session setup. Full graph and warm-run results remain pending.

Cache comparisons must record the Cloud engine identity. The successful cached
probe reused an engine; later "warm" requests were assigned different engines,
so those requests do not establish a workspace-cache invalidation bug. The latest
isolated native-test build passed in 13m20s (6m24s of actual workspace compilation,
plus dependency cooking/setup). The bounded probes below now establish warm
input-layer reuse; warm compiled-phase reuse and the complete revised matrix
remain unverified. No further full-matrix run should be used as the inner
development loop.

## Local cache eviction diagnosis (beta.14)

The fixed local engine `c71728a3d6e2` was deleting cached results between
sessions. Its reported policy was `minFreeSpace=16000000000`,
`reservedSpace=8000000000`, and `maxUsedSpace=59000000000`; the filesystem had
about **15.65 GB available**. Engine logs explicitly recorded pruning the Rust
image, the package-install command, and the input-cache command. The image's
cache digest was the same across the failed repeats. An immediate core-API
repeat sometimes completed before GC, explaining the misleading early contrast
between core queries and module calls.

In the pinned beta.14 source, `pruneTargetBytes` computes the free-space deficit
without clamping it to `reservedSpace` on that path. The relevant primary sources
are [the pruning calculation](https://github.com/dagger/dagger/blob/v1.0.0-beta.14/dagql/cache_prune.go)
and [the GC policy and scheduling](https://github.com/dagger/dagger/blob/v1.0.0-beta.14/engine/server/gc.go).
This explains eviction even when the cached data was below the reported reserve.

An isolated probe engine with an explicit policy retained the result across
sessions and a **35-second GC interval**. A real `date +%s%N` command returned
the identical timestamp, with **27.07s cold / 1.32s warm** CLI wall times
([warm trace](https://dagger.cloud/sagikazarmark/traces/98c537e3050088fb5804ae5b1a8edac9)).
The small-probe policy was:

```json
{
  "gc": {
    "policies": [
      {"all": true, "reservedSpace": "2GB", "maxUsedSpace": "4GB", "minFreeSpace": "2GB"}
    ]
  }
}
```

These are probe-sized limits; a complete multi-profile CI build needs capacity
for its measured working set. A custom Docker engine also needs a volume at
`/var/lib/dagger`: the first custom-engine experiment omitted it, selected the
native snapshotter, copied entire layers, and exceeded the 4 GB probe budget.
With the volume and overlayfs, engine `0b2b103c122c` reused the real project's
package-install, source-attachment, and hash-command layers. Its unchanged
`ci-build --phase input-cache` repeat took **0.4s for the phase**, plus **1.9s
module loading**
([trace](https://dagger.cloud/sagikazarmark/traces/a900c3cfc4e45255e9e46ae9252d837e)).

Editing this documentation kept those layers cached (**0.5s phase**,
[trace](https://dagger.cloud/sagikazarmark/traces/aed6a52b2645cc4f0b35240ccdf29840)).
As a positive invalidation control, adding a temporary file under `crates/`
made the source attachment and hash command execute again (**0.8s phase**,
[trace](https://dagger.cloud/sagikazarmark/traces/23c900e2b839b3d48d53d56a8cf2ab2d)).
The temporary file was then removed. Both `input-boundaries` and `lint:fmt`
passed against the current input views
([trace](https://dagger.cloud/sagikazarmark/traces/04b838f17501dd122945d1781a4766f8)).

The custom engines were isolated diagnostic instances; this experiment does not
change the normal local engine or Cloud policy. The complete optimized CI matrix
still needs verification on an engine sized for its build working set.

For warm-cache investigations, record all of the following before changing the
build graph:

- The engine identity and version, available disk space, and actual GC policy.
- Engine logs around session completion and GC, especially `dagql pruned result`.
- The snapshotter and the volume backing `/var/lib/dagger` for custom engines.
- A timestamp probe repeated across a GC interval, followed by the project's
  `ci-build --phase input-cache` probe on the same engine.

The engine policy is available through a core API query (`--no-load-module`):

```graphql
query {
  engine {
    name
    localCache {
      reservedSpace
      maxUsedSpace
      minFreeSpace
      entrySet { entryCount diskSpaceBytes }
    }
  }
}
```
