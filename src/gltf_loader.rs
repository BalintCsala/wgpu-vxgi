use std::{collections::HashMap, iter::zip, path::Path};

use crate::{
    shader::{Attribute, Shader},
    texture::Texture, console_log,
};
use cgmath::{Matrix4, SquareMatrix};
use futures::future::join_all;
use gltf::{accessor::Dimensions, buffer::View, Node};
use wgpu::util::DeviceExt;

fn gltf_accessor_to_wgpu(accessor: &gltf::Accessor) -> Option<wgpu::VertexFormat> {
    let normalized = accessor.normalized();

    match (normalized, accessor.dimensions(), accessor.data_type()) {
        (false, Dimensions::Vec2, gltf::accessor::DataType::U8) => {
            Some(wgpu::VertexFormat::Uint8x2)
        }
        (false, Dimensions::Vec4, gltf::accessor::DataType::U8) => {
            Some(wgpu::VertexFormat::Uint8x4)
        }
        (false, Dimensions::Vec2, gltf::accessor::DataType::I8) => {
            Some(wgpu::VertexFormat::Sint8x2)
        }
        (false, Dimensions::Vec4, gltf::accessor::DataType::I8) => {
            Some(wgpu::VertexFormat::Sint8x4)
        }
        (true, Dimensions::Vec2, gltf::accessor::DataType::U8) => {
            Some(wgpu::VertexFormat::Unorm8x2)
        }
        (true, Dimensions::Vec4, gltf::accessor::DataType::U8) => {
            Some(wgpu::VertexFormat::Unorm8x4)
        }
        (true, Dimensions::Vec2, gltf::accessor::DataType::I8) => {
            Some(wgpu::VertexFormat::Snorm8x2)
        }
        (true, Dimensions::Vec4, gltf::accessor::DataType::I8) => {
            Some(wgpu::VertexFormat::Snorm8x4)
        }
        (false, Dimensions::Vec2, gltf::accessor::DataType::U16) => {
            Some(wgpu::VertexFormat::Uint16x2)
        }
        (false, Dimensions::Vec4, gltf::accessor::DataType::U16) => {
            Some(wgpu::VertexFormat::Uint16x4)
        }
        (false, Dimensions::Vec2, gltf::accessor::DataType::I16) => {
            Some(wgpu::VertexFormat::Sint16x2)
        }
        (false, Dimensions::Vec4, gltf::accessor::DataType::I16) => {
            Some(wgpu::VertexFormat::Sint16x4)
        }
        (true, Dimensions::Vec2, gltf::accessor::DataType::U16) => {
            Some(wgpu::VertexFormat::Unorm16x2)
        }
        (true, Dimensions::Vec4, gltf::accessor::DataType::U16) => {
            Some(wgpu::VertexFormat::Unorm16x4)
        }
        (true, Dimensions::Vec2, gltf::accessor::DataType::I16) => {
            Some(wgpu::VertexFormat::Snorm16x2)
        }
        (true, Dimensions::Vec4, gltf::accessor::DataType::I16) => {
            Some(wgpu::VertexFormat::Snorm16x4)
        }
        (_, Dimensions::Scalar, gltf::accessor::DataType::F32) => Some(wgpu::VertexFormat::Float32),
        (_, Dimensions::Vec2, gltf::accessor::DataType::F32) => Some(wgpu::VertexFormat::Float32x2),
        (_, Dimensions::Vec3, gltf::accessor::DataType::F32) => Some(wgpu::VertexFormat::Float32x3),
        (_, Dimensions::Vec4, gltf::accessor::DataType::F32) => Some(wgpu::VertexFormat::Float32x4),
        (false, Dimensions::Scalar, gltf::accessor::DataType::U32) => {
            Some(wgpu::VertexFormat::Uint32)
        }
        (false, Dimensions::Vec2, gltf::accessor::DataType::U32) => {
            Some(wgpu::VertexFormat::Uint32x2)
        }
        (false, Dimensions::Vec3, gltf::accessor::DataType::U32) => {
            Some(wgpu::VertexFormat::Uint32x3)
        }
        (false, Dimensions::Vec4, gltf::accessor::DataType::U32) => {
            Some(wgpu::VertexFormat::Uint32x4)
        }
        _ => None,
    }
}

fn gltf_accessor_to_indexformat(accessor: &gltf::Accessor) -> Option<wgpu::IndexFormat> {
    match accessor.data_type() {
        gltf::accessor::DataType::U16 => Some(wgpu::IndexFormat::Uint16),
        gltf::accessor::DataType::U32 => Some(wgpu::IndexFormat::Uint32),
        _ => None,
    }
}

fn get_accessor_component_count(accessor: &gltf::Accessor) -> usize {
    match accessor.dimensions() {
        Dimensions::Scalar => 1,
        Dimensions::Vec2 => 2,
        Dimensions::Vec3 => 3,
        Dimensions::Vec4 => 4,
        Dimensions::Mat2 => 4,
        Dimensions::Mat3 => 9,
        Dimensions::Mat4 => 16,
    }
}

fn get_accessor_type_size(accessor: &gltf::Accessor) -> usize {
    match accessor.data_type() {
        gltf::accessor::DataType::I8 => 1,
        gltf::accessor::DataType::U8 => 1,
        gltf::accessor::DataType::I16 => 2,
        gltf::accessor::DataType::U16 => 2,
        gltf::accessor::DataType::U32 => 4,
        gltf::accessor::DataType::F32 => 4,
    }
}

fn get_default_array_stride(accessor: &gltf::Accessor) -> usize {
    return get_accessor_component_count(accessor) * get_accessor_type_size(accessor);
}

async fn read_buffer(path: &Path, buffer: gltf::Buffer<'_>) -> Result<Vec<u8>, String> {
    match buffer.source() {
        gltf::buffer::Source::Uri(uri) => {
            let bin_path = path.join(uri);

            let url = format_url(bin_path.to_str().unwrap());
            Ok(reqwest::get(url)
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec())
        }
        _ => Err("Builtin buffers are unsupported".to_string()),
    }
}

fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let base = reqwest::Url::parse(&location.href().unwrap()).unwrap();
    base.join(file_name).unwrap()
}

pub async fn load_binary(path: &str) -> anyhow::Result<Vec<u8>> {
    let url = format_url(path);
    Ok(reqwest::get(url).await?.bytes().await?.to_vec())
}

impl From<&gltf::Semantic> for Attribute {
    fn from(semantic: &gltf::Semantic) -> Self {
        match semantic {
            gltf::Semantic::Positions => Attribute::Positions,
            gltf::Semantic::Normals => Attribute::Normals,
            gltf::Semantic::Tangents => Attribute::Tangents,
            gltf::Semantic::Colors(0) => Attribute::Colors,
            gltf::Semantic::TexCoords(0) => Attribute::TexCoords,
            gltf::Semantic::Joints(0) => Attribute::Joints,
            gltf::Semantic::Weights(0) => Attribute::Weights,
            _ => Attribute::Unknown,
        }
    }
}

pub struct VertexBufferLayoutBuilder<'a> {
    attributes: Vec<wgpu::VertexAttribute>,
    result: wgpu::VertexBufferLayout<'a>,
}

impl<'a> VertexBufferLayoutBuilder<'a> {
    fn new(
        array_stride: wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode,
        attributes: Vec<wgpu::VertexAttribute>,
    ) -> Self {
        Self {
            attributes,
            result: wgpu::VertexBufferLayout {
                array_stride,
                step_mode,
                attributes: &[],
            },
        }
    }

    fn build(&'a self) -> wgpu::VertexBufferLayout<'a> {
        let mut layout = self.result.clone();
        layout.attributes = &self.attributes;
        layout
    }
}

pub struct IndexData {
    buffer_id: usize,
    format: wgpu::IndexFormat,
    offset: u64,
}

pub struct PrimitiveRenderData<'a> {
    layouts: Vec<VertexBufferLayoutBuilder<'a>>,
    used_views: Vec<ViewData>,
    draw_count: u32,
    index_data: Option<IndexData>,
    transform_bind_group_id: usize,
    material_bind_group_id: usize,
}

#[derive(Debug)]
pub struct ViewData {
    pub view_index: usize,
    pub offset: u64,
}
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialData {
    base_color_factor: [f32; 4],
    metallic_factor: f32,
    roughness_factor: f32,
    alpha_cut_off: f32,
    filler: u32,
}

pub struct PipelineData {
    pipeline_list: Vec<wgpu::RenderPipeline>,
    bind_group_start_index: u32,
}

pub struct Scene<'a> {
    pub render_datas: Vec<PrimitiveRenderData<'a>>,
    pipeline_lists: HashMap<String, PipelineData>,
    buffers: HashMap<usize, wgpu::Buffer>,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: Vec<wgpu::BindGroup>,
    material_bind_group_layout: wgpu::BindGroupLayout,
}

impl Scene<'_> {
    fn create_buffer_if_new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer_contents: &Vec<Vec<u8>>,
        buffers: &mut HashMap<usize, wgpu::Buffer>,
        view: &View,
        usage: wgpu::BufferUsages,
    ) {
        if !buffers.contains_key(&view.index()) {
            let mut size = view.length();
            if size % 4 != 0 {
                size = (size / 4 + 1) * 4;
            }
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(format!("GLTF View #{}", view.index()).as_str()),
                size: size as u64,
                usage,
                mapped_at_creation: false,
            });

            queue.write_buffer(
                &buffer,
                0,
                &buffer_contents[view.buffer().index()][view.offset()..view.offset() + size],
            );
            buffers.insert(view.index(), buffer);
        }
    }

    pub fn from_gltf(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer_contents: &Vec<Vec<u8>>,
        scene: &gltf::Scene,
        white_texture: &Texture,
        default_normal_texture: &Texture,
        images: &Vec<Texture>,
    ) -> Self {
        let mut buffers = HashMap::<usize, wgpu::Buffer>::new();
        let mut render_datas = Vec::new();
        let mut nodes: Vec<(Node, Matrix4<f32>)> = scene
            .nodes()
            .map(|node| (node, Matrix4::identity()))
            .collect();

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let mut bind_groups = Vec::new();

        while nodes.len() > 0 {
            let (node, parent_transform) = nodes.pop().unwrap();

            let local_transform = Matrix4::from(node.transform().matrix());
            let total_transform = parent_transform * local_transform;

            for child in node.children() {
                nodes.push((child, total_transform));
            }

            let transform_content: [[f32; 4]; 4] = total_transform.into();

            let transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&transform_content),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &transform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &transform_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

            let transform_bind_group_id = bind_groups.len();
            bind_groups.push(transform_bind_group);

            let mesh = match node.mesh() {
                Some(mesh) => mesh,
                None => continue,
            };
            for primitive in mesh.primitives() {
                let mut layouts = Vec::<VertexBufferLayoutBuilder>::new();
                let mut used_views = Vec::<ViewData>::new();
                let mut draw_count = 0;

                let material = primitive.material();
                let pbr = material.pbr_metallic_roughness();

                let base_color_texture = match pbr.base_color_texture() {
                    Some(info) => &images[info.texture().source().index()],
                    None => white_texture,
                };

                let metallic_roughness_texture = match pbr.metallic_roughness_texture() {
                    Some(info) => &images[info.texture().source().index()],
                    None => white_texture,
                };

                let normal_texture = match material.normal_texture() {
                    Some(info) => &images[info.texture().source().index()],
                    None => default_normal_texture,
                };

                let material_data = MaterialData {
                    base_color_factor: pbr.base_color_factor(),
                    metallic_factor: pbr.metallic_factor(),
                    roughness_factor: pbr.roughness_factor(),
                    alpha_cut_off: material.alpha_cutoff().unwrap_or(0f32),
                    filler: 0,
                };

                let material_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&[material_data]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });

                let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &material_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: material_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&base_color_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(
                                &metallic_roughness_texture.view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(
                                &metallic_roughness_texture.sampler,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                        },
                    ],
                });

                let material_bind_group_id = bind_groups.len();
                bind_groups.push(material_bind_group);

                for (semantic, accessor) in primitive.attributes() {
                    let view = match accessor.view() {
                        Some(view) => view,
                        None => continue,
                    };
                    Self::create_buffer_if_new(
                        device,
                        queue,
                        buffer_contents,
                        &mut buffers,
                        &view,
                        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    );

                    draw_count = accessor.count() as u32;
                    layouts.push(VertexBufferLayoutBuilder::new(
                        view.stride().unwrap_or(get_default_array_stride(&accessor)) as u64,
                        wgpu::VertexStepMode::Vertex,
                        vec![wgpu::VertexAttribute {
                            format: gltf_accessor_to_wgpu(&accessor).unwrap(),
                            offset: 0,
                            shader_location: Attribute::from(&semantic) as u32,
                        }],
                    ));

                    used_views.push(ViewData {
                        view_index: view.index(),
                        offset: accessor.offset() as u64,
                    });
                }

                let index_data = match primitive.indices() {
                    Some(accessor) => {
                        let view = accessor.view().unwrap();
                        Self::create_buffer_if_new(
                            device,
                            queue,
                            buffer_contents,
                            &mut buffers,
                            &view,
                            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        );
                        draw_count = accessor.count() as u32;
                        Some(IndexData {
                            buffer_id: view.index(),
                            format: gltf_accessor_to_indexformat(&accessor).unwrap(),
                            offset: accessor.offset() as u64,
                        })
                    }
                    None => None,
                };

                render_datas.push(PrimitiveRenderData {
                    layouts,
                    used_views,
                    draw_count,
                    index_data,
                    transform_bind_group_id,
                    material_bind_group_id,
                });
            }
        }

        Self {
            render_datas,
            pipeline_lists: HashMap::new(),
            buffers,
            transform_bind_group_layout,
            material_bind_group_layout,
            bind_groups,
        }
    }

    pub fn generate_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader: &Shader,
        name: &str,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        targets: &[Option<wgpu::ColorTargetState>],
        depth: bool,
        cull_back_face: bool,
    ) {
        let mut pipelines = Vec::<wgpu::RenderPipeline>::new();

        for render_data in &self.render_datas {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    bind_group_layouts,
                    &[
                        &self.transform_bind_group_layout,
                        &self.material_bind_group_layout,
                    ],
                ]
                .concat(),
                push_constant_ranges: &[],
            });

            let layouts: Vec<wgpu::VertexBufferLayout> = render_data
                .layouts
                .iter()
                .map(|builder| builder.build())
                .collect();

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader.module,
                    entry_point: &shader.vs_entry,
                    buffers: &layouts,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: if cull_back_face {
                        Some(wgpu::Face::Back)
                    } else {
                        None
                    },
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: if depth {
                    Some(wgpu::DepthStencilState {
                        format: crate::texture::Texture::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    })
                } else {
                    None
                },
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader.module,
                    entry_point: &shader.fs_entry,
                    targets,
                }),
                multiview: None,
            });
            pipelines.push(pipeline);
        }
        self.pipeline_lists.insert(
            name.to_string(),
            PipelineData {
                pipeline_list: pipelines,
                bind_group_start_index: bind_group_layouts.len() as u32,
            },
        );
    }

    pub fn draw_pipelines<'a>(&'a self, name: &str, render_pass: &mut wgpu::RenderPass<'a>) {
        for (pipeline, render_data) in
            zip(&self.pipeline_lists[name].pipeline_list, &self.render_datas)
        {
            render_pass.set_pipeline(&pipeline);
            for (slot, view_data) in render_data.used_views.iter().enumerate() {
                let buffer = &self.buffers[&view_data.view_index];
                render_pass.set_vertex_buffer(slot as u32, buffer.slice(&view_data.offset..));
            }
            render_pass.set_bind_group(
                self.pipeline_lists[name].bind_group_start_index,
                &self.bind_groups[render_data.transform_bind_group_id],
                &[],
            );
            render_pass.set_bind_group(
                self.pipeline_lists[name].bind_group_start_index + 1,
                &self.bind_groups[render_data.material_bind_group_id],
                &[],
            );

            if let Some(IndexData {
                buffer_id,
                format,
                offset,
            }) = render_data.index_data
            {
                let buffer = &self.buffers[&buffer_id];
                render_pass.set_index_buffer(buffer.slice(offset..), format);
                render_pass.draw_indexed(0..render_data.draw_count, 0, 0..1);
            } else {
                render_pass.draw(0..render_data.draw_count, 0..1)
            }
        }
    }
}

pub async fn load_gltf<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &str,
) -> Result<Vec<Scene<'a>>, String> {
    let white_texture =
        Texture::create_1_pixel_texture(device, queue, &[255, 255, 255, 255], "white_texture");
    let default_normal_texture = Texture::create_1_pixel_texture(
        device,
        queue,
        &[128, 128, 255, 255],
        "default_normal_texture",
    );

    let bytes = load_binary(path).await.unwrap();

    let gltf = match gltf::Gltf::from_slice(&bytes) {
        Ok(gltf) => gltf,
        Err(_) => return Err("Failed to open gltf file".into()),
    };

    let mut buffer_contents = Vec::new();
    let parent_dir = Path::new(path).parent().unwrap();

    for buffer in gltf.buffers() {
        let content = read_buffer(&parent_dir, buffer).await.unwrap();
        buffer_contents.push(content);
    }

    let uris = gltf.images().map(|image| match image.source() {
        gltf::image::Source::View { .. } => panic!(),
        gltf::image::Source::Uri { uri, .. } => {
            format_url(parent_dir.join(uri).to_str().unwrap()).to_string()
        }
    });

    let images = join_all(uris.map(|uri| async move {
        Texture::from_url(device, queue, uri.as_str(), "loaded image").await
    }))
    .await;

    let scenes = gltf
        .scenes()
        .map(|scene| {
            Scene::from_gltf(
                &device,
                &queue,
                &buffer_contents,
                &scene,
                &white_texture,
                &default_normal_texture,
                &images,
            )
        })
        .collect();

    Ok(scenes)
}
