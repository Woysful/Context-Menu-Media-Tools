//! Shapes egui has no call for: gradients, gloss, glow, auras and sparkles (the Y2K style).

use eframe::egui::{self, epaint::Mesh, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Between two colours, alpha included
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    let (a, b) = (a.to_array(), b.to_array());
    Color32::from_rgba_premultiplied(f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3]))
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// The outline of a rounded rectangle, clockwise from the top left corner
fn outline(rect: Rect, radius: f32) -> Vec<Pos2> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    if r < 0.5 {
        return vec![rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    }
    let steps = ((r / 2.0).ceil() as usize).clamp(3, 12);
    let corners = [
        (pos2(rect.left() + r, rect.top() + r), PI),
        (pos2(rect.right() - r, rect.top() + r), PI + FRAC_PI_2),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
    ];
    let mut pts = Vec::with_capacity(4 * (steps + 1));
    for (c, start) in corners {
        for i in 0..=steps {
            let a = start + FRAC_PI_2 * i as f32 / steps as f32;
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// A rounded rectangle filled with a gradient: left to right (`across`) or top to bottom
pub fn gradient(p: &Painter, rect: Rect, radius: f32, from: Color32, to: Color32, across: bool) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let at = |q: Pos2| {
        let t = if across { (q.x - rect.left()) / rect.width() } else { (q.y - rect.top()) / rect.height() };
        lerp(from, to, t)
    };
    let mut mesh = Mesh::default();
    let c = rect.center();
    mesh.colored_vertex(c, at(c));
    let pts = outline(rect, radius);
    for q in &pts {
        mesh.colored_vertex(*q, at(*q));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

/// The shine on the upper half of a gel button
pub fn gloss(p: &Painter, rect: Rect, radius: f32, strength: f32) {
    let top = Rect::from_min_max(rect.min + vec2(3.0, 1.5), pos2(rect.right() - 3.0, rect.top() + rect.height() * 0.5));
    gradient(p, top, (radius - 2.0).max(0.0), Color32::from_white_alpha((90.0 * strength) as u8), Color32::from_white_alpha(0), false);
}

/// A soft light around a shape
pub fn glow(p: &Painter, rect: Rect, radius: f32, color: Color32, blur: f32) {
    p.add(egui::epaint::Shadow { offset: egui::Vec2::ZERO, blur, spread: 0.0, color }.as_shape(rect, radius));
}

/// A round spot of light that fades out
pub fn aura(p: &Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    let n = 48u32;
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        mesh.colored_vertex(center + vec2(a.cos(), a.sin()) * radius, Color32::TRANSPARENT);
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

/// A four-pointed star
pub fn sparkle(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let w = size * 0.2;
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -size), c + vec2(w, 0.0), c + vec2(0.0, size), c + vec2(-w, 0.0)], color, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![c + vec2(-size, 0.0), c + vec2(0.0, -w), c + vec2(size, 0.0), c + vec2(0.0, w)], color, Stroke::NONE));
}

/// A small diamond (the bullet of a row)
pub fn diamond(p: &Painter, c: Pos2, size: f32, color: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -size), c + vec2(size, 0.0), c + vec2(0.0, size), c + vec2(-size, 0.0)], color, Stroke::NONE));
}
