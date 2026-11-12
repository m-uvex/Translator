//! Symbole als Vektorgrafik.
//!
//! Die Electron-Fassung definiert ihre Symbole als SVG mit `viewBox="0 0 24 24"`,
//! `fill="none"`, `stroke-width="1.8"` und runden Enden. Hier werden dieselben
//! Pfade mit `egui::Painter` gezeichnet, damit die Oberfläche unabhängig von
//! installierten Symbolschriften aussieht. Die Daten stehen in `icons_data.rs`
//! und werden mit `.tools/gen_icons.py` aus den JavaScript-Dateien erzeugt.

use egui::{Color32, Painter, Pos2, Rect, Rounding, Shape as EguiShape, Stroke, Vec2};

use crate::icons_data::ICONS;

pub enum Shape {
    Path(&'static str),
    Circle(f32, f32, f32),
    Rect(f32, f32, f32, f32, f32),
}

pub fn icon(name: &str) -> Option<&'static [Shape]> {
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Zeichnet ein Symbol mittig in `rect` (quadratisch beschnitten).
pub fn paint(painter: &Painter, rect: Rect, name: &str, color: Color32) {
    let Some(shapes) = icon(name) else { return };
    let side = rect.width().min(rect.height());
    if side <= 0.0 {
        return;
    }
    let scale = side / 24.0;
    let origin = rect.center() - Vec2::splat(side * 0.5);
    let stroke = Stroke::new((1.8 * scale).max(1.0), color);
    let map = |p: [f32; 2]| origin + Vec2::new(p[0] * scale, p[1] * scale);

    for shape in shapes {
        match shape {
            Shape::Path(d) => {
                for poly in flatten(d) {
                    if poly.len() < 2 {
                        continue;
                    }
                    let pts: Vec<Pos2> = poly.iter().map(|p| map(*p)).collect();
                    painter.add(EguiShape::line(pts, stroke));
                }
            }
            Shape::Circle(cx, cy, r) => {
                painter.circle_stroke(map([*cx, *cy]), r * scale, stroke);
            }
            Shape::Rect(x, y, w, h, rx) => {
                let r = Rect::from_min_size(map([*x, *y]), Vec2::new(w * scale, h * scale));
                painter.rect_stroke(r, Rounding::same(rx * scale), stroke);
            }
        }
    }
}

enum Tok {
    Cmd(char),
    Num(f32),
}

fn lex(d: &str) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::new();
    let mut num = String::new();

    fn flush(num: &mut String, out: &mut Vec<Tok>) {
        if num.is_empty() {
            return;
        }
        // Ein einzelnes Minus ist kein Zahlwert und wird verworfen.
        if let Ok(v) = num.parse::<f32>() {
            out.push(Tok::Num(v));
        }
        num.clear();
    }

    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            flush(&mut num, &mut out);
            out.push(Tok::Cmd(c));
        } else if c == ',' || c.is_whitespace() {
            flush(&mut num, &mut out);
        } else if (c == '-' || c == '+') && !num.is_empty() {
            flush(&mut num, &mut out);
            num.push(c);
        } else {
            num.push(c);
        }
    }
    flush(&mut num, &mut out);
    out
}

/// Zerlegt einen SVG-Pfad in Polylinien (Koordinaten im 24er-Raster).
fn flatten(d: &str) -> Vec<Vec<[f32; 2]>> {
    let toks = lex(d);
    let mut segs: Vec<(char, Vec<f32>)> = Vec::new();
    for t in toks {
        match t {
            Tok::Cmd(c) => segs.push((c, Vec::new())),
            Tok::Num(v) => {
                if let Some(last) = segs.last_mut() {
                    last.1.push(v);
                }
            }
        }
    }

    let mut polys: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    let mut sx = 0.0_f32;
    let mut sy = 0.0_f32;
    let mut ctrl: Option<[f32; 2]> = None;

    for (cmd, args) in segs {
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        let step = match up {
            'M' | 'L' | 'T' => 2,
            'H' | 'V' => 1,
            'C' => 6,
            'S' | 'Q' => 4,
            'A' => 7,
            'Z' => 0,
            _ => 0,
        };
        if up == 'Z' {
            if cur.len() > 1 {
                cur.push([sx, sy]);
                polys.push(std::mem::take(&mut cur));
            }
            x = sx;
            y = sy;
            ctrl = None;
            continue;
        }
        if step == 0 {
            continue;
        }
        let mut k = 0usize;
        let mut first = true;
        while k + step <= args.len() {
            let a = &args[k..k + step];
            let px = x;
            let py = y;
            match up {
                'M' | 'L' => {
                    let mut nx = a[0];
                    let mut ny = a[1];
                    if rel {
                        nx += x;
                        ny += y;
                    }
                    if up == 'M' && first {
                        if cur.len() > 1 {
                            polys.push(std::mem::take(&mut cur));
                        } else {
                            cur.clear();
                        }
                        cur.push([nx, ny]);
                        sx = nx;
                        sy = ny;
                    } else {
                        if cur.is_empty() {
                            cur.push([x, y]);
                        }
                        cur.push([nx, ny]);
                    }
                    if up == 'M' {
                        // Weitere Paare nach M sind Linien.
                        sx = if first { nx } else { sx };
                        sy = if first { ny } else { sy };
                    }
                    x = nx;
                    y = ny;
                    ctrl = None;
                }
                'H' => {
                    let mut nx = a[0];
                    if rel {
                        nx += x;
                    }
                    if cur.is_empty() {
                        cur.push([x, y]);
                    }
                    cur.push([nx, y]);
                    x = nx;
                    ctrl = None;
                }
                'V' => {
                    let mut ny = a[0];
                    if rel {
                        ny += y;
                    }
                    if cur.is_empty() {
                        cur.push([x, y]);
                    }
                    cur.push([x, ny]);
                    y = ny;
                    ctrl = None;
                }
                'C' | 'S' | 'Q' | 'T' => {
                    let (c1, c2, end) = if up == 'C' {
                        let mut c1 = [a[0], a[1]];
                        let mut c2 = [a[2], a[3]];
                        let mut end = [a[4], a[5]];
                        if rel {
                            c1 = [c1[0] + x, c1[1] + y];
                            c2 = [c2[0] + x, c2[1] + y];
                            end = [end[0] + x, end[1] + y];
                        }
                        (c1, c2, end)
                    } else if up == 'S' {
                        let c1 = match ctrl {
                            Some(c) => [2.0 * x - c[0], 2.0 * y - c[1]],
                            None => [x, y],
                        };
                        let mut c2 = [a[0], a[1]];
                        let mut end = [a[2], a[3]];
                        if rel {
                            c2 = [c2[0] + x, c2[1] + y];
                            end = [end[0] + x, end[1] + y];
                        }
                        (c1, c2, end)
                    } else if up == 'Q' {
                        let mut c = [a[0], a[1]];
                        let mut end = [a[2], a[3]];
                        if rel {
                            c = [c[0] + x, c[1] + y];
                            end = [end[0] + x, end[1] + y];
                        }
                        (c, c, end)
                    } else {
                        let c = match ctrl {
                            Some(c) => [2.0 * x - c[0], 2.0 * y - c[1]],
                            None => [x, y],
                        };
                        let mut end = [a[0], a[1]];
                        if rel {
                            end = [end[0] + x, end[1] + y];
                        }
                        (c, c, end)
                    };
                    if cur.is_empty() {
                        cur.push([x, y]);
                    }
                    let samples = if up == 'Q' || up == 'T' { 10 } else { 14 };
                    for i in 1..=samples {
                        let t = i as f32 / samples as f32;
                        let mt = 1.0 - t;
                        let p = if up == 'Q' || up == 'T' {
                            [
                                mt * mt * px + 2.0 * mt * t * c1[0] + t * t * end[0],
                                mt * mt * py + 2.0 * mt * t * c1[1] + t * t * end[1],
                            ]
                        } else {
                            [
                                mt * mt * mt * px
                                    + 3.0 * mt * mt * t * c1[0]
                                    + 3.0 * mt * t * t * c2[0]
                                    + t * t * t * end[0],
                                mt * mt * mt * py
                                    + 3.0 * mt * mt * t * c1[1]
                                    + 3.0 * mt * t * t * c2[1]
                                    + t * t * t * end[1],
                            ]
                        };
                        cur.push(p);
                    }
                    ctrl = Some(if up == 'Q' || up == 'T' { c1 } else { c2 });
                    x = end[0];
                    y = end[1];
                }
                'A' => {
                    let (rx, ry) = (a[0].abs(), a[1].abs());
                    let rot = a[2];
                    let large = a[3] != 0.0;
                    let sweep = a[4] != 0.0;
                    let mut end = [a[5], a[6]];
                    if rel {
                        end = [end[0] + x, end[1] + y];
                    }
                    if cur.is_empty() {
                        cur.push([x, y]);
                    }
                    for p in arc_points([x, y], rx, ry, rot, large, sweep, end) {
                        cur.push(p);
                    }
                    x = end[0];
                    y = end[1];
                    ctrl = None;
                }
                _ => {}
            }
            first = false;
            k += step;
        }
    }
    if cur.len() > 1 {
        polys.push(cur);
    }
    polys
}

/// Wandelt einen SVG-Bogen in Punkte um (Endpunkt-Parametrisierung).
fn arc_points(
    start: [f32; 2],
    rx: f32,
    ry: f32,
    rot_deg: f32,
    large: bool,
    sweep: bool,
    end: [f32; 2],
) -> Vec<[f32; 2]> {
    let (mut rx, mut ry) = (rx, ry);
    if rx == 0.0 || ry == 0.0 {
        return vec![end];
    }
    let phi = rot_deg.to_radians();
    let (sin_p, cos_p) = phi.sin_cos();
    let dx = (start[0] - end[0]) / 2.0;
    let dy = (start[1] - end[1]) / 2.0;
    let x1 = cos_p * dx + sin_p * dy;
    let y1 = -sin_p * dx + cos_p * dy;

    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    let sign = if large == sweep { -1.0 } else { 1.0 };
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let coef = if den == 0.0 {
        0.0
    } else {
        sign * (num.max(0.0) / den).sqrt()
    };
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = cos_p * cx1 - sin_p * cy1 + (start[0] + end[0]) / 2.0;
    let cy = sin_p * cx1 + cos_p * cy1 + (start[1] + end[1]) / 2.0;

    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| -> f32 {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let theta1 = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }

    let steps = ((delta.abs() / std::f32::consts::FRAC_PI_4).ceil() as usize).max(4);
    let mut pts = Vec::with_capacity(steps);
    for i in 1..=steps {
        let t = theta1 + delta * (i as f32 / steps as f32);
        let (sin_t, cos_t) = t.sin_cos();
        pts.push([
            cx + rx * cos_t * cos_p - ry * sin_t * sin_p,
            cy + rx * cos_t * sin_p + ry * sin_t * cos_p,
        ]);
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfad_mit_negativen_zahlen_wird_zerlegt() {
        let polys = flatten("M9 14L4 9l5-5");
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0][0], [9.0, 14.0]);
        assert_eq!(polys[0][1], [4.0, 9.0]);
        assert_eq!(polys[0][2], [9.0, 4.0]);
    }

    #[test]
    fn bogen_endet_am_zielpunkt() {
        let polys = flatten("M4 9h10a6 6 0 0 1 0 12h-3");
        assert_eq!(polys.len(), 1);
        let last = polys[0].last().unwrap();
        assert_eq!(*last, [11.0, 21.0]);
    }

    #[test]
    fn alle_symbole_lassen_sich_zeichnen() {
        let mut fehler = Vec::new();
        for (name, shapes) in ICONS {
            for shape in *shapes {
                if let Shape::Path(d) = shape {
                    let polys = flatten(d);
                    if polys.is_empty() {
                        fehler.push(*name);
                    }
                }
            }
        }
        assert!(fehler.is_empty(), "leere Pfade bei: {:?}", fehler);
    }
}
