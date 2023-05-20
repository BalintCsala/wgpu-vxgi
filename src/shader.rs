#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Attribute {
    Positions = 0,
    TexCoords = 1,
    Normals = 2,
    Tangents = 3,
    Joints = 4,
    Weights = 5,
    Colors = 6,
    Unknown = -1,
}

pub struct Shader {
    pub vs_entry: String,
    pub fs_entry: String,
    pub module: wgpu::ShaderModule,
}
