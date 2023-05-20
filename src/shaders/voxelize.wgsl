
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
var storage_texture: texture_storage_3d<rgba16float, write>;
@group(1) @binding(3)
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
};

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
    var abs_normal = abs(input.normal);
    if abs_normal.x > abs_normal.y && abs_normal.x > abs_normal.z {
        out.clip_position = vec4(out.model_pos.yzx / vec3(20.0, 20.0, 20.0), 1.0);
    } else if abs_normal.y > abs_normal.z {
        out.clip_position = vec4(out.model_pos.xzy / vec3(20.0, 20.0, 20.0), 1.0);
    } else {
        out.clip_position = vec4(out.model_pos.zyx / vec3(20.0, 20.0, 20.0), 1.0);
    }
    out.clip_position.z *= 0.5;
    out.clip_position.z += 0.5;
    return out;
}


@fragment
fn fs_main(in: VertexOutput) {
    var shadow_screen_pos = in.shadow_clip_position.xyz / in.shadow_clip_position.w * vec3<f32>(0.5, -0.5, 1.0) + vec3<f32>(0.5, 0.5, 0.0);
    var shadow = textureSampleCompare(shadow_texture, shadow_sampler, shadow_screen_pos.xy, shadow_screen_pos.z - 0.004);
    var color = textureSample(base_color_texture, base_color_sampler, in.tex_coords) * material.base_color_factor;
    if color.a < material.alpha_cut_off {
        discard;
    }

    var direct_light_contribution = vec3(0.0, 0.0, 0.0);
    var sun = lights.lights[0];
    var diffuse = clamp(dot(-sun.position.xyz, in.normal), 0.0, 1.0);
    direct_light_contribution += clamp(shadow * diffuse, 0.0, 1.0) * sun.intensity;
    
    // for (var i = 1; i < lights.count; i++) {
    //     var light = lights.lights[i];
    //     var diff = light.position.xyz - in.model_pos * light.position.w;
    //     var dist = length(diff);
    //     diff /= dist;
    //     var lambertian = clamp(dot(diff, in.normal), 0.0, 1.0);
    //     var falloff_amount = pow(dist, light.falloff);
    //     direct_light_contribution += light.intensity * lambertian / falloff_amount;
    // }

    color *= vec4(direct_light_contribution, 1.0);
    var texture_size = vec3<f32>(textureDimensions(storage_texture));
    var pixel_pos = vec3<i32>(in.model_pos / 20.0 * texture_size / 2.0 + texture_size / 2.0);
    textureStore(storage_texture, pixel_pos, color);
}