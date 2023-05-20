@group(0) @binding(0)
var input_texture: texture_3d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn comp_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    var workgroup_size = vec3(4u, 4u, 4u);
    var size = vec3<u32>(textureDimensions(output_texture));
    var levels = i32(round(log2(f32(size.x))));
    
    if (global_id.x >= size.x || global_id.y >= size.y || global_id.z >= size.z) {
        return;
    }
    
    var steps = size / workgroup_size;
    for (var x = 0u; x < steps.x; x++) {
        for (var y = 0u; y < steps.y; y++) {
            for (var z = 0u; z < steps.z; z++) {
                var average = vec4(0.0, 0.0, 0.0, 1.0);
                var pos = vec3<i32>(vec3(x, y, z) * workgroup_size + global_id);
                for (var dx = 0; dx < 2; dx++) {
                    for (var dy = 0; dy < 2; dy++) {
                        for (var dz = 0; dz < 2; dz++) {
                            var offset = pos * 2 + vec3(dx, dy, dz);
                            var color = textureLoad(input_texture, offset, 0);
                            average += color;
                        }
                    }
                }
                average /= 8.0;
                textureStore(output_texture, pos, average);
            }
        }
    }
}