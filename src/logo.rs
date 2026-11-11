// The app's mark: monosodium glutamate as a ball-and-stick model, turning slowly. Drawn with egui's own painter
// (circles and lines), so a still logo costs nothing and a turning one is a few shapes a frame.
// img/logo.py draws the same model into the icon files; keep ATOMS, BONDS and BEST the same in both.
use super::theme::{self, Tokens};
use eframe::egui::{self, Color32, Rect, Stroke, Vec2};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    C,
    O,
    N,
    Na,
}

/// The heavy atoms with the bond length as the unit, centred. The hydrogens are left out so that 16 px still reads.
const ATOMS: [([f32; 3], Kind); 11] = [
    ([-1.288, -0.181, 0.062], Kind::C),  // Cδ, the carboxylate's carbon
    ([-0.472, 0.397, 0.062], Kind::C),   // Cγ
    ([0.344, -0.181, 0.062], Kind::C),   // Cβ
    ([1.160, 0.397, 0.062], Kind::C),    // Cα
    ([1.976, -0.181, 0.062], Kind::C),   // C, the acid's carbon
    ([-1.286, -0.945, 0.500], Kind::O),  // O⁻
    ([-2.008, 0.074, -0.375], Kind::O),  // =O
    ([1.160, 0.945, 0.838], Kind::N),    // NH₂
    ([2.588, -0.079, -0.562], Kind::O),  // =O
    ([2.083, -0.792, 0.687], Kind::O),   // OH
    ([-2.588, 0.719, -0.838], Kind::Na), // Na⁺, next to the carboxylate, no bond
];
const BONDS: [(usize, usize); 9] = [(0, 1), (1, 2), (2, 3), (3, 4), (0, 5), (0, 6), (3, 7), (4, 8), (4, 9)];
/// The farthest atom's centre from the middle, and the model's half height over a full turn (tipped, with the balls).
const REACH: f32 = 2.82;
const HALF_HEIGHT: f32 = 2.45;
/// The pose of the still logo and of the icons.
pub const BEST: f32 = 0.6;
/// The model is tipped towards the viewer, so that the turn shows depth.
const TILT: f32 = 0.45;

/// The angle to draw now. With `spin` (the setting) on, animations on and the window active, the logo turns 20 frames
/// a second while `moving` (the rail: while the pointer is over it, as each turn repaints the whole window) and
/// stays where it was otherwise; with `spin` off it stands at `BEST`. Any number of callers a frame is fine.
pub fn angle(ctx: &egui::Context, spin: bool, moving: bool) -> f32 {
    if !spin || theme::fade() == 0.0 {
        return BEST;
    }
    let (focused, time) = ctx.input(|i| (i.viewport().focused.unwrap_or(true), i.time));
    let id = egui::Id::new("logo angle");
    let (last, mut angle) = ctx.data(|d| d.get_temp::<(f64, f32)>(id)).unwrap_or((time, BEST));
    if moving && focused {
        // A long gap (the window was inactive) doesn't jump.
        angle += (time - last).clamp(0.0, 0.1) as f32 * 0.5;
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    ctx.data_mut(|d| d.insert_temp(id, (time, angle)));
    angle
}

/// Paints the model turned by `angle` into `rect`, the atoms in the theme's colours.
pub fn paint(painter: &egui::Painter, rect: Rect, angle: f32, t: &Tokens) {
    let (sa, ca) = angle.sin_cos();
    let (st, ct) = TILT.sin_cos();
    let unit = (rect.width() / (2.0 * (REACH * 1.12 + 0.55))).min(rect.height() / (2.0 * HALF_HEIGHT));
    let centre = rect.center();
    // Turn about y, tip about x, then a little perspective: z grows towards the viewer.
    let place = |p: [f32; 3]| {
        let (x, y, z) = (p[0] * ca + p[2] * sa, p[1], p[2] * ca - p[0] * sa);
        let (y, z) = (y * ct - z * st, y * st + z * ct);
        let s = 1.0 / (1.0 - z * 0.08);
        (centre + Vec2::new(x, -y) * unit * s, z, s)
    };
    let atoms: Vec<_> = ATOMS.iter().map(|(p, k)| (place(*p), *k)).collect();
    // Everything far to near. A bond is two halves, each just behind its own atom, so that no stick crosses a ball.
    let mut order: Vec<(f32, usize, Option<usize>)> = atoms.iter().enumerate().map(|(n, ((_, z, _), _))| (*z, n, None)).collect();
    order.extend(BONDS.iter().flat_map(|&(a, b)| [(atoms[a].0 .1 - 0.001, a, Some(b)), (atoms[b].0 .1 - 0.001, b, Some(a))]));
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let bond = t.muted.lerp_to_gamma(t.base, 0.4);
    for (z, n, other) in order {
        let depth = ((z + REACH) / (2.0 * REACH)).clamp(0.0, 1.0);
        match other {
            Some(m) => {
                let ((pa, _, s), _) = atoms[n];
                let ((pb, _, _), _) = atoms[m];
                painter.line_segment([pa, pa.lerp(pb, 0.5)], Stroke::new(unit * 0.22 * s, bond.lerp_to_gamma(t.base, (1.0 - depth) * 0.4)));
            }
            None => {
                let ((p, _, s), kind) = atoms[n];
                let (colour, size) = match kind {
                    Kind::C => (t.text2, 0.4),
                    Kind::O => (t.accent, 0.37),
                    Kind::N => (t.lavender, 0.37),
                    Kind::Na => (t.apricot, 0.46),
                };
                let r = unit * size * s;
                let colour = colour.lerp_to_gamma(t.base, (1.0 - depth) * 0.3);
                painter.circle(p, r, colour, Stroke::new((unit * 0.05).max(0.5), t.base.gamma_multiply(0.5)));
                if r >= 3.0 {
                    painter.circle_filled(p + Vec2::new(-r * 0.32, -r * 0.32), r * 0.3, Color32::WHITE.gamma_multiply(0.3));
                }
            }
        }
    }
}
