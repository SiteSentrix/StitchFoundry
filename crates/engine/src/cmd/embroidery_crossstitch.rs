//! Cross-stitch designer (CH-03): a pattern of X stitches drawn cell by cell on a grid, the way
//! counted cross stitch is charted. A pattern is a group holding its grid (thin lines, not sewn)
//! and one object per thread colour, the union of that colour's cells, sewn as a cross fill on
//! the pattern's own grid, so every X lands in its cell. Cells are painted and erased with the
//! Cross Stitch tool (or `embroidery.crossStitch.paint`), and a picture can become a pattern
//! (`embroidery.crossStitch.fromImage`). Public description of the feature:
//! https://docs.wilcom.com/embroiderystudio/27/en/OnlineHelp/CrossStitch/PART_CrossStitch/PART_CrossStitch-1.htm

use std::sync::Arc;

use kurbo::BezPath;
use serde_json::{Value, json};
use sf_core::Rgb;
use sf_core::attr::{Embroidery, StitchType};
use sf_core::model::{CrossStyle, FillKind, FillParams, FillUnderlay};
use sf_core::threads::Thread;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, ObjectAttributes};
use vectorcraft_embroidery::{MM_PER_PT, PT_PER_MM};
use vectorcraft_geom::PathData;

use super::embroidery_photo::{Picture, chart, main_colors, majority, quantise, sample};
use super::*;

/// The data key marking a pattern's group: `"cols rows style"`.
pub const PATTERN_KEY: &str = "crossStitch";
/// The data key marking a pattern's grid lines.
const GRID_KEY: &str = "crossStitchGrid";
/// The data key marking one colour's object: its colour `#rrggbb`.
const COLOR_KEY: &str = "crossStitchColor";
/// The data key holding one colour's cells: runs `row,col,count` apart by spaces.
const CELLS_KEY: &str = "crossStitchCells";
/// Most cells along a side.
const MAX_SIDE: u32 = 300;
/// Cell sizes (mm): the X stitches' side.
const CELL_MM: (f64, f64) = (1.0, 10.0);
/// Most colours in one pattern.
const MAX_COLORS: usize = 64;
/// Most points one paint stroke gives.
const MAX_STROKE: usize = 20_000;
/// The grid lines' colour.
const GRID_COLOR: [u8; 3] = [0xb4, 0xb4, 0xb4];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.crossStitch.new",
            "New Cross-Stitch Pattern…",
            ["Embroidery", "Cross Stitch"],
            None,
            "{cols?: (40; 1 to 300), rows?: (40; 1 to 300), cell?: mm (2.5; 1 to 10, the side of one X), style?: full | half | upright | double (full), at?: [x, y] points (the grid's top left; default: centred on the page)} an empty cross-stitch grid, ready to paint with the Cross Stitch tool; it is selected → {id, cols, rows, cell}",
            has_doc,
            new_pattern
        ),
        cmd!(
            "embroidery.crossStitch.paint",
            "Paint Cross Stitches",
            [],
            None,
            "{id?: the pattern (or one of its objects; default: the selected pattern, else the one under the first point), at?: [[x, y], …] points along a stroke, cells?: [[col, row], …], color?: \"#rrggbb\" (needed unless erasing), erase?: bool, size?: cells (1; 1 to 9, a square brush)} paint (or erase) the cells the stroke passes through, one colour a cell → {id, cells, colors}",
            has_doc,
            paint
        ),
        cmd!(
            "embroidery.crossStitch.fromImage",
            "Cross Stitch from Image…",
            ["Embroidery", "Cross Stitch"],
            None,
            "{id?: an image (the selected one), cols?: (60; 4 to 300), colors?: (8; 2 to 32), cell?: mm (1 to 10; default the image's width over `cols`), style?: full | half | upright | double, background?: bool (false: the colour round the edges is left unsewn), chart?: a thread chart's name} the image as a cross-stitch pattern over it: its main colours matched to threads, a colour a cell, specks cleaned → {id, cols, rows, cell, colors}",
            has_doc,
            from_image
        ),
        cmd!(
            "embroidery.crossStitch.set",
            "Cross-Stitch Pattern Settings",
            [],
            None,
            "{id?: the pattern (the selected one), cell?: mm (1 to 10; the grid grows or shrinks from its top left), style?: full | half | upright | double} change a pattern's cell size or how its cells are sewn → {id, cell, style}",
            has_doc,
            set_pattern
        ),
        cmd!(
            query "embroidery.crossStitch.info",
            "Cross-Stitch Pattern Info",
            [],
            None,
            "{id?: the pattern (the selected one)} → {id, cols, rows, cell (mm), style, size: [w, h] mm, colors: [{color, name, cells}]} (colours in sewing order)",
            has_doc,
            info
        ),
    ]
}

/// A pattern read from the document: its grid and each colour's cells.
struct Pattern {
    group: NodeId,
    cols: u32,
    rows: u32,
    style: CrossStyle,
    /// The grid's top left (points).
    origin: kurbo::Point,
    /// A cell's side (points).
    cell: f64,
    /// The colour objects in sewing order, each with its colour and thread.
    colors: Vec<(Option<NodeId>, Rgb, Option<Thread>)>,
    /// Each cell's colour (index into `colors`), row by row.
    cells: Vec<Option<usize>>,
    /// The colours whose cells changed since the pattern was read (their objects are rebuilt).
    dirty: Vec<bool>,
}

fn style_name(s: CrossStyle) -> &'static str {
    match s {
        CrossStyle::Full => "full",
        CrossStyle::Half => "half",
        CrossStyle::Upright => "upright",
        CrossStyle::Double => "double",
    }
}

fn parse_style(p: &Value, cmd: &str, default: CrossStyle) -> Result<CrossStyle> {
    match p.get("style") {
        None | Some(Value::Null) => Ok(default),
        Some(v) => CrossStyle::ALL
            .into_iter()
            .find(|s| v.as_str() == Some(style_name(*s)))
            .ok_or_else(|| bad(cmd, "`style` is full, half, upright or double")),
    }
}

fn side(p: &Value, key: &str, default: u32, lo: u32, cmd: &str) -> Result<u32> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (lo..=MAX_SIDE).contains(v))
            .ok_or_else(|| bad(cmd, format!("`{key}` is {lo} to {MAX_SIDE}"))),
    }
}

fn cell_mm(p: &Value, cmd: &str) -> Result<Option<f64>> {
    match p.get("cell") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|v| (CELL_MM.0..=CELL_MM.1).contains(v))
            .map(Some)
            .ok_or_else(|| bad(cmd, format!("`cell` is {} to {} mm", CELL_MM.0, CELL_MM.1))),
    }
}

fn data<'a>(n: &'a Node, key: &str) -> Option<&'a str> {
    n.extras().attrs.as_deref()?.data.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn with_data(n: &mut Node, key: &str, value: String) {
    let mut extras = n.extras_mut();
    let attrs = extras.attrs.get_or_insert_with(|| Box::new(ObjectAttributes::default()));
    attrs.data.retain(|(k, _)| k != key);
    attrs.data.push((key.to_string(), value));
}

/// The pattern `id` belongs to (it, or the group holding it).
fn pattern_of(d: &Document, id: NodeId) -> Option<NodeId> {
    let mut at = Some(id);
    for _ in 0..64 {
        let here = at?;
        if d.node(here).is_some_and(|n| data(n, PATTERN_KEY).is_some()) {
            return Some(here);
        }
        at = d.parent_of(here);
    }
    None
}

/// The pattern named by `id`, else the selected one.
fn chosen(s: &Session, p: &Value) -> Option<NodeId> {
    let st = s.doc().ok()?;
    match p.get("id").and_then(Value::as_u64) {
        Some(id) => pattern_of(&st.doc, NodeId(id)),
        None => st.selection.objects.iter().find_map(|id| pattern_of(&st.doc, *id)),
    }
}

/// Every pattern in the document.
fn all_patterns(d: &Document) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<Arc<Node>> = d.children(None).cloned().unwrap_or_default();
    while let Some(n) = stack.pop() {
        if data(&n, PATTERN_KEY).is_some() {
            out.push(n.id);
        } else if let Some(c) = n.children() {
            stack.extend(c.iter().cloned());
        }
    }
    out
}

/// The colour of a `#rrggbb` value.
fn rgb_of(v: &str) -> Option<Rgb> {
    Rgb::from_hex(v)
}

impl Pattern {
    /// Reads pattern `group`: its grid's place and size, and which cells each colour covers.
    fn read(d: &Document, group: NodeId) -> Option<Pattern> {
        let g = d.node(group)?;
        let mut words = data(g, PATTERN_KEY)?.split_whitespace();
        let cols: u32 = words.next()?.parse().ok().filter(|c| (1..=MAX_SIDE).contains(c))?;
        let rows: u32 = words.next()?.parse().ok().filter(|r| (1..=MAX_SIDE).contains(r))?;
        let style = words.next().and_then(|w| CrossStyle::ALL.into_iter().find(|s| style_name(*s) == w)).unwrap_or(CrossStyle::Full);
        let children = g.children()?;
        // the grid's box (mm), from its lines
        let grid = children.iter().find(|c| data(c, GRID_KEY).is_some())?;
        let pts: Vec<sf_core::Point> = vectorcraft_embroidery::outlines_mm(grid).into_iter().flatten().collect();
        let (x0, y0) = pts.iter().fold((f64::INFINITY, f64::INFINITY), |(x, y), p| (x.min(p.x), y.min(p.y)));
        let x1 = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let cell_mm = (x1 - x0) / f64::from(cols);
        if !(x0.is_finite() && y0.is_finite() && cell_mm.is_finite() && cell_mm > 0.01) {
            return None;
        }
        let mut pattern = Pattern {
            group,
            cols,
            rows,
            style,
            origin: kurbo::Point::new(x0 * PT_PER_MM, y0 * PT_PER_MM),
            cell: cell_mm * PT_PER_MM,
            colors: Vec::new(),
            cells: vec![None; cols as usize * rows as usize],
            dirty: Vec::new(),
        };
        for c in children.iter() {
            let Some(color) = data(c, COLOR_KEY).and_then(rgb_of) else { continue };
            if pattern.colors.len() >= MAX_COLORS {
                break;
            }
            let k = pattern.colors.len();
            let thread = c.embroidery.as_deref().and_then(|e| e.thread.clone());
            pattern.colors.push((Some(c.id), color, thread));
            pattern.dirty.push(false);
            if let Some(runs) = data(c, CELLS_KEY) {
                for run in runs.split_whitespace() {
                    let mut n = run.split(',').map(|v| v.parse::<u32>().ok());
                    let (Some(Some(r)), Some(Some(col)), Some(Some(len))) = (n.next(), n.next(), n.next()) else { continue };
                    for cc in col..col.saturating_add(len).min(cols) {
                        if r < rows
                            && let Some(v) = pattern.cells.get_mut((r * cols + cc) as usize)
                        {
                            *v = Some(k);
                        }
                    }
                }
                continue;
            }
            // no cell list (an older or hand-edited object): the cells its shape covers
            if let Some(d) = pattern.dirty.get_mut(k) {
                *d = true;
            }
            for region in vectorcraft_embroidery::regions_mm(c) {
                let bb = region.bbox();
                let c0 = (((bb.min.x - x0) / cell_mm).floor().max(0.0)) as u32;
                let r0 = (((bb.min.y - y0) / cell_mm).floor().max(0.0)) as u32;
                let c1 = (((bb.max.x - x0) / cell_mm).ceil().clamp(0.0, f64::from(cols))) as u32;
                let r1 = (((bb.max.y - y0) / cell_mm).ceil().clamp(0.0, f64::from(rows))) as u32;
                for r in r0..r1 {
                    for col in c0..c1 {
                        let centre = sf_core::Point::new(x0 + (f64::from(col) + 0.5) * cell_mm, y0 + (f64::from(r) + 0.5) * cell_mm);
                        if region.contains(centre)
                            && let Some(v) = pattern.cells.get_mut((r * cols + col) as usize)
                        {
                            *v = Some(k);
                        }
                    }
                }
            }
        }
        Some(pattern)
    }

    fn cell_mm(&self) -> f64 {
        self.cell * MM_PER_PT
    }

    /// The cell holding document point `q`, if it is on the grid.
    fn cell_at(&self, q: kurbo::Point) -> Option<(u32, u32)> {
        let c = ((q.x - self.origin.x) / self.cell).floor();
        let r = ((q.y - self.origin.y) / self.cell).floor();
        (c.is_finite() && r.is_finite() && c >= 0.0 && r >= 0.0 && c < f64::from(self.cols) && r < f64::from(self.rows))
            .then_some((c as u32, r as u32))
    }

    fn contains(&self, q: kurbo::Point) -> bool {
        self.cell_at(q).is_some()
    }

    fn set(&mut self, c: u32, r: u32, v: Option<usize>) -> bool {
        if c >= self.cols || r >= self.rows {
            return false;
        }
        match self.cells.get_mut((r * self.cols + c) as usize) {
            Some(x) if *x != v => {
                for k in [*x, v].into_iter().flatten() {
                    if let Some(d) = self.dirty.get_mut(k) {
                        *d = true;
                    }
                }
                *x = v;
                true
            }
            _ => false,
        }
    }

    /// The colour's index, added when new.
    fn color_index(&mut self, color: Rgb, thread: Option<Thread>) -> Result<usize> {
        if let Some(k) = self.colors.iter().position(|(_, c, _)| *c == color) {
            return Ok(k);
        }
        if self.colors.len() >= MAX_COLORS {
            return Err(bad("embroidery.crossStitch.paint", format!("a pattern has at most {MAX_COLORS} colours")));
        }
        self.colors.push((None, color, thread));
        self.dirty.push(true);
        Ok(self.colors.len() - 1)
    }

    /// Cells of each colour, counted.
    fn counts(&self) -> Vec<usize> {
        let mut n = vec![0usize; self.colors.len()];
        for k in self.cells.iter().flatten() {
            if let Some(c) = n.get_mut(*k) {
                *c += 1;
            }
        }
        n
    }

    /// Colour `k`'s runs of cells along each row: `(row, first col, count)`.
    fn runs(&self, k: usize) -> Vec<(u32, u32, u32)> {
        let mut out = Vec::new();
        for r in 0..self.rows {
            let mut c = 0;
            while c < self.cols {
                if self.cells.get((r * self.cols + c) as usize).copied().flatten() != Some(k) {
                    c += 1;
                    continue;
                }
                let start = c;
                while c < self.cols && self.cells.get((r * self.cols + c) as usize).copied().flatten() == Some(k) {
                    c += 1;
                }
                out.push((r, start, c - start));
            }
        }
        out
    }

    /// Colour `k`'s cells as a path (points): rectangles, a run of cells along a row each, runs
    /// alike in the rows below taken in.
    fn color_path(&self, runs: &[(u32, u32, u32)]) -> BezPath {
        let mut used = vec![false; runs.len()];
        let mut bp = BezPath::new();
        for i in 0..runs.len() {
            let Some(&(r, c, n)) = runs.get(i) else { continue };
            if used.get(i).copied().unwrap_or(true) {
                continue;
            }
            // as many rows below with the same run
            let mut bottom = r + 1;
            for (j, run) in runs.iter().enumerate().skip(i + 1) {
                if run.0 > bottom {
                    break;
                }
                if *run == (bottom, c, n) && !used.get(j).copied().unwrap_or(true) {
                    if let Some(u) = used.get_mut(j) {
                        *u = true;
                    }
                    bottom += 1;
                }
            }
            let (x0, x1) = (self.origin.x + f64::from(c) * self.cell, self.origin.x + f64::from(c + n) * self.cell);
            let (y0, y1) = (self.origin.y + f64::from(r) * self.cell, self.origin.y + f64::from(bottom) * self.cell);
            bp.move_to((x0, y0));
            bp.line_to((x1, y0));
            bp.line_to((x1, y1));
            bp.line_to((x0, y1));
            bp.close_path();
        }
        bp
    }

    /// The grid's lines (points): every cell's sides.
    fn grid_path(&self) -> BezPath {
        let mut bp = BezPath::new();
        let (w, h) = (f64::from(self.cols) * self.cell, f64::from(self.rows) * self.cell);
        for c in 0..=self.cols {
            let x = self.origin.x + f64::from(c) * self.cell;
            bp.move_to((x, self.origin.y));
            bp.line_to((x, self.origin.y + h));
        }
        for r in 0..=self.rows {
            let y = self.origin.y + f64::from(r) * self.cell;
            bp.move_to((self.origin.x, y));
            bp.line_to((self.origin.x + w, y));
        }
        bp
    }

    /// Colour `k`'s object: its cells, sewn as cross stitch on the pattern's grid in its thread.
    fn color_node(&self, k: usize) -> Option<Node> {
        let (_, color, thread) = self.colors.get(k)?;
        let runs = self.runs(k);
        if runs.is_empty() {
            return None;
        }
        let path = self.color_path(&runs);
        let paint = Paint::solid(Color::rgb8(color.r, color.g, color.b));
        let mut node =
            Node::path(self.colors.get(k)?.0.unwrap_or(NodeId(0)), PathData::from_bezpath(&path), Appearance::basic(paint, Paint::None, 0.0));
        let fill = FillParams {
            kind: FillKind::Cross,
            angle: 0.0,
            stitch_length: self.cell_mm(),
            cross: self.style,
            // the grid's corner: the X stitches line up with the cells
            center: Some(sf_core::Point::new(self.origin.x, self.origin.y)),
            underlay: FillUnderlay { edge_run: false, tatami: false, ..FillUnderlay::default() },
            ..FillParams::default()
        };
        let mut e = Embroidery::new(StitchType::Fill(fill));
        e.auto_direction = false;
        e.thread = Some(thread.clone().unwrap_or_else(|| Thread::new(*color, "")));
        node.embroidery = Some(Box::new(e));
        node.name = Some(format!("Cross stitch {}", color.to_hex()));
        with_data(&mut node, COLOR_KEY, color.to_hex());
        let list: Vec<String> = runs.iter().map(|(r, c, n)| format!("{r},{c},{n}")).collect();
        with_data(&mut node, CELLS_KEY, list.join(" "));
        Some(node)
    }

    fn grid_node(&self) -> Node {
        let stroke = Paint::solid(Color::rgb8(GRID_COLOR[0], GRID_COLOR[1], GRID_COLOR[2]));
        let mut n = Node::path(NodeId(0), PathData::from_bezpath(&self.grid_path()), Appearance::basic(Paint::None, stroke, 0.25));
        n.name = Some("Cross stitch grid".into());
        with_data(&mut n, GRID_KEY, "1".into());
        n
    }

    fn header(&self) -> String {
        format!("{} {} {}", self.cols, self.rows, style_name(self.style))
    }

    /// Writes the pattern back: the group's grid and the colour objects whose cells changed
    /// rebuilt (all of them when `all`), colours with no cells left taken out. The group keeps
    /// its id, name and place.
    fn write(&self, d: &mut Document, all: bool) -> Result<()> {
        let mut kids: Vec<Arc<Node>> = Vec::new();
        let old: Vec<Arc<Node>> = d.node(self.group).and_then(Node::children).cloned().unwrap_or_default();
        // the grid first (it keeps its id), then anything else the user put in the group
        let mut grid = self.grid_node();
        grid.id = old.iter().find(|c| data(c, GRID_KEY).is_some()).map_or_else(|| d.alloc_id(), |c| c.id);
        kids.push(Arc::new(grid));
        for c in &old {
            if data(c, GRID_KEY).is_none() && data(c, COLOR_KEY).is_none() {
                kids.push(c.clone());
            }
        }
        for k in 0..self.colors.len() {
            let before = self.colors.get(k).and_then(|c| c.0).and_then(|id| old.iter().find(|o| o.id == id));
            // unchanged colours stay as they are (and keep their stitches cached)
            if !all
                && !self.dirty.get(k).copied().unwrap_or(true)
                && let Some(o) = before
            {
                kids.push(o.clone());
                continue;
            }
            let Some(mut n) = self.color_node(k) else { continue };
            // a colour object keeps its own id, and what the user changed on it
            match before {
                Some(o) => {
                    n.id = o.id;
                    n.visible = o.visible;
                    n.locked = o.locked;
                    n.name = o.name.clone().or(n.name);
                }
                None => n.id = d.alloc_id(),
            }
            kids.push(Arc::new(n));
        }
        let header = self.header();
        let g = d.node_mut(self.group).ok_or(EngineError::NoNode(self.group))?;
        with_data(g, PATTERN_KEY, header);
        if let Some(c) = g.children_mut() {
            *c = kids;
        }
        Ok(())
    }
}

/// The page's centre (points).
fn page_centre(d: &Document) -> kurbo::Point {
    d.artboards.first().map_or(kurbo::Point::new(300.0, 300.0), |a| a.rect.center())
}

fn new_group(
    d: &mut Document,
    sel: &mut vectorcraft_doc::Selection,
    pattern: &mut Pattern,
    name: &str,
    at: (Option<NodeId>, usize),
) -> Result<NodeId> {
    let mut g = Node::group(d.alloc_id(), Vec::new());
    g.name = Some(name.to_string());
    with_data(&mut g, PATTERN_KEY, pattern.header());
    let id = g.id;
    pattern.group = id;
    d.insert(at.0, at.1, g)?;
    pattern.write(d, true)?;
    sel.set([id]);
    Ok(id)
}

fn new_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.crossStitch.new";
    let cols = side(p, "cols", 40, 1, CMD)?;
    let rows = side(p, "rows", 40, 1, CMD)?;
    let cell = cell_mm(p, CMD)?.unwrap_or(2.5) * PT_PER_MM;
    let style = parse_style(p, CMD, CrossStyle::Full)?;
    let at = match p.get("at") {
        None | Some(Value::Null) => None,
        Some(v) => Some(point(v).ok_or_else(|| bad(CMD, "`at` is [x, y] in points"))?),
    };
    let parent = s.doc()?.target_parent()?;
    let id = s.edit("New Cross-Stitch Pattern", |d, sel| {
        let origin = at.unwrap_or_else(|| {
            let c = page_centre(d);
            kurbo::Point::new(c.x - f64::from(cols) * cell * 0.5, c.y - f64::from(rows) * cell * 0.5)
        });
        let mut pattern = Pattern {
            group: NodeId(0),
            cols,
            rows,
            style,
            origin,
            cell,
            colors: Vec::new(),
            cells: vec![None; cols as usize * rows as usize],
            dirty: Vec::new(),
        };
        new_group(d, sel, &mut pattern, "Cross Stitch", (parent, usize::MAX))
    })?;
    Ok(json!({ "id": id.0, "cols": cols, "rows": rows, "cell": cell * MM_PER_PT }))
}

fn point(v: &Value) -> Option<kurbo::Point> {
    let x = v.get(0)?.as_f64()?;
    let y = v.get(1)?.as_f64()?;
    (x.is_finite() && y.is_finite()).then(|| kurbo::Point::new(x, y))
}

fn paint(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.crossStitch.paint";
    let erase = bool_or(p, "erase", false);
    let size = match p.get("size") {
        None | Some(Value::Null) => 1,
        Some(v) => v.as_u64().filter(|v| (1..=9).contains(v)).ok_or_else(|| bad(CMD, "`size` is 1 to 9 cells"))? as i64,
    };
    let color = match str_param(p, "color") {
        Some(c) => Some(rgb_of(c).ok_or_else(|| bad(CMD, "`color` is \"#rrggbb\""))?),
        None if erase => None,
        None => return Err(bad(CMD, "give the `color` to paint with")),
    };
    let stroke: Vec<kurbo::Point> = match p.get("at") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if list.len() <= MAX_STROKE => {
            list.iter().map(|v| point(v).ok_or_else(|| bad(CMD, "`at` is a list of [x, y] points"))).collect::<Result<_>>()?
        }
        Some(_) => return Err(bad(CMD, format!("`at` is a list of at most {MAX_STROKE} [x, y] points"))),
    };
    let listed: Vec<(i64, i64)> = match p.get("cells") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if list.len() <= MAX_STROKE => list
            .iter()
            .map(|v| {
                let c = v.get(0).and_then(Value::as_i64);
                let r = v.get(1).and_then(Value::as_i64);
                c.zip(r).ok_or_else(|| bad(CMD, "`cells` is a list of [col, row]"))
            })
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(CMD, format!("`cells` is a list of at most {MAX_STROKE} [col, row]"))),
    };
    if stroke.is_empty() && listed.is_empty() {
        return Err(bad(CMD, "give the cells to paint: `at` points or `cells`"));
    }
    let d = &s.doc()?.doc;
    let group = chosen(s, p)
        .or_else(|| stroke.first().and_then(|q| all_patterns(d).into_iter().find(|g| Pattern::read(d, *g).is_some_and(|pat| pat.contains(*q)))))
        .ok_or_else(|| bad(CMD, "paint on a cross-stitch pattern (Embroidery › Cross Stitch › New Cross-Stitch Pattern)"))?;
    let mut pattern = Pattern::read(d, group).ok_or_else(|| bad(CMD, "that cross-stitch pattern can't be read"))?;
    let thread = color.map(|c| nearest_thread(s, c));
    let value = match color {
        Some(c) => Some(pattern.color_index(c, thread)?),
        None => None,
    };
    // the cells under the stroke: its points, and between them every half cell; only those the
    // brush can reach the grid from, each once
    let (cols, rows) = (i64::from(pattern.cols), i64::from(pattern.rows));
    let near = |c: i64, r: i64| c > -size && r > -size && c < cols + size && r < rows + size;
    let mut seen = std::collections::HashSet::new();
    let mut hit: Vec<(i64, i64)> = listed.into_iter().filter(|(c, r)| near(*c, *r) && seen.insert((*c, *r))).collect();
    let mut prev: Option<kurbo::Point> = None;
    let mut budget = 2_000_000usize;
    for q in &stroke {
        let from = prev.unwrap_or(*q);
        let steps = ((from.distance(*q) / (pattern.cell * 0.5)).ceil() as usize).clamp(1, 1000).min(budget);
        budget -= steps;
        for k in 1..=steps {
            let at = from.lerp(*q, k as f64 / steps as f64);
            let c = ((at.x - pattern.origin.x) / pattern.cell).floor();
            let r = ((at.y - pattern.origin.y) / pattern.cell).floor();
            if c.is_finite() && r.is_finite() && c.abs() < 1e6 && r.abs() < 1e6 && near(c as i64, r as i64) && seen.insert((c as i64, r as i64)) {
                hit.push((c as i64, r as i64));
            }
        }
        prev = Some(*q);
    }
    let lo = -(size - 1) / 2;
    let mut changed = 0usize;
    for (c, r) in hit {
        for dy in lo..lo + size {
            for dx in lo..lo + size {
                let (cc, rr) = (c.saturating_add(dx), r.saturating_add(dy));
                if cc >= 0 && rr >= 0 && cc < i64::from(pattern.cols) && rr < i64::from(pattern.rows) && pattern.set(cc as u32, rr as u32, value) {
                    changed += 1;
                }
            }
        }
    }
    let label = if erase { "Erase Cross Stitches" } else { "Paint Cross Stitches" };
    if changed > 0 {
        s.edit(label, |d, _| pattern.write(d, false))?;
    }
    let colors = pattern.counts().iter().filter(|n| **n > 0).count();
    Ok(json!({ "id": group.0, "cells": changed, "colors": colors }))
}

/// A thread for `c`: the design's chart's thread of that colour, else one of its own.
fn nearest_thread(s: &Session, c: Rgb) -> Thread {
    match chart(s, &json!({}), "embroidery.crossStitch.paint") {
        Ok(chart) => chart.threads.iter().find(|t| t.color == c).cloned().unwrap_or_else(|| Thread::new(c, "")),
        Err(_) => Thread::new(c, ""),
    }
}

fn from_image(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.crossStitch.fromImage";
    let cols = side(p, "cols", 60, 4, CMD)?;
    let n = match p.get("colors") {
        None | Some(Value::Null) => 8,
        Some(v) => v.as_u64().filter(|v| (2..=32).contains(v)).ok_or_else(|| bad(CMD, "`colors` is 2 to 32"))? as usize,
    };
    let style = parse_style(p, CMD, CrossStyle::Full)?;
    let background = bool_or(p, "background", false);
    let pic = Picture::get(s, p, CMD)?;
    let b = pic.bounds();
    let cell = match cell_mm(p, CMD)? {
        Some(c) => c * PT_PER_MM,
        None => (b.width() / f64::from(cols)).clamp(CELL_MM.0 * PT_PER_MM, CELL_MM.1 * PT_PER_MM),
    };
    let cols = ((b.width() / cell).ceil() as u32).clamp(1, MAX_SIDE);
    let rows64 = (b.height() / cell).ceil();
    if !(rows64.is_finite() && rows64 >= 1.0 && rows64 <= f64::from(MAX_SIDE)) {
        return Err(bad(CMD, format!("the image would be more than {MAX_SIDE} cells tall: use larger cells or fewer columns")));
    }
    let rows = rows64 as u32;
    let chart = chart(s, p, CMD)?;
    // each cell the image's colour at its middle
    let inv = pic.to_doc.inverse();
    let origin = kurbo::Point::new(b.x0, b.y0);
    let grid = vectorcraft_trace::Raster::from_fn(cols, rows, |x, y| {
        let q = inv * kurbo::Point::new(origin.x + (f64::from(x) + 0.5) * cell, origin.y + (f64::from(y) + 0.5) * cell);
        if q.x >= 0.0 && q.y >= 0.0 && q.x < f64::from(pic.raster.width) && q.y < f64::from(pic.raster.height) {
            pic.raster.pixel(q.x as u32, q.y as u32)
        } else {
            [0, 0, 0, 0]
        }
    });
    let palette: Vec<Rgb> = main_colors(&sample(&grid, 40_000), n)
        .into_iter()
        .map(|(c, _)| chart.threads.get(chart.nearest(c)).map_or(c, |t| t.color))
        .fold(Vec::new(), |mut v, c| {
            if !v.contains(&c) {
                v.push(c);
            }
            v
        });
    let (w, h) = (cols as usize, rows as usize);
    let mut cells = quantise(&grid, &palette);
    majority(&mut cells, w, h, 1);
    // the background: the colour covering most of the edges, left unsewn unless asked for
    if !background {
        let mut edge = vec![0usize; palette.len()];
        let mut total = 0usize;
        for y in 0..h {
            for x in 0..w {
                if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                    total += 1;
                    if let Some(Some(i)) = cells.get(y * w + x)
                        && let Some(e) = edge.get_mut(*i)
                    {
                        *e += 1;
                    }
                }
            }
        }
        if let Some((skip, _)) = edge.iter().enumerate().max_by_key(|(_, n)| **n).filter(|(_, n)| **n * 2 > total) {
            for c in cells.iter_mut() {
                if *c == Some(skip) {
                    *c = None;
                }
            }
        }
    }
    // the lightest colour sewn first
    let mut order: Vec<usize> = (0..palette.len()).collect();
    order.sort_by(|a, b| {
        let l = |i: &usize| palette.get(*i).map_or(0.0, |c| c.to_lab()[0]);
        l(b).total_cmp(&l(a))
    });
    let rank: Vec<usize> = (0..palette.len()).map(|i| order.iter().position(|o| *o == i).unwrap_or(0)).collect();
    let colors: Vec<(Option<NodeId>, Rgb, Option<Thread>)> = order
        .iter()
        .filter_map(|i| palette.get(*i))
        .map(|c| (None, *c, Some(chart.threads.iter().find(|t| t.color == *c).cloned().unwrap_or_else(|| Thread::new(*c, "")))))
        .collect();
    let cells: Vec<Option<usize>> = cells.into_iter().map(|c| c.and_then(|i| rank.get(i).copied())).collect();
    let dirty = vec![true; colors.len()];
    let mut pattern = Pattern { group: NodeId(0), cols, rows, style, origin, cell, colors, cells, dirty };
    if pattern.cells.iter().all(Option::is_none) {
        return Err(bad(CMD, "the image is all background: try `background: true`"));
    }
    let used = pattern.counts().iter().filter(|n| **n > 0).count();
    let source = pic.id;
    let id = s.edit("Cross Stitch from Image", |d, sel| {
        let (parent, idx, _) = d.position(source).ok_or(EngineError::NoNode(source))?;
        new_group(d, sel, &mut pattern, "Cross Stitch", (parent, idx + 1))
    })?;
    Ok(json!({ "id": id.0, "cols": cols, "rows": rows, "cell": cell * MM_PER_PT, "colors": used }))
}

fn set_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.crossStitch.set";
    let group = chosen(s, p).ok_or_else(|| bad(CMD, "select a cross-stitch pattern"))?;
    let mut pattern = Pattern::read(&s.doc()?.doc, group).ok_or_else(|| bad(CMD, "that cross-stitch pattern can't be read"))?;
    if let Some(c) = cell_mm(p, CMD)? {
        pattern.cell = c * PT_PER_MM;
    }
    pattern.style = parse_style(p, CMD, pattern.style)?;
    s.edit("Cross-Stitch Pattern Settings", |d, _| pattern.write(d, true))?;
    Ok(json!({ "id": group.0, "cell": pattern.cell_mm(), "style": style_name(pattern.style) }))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.crossStitch.info";
    let group = chosen(s, p).ok_or_else(|| bad(CMD, "select a cross-stitch pattern"))?;
    let pattern = Pattern::read(&s.doc()?.doc, group).ok_or_else(|| bad(CMD, "that cross-stitch pattern can't be read"))?;
    let counts = pattern.counts();
    let colors: Vec<Value> = pattern
        .colors
        .iter()
        .zip(&counts)
        .filter(|(_, n)| **n > 0)
        .map(|((_, c, t), n)| json!({ "color": c.to_hex(), "name": t.as_ref().map_or("", |t| t.name.as_str()), "cells": n }))
        .collect();
    let cell = pattern.cell_mm();
    Ok(json!({
        "id": group.0,
        "cols": pattern.cols,
        "rows": pattern.rows,
        "cell": cell,
        "style": style_name(pattern.style),
        "size": [f64::from(pattern.cols) * cell, f64::from(pattern.rows) * cell],
        "colors": colors,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::pattern::Cmd;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
        s
    }

    /// The pattern's stitches (mm), needle points only.
    fn needle_points(s: &Session) -> Vec<sf_core::Point> {
        let plan = s.embroidery_plan().unwrap();
        plan.pattern().stitches.iter().filter(|r| r.cmd == Cmd::Stitch).map(|r| r.pos()).collect()
    }

    /// CH-03: an empty grid, cells painted in two colours, the X stitches on the grid's corners,
    /// a cell repainted changes colour, erased cells go, and Undo takes a stroke back.
    #[test]
    fn cells_painted_on_the_grid_sew_as_crosses_in_their_cells() {
        let mut s = session();
        // 10 × 6 cells of 3 mm at (72, 72) points (one inch in)
        let r = s.execute("embroidery.crossStitch.new", &json!({"cols": 10, "rows": 6, "cell": 3, "at": [72, 72]})).unwrap();
        let id = r["id"].as_u64().unwrap();
        let info = s.execute("embroidery.crossStitch.info", &json!({})).unwrap();
        assert_eq!((info["cols"].as_u64(), info["rows"].as_u64(), info["colors"].as_array().map(Vec::len)), (Some(10), Some(6), Some(0)));
        assert!((info["cell"].as_f64().unwrap() - 3.0).abs() < 1e-9);
        assert!(s.embroidery_plan().is_none_or(|p| p.pattern().stitch_count() == 0), "an empty grid sews nothing");
        // a row of four red cells by listed cells, then a blue stroke across row 3
        s.execute("embroidery.crossStitch.paint", &json!({"cells": [[0, 0], [1, 0], [2, 0], [3, 0]], "color": "#cc0000"})).unwrap();
        let cell = 3.0 * PT_PER_MM;
        let y = 72.0 + 3.5 * cell;
        let r = s
            .execute("embroidery.crossStitch.paint", &json!({"id": id, "at": [[72.0 + 0.5 * cell, y], [72.0 + 5.5 * cell, y]], "color": "#0000cc"}))
            .unwrap();
        assert_eq!(r["cells"], 6, "the stroke's cells, every one between its ends");
        let info = s.execute("embroidery.crossStitch.info", &json!({"id": id})).unwrap();
        let counts: Vec<(String, u64)> =
            info["colors"].as_array().unwrap().iter().map(|c| (c["color"].as_str().unwrap().to_string(), c["cells"].as_u64().unwrap())).collect();
        assert_eq!(counts, vec![("#cc0000".to_string(), 4), ("#0000cc".to_string(), 6)]);
        // every painted cell's four corners are needle points, and nearly every needle point is
        // on a grid line (corners, or travel along a line; the ties aren't)
        let origin = 72.0 * MM_PER_PT;
        let on = |v: f64| ((v - origin) / 3.0 - ((v - origin) / 3.0).round()).abs() < 1e-6;
        let pts = needle_points(&s);
        let cells = [(0, 0), (1, 0), (2, 0), (3, 0), (0, 3), (1, 3), (2, 3), (3, 3), (4, 3), (5, 3)];
        for (c, r) in cells {
            for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let corner = sf_core::Point::new(origin + f64::from(c + dc) * 3.0, origin + f64::from(r + dr) * 3.0);
                assert!(pts.iter().any(|p| p.distance(corner) < 1e-6), "cell ({c}, {r}) has no stitch at {corner:?}");
            }
        }
        let off = pts.iter().filter(|p| !(on(p.x) || on(p.y))).count();
        assert!(off * 10 < pts.len(), "{off} of {} needle points off the grid", pts.len());
        // two threads, red sewn first
        let plan = s.embroidery_plan().unwrap();
        let threads: Vec<String> = plan.pattern().threads.iter().map(|t| t.color.to_hex()).collect();
        assert_eq!(threads, vec!["#cc0000", "#0000cc"]);
        // repainting a red cell blue moves it; erasing takes cells away
        s.execute("embroidery.crossStitch.paint", &json!({"cells": [[0, 0]], "color": "#0000cc"})).unwrap();
        s.execute("embroidery.crossStitch.paint", &json!({"cells": [[1, 0], [2, 0], [3, 0]], "erase": true})).unwrap();
        let info = s.execute("embroidery.crossStitch.info", &json!({})).unwrap();
        assert_eq!(info["colors"].as_array().unwrap().len(), 1, "red has no cells left: {info}");
        assert_eq!(info["colors"][0]["cells"], 7);
        s.execute("edit.undo", &json!({})).unwrap();
        let info = s.execute("embroidery.crossStitch.info", &json!({})).unwrap();
        assert_eq!(info["colors"].as_array().unwrap().len(), 2, "Undo brings the erased cells back");
    }

    /// Moving a pattern keeps its cells, the grid moves with it and new cells land on it; a bigger
    /// brush paints a square; the pattern survives a save (the document as JSON) unchanged.
    #[test]
    fn a_moved_pattern_keeps_its_cells_and_survives_saving() {
        let mut s = session();
        let id = s.execute("embroidery.crossStitch.new", &json!({"cols": 20, "rows": 20, "cell": 2.5})).unwrap()["id"].as_u64().unwrap();
        s.execute("embroidery.crossStitch.paint", &json!({"cells": [[10, 10]], "color": "#228822", "size": 3})).unwrap();
        assert_eq!(s.execute("embroidery.crossStitch.info", &json!({})).unwrap()["colors"][0]["cells"], 9, "a 3 × 3 brush");
        let before = needle_points(&s);
        s.execute("select.set", &json!({"ids": [id]})).unwrap();
        s.execute("object.move", &json!({"dx": 36.0, "dy": 18.0})).unwrap();
        let after = needle_points(&s);
        assert_eq!(before.len(), after.len());
        let (dx, dy) = (36.0 * MM_PER_PT, 18.0 * MM_PER_PT);
        for (a, b) in before.iter().zip(&after) {
            assert!((b.x - a.x - dx).abs() < 1e-6 && (b.y - a.y - dy).abs() < 1e-6, "{a:?} → {b:?}");
        }
        // painting by point after the move finds the moved grid (no id, nothing selected)
        s.execute("select.set", &json!({"ids": []})).unwrap();
        let grid = Pattern::read(&s.doc().unwrap().doc, NodeId(id)).unwrap();
        let at = grid.origin + kurbo::Vec2::new(0.5 * grid.cell, 0.5 * grid.cell);
        s.execute("embroidery.crossStitch.paint", &json!({"at": [[at.x, at.y]], "color": "#228822"})).unwrap();
        let info = s.execute("embroidery.crossStitch.info", &json!({"id": id})).unwrap();
        assert_eq!(info["colors"][0]["cells"], 10);
        // the cells are kept with the document
        let doc = s.doc().unwrap().doc.clone();
        let text = serde_json::to_string(&*doc).unwrap();
        let back: Document = serde_json::from_str(&text).unwrap();
        let p = Pattern::read(&back, NodeId(id)).unwrap();
        assert_eq!(p.counts(), vec![10]);
        // half crosses and a larger cell: the grid grows from its corner
        let r = s.execute("embroidery.crossStitch.set", &json!({"id": id, "cell": 4, "style": "half"})).unwrap();
        assert_eq!(r["style"], "half");
        let info = s.execute("embroidery.crossStitch.info", &json!({"id": id})).unwrap();
        assert!((info["size"][0].as_f64().unwrap() - 80.0).abs() < 1e-6, "{info}");
        assert_eq!(info["colors"][0]["cells"], 10);
    }

    /// A picture becomes a pattern of its main colours, the background left unsewn.
    #[test]
    fn an_image_becomes_a_pattern_of_its_colours() {
        let mut s = session();
        // a red disc and a blue bar on white
        let img = vectorcraft_trace::Raster::from_fn(120, 80, |x, y| {
            let (dx, dy) = (x as f64 - 40.0, y as f64 - 40.0);
            if dx * dx + dy * dy <= 25.0 * 25.0 {
                [200, 30, 30, 255]
            } else if (75..115).contains(&x) && (30..50).contains(&y) {
                [30, 30, 200, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        s.execute("file.place", &json!({"name": "art.png", "dataBase64": vectorcraft_format::base64_encode(&img.encode_png())})).unwrap();
        let r = s.execute("embroidery.crossStitch.fromImage", &json!({"cols": 30, "colors": 3})).unwrap();
        assert_eq!(r["cols"], 30, "{r}");
        assert_eq!(r["colors"], 2, "red and blue; white is the background: {r}");
        let info = s.execute("embroidery.crossStitch.info", &json!({"id": r["id"]})).unwrap();
        let total: u64 = info["colors"].as_array().unwrap().iter().map(|c| c["cells"].as_u64().unwrap()).sum();
        assert!(total > 50 && total < 30 * 20, "{info}");
        assert!(!needle_points(&s).is_empty());
        // the background sewn too when asked
        s.execute("edit.undo", &json!({})).unwrap();
        let r = s.execute("embroidery.crossStitch.fromImage", &json!({"cols": 30, "colors": 3, "background": true})).unwrap();
        assert_eq!(r["colors"], 3, "{r}");
    }

    /// The Cross Stitch tool: a drag paints a row of squares as one Undo step, Alt-drag erases.
    #[test]
    fn the_cross_stitch_tool_paints_and_erases_by_dragging() {
        use vectorcraft_tools::{Mods, PointerEvent, PointerKind};
        let mut s = session();
        let id = s.execute("embroidery.crossStitch.new", &json!({"cols": 12, "rows": 4, "cell": 4, "at": [100, 100]})).unwrap()["id"].clone();
        let v = crate::ViewInfo::default();
        s.select_tool("embroideryCrossStitch", v).unwrap();
        s.set_tool_options(Some("embroideryCrossStitch"), json!({"color": "#3366aa"}).as_object().unwrap());
        let cell = 4.0 * PT_PER_MM;
        let y = 100.0 + 1.5 * cell;
        let at = |c: f64| 100.0 + (c + 0.5) * cell;
        let ev = |kind, x: f64, alt: bool| PointerEvent::new(kind, x, y).with_mods(Mods { alt, ..Mods::default() });
        s.pointer(&ev(PointerKind::Down, at(1.0), false), v).unwrap();
        s.pointer(&ev(PointerKind::Drag, at(4.0), false), v).unwrap();
        s.pointer(&ev(PointerKind::Up, at(8.0), false), v).unwrap();
        let cells = |s: &mut Session| s.execute("embroidery.crossStitch.info", &json!({"id": id})).unwrap()["colors"][0]["cells"].as_u64();
        assert_eq!(cells(&mut s), Some(8), "squares 1 to 8 of row 1");
        // Alt-drag erases three
        s.pointer(&ev(PointerKind::Down, at(1.0), true), v).unwrap();
        s.pointer(&ev(PointerKind::Up, at(3.0), true), v).unwrap();
        assert_eq!(cells(&mut s), Some(5));
        // one stroke, one Undo step
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(cells(&mut s), Some(8));
    }

    #[test]
    fn bad_params_are_refused() {
        let mut s = session();
        for p in [json!({"cols": 0}), json!({"rows": 301}), json!({"cell": 0.2}), json!({"style": "zigzag"}), json!({"at": "here"})] {
            assert!(s.execute("embroidery.crossStitch.new", &p).is_err(), "{p}");
        }
        // painting needs a pattern, cells and a colour
        assert!(s.execute("embroidery.crossStitch.paint", &json!({"cells": [[0, 0]], "color": "#ff0000"})).is_err());
        s.execute("embroidery.crossStitch.new", &json!({"cols": 5, "rows": 5})).unwrap();
        for p in [
            json!({"cells": [[0, 0]]}),
            json!({"cells": [[0, 0]], "color": "red"}),
            json!({"color": "#ff0000"}),
            json!({"cells": "all", "color": "#ff0000"}),
            json!({"cells": [[0, 0]], "color": "#ff0000", "size": 12}),
            json!({"at": [[f64::NAN, 0.0]], "color": "#ff0000"}),
        ] {
            assert!(s.execute("embroidery.crossStitch.paint", &p).is_err(), "{p}");
        }
        // cells off the grid are left alone
        let r = s.execute("embroidery.crossStitch.paint", &json!({"cells": [[-1, 0], [5, 5], [99, 2]], "color": "#ff0000"})).unwrap();
        assert_eq!(r["cells"], 0);
        assert!(s.execute("embroidery.crossStitch.fromImage", &json!({})).is_err(), "no image");
    }
}
