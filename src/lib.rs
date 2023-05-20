mod camera;
mod gltf_loader;
mod image_future;
mod shader;
mod texture;
mod util;
mod voxel_texture;

use camera::{PerspectiveCamera, ShadowCamera};
use cgmath::{Deg, Euler, InnerSpace, Point3, Vector3};
use shader::Shader;
use texture::Texture;
use voxel_texture::VoxelTexture;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

struct State<'a> {
    window: Window,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    camera_buffer: wgpu::Buffer,
    diffuse_camera_bind_group: wgpu::BindGroup,
    camera: PerspectiveCamera,
    depth_texture: Texture,
    scenes: Vec<gltf_loader::Scene<'a>>,
    diffuse_texture_bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Light {
    position: [f32; 4],
    intensity: [f32; 3],
    falloff: f32,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0, 0.0],
            intensity: [0.0, 0.0, 0.0],
            falloff: 0.0,
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Lights {
    filler: [i32; 3],
    count: i32,
    lights: [Light; 8],
}

impl<'a> State<'a> {
    async fn new(window: Window) -> State<'a> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            dx12_shader_compiler: Default::default(),
        });
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let mut limits = wgpu::Limits::default();
        limits.max_buffer_size = 1024 * 1024 * 1024 * 2;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits,
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
            
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_caps.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        surface.configure(&device, &config);

        let shadow_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow shader module"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });

        let voxelizer_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Voxelizer shader module"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/voxelize.wgsl").into()),
        });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader module"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let shadow_shader = Shader {
            vs_entry: "vs_main".to_string(),
            fs_entry: "fs_main".to_string(),
            module: shadow_shader_module,
        };

        let voxelizer_shader = Shader {
            vs_entry: "vs_main".to_string(),
            fs_entry: "fs_main".to_string(),
            module: voxelizer_shader_module,
        };

        let shader = Shader {
            vs_entry: "vs_main".to_string(),
            fs_entry: "fs_main".to_string(),
            module: shader_module,
        };

        let shadow_camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    visibility: wgpu::ShaderStages::VERTEX,
                }],
            });

        let diffuse_camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Diffuse camera bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        visibility: wgpu::ShaderStages::VERTEX,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        visibility: wgpu::ShaderStages::VERTEX,
                    },
                ],
            });

        let voxel_texture = VoxelTexture::new(
            &device,
            wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 512,
            },
            "Voxel texture",
        );

        let dummy_output =
            Texture::create_target_texture(&device, 512, 512, "Dummy target texture");

        let voxelizer_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Voxelizer texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        count: None,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let diffuse_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Diffuse texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        count: None,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        count: None,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        visibility: wgpu::ShaderStages::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let camera = PerspectiveCamera::new(
            &window,
            Vector3 {
                x: -1.8,
                y: 3.155,
                z: -0.3,
            },
            Euler::new(Deg(0.0), Deg(-270.0), Deg(0.0)),
            0.01,
            1000.0,
            Deg(90.0),
        );

        let shadow_camera = ShadowCamera::new(
            Point3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3::new(1.0, -6.0, 2.0).normalize(),
            -30.0,
            30.0,
            -30.0,
            30.0,
            -30.0,
            30.0,
        );

        let lights = Lights {
            count: 3,
            lights: [
                Light {
                    position: [
                        shadow_camera.direction.x,
                        shadow_camera.direction.y,
                        shadow_camera.direction.z,
                        0.0,
                    ],
                    intensity: [30.0, 30.0, 30.0],
                    falloff: 0.0,
                },
                Light {
                    position: [-9.87, 1.3, -0.22, 1.0],
                    intensity: [0.0, 0.0, 20.0],
                    falloff: 2.0,
                },
                Light {
                    position: [8.7, 1.6, -0.3, 1.0],
                    intensity: [10.0, 10.0, 10.0],
                    falloff: 2.0,
                },
                Light::default(),
                Light::default(),
                Light::default(),
                Light::default(),
                Light::default(),
            ],
            filler: [0, 0, 0],
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&[camera.get_uniform_data()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shadow_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow camera buffer"),
            contents: bytemuck::cast_slice(&[shadow_camera.get_uniform_data()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let lights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights buffer"),
            contents: bytemuck::cast_slice(&[lights]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shadow_depth_texture = Texture::create_depth_texture(
            &device,
            2048,
            2048,
            Some(wgpu::CompareFunction::Less),
            "Shadow depth texture",
        );
        let depth_texture = Texture::create_depth_texture(
            &device,
            config.width,
            config.height,
            Some(wgpu::CompareFunction::LessEqual),
            "Depth texture",
        );

        let diffuse_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bind group"),
            layout: &diffuse_camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: shadow_camera_buffer.as_entire_binding(),
                },
            ],
        });

        let voxelizer_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Voxelizer texture bind group"),
            layout: &voxelizer_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_depth_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_depth_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&voxel_texture.get_mip_0()),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: lights_buffer.as_entire_binding(),
                },
            ],
        });

        let diffuse_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Diffuse texture bind group"),
            layout: &diffuse_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_depth_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_depth_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&voxel_texture.main_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&voxel_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: lights_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow camera bind group"),
            layout: &shadow_camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_camera_buffer.as_entire_binding(),
            }],
        });

        let model = "Sponza";

        let mut scenes = gltf_loader::load_gltf(
            &device,
            &queue,
            format!("models/{}/glTF/{}.gltf", model, model).as_str(),
        )
        .await
        .unwrap();

        scenes[0].generate_pipeline(
            &device,
            &shadow_shader,
            "shadow",
            &[&shadow_camera_bind_group_layout],
            &[],
            true,
            true,
        );

        scenes[0].generate_pipeline(
            &device,
            &voxelizer_shader,
            "voxelization",
            &[
                &diffuse_camera_bind_group_layout,
                &voxelizer_texture_bind_group_layout,
            ],
            &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Uint,
                blend: None,
                write_mask: wgpu::ColorWrites::empty(),
            })],
            false,
            false,
        );

        scenes[0].generate_pipeline(
            &device,
            &shader,
            "main",
            &[
                &diffuse_camera_bind_group_layout,
                &diffuse_texture_bind_group_layout,
            ],
            &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            true,
            true,
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Preprocess encoder"),
        });

        {
            let mut shadow_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow render pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            shadow_render_pass.set_bind_group(0, &shadow_camera_bind_group, &[]);
            scenes[0].draw_pipelines("shadow", &mut shadow_render_pass);
        }
        {
            let mut voxelization_render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Voxelization render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dummy_output.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: false,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
            voxelization_render_pass.set_bind_group(0, &diffuse_camera_bind_group, &[]);
            voxelization_render_pass.set_bind_group(1, &voxelizer_texture_bind_group, &[]);
            scenes[0].draw_pipelines("voxelization", &mut voxelization_render_pass);
        }

        voxel_texture.run_generate_mipmaps(&mut encoder);

        queue.submit(std::iter::once(encoder.finish()));

        State {
            window,
            surface,
            device,
            queue,
            config,
            size,
            camera_buffer,
            diffuse_camera_bind_group,
            diffuse_texture_bind_group,
            camera,
            scenes,
            depth_texture,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config)
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera.process_event(event)
    }

    fn update(&mut self) {
        self.camera.update();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera.get_uniform_data()]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.25,
                            g: 0.23,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_bind_group(0, &self.diffuse_camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.diffuse_texture_bind_group, &[]);
            self.scenes[0].draw_pipelines("main", &mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .expect("Failed to create window");

    use winit::dpi::PhysicalSize;
    window.set_inner_size(PhysicalSize::new(1920, 1080));

    use winit::platform::web::WindowExtWebSys;
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| {
            let dst = doc.get_element_by_id("voxel-gi")?;
            let canvas = web_sys::Element::from(window.canvas());
            canvas.set_id("webgpu");
            dst.append_child(&canvas).ok()?;
            Some(())
        })
        .expect("Couldn't append canvas to document body.");

    let mut state = State::new(window).await;

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            window_id,
            ref event,
        } => {
            if window_id == state.window.id() && !state.input(event) {
                match event {
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {}
                }
            }
        }
        Event::RedrawRequested(window_id) if window_id == state.window.id() => {
            state.update();
            match state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(e) => eprintln!("{:?}", e),
            }
        }
        Event::MainEventsCleared => {
            state.window.request_redraw();
        }
        _ => {}
    })
}
