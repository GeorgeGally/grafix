//! Minimal WebGPU compute pass with an observable result: every frame a
//! compute shader writes an animated field into a storage texture, and the
//! scene samples that texture as an emissive material.

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::schedule::camera_driver,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{texture_storage_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderGraph, RenderQueue},
        texture::GpuImage,
    },
    shader::ShaderCacheError,
};

use crate::GpuTelemetry;

pub const FIELD_SIZE: u32 = 256;
const WORKGROUP_SIZE: u32 = 8;
const FIELD_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// Handle to the storage texture the compute pass writes.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct ComputeFieldImage(pub Handle<Image>);

/// Parameters for the compute pass, controlled from the main world.
#[derive(Resource, Clone, Copy, ExtractResource, ShaderType)]
#[extract_app(RenderApp)]
pub struct ComputeFieldParams {
    pub time: f32,
    pub speed: f32,
    pub scale: f32,
    pub _pad: f32,
}

impl Default for ComputeFieldParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            speed: 0.6,
            scale: 4.0,
            _pad: 0.0,
        }
    }
}

pub struct ComputeFieldPlugin;

impl Plugin for ComputeFieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ComputeFieldParams>()
            .add_plugins((
                ExtractResourcePlugin::<ComputeFieldImage>::default(),
                ExtractResourcePlugin::<ComputeFieldParams>::default(),
            ))
            .add_systems(PreStartup, create_field_image)
            .add_systems(Update, advance_time);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Render,
                prepare_bind_group.in_set(RenderSystems::PrepareBindGroups),
            )
            .add_systems(RenderGraph, dispatch_field.before(camera_driver));
    }
}

fn create_field_image(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(FIELD_SIZE, FIELD_SIZE, FIELD_FORMAT, None);
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    commands.insert_resource(ComputeFieldImage(images.add(image)));
}

fn advance_time(time: Res<Time>, mut params: ResMut<ComputeFieldParams>) {
    params.time = time.elapsed_secs_wrapped();
}

#[derive(Resource)]
struct ComputeFieldPipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline_id: CachedComputePipelineId,
}

#[derive(Resource, Default)]
struct ComputeFieldUniform(UniformBuffer<ComputeFieldParams>);

#[derive(Resource)]
struct ComputeFieldBindGroup {
    texture_view: TextureViewId,
    buffer: BufferId,
    bind_group: BindGroup,
}

fn init_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "grafix_compute_field_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(FIELD_FORMAT, StorageTextureAccess::WriteOnly),
                uniform_buffer::<ComputeFieldParams>(false),
            ),
        ),
    );
    let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("grafix_compute_field_pipeline".into()),
        layout: vec![layout.clone()],
        shader: asset_server.load(graphics_shaders::COMPUTE_FIELD),
        entry_point: Some("main".into()),
        ..default()
    });
    commands.insert_resource(ComputeFieldPipeline {
        layout,
        pipeline_id,
    });
    commands.init_resource::<ComputeFieldUniform>();
}

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ComputeFieldPipeline>,
    mut uniform: ResMut<ComputeFieldUniform>,
    bind_group: Option<Res<ComputeFieldBindGroup>>,
    pipeline_cache: Res<PipelineCache>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    image: Option<Res<ComputeFieldImage>>,
    params: Option<Res<ComputeFieldParams>>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    telemetry: Res<GpuTelemetry>,
) {
    if let CachedPipelineState::Err(err) =
        pipeline_cache.get_compute_pipeline_state(pipeline.pipeline_id)
        && !matches!(err, ShaderCacheError::ShaderNotLoaded(_))
    {
        telemetry.report_error(format!("ShaderError compute_field: {err}"));
    }
    let (Some(image), Some(params)) = (image, params) else {
        return;
    };
    let Some(gpu_image) = gpu_images.get(&image.0) else {
        return;
    };
    uniform.0.set(*params);
    uniform.0.write_buffer(&render_device, &queue);
    let Some(buffer) = uniform.0.buffer() else {
        return;
    };
    let texture_view = gpu_image.texture_view.id();
    if bind_group.is_some_and(|current| {
        current.texture_view == texture_view && current.buffer == buffer.id()
    }) {
        return;
    }
    let bind_group = render_device.create_bind_group(
        "grafix_compute_field_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((&gpu_image.texture_view, &uniform.0)),
    );
    commands.insert_resource(ComputeFieldBindGroup {
        texture_view,
        buffer: buffer.id(),
        bind_group,
    });
}

fn dispatch_field(
    mut ctx: RenderContext,
    bind_group: Option<Res<ComputeFieldBindGroup>>,
    pipeline: Res<ComputeFieldPipeline>,
    pipeline_cache: Res<PipelineCache>,
    telemetry: Res<GpuTelemetry>,
) {
    let Some(bind_group) = bind_group else { return };
    let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id) else {
        return;
    };
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("grafix_compute_field_pass"),
            ..default()
        });
    pass.set_pipeline(compute_pipeline);
    pass.set_bind_group(0, &bind_group.bind_group, &[]);
    let groups = FIELD_SIZE.div_ceil(WORKGROUP_SIZE);
    pass.dispatch_workgroups(groups, groups, 1);
    telemetry.count_compute_dispatch();
}
