//! The object-based design model.
//!
//! Like professional digitizing software, a [`Design`] is a sequence of
//! *embroidery objects*. Each object stores an editable outline (contour, shape
//! or satin column) plus stitch parameters; the actual needle penetrations are
//! generated from them by [`crate::engine`]. This keeps designs scalable and
//! re-editable – changing size, angle or density regenerates the stitches.
//!
//! Angles are degrees measured **counter-clockwise on screen** from the +x axis
//! (see [`crate::geom::screen_dir`]).

use crate::direction::Guide;
use crate::geom::{Affine, BBox, Column, Contour, FLATTEN_TOLERANCE, Point, Shape, screen_angle, screen_dir};
use crate::hoops::Hoop;
use crate::pattern::Stitch;
use crate::threads::{Rgb, Thread, default_palette};
use serde::{Deserialize, Serialize};

pub type ObjectId = u64;

/// Version of the native `.sfd` file format.
pub const FORMAT_VERSION: u32 = 1;

fn yes() -> bool {
    true
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

// ---------------------------------------------------------------------------
// Design
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub name: String,
    /// Thread palette; objects refer to it by index.
    pub palette: Vec<Thread>,
    /// Objects in sewing order.
    pub objects: Vec<EmbObject>,
    #[serde(default)]
    pub hoop: Option<Hoop>,
    /// Name of the fabric preset the design was prepared for.
    #[serde(default)]
    pub fabric: String,
    #[serde(default)]
    pub settings: DesignSettings,
    /// Artwork shown behind the design for manual tracing.
    #[serde(default)]
    pub backdrop: Option<Backdrop>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub next_id: ObjectId,
}

impl Default for Design {
    fn default() -> Self {
        Design::new()
    }
}

impl Design {
    pub fn new() -> Self {
        Design {
            version: FORMAT_VERSION,
            name: "Untitled".into(),
            palette: default_palette(),
            objects: Vec::new(),
            hoop: None,
            fabric: String::new(),
            settings: DesignSettings::default(),
            backdrop: None,
            notes: String::new(),
            next_id: 1,
        }
    }

    /// Allocates a fresh object id.
    pub fn fresh_id(&mut self) -> ObjectId {
        let max_existing = self.objects.iter().map(|o| o.id).max().unwrap_or(0);
        self.next_id = self.next_id.max(max_existing + 1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Appends an object and returns its id.
    pub fn add(&mut self, kind: ObjectKind, color: usize) -> ObjectId {
        let id = self.fresh_id();
        self.objects.push(EmbObject::new(id, kind, color));
        id
    }

    /// Appends a fully built object, assigning it a fresh id. Returns the id.
    pub fn push(&mut self, mut obj: EmbObject) -> ObjectId {
        obj.id = self.fresh_id();
        let id = obj.id;
        self.objects.push(obj);
        id
    }

    pub fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    pub fn get(&self, id: ObjectId) -> Option<&EmbObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut EmbObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn remove(&mut self, id: ObjectId) -> Option<EmbObject> {
        let i = self.index_of(id)?;
        Some(self.objects.remove(i))
    }

    /// Palette index for a colour: an existing thread with exactly this colour,
    /// otherwise a new palette entry.
    pub fn color_index(&mut self, c: Rgb) -> usize {
        if let Some(i) = self.palette.iter().position(|t| t.color == c) {
            return i;
        }
        self.palette.push(Thread::new(c, c.to_hex()));
        self.palette.len() - 1
    }

    /// Palette index for a thread (matched by colour, otherwise appended).
    pub fn thread_index(&mut self, t: &Thread) -> usize {
        if let Some(i) = self.palette.iter().position(|p| p.color == t.color) {
            return i;
        }
        self.palette.push(t.clone());
        self.palette.len() - 1
    }

    pub fn thread(&self, i: usize) -> Thread {
        self.palette.get(i).cloned().unwrap_or_else(|| Thread::new(Rgb::BLACK, "Black"))
    }

    /// Outline bounds of all visible objects.
    pub fn bbox(&self) -> BBox {
        self.objects.iter().filter(|o| o.visible).fold(BBox::EMPTY, |b, o| b.union(o.bbox()))
    }
}

/// Machine and sequencing settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesignSettings {
    /// Connectors longer than this (mm) are trimmed instead of jumped.
    pub trim_threshold: f64,
    /// Sew small locking stitches at the start of each object.
    pub tie_in: bool,
    /// Sew small locking stitches before each trim / colour change.
    pub tie_off: bool,
    /// Size of tie-in / tie-off stitches (mm).
    pub tie_length: f64,
    /// How tie-ins and tie-offs lock the thread.
    #[serde(skip_serializing_if = "TieStyle::is_default")]
    pub tie_style: TieStyle,
    /// Stitches shorter than this are merged away (mm).
    pub min_stitch: f64,
    /// Longest stitch the machine can sew; longer stitches are split (mm).
    pub max_stitch: f64,
    /// Longest single frame move; longer jumps are split (mm).
    pub max_jump: f64,
    /// Machine speed used for time estimates (stitches per minute).
    pub machine_speed: f64,
    /// Seconds per trim, for time estimates.
    pub trim_seconds: f64,
    /// Seconds per colour change, for time estimates.
    pub color_change_seconds: f64,
}

impl Default for DesignSettings {
    fn default() -> Self {
        DesignSettings {
            trim_threshold: 2.0,
            tie_in: true,
            tie_off: true,
            tie_length: 0.8,
            tie_style: TieStyle::Double,
            min_stitch: 0.3,
            max_stitch: 12.0,
            max_jump: 12.0,
            machine_speed: 800.0,
            trim_seconds: 6.0,
            color_change_seconds: 20.0,
        }
    }
}

/// How tie-ins and tie-offs lock the thread: small stitches sewn in place where a
/// thread starts and before it is cut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TieStyle {
    /// One stitch out and back: the lightest lock.
    Single,
    /// Out and back twice.
    #[default]
    Double,
    /// A small triangle: out along the line, across it and back, locking on fabric
    /// that frays or stretches.
    Triangle,
}

impl TieStyle {
    pub const ALL: [TieStyle; 3] = [TieStyle::Single, TieStyle::Double, TieStyle::Triangle];

    pub fn label(self) -> &'static str {
        match self {
            TieStyle::Single => "Single",
            TieStyle::Double => "Double",
            TieStyle::Triangle => "Triangle",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            TieStyle::Single => "single",
            TieStyle::Double => "double",
            TieStyle::Triangle => "triangle",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == TieStyle::default()
    }

    /// The needle points of a tie at `at`, heading along the unit vector `u`, `t` long
    /// (it ends back at `at`).
    pub fn points(self, at: Point, u: Point, t: f64) -> Vec<Point> {
        let q = at + u * t;
        match self {
            TieStyle::Single => vec![q, at],
            TieStyle::Double => vec![q, at, q, at],
            TieStyle::Triangle => vec![q, at + u * (t * 0.5) + Point::new(-u.y, u.x) * (t * 0.6), at],
        }
    }
}

/// Bitmap artwork shown behind the design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Backdrop {
    pub name: String,
    /// The encoded image file (PNG, JPEG, ...); base64 in JSON.
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    /// Top-left corner in design space.
    pub min: Point,
    /// Displayed size in mm.
    pub width: f64,
    pub height: f64,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}

fn default_opacity() -> f32 {
    0.6
}

mod b64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD.decode(s.as_bytes()).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbObject {
    pub id: ObjectId,
    #[serde(default)]
    pub name: String,
    /// Palette index.
    pub color: usize,
    pub kind: ObjectKind,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    /// Objects sharing a group id are selected and transformed together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Preferred start point (the closest valid point is used). `None` = automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Point>,
    /// Preferred end point. `None` = automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Point>,
    /// Always trim after this object, even when the next one is close.
    #[serde(default)]
    pub trim_after: bool,
    /// Lock stitches at the start of this object: `None` follows the design setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tie_in: Option<bool>,
    /// Lock stitches before the thread is cut after this object: `None` follows the
    /// design setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tie_off: Option<bool>,
    /// Stop the machine after this object.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_after: bool,
    /// Leave out the object's last needle point.
    #[serde(default, skip_serializing_if = "is_false")]
    pub omit_last: bool,
    /// Objects sharing a branch id are sewn as one path: the thread travels from one
    /// to the next as running stitches where later ones of the branch cover it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<u64>,
}

impl EmbObject {
    pub fn new(id: ObjectId, kind: ObjectKind, color: usize) -> Self {
        let name = kind.type_name().to_string();
        EmbObject {
            id,
            name,
            color,
            kind,
            visible: true,
            locked: false,
            group: None,
            entry: None,
            exit: None,
            trim_after: false,
            tie_in: None,
            tie_off: None,
            stop_after: false,
            omit_last: false,
            branch: None,
        }
    }

    /// Outline bounds.
    pub fn bbox(&self) -> BBox {
        self.kind.bbox()
    }

    /// Applies an affine transform to the geometry, keeping stitch angles attached
    /// to the shape (a rotated fill keeps its stitch direction relative to the shape).
    pub fn transform(&mut self, m: &Affine) {
        self.kind.transform(m);
        if let Some(p) = &mut self.entry {
            *p = m.apply(*p);
        }
        if let Some(p) = &mut self.exit {
            *p = m.apply(*p);
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.kind.type_name()
    }
}

/// The geometry + stitch type of an object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectKind {
    /// Running-stitch style line along an open or closed contour.
    Run(RunObject),
    /// Satin column between two rails.
    Satin(SatinObject),
    /// Closed shape (with holes) filled with tatami, satin, contour, ... stitches.
    Fill(FillObject),
    /// Appliqué: placement line, tack-down and cover stitch around a fabric piece.
    Applique(AppliqueObject),
    /// Text converted to stitches.
    Lettering(LetteringObject),
    /// Raw stitches (imported machine files or manual stitching).
    Stitches(StitchBlock),
}

impl ObjectKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            ObjectKind::Run(r) => match r.params.kind {
                RunKind::Single => "Run",
                RunKind::Triple => "Triple Run",
                RunKind::Blanket => "Blanket",
                RunKind::Zigzag => "Zigzag",
                RunKind::Motif => "Motif Run",
                RunKind::Back => "Backstitch",
                RunKind::Stem => "Stem Stitch",
            },
            ObjectKind::Satin(_) => "Satin Column",
            ObjectKind::Fill(f) => f.params.kind.label(),
            ObjectKind::Applique(_) => "Appliqué",
            ObjectKind::Lettering(_) => "Lettering",
            ObjectKind::Stitches(_) => "Stitches",
        }
    }

    pub fn bbox(&self) -> BBox {
        match self {
            ObjectKind::Run(r) => r.path.bbox(),
            ObjectKind::Satin(s) => s.column.bbox(),
            ObjectKind::Fill(f) => f.shape.bbox(),
            ObjectKind::Applique(a) => a.shape.bbox(),
            ObjectKind::Lettering(l) => l.glyphs.iter().fold(BBox::EMPTY, |b, g| b.union(g.shape.bbox())),
            ObjectKind::Stitches(s) => BBox::from_points(s.stitches.iter().filter(|s| s.is_move()).map(Stitch::pos)),
        }
    }

    pub fn transform(&mut self, m: &Affine) {
        match self {
            ObjectKind::Run(r) => r.path.transform(m),
            ObjectKind::Satin(s) => s.column.transform(m),
            ObjectKind::Fill(f) => {
                f.shape.transform(m);
                f.params.angle = transform_angle(f.params.angle, m);
                if let Some(c) = &mut f.params.center {
                    *c = m.apply(*c);
                }
                for g in &mut f.guides {
                    g.transform(m);
                }
            }
            ObjectKind::Applique(a) => a.shape.transform(m),
            ObjectKind::Lettering(l) => {
                l.transform = l.transform.then(m);
                for g in &mut l.glyphs {
                    g.shape.transform(m);
                    g.angle = transform_angle(g.angle, m);
                }
            }
            ObjectKind::Stitches(s) => {
                for st in &mut s.stitches {
                    let p = m.apply(st.pos());
                    st.x = p.x;
                    st.y = p.y;
                }
            }
        }
    }

    /// Outline polylines for display: closed rings are returned explicitly closed.
    pub fn outlines(&self) -> Vec<Vec<Point>> {
        let tol = FLATTEN_TOLERANCE;
        let ring = |c: &Contour| crate::geom::polyline::closed(&c.polyline(tol));
        match self {
            ObjectKind::Run(r) => {
                if r.path.closed {
                    vec![ring(&r.path)]
                } else {
                    vec![r.path.polyline(tol)]
                }
            }
            ObjectKind::Satin(s) => vec![crate::geom::polyline::closed(&s.column.outline(tol))],
            ObjectKind::Fill(f) => std::iter::once(&f.shape.outer).chain(&f.shape.holes).map(ring).collect(),
            ObjectKind::Applique(a) => std::iter::once(&a.shape.outer).chain(&a.shape.holes).map(ring).collect(),
            ObjectKind::Lettering(l) => {
                l.glyphs.iter().flat_map(|g| std::iter::once(&g.shape.outer).chain(&g.shape.holes).map(ring).collect::<Vec<_>>()).collect()
            }
            ObjectKind::Stitches(s) => vec![s.stitches.iter().filter(|s| s.is_move()).map(Stitch::pos).collect()],
        }
    }
}

/// Maps a stitch angle through a transform (result normalised to `[0, 180)`).
pub fn transform_angle(degrees: f64, m: &Affine) -> f64 {
    let v = m.apply_vector(screen_dir(degrees));
    if v.length() < 1e-12 {
        return degrees;
    }
    normalize_line_angle(screen_angle(v))
}

/// Normalises a line direction angle to `[0, 180)`.
pub fn normalize_line_angle(deg: f64) -> f64 {
    let a = deg.rem_euclid(180.0);
    if (a - 180.0).abs() < 1e-9 { 0.0 } else { a }
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunKind {
    /// Single running stitch.
    #[default]
    Single,
    /// Bean / triple stitch: every stitch sewn forward, back and forward again.
    Triple,
    /// Blanket (E-stitch): running stitch with prongs to one side.
    Blanket,
    /// Open zigzag along the line.
    Zigzag,
    /// Decorative repeating motif along the line.
    Motif,
    /// Hand-sewn look: each stitch goes forward two steps and back one, so
    /// every stitch overlaps the last.
    Back,
    /// Rope-like hand-embroidery outline: like backstitch, with the stitches
    /// slanted across the line.
    Stem,
}

impl RunKind {
    pub const ALL: [RunKind; 7] = [RunKind::Single, RunKind::Triple, RunKind::Blanket, RunKind::Zigzag, RunKind::Motif, RunKind::Back, RunKind::Stem];

    pub fn label(self) -> &'static str {
        match self {
            RunKind::Single => "Single run",
            RunKind::Triple => "Triple run",
            RunKind::Blanket => "Blanket",
            RunKind::Zigzag => "Zigzag",
            RunKind::Motif => "Motif run",
            RunKind::Back => "Backstitch",
            RunKind::Stem => "Stem stitch",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Motif {
    #[default]
    Wave,
    Scallop,
    Diamond,
    Cross,
    Feather,
}

impl Motif {
    pub const ALL: [Motif; 5] = [Motif::Wave, Motif::Scallop, Motif::Diamond, Motif::Cross, Motif::Feather];

    pub fn label(self) -> &'static str {
        match self {
            Motif::Wave => "Wave",
            Motif::Scallop => "Scallop",
            Motif::Diamond => "Diamond",
            Motif::Cross => "Cross",
            Motif::Feather => "Feather",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunParams {
    pub kind: RunKind,
    /// Needle step along the line (mm).
    pub stitch_length: f64,
    /// Blanket prong length / zigzag width / motif width (mm).
    pub width: f64,
    /// Distance between blanket prongs / zigzag peaks / motif repeats (mm).
    pub spacing: f64,
    /// Put blanket prongs / motifs / stem slants on the other side of the line.
    pub flip: bool,
    pub motif: Motif,
    /// Times the line is sewn, alternating direction over the same needle points:
    /// 1 = once, 2 = backtrack (ends back at the start), 3 or more = repeats.
    pub passes: u32,
}

/// Most passes a run can be sewn.
pub const MAX_PASSES: u32 = 9;

impl Default for RunParams {
    fn default() -> Self {
        RunParams { kind: RunKind::Single, stitch_length: 2.5, width: 2.0, spacing: 2.0, flip: false, motif: Motif::Wave, passes: 1 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunObject {
    pub path: Contour,
    #[serde(default)]
    pub params: RunParams,
}

// ---------------------------------------------------------------------------
// Satin
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SatinUnderlay {
    /// Choose underlay from the column width (ignores the flags below).
    pub auto: bool,
    pub center_run: bool,
    pub edge_run: bool,
    pub zigzag: bool,
    /// Inset of edge-run and zigzag underlay from the rails (mm).
    pub inset: f64,
    /// Zigzag underlay spacing (mm).
    pub zigzag_spacing: f64,
    /// Stitch length of run underlays (mm).
    pub stitch_length: f64,
    /// Sew the zigzag underlay there and back, the second pass crossing the
    /// first (a double zigzag).
    pub double_zigzag: bool,
}

impl Default for SatinUnderlay {
    fn default() -> Self {
        SatinUnderlay {
            auto: true,
            center_run: true,
            edge_run: false,
            zigzag: false,
            inset: 0.4,
            zigzag_spacing: 2.0,
            stitch_length: 2.5,
            double_zigzag: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SatinParams {
    /// Distance between needle penetrations along the rails (mm).
    pub spacing: f64,
    /// Extra width on each side against fabric pull-in (mm); with `pull_comp_2`
    /// set, the extra width on side 1 (the first rail) only.
    pub pull_comp: f64,
    /// Twin-sided pull compensation: the extra width on side 2 (the second rail),
    /// e.g. less where the column lies on other stitching. `None` = `pull_comp`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_comp_2: Option<f64>,
    /// Tighten spacing on narrow columns and open it on wide ones.
    pub auto_spacing: bool,
    /// Split stitches longer than this (mm) into staggered shorter ones; 0 = never.
    pub split_length: f64,
    /// Shorten alternate stitches on the inside of tight curves.
    pub short_stitches: bool,
    /// Keep stitches longer than the machine's longest stitch whole: the frame
    /// moves in steps without a needle drop, so the thread lies as one stitch.
    #[serde(skip_serializing_if = "is_false")]
    pub preserve_long: bool,
    /// How satin lines join at sharp corners.
    #[serde(skip_serializing_if = "Corners::is_auto")]
    pub corners: Corners,
    /// Sew the column in tatami rows instead of satin (a tatami border, for columns too
    /// wide for satin): every stitch across is split into pieces `split_length` long
    /// ([`TATAMI_LENGTH`] when that is 0), shifted a quarter from row to row.
    #[serde(skip_serializing_if = "is_false")]
    pub tatami: bool,
    pub underlay: SatinUnderlay,
}

/// The stitch length of a tatami border when none is set (mm).
pub const TATAMI_LENGTH: f64 = 4.0;

fn is_false(b: &bool) -> bool {
    !*b
}

/// How a satin line's two sides of a sharp corner meet (smart corners). Rows that
/// simply turned with the line would bunch up in the inside of the corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Corners {
    /// By the angle: rows turn round gentle corners, meet on a miter at medium
    /// ones and are mitered with the tip capped at sharp ones.
    #[default]
    Auto,
    /// Rows fan round the inner corner point.
    Turn,
    /// Each side keeps its rows square to it; they meet on the line from the inner
    /// to the outer corner point.
    Miter,
    /// A miter with the outer tip cut off square, for sharp corners.
    Cap,
}

impl Corners {
    pub const ALL: [Corners; 4] = [Corners::Auto, Corners::Turn, Corners::Miter, Corners::Cap];

    pub fn label(self) -> &'static str {
        match self {
            Corners::Auto => "Automatic",
            Corners::Turn => "Turn",
            Corners::Miter => "Miter",
            Corners::Cap => "Capped",
        }
    }

    fn is_auto(&self) -> bool {
        *self == Corners::Auto
    }
}

impl SatinParams {
    /// Pull compensation on (side 1, side 2).
    pub fn pull_sides(&self) -> (f64, f64) {
        (self.pull_comp, self.pull_comp_2.unwrap_or(self.pull_comp))
    }
}

impl Default for SatinParams {
    fn default() -> Self {
        SatinParams {
            spacing: 0.4,
            pull_comp: 0.15,
            pull_comp_2: None,
            auto_spacing: true,
            split_length: 8.0,
            short_stitches: true,
            preserve_long: false,
            corners: Corners::Auto,
            tatami: false,
            underlay: SatinUnderlay::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SatinObject {
    pub column: Column,
    #[serde(default)]
    pub params: SatinParams,
}

// ---------------------------------------------------------------------------
// Fill
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FillKind {
    /// Rows of staggered running stitches (a.k.a. "fill" / "step").
    #[default]
    Tatami,
    /// Long satin stitches across the shape (auto-split when too long).
    Satin,
    /// Rows following the outline, shrinking towards the centre.
    Contour,
    /// One continuous spiral from the outside in.
    Spiral,
    /// Rays from a centre point to the outline.
    Radial,
    /// Cross stitches on a grid.
    Cross,
    /// A continuous quilting-style meander that wanders through the shape.
    Meander,
}

impl FillKind {
    pub const ALL: [FillKind; 7] =
        [FillKind::Tatami, FillKind::Satin, FillKind::Contour, FillKind::Spiral, FillKind::Radial, FillKind::Cross, FillKind::Meander];

    pub fn label(self) -> &'static str {
        match self {
            FillKind::Tatami => "Tatami Fill",
            FillKind::Satin => "Satin Fill",
            FillKind::Contour => "Contour Fill",
            FillKind::Spiral => "Spiral Fill",
            FillKind::Radial => "Radial Fill",
            FillKind::Cross => "Cross Fill",
            FillKind::Meander => "Meander Fill",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillUnderlay {
    /// Run around the shape just inside the outline.
    pub edge_run: bool,
    /// Sparse tatami across the shape.
    pub tatami: bool,
    /// Distance of the underlay from the outline (mm).
    pub inset: f64,
    /// Row spacing of tatami underlay (mm).
    pub spacing: f64,
    pub stitch_length: f64,
    /// Underlay direction relative to the fill angle (degrees).
    pub angle_offset: f64,
}

impl Default for FillUnderlay {
    fn default() -> Self {
        FillUnderlay { edge_run: true, tatami: true, inset: 0.5, spacing: 2.5, stitch_length: 3.0, angle_offset: 90.0 }
    }
}

/// A programmed split pattern: the needle points of a tatami fill follow its lines, so the fill
/// shows a texture. Generated in code and tiled over the fill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TexturePattern {
    #[default]
    Diamonds,
    Waves,
    Herringbone,
    Bricks,
    Circles,
    Scales,
}

impl TexturePattern {
    pub const ALL: [TexturePattern; 6] = [
        TexturePattern::Diamonds,
        TexturePattern::Waves,
        TexturePattern::Herringbone,
        TexturePattern::Bricks,
        TexturePattern::Circles,
        TexturePattern::Scales,
    ];

    /// The pattern's id in params and files.
    pub fn id(self) -> &'static str {
        match self {
            TexturePattern::Diamonds => "diamonds",
            TexturePattern::Waves => "waves",
            TexturePattern::Herringbone => "herringbone",
            TexturePattern::Bricks => "bricks",
            TexturePattern::Circles => "circles",
            TexturePattern::Scales => "scales",
        }
    }

    /// The tile width (mm) the pattern reads best at with stitch length `stitch_length`: bricks
    /// with their joints a long stitch apart (no stitches between them), scales large enough for
    /// their flat bottoms to show.
    pub fn default_size(self, stitch_length: f64) -> f64 {
        let l = if stitch_length.is_finite() { stitch_length } else { 4.0 };
        match self {
            TexturePattern::Bricks => (l * 1.3).clamp(3.0, 12.0),
            TexturePattern::Scales => (l * 2.5).clamp(4.0, 14.0),
            _ => (l * 1.5).clamp(3.0, 12.0),
        }
    }
}

/// Programmed splits of a tatami fill (a texture).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Texture {
    pub pattern: TexturePattern,
    /// Width of one pattern tile (mm).
    pub size: f64,
    /// Turn of the pattern against the stitch rows (degrees).
    pub angle: f64,
}

impl Default for Texture {
    fn default() -> Self {
        Texture { pattern: TexturePattern::Diamonds, size: 6.0, angle: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillParams {
    pub kind: FillKind,
    /// Stitch direction (degrees, counter-clockwise on screen).
    pub angle: f64,
    /// Row spacing (mm). Smaller = denser.
    pub spacing: f64,
    /// Needle step within a row (mm).
    pub stitch_length: f64,
    /// Tatami stagger: shift of each row as a fraction of the stitch length.
    pub offset: f64,
    /// Satin fill: split stitches longer than this (mm); 0 = never.
    pub split_length: f64,
    /// Extension of row ends against fabric pull-in (mm).
    pub pull_comp: f64,
    /// If > 0, row spacing changes linearly from `spacing` to this value across the
    /// shape (gradient / colour-blending effect).
    pub spacing_end: f64,
    /// Centre for radial and spiral fills (`None` = shape centroid).
    pub center: Option<Point>,
    /// Random jitter of tatami needle points (0 = none, 1 = up to a quarter
    /// stitch) for a softer, hand-sewn look. Also randomises meander paths.
    pub random: f64,
    pub underlay: FillUnderlay,
    /// A satin fill made from a satin object: its column settings (underlay by
    /// width, short stitches, twin-sided pull compensation, long stitches…) apply
    /// where the satin turns along inclination lines, instead of `underlay`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satin_column: Option<SatinParams>,
    /// Programmed splits (tatami): the needle points follow a pattern, a texture in the fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<Texture>,
}

impl Default for FillParams {
    fn default() -> Self {
        FillParams {
            kind: FillKind::Tatami,
            angle: 45.0,
            spacing: 0.4,
            stitch_length: 4.0,
            offset: 0.25,
            split_length: 7.0,
            pull_comp: 0.2,
            spacing_end: 0.0,
            center: None,
            random: 0.0,
            underlay: FillUnderlay::default(),
            satin_column: None,
            texture: None,
        }
    }
}

impl FillParams {
    /// Defaults suited to a satin-filled shape.
    pub fn satin() -> Self {
        FillParams { kind: FillKind::Satin, angle: 0.0, spacing: 0.4, ..FillParams::default() }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FillObject {
    pub shape: Shape,
    #[serde(default)]
    pub params: FillParams,
    /// Inclination lines. One guide sets the stitch angle (overriding
    /// `params.angle`); two or more, in order along the shape, make the stitches
    /// turn smoothly from guide to guide ("turning" fill / satin).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
    /// Split lines (tatami): a needle point of each row falls where the row crosses one, so the
    /// lines show in the fill (drawn splits, carved stamps, flexi splits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<Vec<Point>>,
}

impl FillObject {
    pub fn new(shape: Shape, params: FillParams) -> Self {
        FillObject { shape, params, guides: Vec::new(), splits: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Appliqué
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TackdownKind {
    None,
    Run,
    #[default]
    Zigzag,
    Blanket,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CoverKind {
    None,
    #[default]
    Satin,
    Blanket,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppliqueParams {
    /// Sew a placement line, then stop so the fabric can be positioned.
    pub placement: bool,
    pub tackdown: TackdownKind,
    /// Width of zigzag / blanket tack-down (mm).
    pub tackdown_width: f64,
    /// Inset of the tack-down from the outline (mm).
    pub tackdown_inset: f64,
    /// Stop after tack-down so excess fabric can be cut away.
    pub trim_stop: bool,
    pub cover: CoverKind,
    /// Width of the cover stitch (mm).
    pub cover_width: f64,
    /// Spacing of the satin cover stitch (mm).
    pub cover_spacing: f64,
    pub pull_comp: f64,
    /// Colour of the appliqué fabric (display only).
    pub fabric_color: Rgb,
}

impl Default for AppliqueParams {
    fn default() -> Self {
        AppliqueParams {
            placement: true,
            tackdown: TackdownKind::Zigzag,
            tackdown_width: 1.2,
            tackdown_inset: 0.6,
            trim_stop: true,
            cover: CoverKind::Satin,
            cover_width: 3.0,
            cover_spacing: 0.45,
            pull_comp: 0.2,
            fabric_color: Rgb::new(0xe9, 0xd8, 0xa6),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppliqueObject {
    pub shape: Shape,
    #[serde(default)]
    pub params: AppliqueParams,
}

// ---------------------------------------------------------------------------
// Lettering
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Baseline {
    #[default]
    Straight,
    /// Text along a circular arc of the given radius (mm). `clockwise` text runs
    /// over the top of the circle (convex); counter-clockwise text runs along the
    /// bottom (concave), reading left to right in both cases.
    Arc { radius: f64, clockwise: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextParams {
    /// The text; `\n` separates lines.
    pub text: String,
    /// Font name (one of the built-in fonts or a loaded font).
    pub font: String,
    /// Height of capital letters (mm).
    pub height: f64,
    /// Extra space between letters (mm, may be negative).
    pub letter_spacing: f64,
    /// Line pitch as a multiple of `height`.
    pub line_spacing: f64,
    pub align: TextAlign,
    pub baseline: Baseline,
}

impl Default for TextParams {
    fn default() -> Self {
        TextParams {
            text: "Text".into(),
            font: "Sans Bold".into(),
            height: 10.0,
            letter_spacing: 0.0,
            line_spacing: 1.5,
            align: TextAlign::Center,
            baseline: Baseline::Straight,
        }
    }
}

/// A laid-out glyph outline in design space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub ch: char,
    pub shape: Shape,
    /// Direction of the baseline under this glyph (degrees, counter-clockwise on screen).
    #[serde(default)]
    pub angle: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LetteringObject {
    pub text: TextParams,
    /// Placement applied after layout. Layout space has the anchor of the first
    /// baseline at the origin (left end, centre or right end depending on alignment).
    #[serde(default)]
    pub transform: Affine,
    /// Cached glyph outlines in design space (already transformed). Kept in the file
    /// so designs remain usable without the font.
    #[serde(default)]
    pub glyphs: Vec<Glyph>,
    /// Stitch settings for the letters. The fill angle is relative to each glyph's baseline.
    #[serde(default = "FillParams::satin")]
    pub fill: FillParams,
    /// Optional running-stitch outline around each letter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<RunParams>,
}

impl Default for LetteringObject {
    fn default() -> Self {
        LetteringObject { text: TextParams::default(), transform: Affine::IDENTITY, glyphs: Vec::new(), fill: FillParams::satin(), outline: None }
    }
}

// ---------------------------------------------------------------------------
// Raw stitches
// ---------------------------------------------------------------------------

/// Raw stitch records (one colour). May contain jumps, trims and stops, but not
/// colour changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StitchBlock {
    pub stitches: Vec<Stitch>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Node, Polygon};

    fn sample_design() -> Design {
        let mut d = Design::new();
        let fill = ObjectKind::Fill(FillObject::new(Shape::rect(Point::new(0.0, 0.0), Point::new(20.0, 10.0)), FillParams::default()));
        d.add(fill, 0);
        let run = ObjectKind::Run(RunObject {
            path: Contour::new(
                vec![Node::corner(Point::new(0.0, 15.0)), Node::curve(Point::new(10.0, 20.0)), Node::corner(Point::new(20.0, 15.0))],
                false,
            ),
            params: RunParams { kind: RunKind::Triple, ..Default::default() },
        });
        d.add(run, 1);
        d.backdrop = Some(Backdrop {
            name: "logo.png".into(),
            data: vec![1, 2, 3, 250],
            min: Point::new(-5.0, -5.0),
            width: 30.0,
            height: 30.0,
            opacity: 0.5,
            visible: true,
            locked: false,
        });
        d
    }

    #[test]
    fn json_round_trip() {
        let d = sample_design();
        let json = serde_json::to_string_pretty(&d).unwrap();
        let back: Design = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
        assert!(json.contains("\"type\": \"Fill\""));
    }

    #[test]
    fn ids_are_unique() {
        let mut d = sample_design();
        let a = d.objects[0].id;
        let b = d.objects[1].id;
        assert_ne!(a, b);
        let c = d.add(ObjectKind::Stitches(StitchBlock::default()), 0);
        assert!(c > b);
    }

    #[test]
    fn rotating_a_fill_rotates_its_angle() {
        let mut d = sample_design();
        let obj = &mut d.objects[0];
        // +90° in y-down maths = clockwise on screen, so a 45° (screen CCW) angle becomes -45° = 135°.
        obj.transform(&Affine::rotate(std::f64::consts::FRAC_PI_2));
        let ObjectKind::Fill(f) = &obj.kind else { panic!() };
        assert!((f.params.angle - 135.0).abs() < 1e-9, "{}", f.params.angle);
        // mirrored horizontally: 135° -> 45°
        obj.transform(&Affine::scale(-1.0, 1.0));
        let ObjectKind::Fill(f) = &obj.kind else { panic!() };
        assert!((f.params.angle - 45.0).abs() < 1e-9, "{}", f.params.angle);
    }

    #[test]
    fn bbox_and_outlines() {
        let d = sample_design();
        let b = d.objects[0].bbox();
        assert!((b.width() - 20.0).abs() < 1e-9 && (b.height() - 10.0).abs() < 1e-9);
        let outlines = d.objects[0].kind.outlines();
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].first(), outlines[0].last());
        let _ = Polygon::default();
    }
}
