struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    time: f32,
    tones: array<vec4<f32>, 6>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct QuadIn {
    @location(0) rect: vec4<f32>,
    @location(1) top: vec4<f32>,
    @location(2) bottom: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) inset_color: vec4<f32>,
    @location(5) shadow_color: vec4<f32>,
    @location(6) params: vec4<f32>,
    @location(7) shadows: vec4<f32>,
    @location(8) extra: vec4<f32>,
    @location(9) clip: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) rect: vec4<f32>,
    @location(2) top: vec4<f32>,
    @location(3) bottom: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) inset_color: vec4<f32>,
    @location(6) shadow_color: vec4<f32>,
    @location(7) params: vec4<f32>,
    @location(8) shadows: vec4<f32>,
    @location(9) extra: vec4<f32>,
    @location(10) clip: vec4<f32>,
};

fn corner(vertex: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    return corners[vertex];
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, q: QuadIn) -> VsOut {
    let pad = q.shadows.w * 2.0 + abs(q.shadows.z) + 1.0;
    let min = q.rect.xy - vec2(pad);
    let size = q.rect.zw + vec2(pad * 2.0);
    let world = min + corner(vertex) * size;
    let ndc = world / globals.viewport * 2.0 - 1.0;
    var out: VsOut;
    out.position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    out.world = world;
    out.rect = q.rect;
    out.top = q.top;
    out.bottom = q.bottom;
    out.border = q.border;
    out.inset_color = q.inset_color;
    out.shadow_color = q.shadow_color;
    out.params = q.params;
    out.shadows = q.shadows;
    out.extra = q.extra;
    out.clip = q.clip;
    return out;
}

fn sd_round_rect(p: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let half = rect.zw * 0.5;
    let r = min(radius, min(half.x, half.y));
    let q = abs(p - rect.xy - half) - half + vec2(r);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn bayer4(p: vec2<f32>) -> f32 {
    let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let x = u32(p.x) % 4u;
    let y = u32(p.y) % 4u;
    return (m[y * 4u + x] + 0.5) / 16.0;
}

fn meter(local: vec2<f32>, size: vec2<f32>, level: f32) -> vec3<f32> {
    let device = floor(local * globals.scale);
    let u = local.x / max(size.x, 1.0);
    let fade = smoothstep(level - 0.02, level + 0.16, u);
    let lit = globals.tones[0].rgb;
    let checker = select(globals.tones[1].rgb, globals.tones[2].rgb, (u32(device.x + device.y) % 2u) == 0u);
    if bayer4(device) >= fade {
        return lit;
    }
    return checker;
}

fn palette(t: f32) -> vec3<f32> {
    let a = vec3(0.45, 0.95, 0.55);
    let b = vec3(0.85, 0.35, 0.95);
    let c = vec3(0.35, 0.85, 0.95);
    let k = fract(t) * 3.0;
    if k < 1.0 { return mix(a, b, k); }
    if k < 2.0 { return mix(b, c, k - 1.0); }
    return mix(c, a, k - 2.0);
}

fn hue(h: f32) -> vec3<f32> {
    let p = abs(fract(vec3(h) + vec3(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - vec3(3.0));
    return clamp(p - vec3(1.0), vec3(0.0), vec3(1.0));
}

fn ultra(local: vec2<f32>, size: vec2<f32>, phase: f32) -> vec4<f32> {
    let row = floor(local.y * globals.scale);
    let u = local.x / max(size.x, 1.0);
    let speed = 6.0 + 10.0 * hash(vec2(row, 7.0));
    let shift = hash(vec2(row, 3.0)) * 97.0 + globals.time * speed;
    let run_len = 2.0 + 5.0 * hash(vec2(row, 11.0));
    let run = floor((local.x + shift) / run_len);
    let n = hash(vec2(run, row));
    let lit_row = hash(vec2(row, 5.0)) < 0.8;
    let density = mix(0.04, 1.0, pow(u, 2.2));
    let on = lit_row && n < density;
    let tint = hue(phase + u * 0.4 + (hash(vec2(run, row + 0.5)) - 0.5) * 0.12);
    let color = mix(vec3(1.0), tint, 0.55) * (0.55 + 0.4 * hash(vec2(run + 0.3, row)));
    return vec4(select(globals.tones[3].rgb, color, on), 1.0);
}

fn sparkle(local: vec2<f32>, size: vec2<f32>, activity: f32) -> vec4<f32> {
    let device = floor(local * globals.scale);
    let center = size * 0.5;
    let edge = length((local - center) / (size * 0.5));
    let vignette = 1.0 - smoothstep(0.85, 1.2, edge);
    let checker = select(0.0, 1.0, (u32(device.x + device.y) % 2u) == 0u);
    var color = vec4(globals.tones[4].rgb, 0.75 * checker * vignette);
    let spin = globals.time * mix(0.35, 1.6, activity);
    for (var i = 0; i < 5; i = i + 1) {
        let fi = f32(i);
        let angle = spin + fi * 1.2566 + 0.35 * sin(globals.time * 0.7 + fi * 1.7);
        let radius = vec2(size.x * 0.38, size.y * 0.36) * (0.8 + 0.2 * sin(globals.time * 0.5 + fi));
        let p = center + vec2(cos(angle), sin(angle)) * radius;
        let d = length(local - p);
        let core = 1.0 - smoothstep(1.8, 2.8, d);
        let halo = (1.0 - smoothstep(2.8, 3.8, d)) * step(bayer4(device), 0.5);
        let dot = max(core, halo * 0.6);
        if dot > 0.01 {
            let tint = mix(vec3(1.0), hue(0.42 + fi * 0.07 + globals.time * 0.02), 0.35) * 0.78;
            color = vec4(mix(color.rgb, tint, dot), max(color.a, dot * 0.95));
        }
    }
    return color;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if any(in.world < in.clip.xy) || any(in.world > in.clip.xy + in.clip.zw) {
        discard;
    }
    let px = 1.0 / globals.scale;
    let radius = in.params.x;
    let opacity = in.params.z;
    let d = sd_round_rect(in.world, in.rect, radius);

    var shadow = vec4(0.0);
    if in.shadow_color.a > 0.0 {
        let blur = max(in.shadows.w, 0.5);
        let ds = sd_round_rect(in.world - vec2(0.0, in.shadows.z), in.rect, radius);
        shadow = vec4(in.shadow_color.rgb, in.shadow_color.a * (1.0 - smoothstep(-blur, blur, ds)));
    }

    let coverage = clamp(0.5 - d / px, 0.0, 1.0);
    let local = in.world - in.rect.xy;
    let t = clamp(local.y / max(in.rect.w, 1.0), 0.0, 1.0);
    var color = mix(in.top, in.bottom, t);

    let pattern = u32(in.params.w + 0.5);
    if pattern == 1u {
        color = vec4(meter(local, in.rect.zw, in.extra.x), 1.0);
    } else if pattern == 2u {
        color = ultra(local, in.rect.zw, in.extra.x);
    } else if pattern == 3u {
        color = sparkle(local, in.rect.zw, in.extra.x);
    } else if pattern == 4u {
        let band = 0.35;
        let phase = fract(globals.time * 0.65) * (1.0 + band * 2.0) - band;
        let u = local.x / max(in.rect.z, 1.0);
        let glow = exp(-pow((u - phase) / (band * 0.5), 2.0));
        color = vec4(color.rgb + vec3(globals.tones[5].x) * glow, color.a);
    } else if pattern == 5u {
        let n = hash(floor(local * globals.scale)) - 0.5;
        color = vec4(color.rgb + vec3(n * in.extra.x), color.a);
    }

    if in.inset_color.a > 0.0 {
        let blur = max(in.shadows.y, 0.5);
        let di = sd_round_rect(in.world - vec2(0.0, in.shadows.x), in.rect, radius);
        let amount = smoothstep(-blur, blur * 0.5, di) * in.inset_color.a;
        color = vec4(mix(color.rgb, in.inset_color.rgb, amount), color.a);
    }

    if in.params.y > 0.0 {
        let edge = clamp(0.5 - (abs(d + in.params.y * 0.5) - in.params.y * 0.5) / px, 0.0, 1.0);
        color = vec4(mix(color.rgb, in.border.rgb, edge * in.border.a), color.a);
    }

    let body = vec4(color.rgb, color.a * coverage);
    let out_a = body.a + shadow.a * (1.0 - body.a);
    if out_a <= 0.001 {
        discard;
    }
    let out_rgb = (body.rgb * body.a + shadow.rgb * shadow.a * (1.0 - body.a)) / out_a;
    return vec4(out_rgb, out_a * opacity);
}
