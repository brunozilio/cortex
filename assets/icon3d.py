"""Superfik icon, rendered in Blender (Cycles) from measured geometry.

    blender -b -P assets/icon3d.py -- assets/icon.png [seed] [samples] [fill]

The mark is the light ring and the two eyes on a dark brushed-metal tile
(macOS's rounded-square icon grid); pass `fill` to also black out the
ring's inside.

`icon_measure.json` (next to this script) holds the silhouette, ring path
and ring hues sampled per degree from the reference artwork. World units:
1 unit = 100 px of the 1024 canvas, origin at the canvas centre, +y up.
"""
import sys, os, math, json, random, colorsys
import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = argv[0] if argv else "/tmp/icon3d.png"
SEED = int(argv[1]) if len(argv) > 1 else 5
SAMPLES = int(argv[2]) if len(argv) > 2 else 256
FILL = len(argv) > 3 and argv[3] == "fill"
random.seed(SEED)

HERE = os.path.dirname(os.path.abspath(__file__))
M = json.load(open(os.path.join(HERE, "icon_measure.json")))
K = 1.42  # reference px -> canvas px
BX, BY = M["rcx"], M["rcy"]  # reference ring centre -> canvas centre

def canvas(x, y):
    return 512 + (x - BX) * K, 512 + (y - BY) * K

def W(x, y):
    """canvas px -> world xy."""
    return ((x - 512) / 100.0, -(y - 512) / 100.0)

def smooth(vals, win):
    n = len(vals)
    return [sum(vals[(i + j) % n] for j in range(-win, win + 1)) / (2 * win + 1) for i in range(n)]

OUTLINE = smooth(M["outline"], 2)
RING_R = smooth([r["d"] for r in M["ring"]], 4)
RING_IN = smooth([r["dmin"] for r in M["ring"]], 4)
RING_OUT = smooth([r["dmax"] for r in M["ring"]], 4)
RCX, RCY = M["rcx"], M["rcy"]

def ring_hue(deg):
    """Circular-mean hue around `deg` (reference image angle, y down)."""
    hx = hy = 0.0
    for j in range(-6, 7):
        h = M["ring"][int(deg + j) % 360]["hue"]
        hx += math.cos(2 * math.pi * h); hy += math.sin(2 * math.pi * h)
    return (math.atan2(hy, hx) / (2 * math.pi)) % 1.0

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = "CYCLES"
scene.render.resolution_x = scene.render.resolution_y = 1024
scene.render.resolution_percentage = 100
scene.render.film_transparent = True
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.filepath = OUT
scene.cycles.samples = SAMPLES
scene.cycles.use_denoising = True
scene.cycles.max_bounces = 8
scene.cycles.transparent_max_bounces = 32
scene.view_settings.view_transform = "Standard"
scene.view_settings.look = "None"
try:
    prefs = bpy.context.preferences.addons["cycles"].preferences
    prefs.compute_device_type = "METAL"
    prefs.get_devices()
    for d in prefs.devices:
        d.use = True
    scene.cycles.device = "GPU"
except Exception as e:  # noqa: BLE001
    print("GPU setup failed, using CPU:", e)

world = bpy.data.worlds.new("World")
scene.world = world
world.use_nodes = True
bg = world.node_tree.nodes["Background"]
bg.inputs[0].default_value = (0.05, 0.05, 0.055, 1)

cam_data = bpy.data.cameras.new("Cam")
cam_data.type = "ORTHO"
cam_data.ortho_scale = 10.24
cam = bpy.data.objects.new("Cam", cam_data)
cam.location = (0, 0, 20)
scene.collection.objects.link(cam)
scene.camera = cam

def area_light(name, loc, energy, size, color=(1, 1, 1)):
    ld = bpy.data.lights.new(name, "AREA")
    ld.energy = energy
    ld.size = size
    ld.color = color
    ob = bpy.data.objects.new(name, ld)
    ob.location = loc
    ob.rotation_euler = (Vector((0, 0, 0)) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
    scene.collection.objects.link(ob)
    return ob

area_light("Key", (-7, 8, 6), 600, 7, (1.0, 0.98, 0.95))
area_light("Rim", (6, -7, 5), 120, 6, (0.9, 0.95, 1.0))
card = area_light("Card", (0, 10, 7), 500, 14)
card.data.shape = "RECTANGLE"
card.data.size_y = 3

def principled(name, **kw):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes["Principled BSDF"]
    for k, v in kw.items():
        p.inputs[k].default_value = v
    return m, p

def outline_pt(deg, inset=0.0):
    a = math.radians(deg)
    r = OUTLINE[int(deg) % 360] - inset
    return canvas(BX + math.cos(a) * r, BY + math.sin(a) * r)

def polar_curve(name, pts, z, extrude, bevel, dims="2D", cyclic=True, tilt=None, radii=None):
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = dims
    cu.fill_mode = "BOTH" if dims == "2D" else "FULL"
    cu.extrude = extrude
    cu.bevel_depth = bevel
    cu.bevel_resolution = 8
    cu.resolution_u = 3
    sp = cu.splines.new("NURBS")
    sp.points.add(len(pts) - 1)
    sp.order_u = 4
    sp.use_cyclic_u = cyclic
    sp.use_endpoint_u = not cyclic
    for i, (pt, (x, y)) in enumerate(zip(sp.points, pts)):
        pt.co = (*W(x, y), 0, 1.0)
        if tilt is not None:
            pt.tilt = tilt
        if radii is not None:
            pt.radius = radii[i]
    sp.use_smooth = True
    ob = bpy.data.objects.new(name, cu)
    ob.location = (0, 0, z)
    scene.collection.objects.link(ob)
    return ob

def smooth_xy(pts, win):
    n = len(pts)
    return [(sum(pts[(i + j) % n][0] for j in range(-win, win + 1)) / (2 * win + 1),
             sum(pts[(i + j) % n][1] for j in range(-win, win + 1)) / (2 * win + 1)) for i in range(n)]

def rounded_square(name, half, r, z, extrude, bevel):
    k = 0.5523 * r
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "2D"
    cu.fill_mode = "BOTH"
    cu.extrude = extrude
    cu.bevel_depth = bevel
    cu.bevel_resolution = 10
    cu.offset = -bevel
    cu.resolution_u = 24
    sp = cu.splines.new("BEZIER")
    pts = [((-half + r, -half), (-half + r - k, -half), (-half + r, -half)),
           ((half - r, -half), (half - r, -half), (half - r + k, -half)),
           ((half, -half + r), (half, -half + r - k), (half, -half + r)),
           ((half, half - r), (half, half - r), (half, half - r + k)),
           ((half - r, half), (half - r + k, half), (half - r, half)),
           ((-half + r, half), (-half + r, half), (-half + r - k, half)),
           ((-half, half - r), (-half, half - r + k), (-half, half - r)),
           ((-half, -half + r), (-half, -half + r), (-half, -half + r - k))]
    sp.bezier_points.add(len(pts) - 1)
    for bp, (a, hl, hr) in zip(sp.bezier_points, pts):
        bp.co = (a[0], -a[1], 0)
        bp.handle_left = (hl[0], -hl[1], 0)
        bp.handle_right = (hr[0], -hr[1], 0)
        bp.handle_left_type = bp.handle_right_type = "FREE"
    sp.use_cyclic_u = True
    sp.use_smooth = True
    ob = bpy.data.objects.new(name, cu)
    ob.location = (0, 0, z)
    scene.collection.objects.link(ob)
    return ob

TILE_HALF, TILE_R = 4.12, 4.12 * 0.2237 * 2
tile = rounded_square("Tile", TILE_HALF, TILE_R, 0.0, 0.2, 0.07)
tile_mat, _ = principled("Tile", **{
    "Base Color": (0.07, 0.07, 0.075, 1), "Metallic": 0.9, "Roughness": 0.34, "Anisotropic": 0.7,
    "Anisotropic Rotation": 0.25, "Coat Weight": 0.25, "Coat Roughness": 0.2})
tile.data.materials.append(tile_mat)
nt = tile_mat.node_tree
tex = nt.nodes.new("ShaderNodeTexCoord")
mapping = nt.nodes.new("ShaderNodeMapping")
mapping.inputs["Scale"].default_value = (1.0, 60.0, 1.0)  # stretched sideways: brushed streaks
nt.links.new(tex.outputs["Object"], mapping.inputs["Vector"])
noise = nt.nodes.new("ShaderNodeTexNoise")
noise.inputs["Scale"].default_value = 6.0
noise.inputs["Detail"].default_value = 3.0
nt.links.new(mapping.outputs["Vector"], noise.inputs["Vector"])
ramp = nt.nodes.new("ShaderNodeMapRange")
ramp.inputs["From Min"].default_value = 0.3; ramp.inputs["From Max"].default_value = 0.7
ramp.inputs["To Min"].default_value = 0.3; ramp.inputs["To Max"].default_value = 0.4
nt.links.new(noise.outputs["Fac"], ramp.inputs["Value"])
nt.links.new(ramp.outputs["Result"], nt.nodes["Principled BSDF"].inputs["Roughness"])

Z_TOP = 0.27  # tile extrude + bevel

ring_centre = canvas(RCX, RCY)
ring_empty = bpy.data.objects.new("RingCentre", None)
ring_empty.location = (*W(*ring_centre), 0)
scene.collection.objects.link(ring_empty)

def ring_pt(t, scale=1.0, offset=0.0):
    """Point on the measured ring, t in [0,1) by reference angle; offset in canvas px."""
    deg = (t % 1.0) * 360.0
    i = int(deg) % 360
    f = deg - int(deg)
    r = RING_R[i] * (1 - f) + RING_R[(i + 1) % 360] * f
    r = r * scale + offset / K
    a = math.radians(deg)
    return canvas(RCX + math.cos(a) * r, RCY + math.sin(a) * r)

def normal_at(t, scale):
    x, y = ring_pt(t, scale)
    x2, y2 = ring_pt(t + 0.002, scale)
    dx, dy = x2 - x, y2 - y
    L = math.hypot(dx, dy) or 1
    return x, y, -dy / L, dx / L

def ribbon_material():
    m = bpy.data.materials.new("Ribbon")
    m.use_nodes = True
    nt = m.node_tree
    n = nt.nodes
    p = n["Principled BSDF"]
    tc = n.new("ShaderNodeTexCoord"); tc.object = ring_empty
    sep = n.new("ShaderNodeSeparateXYZ")
    nt.links.new(tc.outputs["Object"], sep.inputs[0])
    atan = n.new("ShaderNodeMath"); atan.operation = "ARCTAN2"
    nt.links.new(sep.outputs["Y"], atan.inputs[0])
    nt.links.new(sep.outputs["X"], atan.inputs[1])
    neg = n.new("ShaderNodeMath"); neg.operation = "MULTIPLY"; neg.inputs[1].default_value = -1.0
    nt.links.new(atan.outputs[0], neg.inputs[0])
    oi0 = n.new("ShaderNodeObjectInfo")
    jit = n.new("ShaderNodeMath"); jit.operation = "MULTIPLY_ADD"
    jit.inputs[1].default_value = 0.16; jit.inputs[2].default_value = -0.08
    nt.links.new(oi0.outputs["Random"], jit.inputs[0])
    add = n.new("ShaderNodeMath"); add.operation = "ADD"
    nt.links.new(neg.outputs[0], add.inputs[0]); nt.links.new(jit.outputs[0], add.inputs[1])
    wrap = n.new("ShaderNodeMath"); wrap.operation = "WRAP"
    wrap.inputs[1].default_value = 0.0; wrap.inputs[2].default_value = 2 * math.pi
    nt.links.new(add.outputs[0], wrap.inputs[0])
    div = n.new("ShaderNodeMath"); div.operation = "DIVIDE"; div.inputs[1].default_value = 2 * math.pi
    nt.links.new(wrap.outputs[0], div.inputs[0])
    ramp = n.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "LINEAR"
    els = ramp.color_ramp.elements
    while len(els) > 1:
        els.remove(els[-1])
    steps = 30
    for i in range(steps + 1):
        t = i / steps
        el = els[0] if i == 0 else els.new(min(t, 1.0))
        el.position = min(t, 1.0)
        r, g, b = colorsys.hsv_to_rgb(ring_hue(t * 360), 0.95, 1.0)
        el.color = (r, g, b, 1)
    nt.links.new(div.outputs[0], ramp.inputs[0])
    oi = n.new("ShaderNodeObjectInfo")
    mul = n.new("ShaderNodeMix"); mul.data_type = "RGBA"; mul.blend_type = "MULTIPLY"
    mul.inputs[0].default_value = 1.0
    nt.links.new(ramp.outputs["Color"], mul.inputs[6])
    nt.links.new(oi.outputs["Color"], mul.inputs[7])
    nt.links.new(mul.outputs[2], p.inputs["Emission Color"])
    nt.links.new(mul.outputs[2], p.inputs["Base Color"])
    p.inputs["Emission Strength"].default_value = 0.95
    p.inputs["Roughness"].default_value = 0.2
    p.inputs["Anisotropic"].default_value = 0.8
    p.inputs["Coat Weight"].default_value = 1.0
    p.inputs["Coat Roughness"].default_value = 0.08
    p.inputs["Alpha"].default_value = 0.7
    return m

if FILL:
    fill_pts = [ring_pt(i / 180, 0.93) for i in range(180)]
    fill = polar_curve("Fill", fill_pts, Z_TOP - 0.02, 0.01, 0.0)
    fill_mat, _ = principled("Fill", **{"Base Color": (0.003, 0.003, 0.003, 1), "Roughness": 0.4})
    fill.data.materials.append(fill_mat)

RIB_MAT = ribbon_material()
Z_RING = Z_TOP + 0.02

def ribbon(name, t0, length, scale, width, amp, freq, ph, zamp, bright, twist, taper=0.5, n=None, offset=0.0):
    n = n or max(24, int(length * 160))
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "3D"
    cu.fill_mode = "FULL"
    cu.extrude = width / 2 / 100.0
    cu.bevel_depth = 0.005
    cu.bevel_resolution = 2
    cu.resolution_u = 6
    cu.use_fill_caps = True
    sp = cu.splines.new("NURBS")
    sp.points.add(n)
    sp.order_u = 4
    sp.use_endpoint_u = True
    full = length >= 1.0 and taper == 0.0
    for i, pt in enumerate(sp.points):
        u = i / n
        t = t0 + u * length
        x, y, nx, ny = normal_at(t, scale)
        env = 1.0 if full else math.sin(math.pi * u)
        o = offset + amp * env * math.sin(2 * math.pi * freq * u + ph)
        z = Z_RING + zamp * env * math.sin(2 * math.pi * freq * u + ph + 1.3) / 100.0
        wx, wy = W(x + nx * o, y + ny * o)
        pt.co = (wx, wy, z, 1.0)
        pt.radius = 1.0 if full else max(0.04, math.sin(math.pi * u) ** taper)
        pt.tilt = math.pi / 2 + twist * math.sin(math.pi * u) * 0.6
    sp.use_smooth = True
    ob = bpy.data.objects.new(name, cu)
    ob.color = (bright, bright, bright, 1)
    scene.collection.objects.link(ob)
    ob.data.materials.append(RIB_MAT)
    return ob

BAND = (sum(RING_OUT) - sum(RING_IN)) / 360 * K  # measured band width, canvas px
ribbon("Core", 0.0, 1.0, 1.0, BAND * 0.46, 0, 1, 0, 0, 0.8, 0, taper=0.0, n=260)
ribbon("Core2", 0.0, 1.0, 1.0, 4.5, 0, 1, 0, 0.8, 1.25, 0, taper=0.0, n=260, offset=-BAND * 0.08)
for k in range(8):
    ribbon(f"Loop{k}", random.random(), 1.0, 1.0, random.uniform(2.5, 6), random.uniform(0.25, 0.55) * BAND,
           random.choice([1, 2]), random.uniform(0, 6.28), random.uniform(2, 8), random.uniform(0.6, 0.95),
           random.uniform(-1.0, 1.0), taper=0.0, n=260)
for k in range(4):
    ribbon(f"Arc{k}", random.random(), random.uniform(0.35, 0.7), 1.0, random.uniform(4, 9),
           random.uniform(0.3, 0.6) * BAND, random.uniform(0.5, 1.0), random.uniform(0, 6.28),
           random.uniform(2, 8), random.uniform(0.6, 1.0), random.uniform(-1.0, 1.0), taper=1.0)

def pill(name, cx, cy, w, h, z, depth, mat):
    rot = w > h
    if rot:
        w, h = h, w
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "2D"
    cu.fill_mode = "BOTH"
    cu.extrude = depth
    cu.bevel_depth = depth * 0.9
    cu.bevel_resolution = 6
    sp = cu.splines.new("BEZIER")
    r = w / 2
    k = 0.5523 * r
    top, bot = cy - h / 2 + r, cy + h / 2 - r
    pts = [((cx - r, top), (cx - r, top), (cx - r, top - k)),
           ((cx, top - r), (cx - k, top - r), (cx + k, top - r)),
           ((cx + r, top), (cx + r, top - k), (cx + r, top)),
           ((cx + r, bot), (cx + r, bot), (cx + r, bot + k)),
           ((cx, bot + r), (cx + k, bot + r), (cx - k, bot + r)),
           ((cx - r, bot), (cx - r, bot + k), (cx - r, bot))]
    sp.bezier_points.add(len(pts) - 1)
    def R(p):
        return (cx + (p[1] - cy), cy - (p[0] - cx)) if rot else p
    for bp, (a, hl, hr) in zip(sp.bezier_points, pts):
        bp.co = (*W(*R(a)), 0)
        bp.handle_left = (*W(*R(hl)), 0)
        bp.handle_right = (*W(*R(hr)), 0)
        bp.handle_left_type = bp.handle_right_type = "FREE"
    sp.use_cyclic_u = True
    sp.use_smooth = True
    ob = bpy.data.objects.new(name, cu)
    ob.location = (0, 0, z)
    scene.collection.objects.link(ob)
    ob.data.materials.append(mat)
    return ob

eye_mat, ep = principled("Eye", **{"Base Color": (1, 1, 1, 1), "Emission Color": (1, 1, 1, 1),
                                   "Emission Strength": 0.85})
E = M["eyes"]
ey = (E["y"][0] + E["y"][1]) / 2
eh = (E["y"][1] - E["y"][0]) * K
for name, (x0, x1) in (("EyeL", E["l"]), ("EyeR", E["r"])):
    cx, cy = canvas((x0 + x1) / 2, ey)
    pill(name, cx, cy, (x1 - x0) * K, eh, Z_RING, 0.02, eye_mat)

try:
    ng = bpy.data.node_groups.new("Comp", "CompositorNodeTree")
    ng.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    out = ng.nodes.new("NodeGroupOutput")
    rl = ng.nodes.new("CompositorNodeRLayers"); rl.scene = scene
    glare = ng.nodes.new("CompositorNodeGlare")
    for cand in ("Bloom", "BLOOM"):
        try:
            glare.inputs["Type"].default_value = cand; break
        except Exception:
            pass
    for k, v in (("Threshold", 0.95), ("Strength", 0.12), ("Size", 0.5)):
        if k in glare.inputs:
            glare.inputs[k].default_value = v
    sa = ng.nodes.new("CompositorNodeSetAlpha")
    ng.links.new(rl.outputs["Image"], glare.inputs["Image"])
    ng.links.new(glare.outputs["Image"], sa.inputs["Image"])
    ng.links.new(rl.outputs["Alpha"], sa.inputs["Alpha"])
    ng.links.new(sa.outputs["Image"], out.inputs[0])
    scene.compositing_node_group = ng
    scene.render.use_compositing = True
except Exception as e:  # noqa: BLE001
    print("bloom: skipped:", e)

bpy.ops.render.render(write_still=True)
print("wrote", OUT)
