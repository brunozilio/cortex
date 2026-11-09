diagnostic(off, derivative_uniformity);

struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    time: f32,
    tones: array<vec4<f32>, 6>,
};

struct Prim {
    kind: u32,
    flags: u32,
    neighbor_start: u32,
    neighbor_count: u32,
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    reach: vec4<f32>,
    top: vec4<f32>,
    bottom: vec4<f32>,
    style: vec4<f32>,
};

struct Lighting {
    highlight: vec4<f32>,
    shadow: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> prims: array<Prim>;
@group(0) @binding(2) var<storage, read> neighbors: array<u32>;
@group(0) @binding(3) var<uniform> lighting: Lighting;

const GHOST: u32 = 1u;
const LINE: u32 = 2u;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

fn corner(vertex: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    return corners[vertex];
}

fn quad(vertex: u32, index: u32, pad: f32) -> VsOut {
    let prim = prims[index];
    var out: VsOut;
    out.index = index;
    if (prim.flags & GHOST) != 0u {
        out.position = vec4(0.0, 0.0, 0.0, 1.0);
        out.world = vec2(0.0);
        return out;
    }
    let min = prim.reach.xy - vec2(pad);
    let size = prim.reach.zw + vec2(pad * 2.0);
    let world = min + corner(vertex) * size;
    let ndc = world / globals.viewport * 2.0 - 1.0;
    out.position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    out.world = world;
    return out;
}

fn sd_round_rect(p: vec2<f32>, center: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half.x, half.y));
    let q = abs(p - center) - half + vec2(r);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn sd_capsule(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, radius: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h) - radius;
}

fn sd_arc(p: vec2<f32>, center: vec2<f32>, radius: f32, start: f32, sweep: f32, thickness: f32) -> f32 {
    let d = p - center;
    var angle = atan2(d.y, d.x) - start;
    angle = angle - floor(angle / 6.28318530718) * 6.28318530718;
    if angle <= sweep {
        return abs(length(d) - radius) - thickness;
    }
    let e0 = center + radius * vec2(cos(start), sin(start));
    let e1 = center + radius * vec2(cos(start + sweep), sin(start + sweep));
    return min(length(p - e0), length(p - e1)) - thickness;
}

fn sd_notched(p: vec2<f32>, prim: Prim) -> f32 {
    let body = sd_round_rect(p, prim.a.xy, prim.a.zw, prim.b.x);
    let notch = sd_round_rect(p, prim.c.xy, prim.c.zw, prim.b.y);
    let k = max(prim.b.z, 1e-3);
    let h = clamp(0.5 - 0.5 * (-notch - body) / k, 0.0, 1.0);
    return mix(-notch, body, h) + k * h * (1.0 - h);
}

fn sdf(prim: Prim, p: vec2<f32>) -> f32 {
    switch prim.kind {
        case 4u: { return sd_notched(p, prim); }
        case 0u: { return sd_round_rect(p, prim.a.xy, prim.a.zw, prim.b.x); }
        case 1u: { return sd_capsule(p, prim.a.xy, prim.a.zw, prim.b.x); }
        case 2u: { return sd_arc(p, prim.a.xy, prim.a.z, prim.b.x, prim.b.y, prim.a.w); }
        default: { return length(p - prim.a.xy) - prim.a.z; }
    }
}

fn fill(prim: Prim, p: vec2<f32>) -> vec4<f32> {
    let t = clamp((p.y - prim.style.z) / max(prim.style.w, 1.0), 0.0, 1.0);
    let c = mix(prim.top, prim.bottom, t);
    return vec4(c.rgb, c.a * prim.style.y);
}

fn in_reach(prim: Prim, p: vec2<f32>) -> bool {
    return all(p >= prim.reach.xy) && all(p <= prim.reach.xy + prim.reach.zw);
}

struct Union {
    d: f32,
    color: vec4<f32>,
    owner: u32,
    in_ghost: bool,
};

fn smin(a: f32, b: f32, k: f32) -> vec2<f32> {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return vec2(mix(b, a, h) - k * h * (1.0 - h), h);
}

fn union_at(index: u32, p: vec2<f32>) -> Union {
    let own = prims[index];
    var u: Union;
    u.owner = 0xffffffffu;
    u.in_ghost = false;
    var surface = 1e9;
    var surface_color = vec4(0.0);
    var line = 1e9;
    var line_color = vec4(0.0);
    var line_k = 1e-3;
    var best = 1e9;
    for (var i = 0u; i < own.neighbor_count; i++) {
        let j = neighbors[own.neighbor_start + i];
        let prim = prims[j];
        if !in_reach(prim, p) {
            continue;
        }
        let d = sdf(prim, p);
        let color = fill(prim, p);
        let ghost = (prim.flags & GHOST) != 0u;
        if ghost && d < 0.0 {
            u.in_ghost = true;
        }
        if !ghost && d < best {
            best = d;
            u.owner = j;
        }
        if (prim.flags & LINE) != 0u {
            line_k = max(line_k, prim.style.x);
            if d < line {
                line = d;
                line_color = color;
            }
            continue;
        }
        if surface > 1e8 {
            surface = d;
            surface_color = color;
            continue;
        }
        let k = max(min(own.style.x, prim.style.x), 1e-3);
        let m = smin(surface, d, k);
        surface = m.x;
        surface_color = mix(color, surface_color, m.y);
    }
    if line > 1e8 {
        u.d = surface;
        u.color = surface_color;
    } else if surface > 1e8 {
        u.d = line;
        u.color = line_color;
    } else {
        let m = smin(surface, line, line_k);
        u.d = m.x;
        u.color = mix(line_color, surface_color, m.y);
    }
    return u;
}

@vertex
fn vs_fill(@builtin(vertex_index) vertex: u32, @builtin(instance_index) index: u32) -> VsOut {
    return quad(vertex, index, 1.0);
}

@fragment
fn fs_fill(in: VsOut) -> @location(0) vec4<f32> {
    let u = union_at(in.index, in.world);
    let grad = vec2(dpdx(u.d), dpdy(u.d));
    if u.owner != in.index || u.in_ghost {
        discard;
    }
    let px = 1.0 / globals.scale;
    let coverage = clamp(0.5 - u.d / px, 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    let normal = grad / max(length(grad), 1e-6);
    let rim = clamp(1.0 + u.d, 0.0, 1.0);
    let top_light = pow(max(-normal.y, 0.0), 1.5);
    let lit = mix(u.color.rgb, lighting.highlight.rgb, lighting.highlight.a * rim * top_light);
    return vec4(lit, u.color.a * coverage);
}

@vertex
fn vs_shadow(@builtin(vertex_index) vertex: u32, @builtin(instance_index) index: u32) -> VsOut {
    let pad = lighting.params.y * 2.0 + abs(lighting.params.x) + 1.0;
    return quad(vertex, index, pad);
}

@fragment
fn fs_shadow(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.world - vec2(0.0, lighting.params.x);
    let u = union_at(in.index, p);
    let at_pixel = union_at(in.index, in.world);
    if u.owner != in.index || u.in_ghost || at_pixel.in_ghost {
        discard;
    }
    let blur = max(lighting.params.y, 0.5);
    let alpha = lighting.shadow.a * (1.0 - smoothstep(-blur, blur, u.d)) * u.color.a;
    if alpha <= 0.002 {
        discard;
    }
    return vec4(lighting.shadow.rgb, alpha);
}
