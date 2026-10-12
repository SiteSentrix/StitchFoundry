//! Embroidery settings attached to the objects of a host vector document.
//!
//! In a vector editor every path, compound path or text object can carry an
//! [`Embroidery`] record saying how it is stitched; the stitch plan is generated
//! from the artwork itself (the way Ink/Stitch works inside an SVG editor).
//!
//! **Units:** positions stored here (inclination lines, start/stop points) are in
//! the *host document's* coordinates (VectorCraft: points, y down), so they move
//! with the object when the host transforms it. Stitch parameters (spacing,
//! lengths, widths) are physical millimetres.

use std::collections::BTreeMap;

use crate::brush::WidthProfile;
use crate::direction::Guide;
use crate::fabrics::Fabric;
use crate::geom::{Affine, Point};
use crate::hoops::Hoop;
use crate::model::{AppliqueParams, DesignSettings, FillKind, FillParams, RunKind, RunParams, SatinParams, transform_angle};
use crate::pattern::Stitch;
use crate::threads::Thread;
use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_constant(p: &WidthProfile) -> bool {
    *p == WidthProfile::Constant
}

/// How an object is stitched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StitchType {
    /// Running stitch along the outline or stroke (single, triple, blanket, ...).
    Run(RunParams),
    /// Satin: along an open path (with `width`/`profile`), between two rails of a
    /// compound path, or across a closed shape following its inclination lines.
    Satin(SatinParams),
    /// Area fill of a closed shape (tatami, satin fill, contour, meander, ...).
    Fill(FillParams),
    /// Appliqué placement, tack-down and cover around a closed shape.
    Applique(AppliqueParams),
    /// Every path vertex is a needle penetration (imported or hand-placed stitches).
    Manual,
    /// A group whose stitched members are sewn as one path (branching): ordered for
    /// the shortest travel, with the thread carried between them as running stitches
    /// where later members cover it. The group itself sews nothing.
    Branch,
}

impl StitchType {
    /// The stitch type a stitch type menu entry (an `embroidery.set` type) makes, with
    /// default parameters; aliases (`single`, `triple`, `back`, `stem`, `steil`,
    /// `tatami`) too. `None` for an unknown name (and for `none`, no stitches).
    pub fn from_name(name: &str) -> Option<StitchType> {
        let fill = |kind: FillKind| StitchType::Fill(FillParams { kind, ..FillParams::default() });
        let run = |kind: RunKind| StitchType::Run(RunParams { kind, ..RunParams::default() });
        Some(match name {
            "run" | "single" => run(RunKind::Single),
            "bean" | "triple" => run(RunKind::Triple),
            "blanket" => run(RunKind::Blanket),
            "backstitch" | "back" => run(RunKind::Back),
            "stemstitch" | "stem" => run(RunKind::Stem),
            "zigzag" => StitchType::Run(RunParams { kind: RunKind::Zigzag, spacing: 0.8, ..RunParams::default() }),
            "motif" => StitchType::Run(RunParams { kind: RunKind::Motif, width: 3.0, spacing: 4.0, ..RunParams::default() }),
            "satin" | "steil" => StitchType::Satin(SatinParams::default()),
            "fill" | "tatami" => fill(FillKind::Tatami),
            "satinfill" => StitchType::Fill(FillParams::satin()),
            "contour" => StitchType::Fill(FillParams { kind: FillKind::Contour, spacing: 0.6, ..FillParams::default() }),
            "spiral" => StitchType::Fill(FillParams { kind: FillKind::Spiral, spacing: 0.6, ..FillParams::default() }),
            "radial" => StitchType::Fill(FillParams { kind: FillKind::Radial, spacing: 0.8, ..FillParams::default() }),
            "cross" => StitchType::Fill(FillParams { kind: FillKind::Cross, stitch_length: 2.5, ..FillParams::default() }),
            "meander" => StitchType::Fill(FillParams { kind: FillKind::Meander, spacing: 2.0, stitch_length: 2.5, ..FillParams::default() }),
            "motiffill" => StitchType::Fill(FillParams { kind: FillKind::Motif, spacing: 5.0, stitch_length: 2.0, ..FillParams::default() }),
            "applique" => StitchType::Applique(AppliqueParams::default()),
            "manual" => StitchType::Manual,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            StitchType::Run(r) => r.kind.label(),
            StitchType::Satin(_) => "Satin",
            StitchType::Fill(f) => f.kind.label(),
            StitchType::Applique(_) => "Appliqué",
            StitchType::Manual => "Manual stitches",
            StitchType::Branch => "Branch",
        }
    }

    /// The stitch type menu entry (the `embroidery.set` type) this is: `run`, `bean`,
    /// `blanket`, `backstitch`, `stemstitch`, `satin`, `fill`, `satinfill`, `contour`,
    /// `spiral`, `radial`, `cross`, `meander`, `motiffill`, `applique`, `manual` or `branch`. Zigzag
    /// and motif runs are runs (styles of the Run entry).
    pub fn type_name(&self) -> &'static str {
        match self {
            StitchType::Run(r) => match r.kind {
                RunKind::Triple => "bean",
                RunKind::Blanket => "blanket",
                RunKind::Back => "backstitch",
                RunKind::Stem => "stemstitch",
                RunKind::Single | RunKind::Zigzag | RunKind::Motif | RunKind::Hand => "run",
            },
            StitchType::Satin(_) => "satin",
            StitchType::Fill(f) => match f.kind {
                FillKind::Tatami => "fill",
                FillKind::Satin => "satinfill",
                FillKind::Contour => "contour",
                FillKind::Spiral => "spiral",
                FillKind::Radial => "radial",
                FillKind::Cross => "cross",
                FillKind::Meander => "meander",
                FillKind::Motif => "motiffill",
            },
            StitchType::Applique(_) => "applique",
            StitchType::Manual => "manual",
            StitchType::Branch => "branch",
        }
    }

    /// Short machine-readable name (`run`, `satin`, `fill`, `applique`, `manual`,
    /// `branch`).
    pub fn id(&self) -> &'static str {
        match self {
            StitchType::Run(_) => "run",
            StitchType::Satin(_) => "satin",
            StitchType::Fill(_) => "fill",
            StitchType::Applique(_) => "applique",
            StitchType::Manual => "manual",
            StitchType::Branch => "branch",
        }
    }
}

/// Embroidery settings of one object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Embroidery {
    pub stitch: StitchType,
    /// Thread; `None` uses the object's fill colour (strokes: stroke colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<Thread>,
    /// Satin width along an open path (mm); `None` uses the stroke width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Width profile of satin along an open path (tapered, calligraphy, pressure).
    #[serde(default, skip_serializing_if = "is_constant")]
    pub profile: WidthProfile,
    /// Inclination lines (host coordinates): one sets the stitch angle, two or
    /// more make the stitches turn along the shape.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
    /// Split lines (host coordinates; tatami fills): each stitch row has a needle point where
    /// it crosses one, so the lines show in the fill (drawn splits, carved stamps, flexi
    /// splits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<Vec<Point>>,
    /// Pick the stitch direction automatically while there are no guides.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_direction: bool,
    /// Start bead (host coordinates); `None` = closest join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Point>,
    /// Stop bead (host coordinates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Point>,
    /// Always trim after this object.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trim_after: bool,
    /// Lock stitches at the start: `None` follows the design setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tie_in: Option<bool>,
    /// Lock stitches before the thread is cut after this object: `None` follows
    /// the design setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tie_off: Option<bool>,
    /// Remove underlying stitching: fills sewn before this object leave out what
    /// it covers, except a margin under its edge (see [`Embroidery::cut_overlap`]).
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove_underlying: bool,
    /// How far (mm) the fills below still run in under this object's edge when
    /// they are cut; `None` = [`UNDERLYING_OVERLAP`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_overlap: Option<f64>,
    /// Stop the machine after this object (to place appliqué fabric, or change
    /// thread by hand on a single-needle machine).
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_after: bool,
    /// Leave out the last needle point (Omit last stitch): one stitch less where the
    /// object's last stitch would sew back over its edge or into the next object.
    #[serde(default, skip_serializing_if = "is_false")]
    pub omit_last: bool,
    /// Stitches edited by hand: the object's own records (host coordinates, in
    /// sewing order, with their trims, stops and jumps), sewn instead of stitches
    /// generated from its shape. `None` = generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<Vec<Stitch>>,
    /// The style (a named set of stitch properties) the object follows: redefining
    /// the style changes the object too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

/// How far (mm) fills cut by an object above still run in under its edge, so no
/// gap opens when the fabric pulls.
pub const UNDERLYING_OVERLAP: f64 = 1.0;

impl Embroidery {
    pub fn new(stitch: StitchType) -> Self {
        Embroidery {
            stitch,
            thread: None,
            width: None,
            profile: WidthProfile::Constant,
            guides: Vec::new(),
            splits: Vec::new(),
            auto_direction: true,
            entry: None,
            exit: None,
            trim_after: false,
            tie_in: None,
            tie_off: None,
            remove_underlying: false,
            underlying_overlap: None,
            stop_after: false,
            omit_last: false,
            frozen: None,
            style: None,
        }
    }

    /// The object's stitch properties without what belongs to its shape (inclination
    /// lines, split lines, beads, hand-edited stitches, a fill's centre), its thread or its
    /// style: what current properties and styles hold.
    pub fn properties(&self) -> Embroidery {
        let mut e = self.clone();
        e.thread = None;
        e.guides.clear();
        e.splits.clear();
        e.entry = None;
        e.exit = None;
        e.frozen = None;
        e.style = None;
        if let StitchType::Fill(f) = &mut e.stitch {
            f.center = None;
        }
        e
    }

    /// Takes the stitch properties of `props` (see [`Embroidery::properties`]),
    /// keeping this object's thread, inclination and split lines, beads, hand-edited
    /// stitches, fill centre and style.
    pub fn take_properties(&mut self, props: &Embroidery) {
        let center = match &self.stitch {
            StitchType::Fill(f) => f.center,
            _ => None,
        };
        let mut e = props.properties();
        e.thread = self.thread.take();
        e.guides = std::mem::take(&mut self.guides);
        e.splits = std::mem::take(&mut self.splits);
        e.entry = self.entry;
        e.exit = self.exit;
        e.frozen = self.frozen.take();
        e.style = self.style.take();
        if let StitchType::Fill(f) = &mut e.stitch
            && f.center.is_none()
        {
            f.center = center;
        }
        *self = e;
    }

    /// The overlap (mm) kept under this object's edge when it removes the
    /// stitching below it.
    pub fn cut_overlap(&self) -> f64 {
        self.underlying_overlap.unwrap_or(UNDERLYING_OVERLAP).clamp(0.0, 10.0)
    }

    pub fn run() -> Self {
        Self::new(StitchType::Run(RunParams::default()))
    }

    pub fn satin() -> Self {
        Self::new(StitchType::Satin(SatinParams::default()))
    }

    pub fn fill() -> Self {
        Self::new(StitchType::Fill(FillParams::default()))
    }

    /// A closed shape stitched with satin that follows its inclination lines.
    pub fn satin_fill() -> Self {
        Self::new(StitchType::Fill(FillParams { kind: FillKind::Satin, ..FillParams::satin() }))
    }

    pub fn applique() -> Self {
        Self::new(StitchType::Applique(AppliqueParams::default()))
    }

    pub fn manual() -> Self {
        Self::new(StitchType::Manual)
    }

    /// Follows a host transform: positions move, the fill angle turns with the
    /// shape and satin widths scale with it.
    pub fn transform(&mut self, m: &Affine) {
        for g in &mut self.guides {
            g.transform(m);
        }
        for p in self.splits.iter_mut().flatten() {
            *p = m.apply(*p);
        }
        for s in self.frozen.iter_mut().flatten() {
            let p = m.apply(s.pos());
            (s.x, s.y) = (p.x, p.y);
        }
        if let Some(p) = &mut self.entry {
            *p = m.apply(*p);
        }
        if let Some(p) = &mut self.exit {
            *p = m.apply(*p);
        }
        if let StitchType::Fill(f) = &mut self.stitch {
            f.angle = transform_angle(f.angle, m);
            if let Some(c) = &mut f.center {
                *c = m.apply(*c);
            }
        }
        let k = m.mean_scale();
        if k.is_finite()
            && k > 1e-9
            && (k - 1.0).abs() > 1e-12
            && let Some(w) = &mut self.width
        {
            *w *= k;
        }
    }
}

/// Document-wide embroidery settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocEmbroidery {
    pub hoop: Option<Hoop>,
    /// Fabric preset name.
    pub fabric: String,
    pub settings: DesignSettings,
    /// Thread chart used to name colours ("Generic", "Brother", "Janome", ...).
    pub chart: String,
    /// User-defined threads (in addition to the chart).
    pub palette: Vec<Thread>,
    /// The machine profile chosen for the design: a standard profile's file format
    /// (`"pes"`) or the name of one of the user's; empty when none is chosen.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub machine: String,
    /// The file format of the design's machine when it is one of the user's profiles,
    /// so the design keeps it where that profile isn't saved.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub machine_format: String,
    /// Properties new objects take, by stitch type ([`StitchType::type_name`]): the
    /// digitizing presets, or "current properties".
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub current: BTreeMap<String, Embroidery>,
    /// Named sets of stitch properties.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<EmbStyle>,
    /// Fabrics made for this design, next to the built-in ones.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fabrics: Vec<Fabric>,
    /// What Check Design flags.
    #[serde(skip_serializing_if = "CheckLimits::is_default")]
    pub checks: CheckLimits,
}

/// The limits Check Design measures the design against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckLimits {
    /// Layers of thread over a 2 × 2 mm square flagged as too dense. A tatami fill is
    /// about 1.2 with its underlay, a satin column 2.5 to 3.5.
    pub dense_layers: f64,
    /// Stitches longer than this (mm) can snag.
    pub long_stitch: f64,
    /// Lettering lower than this (mm) is hard to sew legibly.
    pub small_lettering: f64,
    /// Objects smaller than this (mm) both ways are tiny.
    pub tiny_object: f64,
}

impl Default for CheckLimits {
    fn default() -> Self {
        CheckLimits { dense_layers: 4.5, long_stitch: 8.0, small_lettering: 5.0, tiny_object: 1.0 }
    }
}

impl CheckLimits {
    pub fn is_default(&self) -> bool {
        *self == CheckLimits::default()
    }
}

/// A named set of stitch properties ([`Embroidery::properties`]) objects can follow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbStyle {
    pub name: String,
    pub properties: Embroidery,
}

impl DocEmbroidery {
    /// The properties a new object of stitch type `name` ([`StitchType::type_name`])
    /// takes, when they were made current.
    pub fn current_for(&self, name: &str) -> Option<&Embroidery> {
        self.current.get(name).filter(|e| e.stitch.type_name() == name)
    }

    pub fn style(&self, name: &str) -> Option<&EmbStyle> {
        self.styles.iter().find(|s| s.name == name)
    }

    /// A fabric by name: one made for the design, else a built-in one.
    pub fn fabric(&self, name: &str) -> Option<Fabric> {
        self.fabrics.iter().find(|f| f.name == name).cloned().or_else(|| crate::fabrics::find(name))
    }

    /// The file format the design's machine reads (`"vp3"`): the format of the profile
    /// it names among the standard ones and the user's `saved` ones, else the one the
    /// design kept; none when no machine is chosen.
    pub fn machine_file_format(&self, saved: &[crate::machines::MachineProfile]) -> Option<String> {
        crate::machines::find(saved, &self.machine).map(|m| m.format).or_else(|| Some(self.machine_format.clone()).filter(|f| !f.is_empty()))
    }
}

impl Default for DocEmbroidery {
    fn default() -> Self {
        DocEmbroidery {
            hoop: None,
            fabric: String::new(),
            settings: DesignSettings::default(),
            chart: "Generic".into(),
            palette: Vec::new(),
            machine: String::new(),
            machine_format: String::new(),
            current: BTreeMap::new(),
            styles: Vec::new(),
            fabrics: Vec::new(),
            checks: CheckLimits::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_compact_and_round_trips() {
        let mut e = Embroidery::fill();
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.starts_with(r#"{"stitch":{"type":"fill""#), "{json}");
        assert!(!json.contains("guides") && !json.contains("auto_direction"));
        e.guides.push(Guide::new(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        e.entry = Some(Point::new(1.0, 2.0));
        e.auto_direction = false;
        let back: Embroidery = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(e, back);
        let manual: Embroidery = serde_json::from_str(r#"{"stitch":{"type":"manual"}}"#).unwrap();
        assert_eq!(manual, Embroidery::manual());
    }

    #[test]
    fn transform_moves_guides_and_turns_angle() {
        let mut e = Embroidery::fill();
        e.guides.push(Guide::new(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        e.width = Some(2.0);
        e.transform(&Affine::translate(Point::new(5.0, 5.0)).then(&Affine::scale(2.0, 2.0)));
        assert!(e.guides[0].a.approx_eq(Point::new(10.0, 10.0), 1e-9));
        assert!((e.width.unwrap() - 4.0).abs() < 1e-9);
        e.transform(&Affine::rotate(std::f64::consts::FRAC_PI_2));
        let StitchType::Fill(f) = &e.stitch else { panic!() };
        assert!((f.angle - 135.0).abs() < 1e-9, "{}", f.angle);
    }
}
