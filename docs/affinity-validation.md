# Affinity 3 `.af`: source audit and validation boundaries

Audit date: 2026-10-09. This audit focuses on the unified `.af` document format,
not on increasing the number of legacy `.afdesign`, `.afphoto` or `.afpub` files.
The source expansion adds 28 original Affinity 3.2.3 saves and one derived
regression input to the four existing current-version `.af` illustrations.
Passing a reader test, retaining editable objects and matching a saved picture
are separate claims. None demonstrates compatibility with every Affinity feature.

## Clean-room evidence

The inputs for this work are public documentation, fixture licensing/provenance
notices and public document data. No Affinity executable, installation resources,
private API or third-party Affinity parser implementation was read. No Adobe
application was run or used as an oracle. Existing project restrictions still
apply to Adobe-authored assets and software.

The additional Patchy files below were fetched as individual document blobs.
Only its fixture directory listing, MIT license and third-party notice were read;
its importer, tests and authoring scripts were not consulted. These files provide
independently authored input data, not an implementation to translate.

The upstream notice attributes the ordinary `tiny-*.af` files to the Patchy team,
using licensed Affinity 3.2.3 through its public JavaScript SDK, and describes the
content as original, with no third-party content. It separately identifies
`tiny-lazy-placed.af` as a deterministic byte-level derivation. That file is a
regression/hostile-input case, not evidence that Affinity saved the derived layout.
See the pinned [provenance notice](https://github.com/SethRobinson/Patchy/blob/de84eab550758b30fa062e479f5778cce7693b73/NOTICE-THIRD-PARTY.md)
and [MIT license](https://github.com/SethRobinson/Patchy/blob/de84eab550758b30fa062e479f5778cce7693b73/LICENSE).

An audit using this project's parser found absolute personal-path metadata in all
33 current `.af` files, including the four existing CC0 illustrations. These
values were not copied into reports or source, and are not imported into the
engine document. Do not claim that the downloadable inputs contain no personal
paths. Inspection of archive-entry names/signatures and stream byte/blob values
found no TTF, OTF, TTC, WOFF or WOFF2 binaries; this signature check is not a
complete font-file validation.

## Public-source decisions

| Source | Current `.af` evidence | Features or equivalent files advertised | Rights and decision |
|---|---|---|---|
| [samuel-etver/vector-art](https://github.com/samuel-etver/vector-art/tree/255f8add3c8f0740196e22bd59502b811b532f0b/simple) | Four `.af` files already pinned by this project | Mexican character illustrations and playing cards; real vector art | CC0. Retain the current-version vector examples; no paired PSD identified. |
| [SethRobinson/Patchy fixture data](https://github.com/SethRobinson/Patchy/tree/de84eab550758b30fa062e479f5778cce7693b73/test-fixtures/af) | 29 `.af` blobs at commit `de84eab550758b30fa062e479f5778cce7693b73`; author states Affinity 3.2.3, with one separately identified derived file | Artboards, text, placed images, groups, vectors, masks, shapes, transforms, raster depth, color spaces and unsupported-feature examples | MIT, copyright Seth A. Robinson, 2026; original fixture content per notice. Fetch pinned data only, SHA-256 verify, preserve attribution. No paired PSD identified in this directory. |
| [AoStock manuscript-line material](https://booth.pm/ja/items/8639126) | Publisher explicitly identifies the `.af` as Affinity Studio; build unverified | Original line/grid art; `Allfile.zip` advertises `.af`, SVG and PNG together | Explicit CC0. The archive download redirects to Pixiv sign-in, so it was not fetched or validated. Useful future small vector/export oracle, not a complex font/artboard example. |
| [N.C. A&T poster template](https://www.ncat.edu/caes/agricultural-communications/poster-guidelines-and-printing.php) | University explicitly marks its native template Affinity 3.x only | Large poster, layout and university branding; PowerPoint and Canva alternatives | No open redistribution license located. Do not add to the corpus. Public download is [caes-36x48-poster-affinity.af](https://www.ncat.edu/caes/agricultural-communications/files/caes-36x48-poster-affinity.af), but authoring version and contents were not parsed: an ordinary private download was blocked by this cloud’s restricted network policy. The page permits poster-template use, without granting corpus redistribution. |
| [ALDC summer campaigning templates](https://www.aldc.org/latest-templates-summer-of-campaigning/) | Page says most current templates use Affinity 3; individual files unverified | A3/A4 leaflets, surveys and mailings, with paired PDF links and a handwriting font requirement | No permissive redistribution grant located; exclude. A useful example of why a downloadable template and a free font are not sufficient provenance. |
| [JREAMI Instagram interface templates](https://shop.jreamidesign.com/products/instagram-interface-templates-6-canva-affinity) | Product explicitly advertises `.af` documents; build unverified | Six artboards with embedded images; Canva equivalents | Commercial product; no corpus redistribution grant located. Not purchased or downloaded. |
| [Beauty-clinic flyer templates](https://www.etsy.com/listing/4435225062/aesthetic-beauty-clinic-flyer-template) | Seller advertises six `.af` files | Six matching PSD files, A4/US Letter variants, editable typography | Commercial product; no corpus redistribution grant located. The pairing is advertised, not inspected. Not purchased or downloaded. |
| [Wrapping-paper mockup](https://creativemarket.com/SDMockup/292120862-Wrapping-Paper-with-Sticker-Mockup) | Seller advertises native `.af` | PSD and `.af` together; large photographic mockup | Commercial Creative Market asset; no corpus redistribution grant located. Not purchased or downloaded. |
| [OpenGameArt VR controller diagrams](https://opengameart.org/content/vr-controller-diagrams) | Published 2021-11-22, before the unified format; the page's “.AF” wording does not establish Affinity 3 | Author advertises Affinity vector sources, PSD and PNG for controller diagrams | CC0 by psychicparrot. Exclude from the current-version corpus. ZIP retrieval returned HTTP 403 in this environment; filenames and PSD generator were not inspected. |

Searches combined `.af`, Affinity 3, artboards, sample files, templates, CC0,
Creative Commons and PSD on public web/GitHub listings. This is a documented
sample of discovered sources, not a claim that every public file was found.
No openly redistributable, inspected Affinity 3 `.af`/PSD pair covering complex
typography and artboards was located in this audit.

## Additional current-version fixture inventory

The corpus names below map to upstream `test-fixtures/af/tiny-<suffix>.af`.
For example, `patchy-artboards.af` comes from
[tiny-artboards.af](https://github.com/SethRobinson/Patchy/blob/de84eab550758b30fa062e479f5778cce7693b73/test-fixtures/af/tiny-artboards.af).
The source pin, filenames and SHA-256 digest manifest are the reproducibility
boundary; never fetch a floating branch for CI.

| Area | Corpus files | What the file selection can test |
|---|---|---|
| Artboards and units | `patchy-artboards.af`, `patchy-dpi300.af` | Board identity, rectangles, content association, clipping and conversion from document resolution to points |
| Editable typography | `patchy-text-artistic.af`, `patchy-text-frame.af`, `patchy-text-runs.af`, `patchy-text-rotated.af` | Native text retention, per-run styling, frame geometry and composed transforms |
| Paragraph/character differences | `patchy-text-indent.af`, `patchy-text-para-spacing.af`, `patchy-text-caps.af` | Detect unsupported text layout and case features; a passing parse is insufficient evidence of layout fidelity |
| Curves, shapes and masks | `patchy-vector.af`, `patchy-vector-mask.af`, `patchy-shapes.af`, `patchy-shapes-2.af` | Editable paths, mask attachment and known/fallback parametric shapes |
| Structure and transforms | `patchy-group.af`, `patchy-transform.af` | Nested groups, names, stacking and transform composition |
| Images and pixels | `patchy-embedded-jpeg.af`, `patchy-rgba8.af`, `patchy-rgba16.af`, `patchy-cmyk.af`, `patchy-lab.af` | Original image retention, tiled pixels, supported depths and explicit color-space limitations |
| Effects/adjustments/blends | `patchy-fx.af`, `patchy-fx-blur.af`, `patchy-fx-gradient.af`, `patchy-adjust-curves.af`, `patchy-adjust-hsl.af`, `patchy-live-filter.af`, `patchy-blend-affinity.af` | Warnings and safe fallbacks for unsupported visual operations; not automatic feature support |
| Saved revisions and derived input | `patchy-incremental-chain.af`, `patchy-lazy-placed.af` | Revision selection and bounded handling of a derived placement case |

These are focused feature probes. They complement the larger vector illustrations
but do not replace a large production document with many interacting features.
Fixture filenames alone do not prove the imported model preserved those features;
assert native model structure and imported engine behavior independently.

## PSD compatibility oracle design

Affinity's public [PSD-import guide](https://www.affinity.studio/blog/how-to-open-a-psd-file)
documents import settings that change the result: smart objects can be rasterized
or retained, and text can be imported as text or bitmap. It also describes artboard
mapping and profile conversion. Therefore, a same-looking PSD is not inherently
an equivalent editable document, and an export can lose features before our
reader sees it.

For a future paired oracle, record the original author, license for all embedded
art, Affinity version, platform, import settings, profile, resolution, fonts,
export preset and each expected semantic loss. Keep an original `.af`, a native
Affinity-rendered PNG per board at full resolution, an Affinity-exported PSD and
a human-readable object/layout inventory. Do not run Photoshop or another Adobe
application to generate or validate the comparison. A PSD written by VectorCraft
after import tests our writer/round trip; it is not an independent Affinity oracle.

Compare board dimensions and names, object counts/ordering/visibility, text and
run boundaries, placed-image transforms, mask geometry and transparency, then
render each board with the documented background. Document which PSD objects
are rasterized, approximated or absent. A flat full-document thumbnail can hide
lost boards, editable text or misplaced children. Require native-import success
where native support is claimed; preview fallback must never satisfy that test.

## Typography and complex-document coverage still required

Affinity publicly documents [variable fonts, OpenType features, path text,
artistic/frame text and precision spacing](https://www.affinity.studio/typography-software).
The discovered real-file set does not establish faithful support for all of them.

| Required independent fixture | Evidence needed before claiming support |
|---|---|
| Variable fonts and named instances | Font family plus axes/instance settings preserved; exact pinned font version; non-default weight/width/optical-size rendering |
| OpenType feature combinations | Ligatures, discretionary ligatures, stylistic sets, alternate glyphs, small caps, numeric forms and kerning controls; compare both text content and glyph output |
| Mixed scripts and direction | Arabic joining/RTL, Hebrew, Devanagari conjuncts, Thai combining marks, CJK line breaks, bidi mixed numbers, combining accents and supplementary-plane characters |
| Mixed runs and font fallback | Multiple families, sizes, colors and baseline shifts within one paragraph; missing font and missing glyph outcomes reported without silently changing text |
| Advanced frames and flow | Columns, insets, linked frames, overflow, tabs/indents, paragraph spacing, justified text, wrapping around objects and text on a transformed path |
| Font representations | TrueType, CFF/OpenType, collection faces, variable outlines and color glyph formats; avoid treating family-name retention as outline support |
| Large interacting document | Multiple unequal/overlapping boards, off-board artwork, nested clips/masks, rotations/skews, repeated placed/embedded documents, symbols, images and text at non-72 dpi |
| Pixel/vector interaction | Raster depths/color spaces, blend/opacity combinations, effects, live adjustments, brushes and pressure profiles with explicit warnings for unsupported cases |

Acquire these as original contributor documents with explicit redistribution
rights, or add independently published originals with complete provenance.
Fonts belong in [storytold/craft-fonts](https://github.com/storytold/craft-fonts),
not this repository; a fixture's font names do not grant redistribution rights
to font binaries. Synthetic containers remain useful for parser limits and
precise regression tests, but cannot establish real Affinity saves or rendering
semantics for an unobserved feature.

Tests must distinguish supported behavior, approximated behavior with warnings,
rejected input and unvalidated behavior. Keep the unmet rows above visible when
reporting results: there is no evidence for universal `.af` compatibility.

## Checks added in this change

The pinned corpus contains 51 documents, including 33 current `.af` files with
34 artboards. Native model signatures freeze retained geometry, paints, masks,
images, text runs, fonts, bounds and warnings. Every current board is saved and
reloaded in `.vectorcraft`, then exported through SVG, PDF and PSD. SVG/PDF
dimensions and reimport are checked; a test-only decoder written from the public
PSD specification compares merged pixels and alpha with VectorCraft's PNG output.
These export checks establish internal consistency, not independent Affinity fidelity.

A headless VectorCraft MCP session also opens the real two-board `.af`, saves
and reopens `.vectorcraft` with the complete saved model unchanged, exports both
boards as SVG/PSD, then reopens and exports the two-page PDF again with embedded
editing data disabled, exercising the PDF reader rather than native restoration. The normal
save command adds creation/modification metadata for the new native file.

Saved-thumbnail comparisons cover 36 board crops across 31 documents. Both boards
in the Affinity 3 artboard probe match exactly; the embedded JPEG probe has a mean
difference of 0.50/255. Per-file ceilings preserve measured regression limits;
unsupported text layout and effects are tested structurally and through warnings.

The cloud runs Linux with no Affinity installation, exposed Affinity MCP, graphical
display or GPU/VM device. [Current Affinity supports Windows and macOS](https://www.affinity.studio/download);
ordinary prerequisite installation also failed because this sandbox has no root
access or sudo. A supported desktop with ordinary Canva activation is needed for
the independent Affinity reopen procedure above. No Affinity reference reopen or
`.af` export is claimed by this change.
