struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    time: f32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct SpriteIn {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) clip: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) clip: vec4<f32>,
    @location(5) rect: vec4<f32>,
};

fn corner(vertex: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    return corners[vertex];
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, s: SpriteIn) -> VsOut {
    let c = corner(vertex);
    let world = s.rect.xy + c * s.rect.zw;
    let ndc = world / globals.viewport * 2.0 - 1.0;
    var out: VsOut;
    out.position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    out.world = world;
    out.uv = s.uv.xy + c * s.uv.zw;
    out.color = s.color;
    out.params = s.params;
    out.clip = s.clip;
    out.rect = s.rect;
    return out;
}

fn sample_blurred(uv: vec2<f32>, blur: f32) -> vec4<f32> {
    if blur < 0.25 {
        return textureSampleLevel(tex, samp, uv, 0.0);
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(tex));
    let step = blur * 0.5;
    var acc = vec4(0.0);
    var total = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let o = vec2(f32(x), f32(y));
            let w = exp(-dot(o, o) / 4.5);
            acc += textureSampleLevel(tex, samp, uv + o * step * texel, 0.0) * w;
            total += w;
        }
    }
    return acc / total;
}

fn sd_round_rect(p: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let half = rect.zw * 0.5;
    let r = min(radius, min(half.x, half.y));
    let q = abs(p - rect.xy - half) - half + vec2(r);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if any(in.world < in.clip.xy) || any(in.world > in.clip.xy + in.clip.zw) {
        discard;
    }
    let kind = u32(in.params.x + 0.5);
    let texel = sample_blurred(in.uv, in.params.y);
    var out: vec4<f32>;
    if kind == 0u {
        out = vec4(in.color.rgb, in.color.a * texel.r);
    } else if kind == 1u && in.params.w > 0.5 {
        out = vec4(in.color.rgb, in.color.a * texel.a);
    } else {
        out = vec4(texel.rgb, texel.a * in.color.a);
    }
    if in.params.z > 0.0 {
        let d = sd_round_rect(in.world, in.rect, in.params.z);
        out.a *= clamp(0.5 - d * globals.scale, 0.0, 1.0);
    }
    if out.a <= 0.001 {
        discard;
    }
    return out;
}
