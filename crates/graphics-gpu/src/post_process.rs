//! One custom fullscreen post-process pass (chromatic aberration + vignette),
//! scheduled in `Core3dSystems::PostProcess` after Bevy's tonemapping.

use bevy::{
    core_pipeline::{Core3dSystems, FullscreenShader, schedule::Core3d, tonemapping::tonemapping},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
};

use crate::GpuTelemetry;

/// Per-camera settings. Adding this component to a camera enables the pass.
/// The pass targets HDR cameras (`Rgba16Float` main texture).
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
#[extract_app(RenderApp)]
pub struct PostProcessSettings {
    pub aberration: f32,
    pub vignette: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Default for PostProcessSettings {
    fn default() -> Self {
        Self {
            aberration: 0.006,
            vignette: 0.35,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

const TARGET_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub struct PostProcessPlugin;

impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<PostProcessSettings>::default(),
            UniformComponentPlugin::<PostProcessSettings>::default(),
        ));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Render,
                (
                    prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                    check_pipeline.in_set(RenderSystems::Prepare),
                ),
            )
            .add_systems(
                Core3d,
                post_process_pass
                    .after(tonemapping)
                    .in_set(Core3dSystems::PostProcess),
            );
    }
}

#[derive(Resource)]
struct PostProcessPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

#[derive(Component)]
struct PostProcessBindGroups {
    a: (TextureViewId, BindGroup),
    b: (TextureViewId, BindGroup),
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "grafix_post_process_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<PostProcessSettings>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = asset_server.load(graphics_shaders::POST_PROCESS);
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("grafix_post_process_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: TARGET_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(PostProcessPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}

/// Surfaces shader/pipeline failures instead of leaving a silent black frame.
fn check_pipeline(
    pipeline: Option<Res<PostProcessPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    telemetry: Res<GpuTelemetry>,
) {
    let Some(pipeline) = pipeline else { return };
    if let CachedPipelineState::Err(err) =
        pipeline_cache.get_render_pipeline_state(pipeline.pipeline_id)
        && !matches!(err, bevy::shader::ShaderCacheError::ShaderNotLoaded(_))
    {
        telemetry.report_error(format!("ShaderError post_process: {err}"));
    }
}

fn prepare_bind_groups(
    mut commands: Commands,
    mut views: Query<(Entity, &ViewTarget, Option<&mut PostProcessBindGroups>)>,
    pipeline: Option<Res<PostProcessPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    settings_uniforms: Res<ComponentUniforms<PostProcessSettings>>,
    render_device: Res<RenderDevice>,
) {
    let Some(pipeline) = pipeline else { return };
    let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
        return;
    };
    let create = |texture: &TextureView| {
        (
            texture.id(),
            render_device.create_bind_group(
                "grafix_post_process_bind_group",
                &pipeline_cache.get_bind_group_layout(&pipeline.layout),
                &BindGroupEntries::sequential((
                    texture,
                    &pipeline.sampler,
                    settings_binding.clone(),
                )),
            ),
        )
    };
    for (entity, view_target, existing) in &mut views {
        let main = view_target.main_texture_view();
        let other = view_target.main_texture_other_view();
        if let Some(mut groups) = existing {
            if groups.a.0 != main.id() {
                groups.a = create(main);
            }
            if groups.b.0 != other.id() {
                groups.b = create(other);
            }
        } else {
            commands.entity(entity).insert(PostProcessBindGroups {
                a: create(main),
                b: create(other),
            });
        }
    }
}

fn post_process_pass(
    view: ViewQuery<(
        &ViewTarget,
        &DynamicUniformIndex<PostProcessSettings>,
        &PostProcessBindGroups,
    )>,
    pipeline: Option<Res<PostProcessPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let Some(pipeline) = pipeline else { return };
    let (view_target, settings_index, bind_groups) = view.into_inner();
    if view_target.main_texture_format() != TARGET_FORMAT {
        return;
    }
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
        return;
    };
    let post_process = view_target.post_process_write();
    let (_, bind_group) = if bind_groups.a.0 == post_process.source.id() {
        &bind_groups.a
    } else {
        &bind_groups.b
    };
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("grafix_post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, bind_group, &[settings_index.index()]);
    pass.draw(0..3, 0..1);
}
