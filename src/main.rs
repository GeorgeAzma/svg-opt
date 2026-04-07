use svg::node::element::path::{Command, Data, Position};
use svg::parser::Event;

#[derive(Debug, Clone)]
enum Primitive {
    Move(f32, f32),
    Line(f32, f32),
    Quadratic(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Arc(f32, f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug)]
struct SvgReader {
    primitives: Vec<Primitive>,
}

const TOLERANCE: f32 = 1.0;
const C2Q_TOLERANCE: f32 = 1.0 * TOLERANCE;
const Q2L_TOLERANCE: f32 = 1.0 * TOLERANCE;
const MERGE_Q_TOLERANCE: f32 = 1.0 * TOLERANCE;
const MERGE_L_TOLERANCE: f32 = 1.0 * TOLERANCE;

impl SvgReader {
    pub fn new(path: &str) -> Self {
        let mut content = String::new();
        let mut x = 0.0;
        let mut y = 0.0;
        let mut prev_ctrl_x = 0.0;
        let mut prev_ctrl_y = 0.0;
        let mut primitives = Vec::new();

        for event in svg::open(path, &mut content).unwrap() {
            match event {
                Event::Error(_error) => {}
                Event::Tag(_path, _, attributes) => {
                    let Some(data) = attributes.get("d") else {
                        continue;
                    };
                    let data = Data::parse(data).unwrap();
                    for command in data.into_iter() {
                        let parameters = match command {
                            Command::Move(_, p)
                            | Command::Line(_, p)
                            | Command::HorizontalLine(_, p)
                            | Command::VerticalLine(_, p)
                            | Command::QuadraticCurve(_, p)
                            | Command::SmoothQuadraticCurve(_, p)
                            | Command::CubicCurve(_, p)
                            | Command::SmoothCubicCurve(_, p)
                            | Command::EllipticalArc(_, p) => p,
                            Command::Close => &vec![].into(),
                        };
                        let mut it = parameters.iter().peekable();
                        let read_next =
                            |it: &mut std::iter::Peekable<std::slice::Iter<'_, f32>>| -> f32 {
                                it.next().copied().unwrap_or_default()
                            };
                        while it.peek().is_some() {
                            let read_pos =
                                |it: &mut std::iter::Peekable<std::slice::Iter<'_, f32>>,
                                 position: &Position|
                                 -> (f32, f32) {
                                    let is_rel = matches!(position, Position::Relative);
                                    let mut px = read_next(it);
                                    let mut py = read_next(it);
                                    if is_rel {
                                        px += x;
                                        py += y;
                                    }
                                    (px, py)
                                };
                            match command {
                                Command::Move(position, _) => {
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Move(x, y));
                                }
                                Command::Line(position, _) => {
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Line(x, y));
                                }
                                Command::HorizontalLine(position, _) => {
                                    let is_rel = matches!(position, Position::Relative);
                                    let mut nx = read_next(&mut it);
                                    if is_rel {
                                        nx += x;
                                    }
                                    x = nx;
                                    primitives.push(Primitive::Line(x, y));
                                }
                                Command::VerticalLine(position, _) => {
                                    let is_rel = matches!(position, Position::Relative);
                                    let mut ny = read_next(&mut it);
                                    if is_rel {
                                        ny += y;
                                    }
                                    y = ny;
                                    primitives.push(Primitive::Line(x, y));
                                }
                                Command::QuadraticCurve(position, _) => {
                                    let (x1, y1) = read_pos(&mut it, position);
                                    (prev_ctrl_x, prev_ctrl_y) = (x1, y1);
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Quadratic(x1, y1, x, y));
                                }
                                Command::SmoothQuadraticCurve(position, _) => {
                                    let (x1, y1) = (2.0 * x - prev_ctrl_x, 2.0 * y - prev_ctrl_y);
                                    (prev_ctrl_x, prev_ctrl_y) = (x1, y1);
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Quadratic(x1, y1, x, y));
                                }
                                Command::CubicCurve(position, _) => {
                                    let (x1, y1) = read_pos(&mut it, position);
                                    let (x2, y2) = read_pos(&mut it, position);
                                    (prev_ctrl_x, prev_ctrl_y) = (x2, y2);
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Cubic(x1, y1, x2, y2, x, y));
                                }
                                Command::SmoothCubicCurve(position, _) => {
                                    let (x1, y1) = (2.0 * x - prev_ctrl_x, 2.0 * y - prev_ctrl_y);
                                    let (x2, y2) = read_pos(&mut it, position);
                                    (prev_ctrl_x, prev_ctrl_y) = (x2, y2);
                                    (x, y) = read_pos(&mut it, position);
                                    primitives.push(Primitive::Cubic(x1, y1, x2, y2, x, y));
                                }
                                Command::EllipticalArc(position, _) => {
                                    let rx = read_next(&mut it);
                                    let ry = read_next(&mut it);
                                    let xrot = read_next(&mut it);
                                    let large = read_next(&mut it);
                                    let sweep = read_next(&mut it);
                                    (x, y) = read_pos(&mut it, position);
                                    primitives
                                        .push(Primitive::Arc(rx, ry, xrot, large, sweep, x, y));
                                }
                                Command::Close => {
                                    primitives.push(Primitive::Close);
                                }
                            }
                        }
                    }
                }
                Event::Text(_) => {}
                Event::Comment(_) => {}
                Event::Declaration(_) => {}
                Event::Instruction(_) => {}
            }
        }

        Self { primitives }
    }

    fn move_origin(prim: &Primitive) -> (f32, f32) {
        match prim {
            Primitive::Move(x, y)
            | Primitive::Line(x, y)
            | Primitive::Quadratic(_, _, x, y)
            | Primitive::Cubic(_, _, _, _, x, y)
            | Primitive::Arc(_, _, _, _, _, x, y) => (*x, *y),
            Primitive::Close => (0.0, 0.0),
        }
    }

    fn cubic_arc_length(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> f32 {
        // 5-point Gauss-Legendre quadrature on [0, 1]
        const NODES: [f32; 5] = [
            0.046910077,
            0.230765346,
            0.500000000,
            0.769234654,
            0.953089923,
        ];
        const WEIGHTS: [f32; 5] = [
            0.118463443,
            0.239314335,
            0.284444444,
            0.239314335,
            0.118463443,
        ];

        let deriv_len = |t: f32| {
            let s = 1.0 - t;
            // Derivative of cubic bezier / 3
            let dx = s * s * (x1 - x0) + 2.0 * s * t * (x2 - x1) + t * t * (x3 - x2);
            let dy = s * s * (y1 - y0) + 2.0 * s * t * (y2 - y1) + t * t * (y3 - y2);
            // * 3 for the actual derivative, but it cancels in the ratio so keep it
            (dx * dx + dy * dy).sqrt()
        };

        // The * 3.0 for the true derivative magnitude
        3.0 * NODES
            .iter()
            .zip(WEIGHTS.iter())
            .map(|(&n, &w)| w * deriv_len(n))
            .sum::<f32>()
    }

    fn quadratic_arc_length(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        const NODES: [f32; 5] = [
            0.046910077,
            0.230765346,
            0.500000000,
            0.769234654,
            0.953089923,
        ];
        const WEIGHTS: [f32; 5] = [
            0.118463443,
            0.239314335,
            0.284444444,
            0.239314335,
            0.118463443,
        ];

        let deriv_len = |t: f32| {
            let s = 1.0 - t;
            // Derivative of quadratic bezier / 2
            let dx = s * (x1 - x0) + t * (x2 - x1);
            let dy = s * (y1 - y0) + t * (y2 - y1);
            (dx * dx + dy * dy).sqrt()
        };

        // * 2.0 for the true derivative magnitude
        2.0 * NODES
            .iter()
            .zip(WEIGHTS.iter())
            .map(|(&n, &w)| w * deriv_len(n))
            .sum::<f32>()
    }

    fn cubic_tangent(
        t: f32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> (f32, f32) {
        let s = 1.0 - t;
        let dx = 3.0 * (s * s * (x1 - x0) + 2.0 * s * t * (x2 - x1) + t * t * (x3 - x2));
        let dy = 3.0 * (s * s * (y1 - y0) + 2.0 * s * t * (y2 - y1) + t * t * (y3 - y2));
        (dx, dy)
    }

    fn quadratic_tangent(
        t: f32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) -> (f32, f32) {
        let s = 1.0 - t;
        let dx = 2.0 * (s * (x1 - x0) + t * (x2 - x1));
        let dy = 2.0 * (s * (y1 - y0) + t * (y2 - y1));
        (dx, dy)
    }

    /// Angle between two tangent vectors in [0, π]. Returns 0 for degenerate inputs.
    fn tangent_angle_diff(dx0: f32, dy0: f32, dx1: f32, dy1: f32) -> f32 {
        let len0 = (dx0 * dx0 + dy0 * dy0).sqrt();
        let len1 = (dx1 * dx1 + dy1 * dy1).sqrt();
        if len0 < f32::EPSILON || len1 < f32::EPSILON {
            return 0.0;
        }
        let dot = ((dx0 * dx1 + dy0 * dy1) / (len0 * len1)).clamp(-1.0, 1.0);
        dot.acos()
    }

    /// Finds the control point C = (cx, cy) that minimises a weighted combination of:
    ///   - positional error:  ||Q(t_i) - cubic(t_i)||²  (keeps shape accurate)
    ///   - tangent alignment: (Q'(t_i) × cubic'(t_i))²  (prevents join artifacts)
    ///
    /// The two criteria live in different units, so `tan_weight` scales the tangent
    /// terms relative to position. higher values bias the control point
    /// toward preserving direction at the cost of shape fit.
    fn cubic_to_quadratic_least_squares(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        tan_weight: f32,
    ) -> (f32, f32) {
        // For Q(t) = (1-t)²·P0 + 2t(1-t)·C + t²·P3:
        //
        // Positional rows (decoupled x/y):
        //   a_i·cx = rx_i    where a_i = 2t(1-t),  rx_i = cubic_x(t) - (1-t)²x0 - t²x3
        //   a_i·cy = ry_i
        //
        // Tangent row (couples cx and cy via cross product):
        //   Q'(t)/2 = (1-2t)·C - (1-t)·P0 + t·P3
        //   Cross product with unit cubic tangent (gdx, gdy) = 0 for perfect alignment:
        //   (1-2t)·(cx·gdy - cy·gdx) = (1-t)·(x0·gdy - y0·gdx) + t·(x3·gdy - y3·gdx)
        //   i.e. [(1-2t)·gdy, -(1-2t)·gdx] · [cx, cy] = K_i
        //
        // Assembles into a 2×2 normal equations: (AᵀA)·[cx,cy]ᵀ = Aᵀb

        const SAMPLES: usize = 64;

        // Normal equations accumulators: M = AᵀA, r = Aᵀb
        let mut m00 = 0.0f32; // cx·cx
        let mut m01 = 0.0f32; // cx·cy (symmetric)
        let mut m11 = 0.0f32; // cy·cy
        let mut r0 = 0.0f32; // rhs for cx
        let mut r1 = 0.0f32; // rhs for cy

        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;
            let s = 1.0 - t;
            let a = 2.0 * t * s; // basis weight for C in Q(t)

            // --- Positional contribution ---
            let (cx_cubic, cy_cubic) = Self::cubic(t, x0, y0, x1, y1, x2, y2, x3, y3);
            let rx = cx_cubic - s * s * x0 - t * t * x3;
            let ry = cy_cubic - s * s * y0 - t * t * y3;

            // Row [a, 0] -> cx equation; row [0, a] -> cy equation (weight 1.0)
            m00 += a * a;
            m11 += a * a;
            r0 += a * rx;
            r1 += a * ry;

            // --- Tangent contribution ---
            // Skip t = 0.5 exactly: (1-2t) = 0, tangent row degenerates
            let coeff = 1.0 - 2.0 * t;
            if coeff.abs() < 1e-4 {
                continue;
            }

            let (tdx, tdy) = Self::cubic_tangent(t, x0, y0, x1, y1, x2, y2, x3, y3);
            let tlen = (tdx * tdx + tdy * tdy).sqrt();
            if tlen < f32::EPSILON {
                continue;
            }
            // Normalise so tangent rows are scale-independent
            let gdx = tdx / tlen;
            let gdy = tdy / tlen;

            // Tangent row: [coeff·gdy, -coeff·gdx] · [cx, cy] = K
            let k = s * (x0 * gdy - y0 * gdx) - t * (x3 * gdy - y3 * gdx);
            let row_x = coeff * gdy;
            let row_y = -coeff * gdx;

            let w = tan_weight;
            m00 += w * row_x * row_x;
            m01 += w * row_x * row_y;
            m11 += w * row_y * row_y;
            r0 += w * row_x * k;
            r1 += w * row_y * k;
        }

        // Solve 2×2 symmetric system (M is symmetric so m01 == m10)
        let det = m00 * m11 - m01 * m01;
        if det.abs() < f32::EPSILON {
            // Degenerate: fall back to pure positional least-squares result
            let a2_sum = m00 - {
                // recompute just positional diagonal (approx)
                let mut s = 0.0f32;
                for i in 1..SAMPLES {
                    let t = i as f32 / SAMPLES as f32;
                    let a = 2.0 * t * (1.0 - t);
                    s += a * a;
                }
                s * tan_weight // subtract tangent part... just use r0/m00 as fallback
            };
            let _ = a2_sum;
            return (r0 / m00.max(f32::EPSILON), r1 / m11.max(f32::EPSILON));
        }

        let cx = (r0 * m11 - r1 * m01) / det;
        let cy = (r1 * m00 - r0 * m01) / det;
        (cx, cy)
    }

    fn try_cubic_to_quadratic(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> Option<(f32, f32)> {
        // tan_weight: 0.0 = pure shape fit, higher = more tangent-biased.
        // 0.5 is a good default: tangent errors (which cause visible kinks) are
        // penalised meaningfully without sacrificing shape accuracy.
        let (qx, qy) = Self::cubic_to_quadratic_least_squares(x0, y0, x1, y1, x2, y2, x3, y3, 16.0);

        let arc_len = Self::cubic_arc_length(x0, y0, x1, y1, x2, y2, x3, y3);
        let pos_threshold = (C2Q_TOLERANCE * arc_len * 0.015).max(f32::EPSILON);
        const TAN_THRESHOLD: f32 = 0.05;

        const SAMPLES: usize = 64;
        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;

            let (cx, cy) = Self::cubic(t, x0, y0, x1, y1, x2, y2, x3, y3);
            let (qbx, qby) = Self::quadratic(t, x0, y0, qx, qy, x3, y3);
            let pos_err = ((cx - qbx) * (cx - qbx) + (cy - qby) * (cy - qby)).sqrt();

            let (cdx, cdy) = Self::cubic_tangent(t, x0, y0, x1, y1, x2, y2, x3, y3);
            let (qdx, qdy) = Self::quadratic_tangent(t, x0, y0, qx, qy, x3, y3);
            let tan_err = Self::tangent_angle_diff(cdx, cdy, qdx, qdy);

            if pos_err > pos_threshold || tan_err > TAN_THRESHOLD {
                return None;
            }
        }

        Some((qx, qy))
    }

    fn quadratic_is_line_like(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let dx = x2 - x0;
        let dy = y2 - y0;
        let chord_len = (dx * dx + dy * dy).sqrt();
        let perp_dist = (dx * (x1 - x0) - dy * (y1 - y0)).abs() / chord_len.max(1e-6);
        0.5 * perp_dist < Q2L_TOLERANCE * 0.0003
    }

    fn lines_are_collinear(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let chord_x = x2 - x0;
        let chord_y = y2 - y0;
        let chord_len = (chord_x * chord_x + chord_y * chord_y).sqrt();

        let cross = ((x1 - x0) * chord_y - (y1 - y0) * chord_x).abs();
        let perp_dist = cross / chord_len.max(1e-6);

        perp_dist < MERGE_L_TOLERANCE * 0.001
    }

    // returns merged control point if within tolerance
    fn merge_quadratics(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        x4: f32,
        y4: f32,
    ) -> Option<(f32, f32)> {
        let d1 = Self::quadratic_arc_length(x0, y0, x1, y1, x2, y2);
        let d2 = Self::quadratic_arc_length(x2, y2, x3, y3, x4, y4);
        let total = d1 + d2;
        if total < 1e-8 {
            return None;
        }
        let t_split = d1 / total;

        // Helper: evaluate the piecewise quadratic at merged-t
        let piecewise = |t: f32| -> (f32, f32) {
            if t <= t_split {
                let t1 = t / t_split;
                Self::quadratic(t1, x0, y0, x1, y1, x2, y2)
            } else {
                let t2 = (t - t_split) / (1.0 - t_split);
                Self::quadratic(t2, x2, y2, x3, y3, x4, y4)
            }
        };

        // Piecewise quadratic tangent at merged-t (unnormalized)
        let piecewise_tangent = |t: f32| -> (f32, f32) {
            if t <= t_split {
                let t1 = t / t_split;
                Self::quadratic_tangent(t1, x0, y0, x1, y1, x2, y2)
            } else {
                let t2 = (t - t_split) / (1.0 - t_split);
                Self::quadratic_tangent(t2, x2, y2, x3, y3, x4, y4)
            }
        };

        // --- Least-squares with tangent weighting (same pattern as cubic_to_quadratic) ---
        const SAMPLES: usize = 64;
        const TAN_WEIGHT: f32 = 16.0;

        let mut m00 = 0.0f32;
        let mut m01 = 0.0f32;
        let mut m11 = 0.0f32;
        let mut r0 = 0.0f32;
        let mut r1 = 0.0f32;

        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;
            let s = 1.0 - t;
            let a = 2.0 * t * s;

            // Positional contribution
            let (px, py) = piecewise(t);
            let rx = px - s * s * x0 - t * t * x4;
            let ry = py - s * s * y0 - t * t * y4;

            m00 += a * a;
            m11 += a * a;
            r0 += a * rx;
            r1 += a * ry;

            // Tangent contribution
            let coeff = 1.0 - 2.0 * t;
            if coeff.abs() < 1e-4 {
                continue;
            }

            let (tdx, tdy) = piecewise_tangent(t);
            let tlen = (tdx * tdx + tdy * tdy).sqrt();
            if tlen < f32::EPSILON {
                continue;
            }
            let gdx = tdx / tlen;
            let gdy = tdy / tlen;

            let k = s * (x0 * gdy - y0 * gdx) - t * (x4 * gdy - y4 * gdx);
            let row_x = coeff * gdy;
            let row_y = -coeff * gdx;

            m00 += TAN_WEIGHT * row_x * row_x;
            m01 += TAN_WEIGHT * row_x * row_y;
            m11 += TAN_WEIGHT * row_y * row_y;
            r0 += TAN_WEIGHT * row_x * k;
            r1 += TAN_WEIGHT * row_y * k;
        }

        let det = m00 * m11 - m01 * m01;
        let (cx, cy) = if det.abs() < f32::EPSILON {
            (r0 / m00.max(f32::EPSILON), r1 / m11.max(f32::EPSILON))
        } else {
            ((r0 * m11 - r1 * m01) / det, (r1 * m00 - r0 * m01) / det)
        };

        // --- Validate: position and tangent ---
        let pos_threshold = (MERGE_Q_TOLERANCE * total * 0.005).max(f32::EPSILON);
        let tan_threshold = 0.05 * MERGE_Q_TOLERANCE.sqrt();

        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;

            let (ox, oy) = piecewise(t);
            let (ax, ay) = Self::quadratic(t, x0, y0, cx, cy, x4, y4);
            let pos_err = ((ox - ax) * (ox - ax) + (oy - ay) * (oy - ay)).sqrt();
            if pos_err > pos_threshold {
                return None;
            }

            let (tdx, tdy) = piecewise_tangent(t);
            let (qdx, qdy) = Self::quadratic_tangent(t, x0, y0, cx, cy, x4, y4);
            let tan_err = Self::tangent_angle_diff(tdx, tdy, qdx, qdy);
            if tan_err > tan_threshold {
                return None;
            }
        }

        Some((cx, cy))
    }

    fn optimize(&mut self) {
        self.primitives = self.normalized(false);
        let (mut x0, mut y0);

        // Cubic ~> Quadratic
        (x0, y0) = (0.0, 0.0);
        for prim in self.primitives.iter_mut() {
            match prim.clone() {
                Primitive::Cubic(x1, y1, x2, y2, x3, y3) => {
                    if let Some((qx, qy)) =
                        Self::try_cubic_to_quadratic(x0, y0, x1, y1, x2, y2, x3, y3)
                    {
                        *prim = Primitive::Quadratic(qx, qy, x3, y3);
                    }
                    // let (qx, qy) =
                    //     Self::cubic_to_quadratic_tangent_preserving(x0, y0, x1, y1, x2, y2, x3, y3)
                    //         .unwrap_or((
                    //             (3.0 * (x1 + x2) - x3 - x0) * 0.25,
                    //             (3.0 * (y1 + y2) - y3 - y0) * 0.25,
                    //         ));
                    // if Self::cubic_is_quadratic_like(x0, y0, x1, y1, x2, y2, x3, y3, qx, qy) {
                    //     *prim = Primitive::Quadratic(qx, qy, x3, y3);
                    // }
                    // let (qx, qy) = (
                    //     (3.0 * (x1 + x2) - x3 - x0) * 0.25,
                    //     (3.0 * (y1 + y2) - y3 - y0) * 0.25,
                    // );
                    // if Self::cubic_is_quadratic_like(x0, y0, x1, y1, x2, y2, x3, y3, qx, qy) {
                    //     *prim = Primitive::Quadratic(qx, qy, x3, y3);
                    // }
                }
                _ => {}
            }
            (x0, y0) = Self::move_origin(prim);
        }

        // Cubic ~> 2x Quadratic
        (x0, y0) = (0.0, 0.0);
        let mut primitives = Vec::new();
        for prim in self.primitives.iter_mut() {
            match prim.clone() {
                Primitive::Cubic(x1, y1, x2, y2, x3, y3) => {
                    let ax = (x0 + x1) * 0.5;
                    let ay = (y0 + y1) * 0.5;
                    let bx = (x1 + x2) * 0.5;
                    let by = (y1 + y2) * 0.5;
                    let cx = (x2 + x3) * 0.5;
                    let cy = (y2 + y3) * 0.5;
                    let dx = (ax + bx) * 0.5;
                    let dy = (ay + by) * 0.5;
                    let ex = (bx + cx) * 0.5;
                    let ey = (by + cy) * 0.5;
                    let mx = (dx + ex) * 0.5;
                    let my = (dy + ey) * 0.5;
                    let qx1 = 0.75 * (ax + dx) - 0.25 * (x0 + mx);
                    let qy1 = 0.75 * (ay + dy) - 0.25 * (y0 + my);
                    let qx3 = 0.75 * (cx + ex) - 0.25 * (x3 + mx);
                    let qy3 = 0.75 * (cy + ey) - 0.25 * (y3 + my);
                    primitives.push(Primitive::Quadratic(qx1, qy1, mx, my));
                    primitives.push(Primitive::Quadratic(qx3, qy3, x3, y3));
                }
                p => primitives.push(p),
            }
            (x0, y0) = Self::move_origin(prim);
        }
        self.primitives = primitives;

        // x2 Quadratic ~> Quadratic
        let (mut current_x, mut current_y) = (0.0, 0.0);
        let mut primitives = Vec::new();
        for prim in &self.primitives {
            match (primitives.last().cloned(), prim.clone()) {
                (
                    Some(Primitive::Quadratic(x1, y1, x2, y2)),
                    Primitive::Quadratic(x3, y3, x4, y4),
                ) => {
                    let merged = Self::merge_quadratics(x0, y0, x1, y1, x2, y2, x3, y3, x4, y4);

                    if let Some((cx, cy)) = merged {
                        primitives.pop();
                        primitives.push(Primitive::Quadratic(cx, cy, x4, y4));
                        (current_x, current_y) = (x4, y4);
                    } else {
                        primitives.push(Primitive::Quadratic(x3, y3, x4, y4));
                        (x0, y0) = (x2, y2);
                        (current_x, current_y) = (x4, y4);
                    }
                }

                (_, p) => {
                    let start_x = current_x;
                    let start_y = current_y;

                    primitives.push(p.clone());
                    (current_x, current_y) = Self::move_origin(&p);

                    if matches!(&p, Primitive::Quadratic(..)) {
                        (x0, y0) = (start_x, start_y);
                    } else {
                        (x0, y0) = (current_x, current_y);
                    }
                }
            }
        }

        self.primitives = primitives;

        // Quadratic ~> Line
        (x0, y0) = (0.0, 0.0);
        for prim in self.primitives.iter_mut() {
            match prim.clone() {
                Primitive::Quadratic(x1, y1, x2, y2)
                    if Self::quadratic_is_line_like(x0, y0, x1, y1, x2, y2) =>
                {
                    *prim = Primitive::Line(x2, y2);
                }
                _ => {}
            }
            (x0, y0) = Self::move_origin(prim);
        }

        // x2 Line ~> Line
        let mut primitives = Vec::new();
        let (mut x0, mut y0) = (0.0, 0.0); // Current position (end of last primitive)
        let (mut prev_start_x, mut prev_start_y) = (0.0, 0.0); // Start of last line
        for prim in self.primitives.iter() {
            match (primitives.last().cloned(), prim.clone()) {
                (Some(Primitive::Line(x1, y1)), Primitive::Line(x2, y2))
                    if Self::lines_are_collinear(prev_start_x, prev_start_y, x1, y1, x2, y2) =>
                {
                    primitives.pop();
                    primitives.push(Primitive::Line(x2, y2));
                    (x0, y0) = (x2, y2);
                }
                (_, Primitive::Line(x, y)) => {
                    primitives.push(Primitive::Line(x, y));
                    (prev_start_x, prev_start_y) = (x0, y0);
                    (x0, y0) = (x, y);
                }
                (_, p) => {
                    primitives.push(p.clone());
                    (x0, y0) = Self::move_origin(&p);
                    (prev_start_x, prev_start_y) = (x0, y0);
                }
            }
        }
        self.primitives = primitives;

        let mut counts =
            self.primitives
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, p| {
                    *acc.entry(match p {
                        Primitive::Cubic(..) => "Cubic",
                        Primitive::Quadratic(..) => "Quadr",
                        Primitive::Line(..) => " Line",
                        Primitive::Move(..) => "",
                        Primitive::Arc(..) => " Arc",
                        Primitive::Close => "",
                    })
                    .or_insert(0) += 1;
                    acc
                });
        let total = counts.get("Cubic").unwrap_or(&0)
            + counts.get("Quadr").unwrap_or(&0)
            + counts.get(" Line").unwrap_or(&0);
        counts.insert("Total", total);
        let mut counts = counts
            .iter()
            .filter_map(|(k, v)| (*k != "").then_some(format!("{k}: {v}")))
            .collect::<Vec<_>>();
        counts.sort();
        println!("{}", counts.join("\n"));
    }

    pub fn save(&self, path: &str) {
        use std::fs::File;
        use std::io::{BufWriter, Write};
        let mut file = BufWriter::new(File::create(path).expect("Failed to create file"));
        let (_, _, vw, vh) = self.compute_viewport();
        let scale = vw.max(vh);
        let width = vw / scale;
        let height = vh / scale;
        writeln!(
            file,
            "<svg viewBox='0 0 {} {}' xmlns='http://www.w3.org/2000/svg'>",
            width, height
        )
        .unwrap();
        let mut d = String::new();
        let primitives = self.normalized(false);
        for prim in &primitives {
            match prim {
                Primitive::Move(x0, y0) => {
                    d.push_str(&format!("M{x0} {y0} "));
                }
                Primitive::Line(x1, y1) => {
                    d.push_str(&format!("L{x1} {y1} "));
                }
                Primitive::Quadratic(x1, y1, x2, y2) => {
                    d.push_str(&format!("Q{x1} {y1} {x2} {y2} "));
                }
                Primitive::Cubic(x1, y1, x2, y2, x3, y3) => {
                    d.push_str(&format!("C{x1} {y1} {x2} {y2} {x3} {y3} "));
                }
                Primitive::Arc(rx, ry, xrot, large, sweep, x, y) => {
                    d.push_str(&format!("A{rx} {ry} {xrot} {large} {sweep} {x} {y} "));
                }
                Primitive::Close => d.push_str("Z "),
            }
        }
        writeln!(
            file,
            "<path d='{}' fill='white' stroke='black' stroke-width='0'/>",
            d.trim()
        )
        .unwrap();
        writeln!(file, "</svg>").unwrap();
    }

    fn normalized(&self, invert: bool) -> Vec<Primitive> {
        let (min_x, min_y, vw, vh) = self.compute_viewport();

        let scale = vw.max(vh);
        let norm_vh = vh / scale;

        let map = |x: f32, y: f32| -> (f32, f32) {
            let nx = (x - min_x) / scale;
            let ny = if invert {
                norm_vh - (y - min_y) / scale
            } else {
                (y - min_y) / scale
            };
            (nx, ny)
        };

        self.primitives
            .iter()
            .map(|p| match *p {
                Primitive::Move(x, y) => {
                    let (x, y) = map(x, y);
                    Primitive::Move(x, y)
                }
                Primitive::Line(x, y) => {
                    let (x, y) = map(x, y);
                    Primitive::Line(x, y)
                }
                Primitive::Quadratic(x1, y1, x2, y2) => {
                    let (x1, y1) = map(x1, y1);
                    let (x2, y2) = map(x2, y2);
                    Primitive::Quadratic(x1, y1, x2, y2)
                }
                Primitive::Cubic(x1, y1, x2, y2, x3, y3) => {
                    let (x1, y1) = map(x1, y1);
                    let (x2, y2) = map(x2, y2);
                    let (x3, y3) = map(x3, y3);
                    Primitive::Cubic(x1, y1, x2, y2, x3, y3)
                }
                Primitive::Arc(rx, ry, rot, large, sweep, x, y) => {
                    let (x, y) = map(x, y);
                    let rxn = (rx / scale).max(0.0);
                    let ryn = (ry / scale).max(0.0);
                    Primitive::Arc(rxn, ryn, rot, large, sweep, x, y)
                }
                Primitive::Close => Primitive::Close,
            })
            .collect()
    }

    pub fn shader(&self) {
        let mut s = String::new();
        let (mut px, mut py) = (0.0, 0.0);
        let f = |x: f32| -> String {
            let mut s = if x < 0.001 {
                format!("{x:.0}.")
            } else {
                format!("{x:.3}")
            };
            s = s.trim_end_matches("0").to_string();
            s
        };
        let primitives = self.normalized(true);
        for prim in &primitives {
            match prim.clone() {
                Primitive::Move(_, _) => {}
                Primitive::Line(x1, y1) => {
                    s += &format!("L({},{},{},{})", f(px), f(py), f(x1), f(y1))
                }
                Primitive::Quadratic(x1, y1, x2, y2) => {
                    s += &format!(
                        "Q({},{},{},{},{},{})",
                        f(px),
                        f(py),
                        f(x1),
                        f(y1),
                        f(x2),
                        f(y2)
                    )
                }
                Primitive::Cubic(_, _, _, _, _, _) => {}
                Primitive::Arc(_, _, _, _, _, _, _) => {}
                Primitive::Close => {}
            }
            (px, py) = Self::move_origin(prim);
        }
        s.push(';');
        println!("{s}");
    }

    pub fn shader_arr(&self) {
        let mut total_lines = 0;
        let mut total_quads = 0;
        for prim in &self.primitives {
            match prim.clone() {
                Primitive::Line(..) => {
                    total_lines += 1;
                }
                Primitive::Quadratic(..) => {
                    total_quads += 1;
                }
                _ => {}
            }
        }
        let mut lines = format!("vec4 lines[{total_lines}] = vec4[](");
        let mut quad_ab = format!("vec4 quad_ab[{total_quads}] = vec4[](");
        let mut quad_c = format!("vec2 quad_c[{total_quads}] = vec2[](");
        let (mut x0, mut y0) = (0.0, 0.0);
        let f = |x: f32| -> String {
            let mut s = if x < 0.001 {
                format!("{x:.0}.")
            } else {
                format!("{x:.3}")
            };
            s = s.trim_end_matches("0").to_string();
            s
        };
        let primitives = self.normalized(true);
        for prim in &primitives {
            match prim.clone() {
                Primitive::Line(x1, y1) => {
                    lines += &format!("vec4({},{},{},{}),", f(x0), f(y0), f(x1), f(y1));
                }
                Primitive::Quadratic(x1, y1, x2, y2) => {
                    quad_ab += &format!("vec4({},{},{},{}),", f(x0), f(y0), f(x1), f(y1));
                    quad_c += &format!("vec2({}, {}),", f(x2), f(y2));
                }
                _ => {}
            }
            (x0, y0) = Self::move_origin(prim);
        }
        let lines = &lines[..lines.len() - 1];
        let quad_ab = &quad_ab[..quad_ab.len() - 1];
        let quad_c = &quad_c[..quad_c.len() - 1];
        println!("{lines});");
        println!("{quad_ab});");
        println!("{quad_c});");
        println!(
            "for (int i = 0; i < lines.length(); ++i) combine(d, sdf_line(p, lines[i].xy, lines[i].zw));"
        );
        println!(
            "for (int i = 0; i < quad_ab.length(); ++i) combine(d, sdf_bezier(p, quad_ab[i].xy, quad_ab[i].zw, quad_c[i]));"
        );
    }

    fn aabb_cubic(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> (f32, f32, f32, f32) {
        let cx = -x0 + x1;
        let bx = x0 - 2.0 * x1 + x2;
        let ax = -x0 + 3.0 * x1 - 3.0 * x2 + x3;
        let gx = ((bx * bx - ax * cx).max(0.0)).sqrt();
        let t1x = ((-bx - gx) / ax).clamp(0.0, 1.0);
        let t2x = ((-bx + gx) / ax).clamp(0.0, 1.0);
        let q1x = x0 + t1x * (3.0 * cx + t1x * (3.0 * bx + t1x * ax));
        let q2x = x0 + t2x * (3.0 * cx + t2x * (3.0 * bx + t2x * ax));
        let min_x = x0.min(x3).min(q1x).min(q2x);
        let max_x = x0.max(x3).max(q1x).max(q2x);

        let cy = -y0 + y1;
        let by = y0 - 2.0 * y1 + y2;
        let ay = -y0 + 3.0 * y1 - 3.0 * y2 + y3;
        let gy = ((by * by - ay * cy).max(0.0)).sqrt();
        let t1y = ((-by - gy) / ay).clamp(0.0, 1.0);
        let t2y = ((-by + gy) / ay).clamp(0.0, 1.0);
        let q1y = y0 + t1y * (3.0 * cy + t1y * (3.0 * by + t1y * ay));
        let q2y = y0 + t2y * (3.0 * cy + t2y * (3.0 * by + t2y * ay));
        let min_y = y0.min(y3).min(q1y).min(q2y);
        let max_y = y0.max(y3).max(q1y).max(q2y);

        (min_x, min_y, max_x, max_y)
    }

    fn aabb_quadratic(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) -> (f32, f32, f32, f32) {
        let ax = x0 - 2.0 * x1 + x2;
        let bx = x1 - x0;
        let tx = (-bx / ax).clamp(0.0, 1.0);
        let qx = x0 + tx * (2.0 * bx + tx * ax);
        let min_x = x0.min(x2).min(qx);
        let max_x = x0.max(x2).max(qx);

        let ay = y0 - 2.0 * y1 + y2;
        let by = y1 - y0;
        let ty = (-by / ay).clamp(0.0, 1.0);
        let qy = y0 + ty * (2.0 * by + ty * ay);
        let min_y = y0.min(y2).min(qy);
        let max_y = y0.max(y2).max(qy);

        (min_x, min_y, max_x, max_y)
    }

    fn quadratic(t: f32, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
        let x = t * (t * (x2 - 2.0 * x1 + x0) + 2.0 * (x1 - x0)) + x0;
        let y = t * (t * (y2 - 2.0 * y1 + y0) + 2.0 * (y1 - y0)) + y0;
        (x, y)
    }

    fn cubic(
        t: f32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> (f32, f32) {
        let s = 1.0 - t;
        let x = s * s * s * x0 + 3.0 * s * s * t * x1 + 3.0 * s * t * t * x2 + t * t * t * x3;
        let y = s * s * s * y0 + 3.0 * s * s * t * y1 + 3.0 * s * t * t * y2 + t * t * t * y3;
        (x, y)
    }

    fn compute_viewport(&self) -> (f32, f32, f32, f32) {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let (mut px, mut py) = (0.0, 0.0);

        for prim in &self.primitives {
            match *prim {
                Primitive::Quadratic(x1, y1, x, y) => {
                    let (ax, ay, bx, by) = Self::aabb_quadratic(px, py, x1, y1, x, y);
                    min_x = min_x.min(ax);
                    min_y = min_y.min(ay);
                    max_x = max_x.max(bx);
                    max_y = max_y.max(by);
                }
                Primitive::Cubic(x1, y1, x2, y2, x, y) => {
                    let (ax, ay, bx, by) = Self::aabb_cubic(px, py, x1, y1, x2, y2, x, y);
                    min_x = min_x.min(ax);
                    min_y = min_y.min(ay);
                    max_x = max_x.max(bx);
                    max_y = max_y.max(by);
                }
                Primitive::Arc(_, _, _, _, _, _, _) => {}
                _ => {}
            }
            // move relative origin
            match prim {
                Primitive::Move(x, y)
                | Primitive::Line(x, y)
                | Primitive::Quadratic(_, _, x, y)
                | Primitive::Cubic(_, _, _, _, x, y)
                | Primitive::Arc(_, _, _, _, _, x, y) => {
                    (px, py) = (*x, *y);
                    min_x = min_x.min(*x);
                    min_y = min_y.min(*y);
                    max_x = max_x.max(*x);
                    max_y = max_y.max(*y);
                }
                Primitive::Close => {}
            }
        }

        let vw = max_x - min_x;
        let vh = max_y - min_y;
        (min_x, min_y, vw, vh)
    }
}
fn main() {
    let mut svg = SvgReader::new("logo.svg");
    svg.optimize();
    svg.save("save.svg");
    svg.shader_arr();
}
