fn shader_src(vs: &str, fs: &str) -> String { format!("{vs}\n{fs}") }

fn main() {
    // wgpu 30 API notes (verified against the real crate, differs from older tutorials):
    //  * InstanceDescriptor has NO Default -> use InstanceDescriptor::new_without_display_handle()
    //  * Instance::enumerate_adapters is async (returns impl Future)
    //  * RequestAdapterOptions gained apply_limit_buckets
    //  * RenderPipelineDescriptor.multiview renamed to multiview_mask
    //  * RenderPassDescriptor gained multiview_mask
    //  * PollType::Wait is a STRUCT variant: { submission_index, timeout }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    println!("adapter count = {}", adapters.len());
    for a in &adapters {
        let i = a.get_info();
        println!("  [{:?}] {} | backend={:?} | driver={} {} | vendor=0x{:x} device=0x{:x}",
            i.device_type, i.name, i.backend, i.driver, i.driver_info, i.vendor, i.device);
    }
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    })).expect("no adapter");
    let info = adapter.get_info();
    println!("chosen adapter: {} ({:?}, {:?})", info.name, info.device_type, info.backend);
    let limits = adapter.limits();
    println!("max_texture_dimension_2d = {}", limits.max_texture_dimension_2d);
    println!("max_buffer_size = {} MiB", limits.max_buffer_size / 1024 / 1024);

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("no device");

    // The real game's logical screen size, from Game.dat.
    const W: u32 = 1024;
    const H: u32 = 576;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

    let vs = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) col: vec4<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 3>(vec2(-0.8, -0.8), vec2(0.8, -0.8), vec2(0.0, 0.8));
    var o: VOut;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    o.col = vec4<f32>(0.2, 0.9, 0.4, 1.0);
    return o;
}"#;
    let fs = r#"
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> { return in.col; }"#;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tri"),
        source: wgpu::ShaderSource::Wgsl(shader_src(vs, fs).into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipe"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[None],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rp"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    let sub = queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(sub), timeout: None });
    println!("OFFSCREEN RENDER OK");
}
