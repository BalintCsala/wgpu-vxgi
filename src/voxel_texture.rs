use wgpu::TextureView;

pub struct VoxelTexture {
    views: Vec<wgpu::TextureView>,
    pub main_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    mip_level_count: u32,
    pipelines: Vec<wgpu::ComputePipeline>,
    bind_groups: Vec<wgpu::BindGroup>,
}

impl VoxelTexture {
    pub fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        label: &str,
    ) -> Self {
        let mip_level_count = size.max_mips(wgpu::TextureDimension::D3);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba16Float],
        });

        let views: Vec<TextureView> = (0..mip_level_count)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(format!("{} view mip #{}", label, i).as_str()),
                    format: Some(wgpu::TextureFormat::Rgba16Float),
                    dimension: Some(wgpu::TextureViewDimension::D3),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: i,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: Some(1),
                })
            })
            .collect();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(format!("{} sampler", label).as_str()),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: mip_level_count as f32,
            ..Default::default()
        });

        let main_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(format!("{} shader module", label).as_str()),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mipmap_3d.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(format!("{} bind group layout", label).as_str()),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
            ],
        });

        let bind_groups = (0..mip_level_count - 1).map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(format!("{} bind group #{}", label, i).as_str()),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[i as usize]),
                }, wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&views[(i + 1) as usize]),
                }],
            })
        }).collect();

        let pipelines = (0..mip_level_count - 1)
            .map(|i| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(format!("{} compute pipeline layout #{}", label, i).as_str()),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(format!("{} compute pipeline #{}", label, i).as_str()),
                    layout: Some(&pipeline_layout),
                    module: &shader_module,
                    entry_point: "comp_main",
                })
            })
            .collect();

        Self {
            views,
            sampler,
            main_view,
            mip_level_count,
            pipelines,
            bind_groups,
        }
    }

    pub fn get_mip_0(&self) -> &TextureView {
        return &self.views[0];
    }

    pub fn run_generate_mipmaps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Mipmap compute pass"),
        });
        
        (0..self.mip_level_count - 1).for_each(|i| {
            compute_pass.set_pipeline(&self.pipelines[i as usize]);
            compute_pass.set_bind_group(0, &self.bind_groups[i as usize], &[]);
            compute_pass.dispatch_workgroups(1, 1, 1);
        });
    }
}
