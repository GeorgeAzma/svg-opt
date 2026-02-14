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

const TOLERANCE: f32 = 2.0;
const C2Q_TOLERANCE: f32 = 0.0009 * TOLERANCE;
const Q2L_TOLERANCE: f32 = 0.009 * TOLERANCE;
const MERGE_Q_TOLERANCE: f32 = 0.00005 * TOLERANCE;
const MERGE_L_TOLERANCE: f32 = 0.005 * TOLERANCE;

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

    fn cubic_is_quadratic_like(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) -> bool {
        // Check if control points are roughly collinear with their ideal quadratic position
        // Ideal: p1 and p2 should both be near (2*q - 0.5*(p0+p3))
        let qx = (3.0 * (x1 + x2) - x3 - x0) * 0.25;
        let qy = (3.0 * (y1 + y2) - y3 - y0) * 0.25;

        // Expected cubic control points from this quadratic
        let expected_p1_x = x0 + (2.0 / 3.0) * (qx - x0);
        let expected_p1_y = y0 + (2.0 / 3.0) * (qy - y0);
        let expected_p2_x = x3 + (2.0 / 3.0) * (qx - x3);
        let expected_p2_y = y3 + (2.0 / 3.0) * (qy - y3);

        let error1 = ((x1 - expected_p1_x).powi(2) + (y1 - expected_p1_y).powi(2)).sqrt();
        let error2 = ((x2 - expected_p2_x).powi(2) + (y2 - expected_p2_y).powi(2)).sqrt();

        error1 < C2Q_TOLERANCE && error2 < C2Q_TOLERANCE
    }

    fn quadratic_is_line_like(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let abx = x1 - x0;
        let aby = y1 - y0;
        let bcx = x2 - x1;
        let bcy = y2 - y1;
        let abl = (abx * abx + aby * aby).sqrt();
        let bcl = (bcx * bcx + bcy * bcy).sqrt();
        ((abx * bcx + aby * bcy) / (abl * bcl) - 1.0).abs() < Q2L_TOLERANCE
    }

    fn lines_are_collinear(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        let d1 = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let d2 = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        let max_dist = d1.max(d2);
        if max_dist < 1e-6 {
            return true;
        }
        (area.abs() / max_dist) < MERGE_L_TOLERANCE
    }

    fn optimize(&mut self) {
        self.primitives = self.normalized(false);
        let (mut x0, mut y0);

        // Cubic ~> Quadratic
        (x0, y0) = (0.0, 0.0);
        for prim in self.primitives.iter_mut() {
            match prim.clone() {
                Primitive::Cubic(x1, y1, x2, y2, x3, y3)
                    if Self::cubic_is_quadratic_like(x0, y0, x1, y1, x2, y2, x3, y3) =>
                {
                    let qx = (3.0 * (x1 + x2) - x3 - x0) * 0.25;
                    let qy = (3.0 * (y1 + y2) - y3 - y0) * 0.25;
                    *prim = Primitive::Quadratic(qx, qy, x3, y3);
                }
                _ => {}
            }
            (x0, y0) = Self::move_origin(prim);
        }

        // // Cubic ~> 2x Quadratic
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
                    let dx1 = x2 - x1;
                    let dy1 = y2 - y1;
                    let dx2 = x3 - x2;
                    let dy2 = y3 - y2;

                    let len1_sq = dx1 * dx1 + dy1 * dy1;
                    let len2_sq = dx2 * dx2 + dy2 * dy2;

                    let d1 = len1_sq.sqrt();
                    let d2 = len2_sq.sqrt();
                    let total = d1 + d2;
                    let t_split = if total > 1e-8 { d1 / total } else { 0.5 };

                    let u = 1.0 - t_split;
                    let v = t_split;
                    let denom = 2.0 * u * v;
                    let (cx, cy) = if denom.abs() > 1e-8 {
                        (
                            (x2 - u * u * x0 - v * v * x4) / denom,
                            (y2 - u * u * y0 - v * v * y4) / denom,
                        )
                    } else {
                        (x2, y2)
                    };

                    let should_merge = || -> bool {
                        const SAMPLES: usize = 16;
                        let threshold_sq = MERGE_Q_TOLERANCE * MERGE_Q_TOLERANCE;
                        for i in 1..SAMPLES {
                            let t = i as f32 / SAMPLES as f32;
                            let (ox, oy) = if t <= t_split {
                                let t1 = t / t_split;
                                Self::quadratic(t1, x0, y0, x1, y1, x2, y2)
                            } else {
                                let t2 = (t - t_split) / (1.0 - t_split);
                                Self::quadratic(t2, x2, y2, x3, y3, x4, y4)
                            };

                            let (ax, ay) = Self::quadratic(t, x0, y0, cx, cy, x4, y4);

                            if (ox - ax).powi(2) + (oy - ay).powi(2) > threshold_sq {
                                return false;
                            }
                        }
                        true
                    };

                    if should_merge() {
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

        let counts = self
            .primitives
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, p| {
                *acc.entry(match p {
                    Primitive::Cubic(..) => "Cubic",
                    Primitive::Quadratic(..) => "Quadratic",
                    Primitive::Line(..) => "Line",
                    Primitive::Move(..) => "Move",
                    Primitive::Arc(..) => "Arc",
                    Primitive::Close => "Close",
                })
                .or_insert(0) += 1;
                acc
            });
        println!("{:#?}\nTotal: {}", counts, self.primitives.len());
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
            "<path d='{}' fill='black' stroke='black' stroke-width='0'/>",
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
        println!("for (int i = 0; i < lines.length(); ++i) combine(d, sdf_line(p, lines[i]));");
        println!(
            "for (int i = 0; i < quad_ab.length(); ++i) combine(d, sdf_bezier(p, quad_ab[i], quad_c[i]));"
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

    fn h_quadratic(y0: f32, y1: f32, y2: f32) -> f32 {
        return ((y0 - y1) / (y0 - 2.0 * y1 + y2)).clamp(0.0, 1.0);
    }

    fn quadratic(t: f32, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
        let x = t * (t * (x2 - 2.0 * x1 + x0) + 2.0 * (x1 - x0)) + x0;
        let y = t * (t * (y2 - 2.0 * y1 + y0) + 2.0 * (y1 - y0)) + y0;
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
    let mut svg = SvgReader::new("test.svg");
    svg.optimize();
    // svg.save("save.svg");
    svg.shader_arr();
}
