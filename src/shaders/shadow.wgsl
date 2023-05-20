struct MaterialData {
    base_color_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
    alpha_cut_off: f32,
    filler: u32,
}
@group(0) @binding(0)
var<uniform> view_projection: mat4x4<f32>;
@group(1) @binding(0)
var<uniform> model: mat4x4<f32>;
@group(2) @binding(0)
var<uniform> material: MaterialData;
@group(2) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(2) @binding(2)
var base_color_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) texCoords: vec2<f32>,
}
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) texCoords: vec2<f32>,
};

@vertex
fn vs_main(
    input: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view_projection * model * vec4<f32>(input.position, 1.0);
    out.texCoords = input.texCoords;
    return out;
}


@fragment
fn fs_main(in: VertexOutput) {
    var color = textureSample(base_color_texture, base_color_sampler, in.texCoords) * material.base_color_factor;
    if (color.a < material.alpha_cut_off) {
        discard;
    }
}