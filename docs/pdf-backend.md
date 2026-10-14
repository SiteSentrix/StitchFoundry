# PDF writer backend

PDF writing uses `craft-pdf` from `storytold/craft-libs`, pinned to
`c3487320b2d1de657b860b3c8abb44e1f2b32bd8`. Its `write` feature supplies Krilla configuration,
metadata construction, fallible font preparation and serialization, and native unwind guards.
The dependency disables default features and enables only `write`.

VectorCraft owns document traversal, drawing, font selection and embedding policy, colour
conversion, presets, page boxes, layers, editing data, postprocessing and print composition.
The existing Hayro reader dependencies and editable import adapters keep their current sources.
Desktop printer submission remains in `craft-print`.

The shared writer vendors Krilla 0.8.2 with an unwind-safe cleanup and poisoning patch.
The root `[patch.crates-io]` selects that copy from the same immutable revision: Cargo does
not inherit a dependency workspace's patches. A failed page or surface cleanup poisons the
backend document, so serialization cannot return an incomplete PDF after unwinding.
The shared crate's compile-time backend marker and `cargo xtask pdf-deps` (included in CI)
check that the patched writer is used. Krilla retains its MIT OR Apache-2.0 licensing;
its upstream provenance and authored patch are recorded in the shared repository.

Export and print construction, drawing and postprocessing run inside the shared native
unwind guard. These guards do not catch WASM aborts or process-wide allocation failures.
VectorCraft retains its finite-coordinate checks and the existing behavior that omits art
outside the writer's supported coordinate range. Font preparation checks the shared
512 MiB per-font copy limit and variation coordinates before allocation; failure preserves
VectorCraft's reported outline fallback. The writer is not a peak-memory or runtime budget.
