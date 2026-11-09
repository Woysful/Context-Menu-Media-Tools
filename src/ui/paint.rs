//! Shapes egui has no call for, the parts of the cyber stickers of Y2K: frames with cut corners,
//! four-pointed stars, wires with round ends, rows of squares, barcodes.

use eframe::egui::{epaint::Mesh, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

/// Which corners are cut: top left, top right, bottom right, bottom left
pub type Corners = [bool; 4];
/// The top left and the bottom right corner: the cut of buttons and cards
pub const DIAGONAL: Corners = [true, false, true, false];

fn cut_outline(rect: Rect, cut: f32, corners: Corners) -> Vec<Pos2> {
    let c = cut.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts = Vec::with_capacity(8);
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    if corners[0] {
        pts.extend([pos2(l, t + c), pos2(l + c, t)]);
    } else {
        pts.push(pos2(l, t));
    }
    if corners[1] {
        pts.extend([pos2(r - c, t), pos2(r, t + c)]);
    } else {
        pts.push(pos2(r, t));
    }
    if corners[2] {
        pts.extend([pos2(r, b - c), pos2(r - c, b)]);
    } else {
        pts.push(pos2(r, b));
    }
    if corners[3] {
        pts.extend([pos2(l + c, b), pos2(l, b - c)]);
    } else {
        pts.push(pos2(l, b));
    }
    pts
}

/// A rectangle with cut corners, filled and framed
pub fn cut_shape(rect: Rect, cut: f32, corners: Corners, fill: Color32, stroke: Stroke) -> Shape {
    let pts = cut_outline(rect, cut, corners);
    let mut shapes = Vec::with_capacity(2);
    if fill != Color32::TRANSPARENT {
        shapes.push(Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
    }
    if stroke.width > 0.0 {
        shapes.push(Shape::closed_line(pts, stroke));
    }
    Shape::Vec(shapes)
}

pub fn cut_rect(p: &Painter, rect: Rect, cut: f32, corners: Corners, fill: Color32, stroke: Stroke) {
    p.add(cut_shape(rect, cut, corners, fill, stroke));
}

/// A card whose top edge is higher at the left, over its label: the tab is `tab` wide and
/// `step` higher than the rest of the edge; the top left and the bottom right corners are cut
pub fn tab_shape(rect: Rect, tab: f32, step: f32, cut: f32, fill: Color32, stroke: Stroke) -> Shape {
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let c = cut.min(step + 2.0).min(tab / 2.0);
    let tab = tab.min(rect.width() - step - c);
    let pts =
        vec![pos2(l, t + c), pos2(l + c, t), pos2(l + tab, t), pos2(l + tab + step, t + step), pos2(r, t + step), pos2(r, b - c), pos2(r - c, b), pos2(l, b)];
    let mut shapes = Vec::with_capacity(2);
    if fill != Color32::TRANSPARENT {
        // not convex, but every corner can be seen from a point under the tab: a fan of triangles
        let mut mesh = Mesh::default();
        mesh.colored_vertex(pos2(l + tab / 2.0, (t + step + b) / 2.0), fill);
        for p in &pts {
            mesh.colored_vertex(*p, fill);
        }
        let n = pts.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        shapes.push(Shape::mesh(mesh));
    }
    if stroke.width > 0.0 {
        shapes.push(Shape::closed_line(pts, stroke));
    }
    Shape::Vec(shapes)
}

/// A four-pointed star with thin arms, like the sparkles of the stickers
pub fn star(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, color);
    for i in 0..8 {
        let a = -FRAC_PI_2 + FRAC_PI_4 * i as f32;
        let d = if i % 2 == 0 { size } else { size * 0.16 };
        mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * d, color);
    }
    for i in 0..8u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 8);
    }
    p.add(Shape::mesh(mesh));
}

/// A round end of a wire: a ring on the background
pub fn node(p: &Painter, c: Pos2, r: f32, color: Color32, fill: Color32) {
    p.circle(c, r, fill, Stroke::new(1.2_f32, color));
}

/// A line with a ring at each end
pub fn wire(p: &Painter, a: Pos2, b: Pos2, color: Color32, bg: Color32) {
    p.line_segment([a, b], Stroke::new(1.0_f32, color));
    node(p, a, 3.0, color, bg);
    node(p, b, 3.0, color, bg);
}

/// A row of small squares across `rect`, the first part of them filled (a loading bar)
/// Of the `n` cells of a bar with `lit` done: whether cell `i` is in the part that moves
/// over the rest of the bar while a step tells nothing of its progress (`phase`: 0..1, again
/// and again)
pub fn swept(i: usize, n: usize, lit: usize, phase: f32) -> bool {
    let span = n.saturating_sub(lit);
    if span == 0 || i < lit {
        return false;
    }
    let w = (span / 5).clamp(2, 6);
    let head = (phase.clamp(0.0, 1.0) * (span + w) as f32) as usize;
    let j = i - lit;
    j < head && j + w >= head
}

/// `sweep`: the phase of the moving part of a busy bar (see `swept`)
pub fn squares(p: &Painter, rect: Rect, done: f32, lit: Color32, empty: Color32, sweep: Option<f32>) {
    let side = rect.height();
    let gap = (side * 0.4).max(2.0);
    let n = ((rect.width() + gap) / (side + gap)).floor().max(1.0) as usize;
    let gap = if n > 1 { (rect.width() - n as f32 * side) / (n - 1) as f32 } else { 0.0 };
    let on = (done.clamp(0.0, 1.0) * n as f32).round() as usize;
    for i in 0..n {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * (side + gap), rect.top()), vec2(side, side));
        if i < on {
            p.rect_filled(r, 0.0, lit);
        } else if sweep.is_some_and(|phase| swept(i, n, on, phase)) {
            p.rect_filled(r, 0.0, lit.gamma_multiply(0.55));
        } else {
            p.rect_stroke(r.shrink(0.5), 0.0, Stroke::new(1.0_f32, empty));
        }
    }
}

/// The bars of a barcode, always the same for the same `seed`
pub fn barcode(p: &Painter, rect: Rect, color: Color32, seed: u64) {
    let mut x = rect.left();
    let mut s = seed | 1;
    while x < rect.right() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let w = 1.0 + (s % 3) as f32;
        let gap = 1.0 + ((s >> 8) % 3) as f32;
        let bar = Rect::from_min_max(pos2(x, rect.top()), pos2((x + w).min(rect.right()), rect.bottom()));
        p.rect_filled(bar, 0.0, color);
        x += w + gap;
    }
}
