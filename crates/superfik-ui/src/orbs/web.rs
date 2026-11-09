//! Web: a constellation wires itself, the `connecting` state. Nodes drift
//! on the sphere under slow value noise; any pair closer than `thr` grows
//! an edge, and bright packets run along randomly re-picked node pairs.

use super::core::{Dot, Line, OrbFrame, Projector, fib_dir, finalize_frame, frac, hash_d, lerp, radius_scale, vnoise};
use super::profiles::Opts;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.8 * o.get("spread", 1.0);
    let pt = Projector::new(t * 0.12, 0.32, cx, cy, big_r);
    let rs = radius_scale(size, o.get("rsPow", 0.6));

    let node_n = o.get("nodeN", 30.0) as usize;
    let thr = o.get("thr", 0.72);
    let node_r = o.get("nodeR", 1.4);
    let node_r_depth = o.get("nodeRDepth", 1.8);

    let nodes: Vec<[f64; 3]> = (0..node_n)
        .map(|i| {
            let d = fib_dir(i, node_n);
            let i = i as f64;
            let x = d[0] + 0.3 * (vnoise(i * 0.31 + 9.0, t * 0.24) - 0.5) * 2.0;
            let y = d[1] + 0.3 * (vnoise(i * 0.53 + 27.0, t * 0.21) - 0.5) * 2.0;
            let z = d[2] + 0.3 * (vnoise(i * 0.77 + 55.0, t * 0.27) - 0.5) * 2.0;
            let l = (x * x + y * y + z * z).sqrt();
            [x / l, y / l, z / l]
        })
        .collect();

    let mut lines = Vec::new();
    let mut dots = Vec::new();

    for i in 0..node_n {
        for j in (i + 1)..node_n {
            let dx = nodes[i][0] - nodes[j][0];
            let dy = nodes[i][1] - nodes[j][1];
            let dz = nodes[i][2] - nodes[j][2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist >= thr {
                continue;
            }
            let (x1, y1, z1) = pt.project(nodes[i][0], nodes[i][1], nodes[i][2]);
            let (x2, y2, z2) = pt.project(nodes[j][0], nodes[j][1], nodes[j][2]);
            let depth = ((z1 + z2) / 2.0 + 1.0) / 2.0;
            lines.push(Line {
                x1,
                y1,
                x2,
                y2,
                white: 0.42,
                a: (1.0 - dist / thr) * (0.3 + 0.55 * depth),
                w: (o.get("lineW", 0.8) * rs).max(0.6),
            });
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        let (px, py, z) = pt.project(node[0], node[1], node[2]);
        let depth = (z + 1.0) / 2.0;
        let pulse = 1.0 + 0.25 * (t * 1.4 + i as f64 * 2.7).sin();
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: (node_r + node_r_depth * depth) * pulse * rs,
            white: 0.55 - 0.45 * depth,
            a: 1.0,
        });
    }

    let signals = o.get("signals", 5.0) as usize;
    for s in 0..signals {
        let s = s as f64;
        let seg = (t * 0.55 + s * 7.31).floor();
        let a = (hash_d(seg, s * 3.1 + 1.7) * node_n as f64).floor() as usize;
        let b = (hash_d(seg, s * 5.7 + 4.2) * node_n as f64).floor() as usize;
        if a == b {
            continue;
        }
        let f = frac(t * 0.55 + s * 7.31);
        let x = lerp(nodes[a][0], nodes[b][0], f);
        let y = lerp(nodes[a][1], nodes[b][1], f);
        let z = lerp(nodes[a][2], nodes[b][2], f);
        let l = (x * x + y * y + z * z).sqrt().max(1e-6);
        let (px, py, zr) = pt.project(x / l, y / l, z / l);
        let depth = (zr + 1.0) / 2.0;
        dots.push(Dot {
            x: px,
            y: py,
            z: zr,
            r: (node_r * 1.5 + node_r_depth * depth) * rs,
            white: 0.05,
            a: 0.5 + 0.5 * depth,
        });
    }

    finalize_frame(dots, lines, o.get("rMin", 0.3))
}
