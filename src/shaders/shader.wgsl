struct MaterialData {
    base_color_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
    alpha_cut_off: f32,
    filler: u32,
}

struct Light {
    position: vec4<f32>,
    intensity: vec3<f32>,
    falloff: f32,
}

struct Lights {
    filler: vec3<i32>,
    count: i32,
    lights: array<Light, 8>,
}

@group(0) @binding(0)
var<uniform> view_projection: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> shadow_view_projection: mat4x4<f32>;

@group(1) @binding(0)
var shadow_texture: texture_depth_2d;
@group(1) @binding(1)
var shadow_sampler: sampler_comparison;
@group(1) @binding(2)
var storage_texture: texture_3d<f32>;
@group(1) @binding(3)
var storage_sampler: sampler;
@group(1) @binding(4)
var<uniform> lights: Lights;


@group(2) @binding(0)
var<uniform> model: mat4x4<f32>;
@group(3) @binding(0)
var<uniform> material: MaterialData;
@group(3) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(3) @binding(2)
var base_color_sampler: sampler;
@group(3) @binding(3)
var metallic_roughness_texture: texture_2d<f32>;
@group(3) @binding(4)
var metallic_roughness_sampler: sampler;
@group(3) @binding(5)
var normal_texture: texture_2d<f32>;
@group(3) @binding(6)
var normal_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
}
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) shadow_clip_position: vec4<f32>,
    @location(3) model_pos: vec3<f32>,
}

@vertex
fn vs_main(
    input: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.model_pos = (model * vec4<f32>(input.position, 1.0)).xyz;
    out.clip_position = view_projection * vec4<f32>(out.model_pos, 1.0);
    out.normal = input.normal;
    out.tex_coords = input.tex_coords;
    out.shadow_clip_position = shadow_view_projection * vec4<f32>(out.model_pos, 1.0);
    return out;
}

fn cone_trace(start: vec3<f32>, direction: vec3<f32>, slope: f32, steps: i32) -> vec4<f32> {
    var indirect_light = vec4(0.0, 0.0, 0.0, 1.0);
    var dist: f32 = 0.3;
    for (var i = 0; i < steps; i++) {
        var radius = slope * dist;
        var level = log2(radius / 40.0 * 512.0) + 1.0;
        var sample_color = textureSampleLevel(
            storage_texture,
            storage_sampler,
            (start + direction * dist) / 20.0 * 0.5 + 0.5,
            level
        );
        if sample_color.a <= 0.01 {
            continue;
        }

        sample_color /= sample_color.a;
        var alpha_blending_fac = (1.0 - indirect_light.a) * sample_color.a;
        // indirect_light += vec4(alpha_blending_fac * sample_color.rgb, alpha_blending_fac);
        indirect_light += sample_color;

        dist += 0.3;
    }
    return indirect_light;
}

fn uchimura(x: vec3<f32>, P: f32, a: f32, m: f32, l: f32, c: f32, b: f32) -> vec3<f32> {
    var l0 = ((P - m) * l) / a;
    var L0 = m - m / a;
    var L1 = m + (1.0 - m) / a;
    var S0 = m + l0;
    var S1 = m + a * l0;
    var C2 = (a * P) / (P - S1);
    var CP = -C2 / P;
    var w0 = vec3(1.0 - smoothstep(vec3(0.0), vec3(m), x));
    var w2 = vec3(step(vec3(m + l0), x));
    var w1 = vec3(1.0 - w0 - w2);
    var T = vec3(m * pow(x / m, vec3(c)) + b);
    var S = vec3(P - (P - S1) * exp(CP * (x - S0)));
    var L = vec3(m + a * (x - m));
    return T * w0 + L * w1 + S * w2;
}

fn uchimura_fixed(x: vec3<f32>) -> vec3<f32> {
    var P = 1.0;  // max display brightness
    var a = 1.0;  // contrast
    var m = 0.22; // linear section start
    var l = 0.4;  // linear section length
    var c = 1.33; // black
    var b = 0.0;  // pedestal

    return uchimura(x, P, a, m, l, c, b);
}

struct Hit {
    normal: vec3<f32>,
    color: vec3<f32>,
}

fn trace(origin: vec3<f32>, direction: vec3<f32>) -> Hit {
    var step_sizes = 1.0 / abs(direction);
    var step_dir = sign(direction);
    var next_dist = (step_dir * 0.5 + 0.5 - fract(origin)) / direction;
    var voxel_pos = floor(origin);
    var curr_pos = origin;
    for (var i = 0; i < 800; i++) {
        var closest_dist = min(next_dist.x, min(next_dist.y, next_dist.z));
        curr_pos += direction * closest_dist;
        var step_axis: vec3<f32>;
        if closest_dist == next_dist.x {
            step_axis = vec3(1.0, 0.0, 0.0);
        } else if closest_dist == next_dist.y {
            step_axis = vec3(0.0, 1.0, 0.0);
        } else {
            step_axis = vec3(0.0, 0.0, 1.0);
        }
        voxel_pos += step_axis * step_dir;
        next_dist -= closest_dist;
        next_dist += step_sizes * step_axis;
        var normal = -step_dir * step_axis;

        var color = textureLoad(storage_texture, vec3<i32>(round(voxel_pos)) + 256, 0);
        if color.a > 0.1 {
            return Hit(normal, color.xyz);
        }
    }
    return Hit(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, 0.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var ray_origin = vec3(-1.8, 3.155, -0.3) / 20.0 * 256.0;
    var ray_dir = normalize(in.model_pos / 20.0 * 256.0 - ray_origin);

    var shadow_screen_pos = in.shadow_clip_position.xyz / in.shadow_clip_position.w * vec3<f32>(0.5, -0.5, 1.0) + vec3<f32>(0.5, 0.5, 0.0);
    var shadow = textureSampleCompare(shadow_texture, shadow_sampler, shadow_screen_pos.xy, shadow_screen_pos.z - 0.002);
    var color = textureSample(base_color_texture, base_color_sampler, in.tex_coords) * material.base_color_factor;

    var indirect_light = vec4(0.0, 0.0, 0.0, 0.0);
    var PI = 3.141592654;
    var ANGLE = PI / 6.0;
    var slope = tan(ANGLE);

    var tangent = vec3(1.0, 0.0, 0.0);
    var bitangent = vec3(0.0, 0.0, 1.0);
    var normal = in.normal;
    if normal.y < 0.99 && normal.y > -0.99 {
        var tangent = normalize(cross(vec3(0.0, 1.0, 0.0), in.normal));
        var bitangent = normalize(cross(tangent, in.normal));
    }

    indirect_light += cone_trace(in.model_pos, normal, slope, 8);
    indirect_light += cone_trace(in.model_pos, 0.866 * tangent + 0.500 * normal + 0.000 * bitangent, slope, 8);
    indirect_light += cone_trace(in.model_pos, 0.433 * tangent + 0.500 * normal + 0.750 * bitangent, slope, 8);
    indirect_light += cone_trace(in.model_pos, -0.433 * tangent + 0.500 * normal + 0.750 * bitangent, slope, 8);
    indirect_light += cone_trace(in.model_pos, -0.866 * tangent + 0.500 * normal + 0.000 * bitangent, slope, 8);
    indirect_light += cone_trace(in.model_pos, -0.433 * tangent + 0.500 * normal + -0.750 * bitangent, slope, 8);
    indirect_light += cone_trace(in.model_pos, 0.433 * tangent + 0.500 * normal + -0.750 * bitangent, slope, 8);

    var metallic_roughness = textureSample(metallic_roughness_texture, metallic_roughness_sampler, in.tex_coords);
    var roughness = metallic_roughness.g;

    var direct_light_contribution = vec3(0.0, 0.0, 0.0);
    var sun = lights.lights[0];
    var diffuse = clamp(dot(-sun.position.xyz, in.normal), 0.0, 1.0);
    direct_light_contribution += shadow * diffuse * sun.intensity;

    for (var i = 1; i < lights.count; i++) {
        var light = lights.lights[i];
        var diff = light.position.xyz - in.model_pos * light.position.w;
        var dist = length(diff);
        diff /= dist;
        var lambertian = clamp(dot(diff, in.normal), 0.0, 1.0);
        var falloff_amount = pow(dist, light.falloff);
        direct_light_contribution += light.intensity * lambertian / falloff_amount;
    }

    if color.a < material.alpha_cut_off {
        discard;
    }
    color *= vec4(direct_light_contribution + indirect_light.rgb, 1.0);
    color = vec4(uchimura_fixed(color.rgb), 1.0);
    return color;
}