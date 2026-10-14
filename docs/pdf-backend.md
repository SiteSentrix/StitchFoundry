# PDF backend

PDF writing uses `craft-pdf` from `storytold/craft-libs`, pinned to
`89dcbb3060377a26c99a6d80caa16084366704c4`. Its `write` feature supplies Krilla configuration,
metadata construction, fallible font preparation and serialization, and native unwind guards.
The dependency disables default features and enables `write` and `interpret`.

VectorCraft owns document traversal, drawing, font selection and embedding policy, colour
conversion, presets, page boxes, layers, editing data, postprocessing and print composition.
Editable import keeps its VectorCraft adapter, layer routing, colour policy and text decisions.
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

PDF reading uses the same shared revision's patched Hayro syntax 0.7.2, interpreter 0.7.0
and JBIG2 0.3.0. The application explicitly preserves its existing accepted file sizes with
`OpenOptions { max_input_bytes: usize::MAX, ..Default::default() }`; this migration adds no
new file-size ceiling. Ordinary decoded streams use the shared backend's immutable
per-document limits. Those limits are separate from streaming page-content budgets and
do not bound aggregate document or process memory. Password classification and the
existing permissions-password fallback remain application policy.

The layer and overprint pre-scan consumes `ContentReader` windows through `TypedIter`,
retaining graphics and marked-content state between windows. It does not collect a whole
page before scanning. Reaching the scan's operator, nesting or instruction-window limit,
a reported failed/truncated stream, a cap-reaching filtered form, or an undecidable final
operand tail returns an import error and discards the partial document. Previously the
operator limit could return a partial layer routing with a warning. Interpreter completion
warnings also become errors before a document is returned. Whitespace and comments at
EOF remain valid. The typed iterator does not classify every malformed operator, so this
is known-incomplete rejection rather than a general PDF validity certificate.

The three patched Hayro crates and Krilla are selected in the root `[patch.crates-io]`
from the same immutable revision as `craft-pdf`. `cargo xtask pdf-deps` rejects missing,
duplicate, local-path, registry and mismatched revisions for those five packages.
`craft-print` and `craft-ui` keep their existing independent revisions.

Dependency asset hashes and sources are in `ASSETS.md`; redistributed license texts are
in `licenses/pdf-backend`. Package staging carries those texts together with `NOTICE`
and `ASSETS.md`. Fonts and profiles remain dependency data; this repository adds no font
or profile binaries. Native page interpretation has a shared unwind guard; WASM aborts
and process-wide allocation failures cannot be caught.

The app retains the raw PDF-object nesting guard and the imported document-depth cap.
Before recursive operand parsing, decoded page content is checked through the same incremental
reader used by layer scanning and interpretation. Its nesting check carries literal-string,
comment, hexadecimal-string and dictionary-delimiter state across 64 KiB scratch windows;
lowering the ordinary stream-materialization limit still produces the app's specific error for
content nested more than 256 levels. The shared parser separately caps direct containers at
128 levels: the low-limit regression previously returned an incomplete-content error, so it
demonstrates restored preflight coverage and error classification, not a successful import or
a crash bypass.
Known decoder truncation remains an import error. The scratch bound does not cover whole-filter
fallback decoding or aggregate parser/process memory. Synthetic regressions exercise lowered
ordinary limits, split delimiters and lexical state across content boundaries.
