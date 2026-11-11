// A height-displaced spherical mesh, rendered by wgpu inside egui.
// Map styling comes from the same native-resolution atlas that powers 2D.
struct Camera {
    angles: vec4<f32>,   // center lon/lat (radians), unused z/w
    shape: vec4<f32>,    // vertical exaggeration, radius meters, viewport scale
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var surface_map: texture_2d<f32>;
@group(0) @binding(2) var surface_sampler: sampler;

struct InVertex {
    @location(0) lon_lat: vec2<f32>,
    @location(1) elev_m: f32,
    @location(2) uv: vec2<f32>,
};
struct OutVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) visible_z: f32,
    @location(2) world_normal: vec3<f32>,
};

@vertex
fn vs_main(v: InVertex) -> OutVertex {
    let latitude = v.lon_lat.y;
    let longitude = v.lon_lat.x - camera.angles.x;
    let center_lat = camera.angles.y;
    let cp = cos(latitude);
    let sp = sin(latitude);
    let cl = cos(longitude);
    let sl = sin(longitude);
    let c0 = cos(center_lat);
    let s0 = sin(center_lat);
    let view_direction = vec3<f32>(
        cp * sl,
        sp * c0 - cp * s0 * cl,
        sp * s0 + cp * c0 * cl
    );
    let radius = max(
        0.90,
        1.0 + v.elev_m / camera.shape.y * camera.shape.x
    );
    var out: OutVertex;
    out.clip = vec4<f32>(
        view_direction.x * radius * camera.shape.z,
        view_direction.y * radius * camera.shape.z,
        (1.0 - view_direction.z) * 0.5,
        1.0
    );
    out.visible_z = view_direction.z;
    out.world_normal = view_direction;
    out.uv = v.uv;
    return out;
}
@fragment
fn fs_main(v: OutVertex) -> @location(0) vec4<f32> {
    // No depth attachment in egui's UI pass. For this star-shaped globe the
    // back hemisphere has no visible surface, so discard it before coloring.
    if (v.visible_z < -0.005) { discard; }
    let color = textureSample(surface_map, surface_sampler, v.uv);
    let lamp = normalize(vec3<f32>(-0.30, 0.45, 0.84));
    let illum = clamp(0.67 + 0.34 * dot(normalize(v.world_normal), lamp), 0.38, 1.08);
    return vec4<f32>(color.rgb * illum, 1.0);
}
