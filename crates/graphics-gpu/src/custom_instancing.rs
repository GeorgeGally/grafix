//! Low-level custom instancing: one entity, one per-instance vertex buffer,
//! one draw call. Instance data is uploaded only when it changes; animation
//! runs in the vertex shader, so CPU cost does not scale with instance count.
//!
//! Requires `NoIndirectDrawing` on the camera, because the draw command issues
//! `draw_indexed` directly rather than Bevy's GPU-preprocessed indirect draws.

use std::sync::Arc;

use bevy::{
    camera::visibility::NoFrustumCulling,
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::*},
    },
    mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout},
    pbr::{
        self, MeshInputUniform, MeshPipeline, MeshPipelineKey, MeshPipelineSystems, MeshUniform,
        RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
        SetMeshViewBindingArrayBindGroup, ViewKeyCache,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        batching::gpu_preprocessing::BatchedInstanceBuffers,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::*,
        renderer::RenderDevice,
        sync_component::SyncComponent,
        sync_world::MainEntity,
        view::ExtractedView,
    },
};
use bytemuck::{Pod, Zeroable};

use crate::GpuTelemetry;

/// One instance: position + uniform scale, linear RGB colour + animation phase.
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C)]
pub struct InstanceData {
    pub position: Vec3,
    pub scale: f32,
    /// `rgb` is colour, `a` is a 0..1 animation phase.
    pub color: [f32; 4],
}

/// A set of instances drawn with one call. Bump `generation` whenever `data`
/// changes; the render world re-uploads only on a new generation.
#[derive(Component, Clone)]
#[require(NoFrustumCulling)]
pub struct InstanceSet {
    pub data: Arc<Vec<InstanceData>>,
    pub generation: u64,
}

impl SyncComponent<RenderApp> for InstanceSet {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for InstanceSet {
    type QueryData = &'static InstanceSet;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

pub struct CustomInstancingPlugin;

impl Plugin for CustomInstancingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<InstanceSet>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_render_command::<Transparent3d, DrawInstanced>()
            .init_resource::<SpecializedMeshPipelines<InstancingPipeline>>()
            .add_systems(
                RenderStartup,
                (
                    init_pipeline.after(MeshPipelineSystems),
                    drop_device_buffers,
                ),
            )
            .add_systems(
                Render,
                (
                    queue_instanced.in_set(RenderSystems::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSystems::PrepareResources),
                ),
            );
    }
}

#[derive(Component)]
struct InstanceBuffer {
    buffer: Buffer,
    length: u32,
    generation: u64,
}

/// `RenderStartup` runs again after device-loss recovery. Buffers created on
/// the old device, and specialized pipeline IDs that pointed into the old
/// `PipelineCache`, must not survive into the new one.
fn drop_device_buffers(mut commands: Commands, buffers: Query<Entity, With<InstanceBuffer>>) {
    for entity in &buffers {
        commands.entity(entity).remove::<InstanceBuffer>();
    }
    commands.insert_resource(SpecializedMeshPipelines::<InstancingPipeline>::default());
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<(Entity, &InstanceSet, Option<&InstanceBuffer>)>,
    render_device: Res<RenderDevice>,
    telemetry: Res<GpuTelemetry>,
) {
    for (entity, set, existing) in &query {
        if existing.is_some_and(|b| b.generation == set.generation) {
            continue;
        }
        if set.data.is_empty() {
            commands.entity(entity).remove::<InstanceBuffer>();
            continue;
        }
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grafix_instance_buffer"),
            contents: bytemuck::cast_slice(set.data.as_slice()),
            usage: BufferUsages::VERTEX,
        });
        telemetry.count_instance_upload();
        commands.entity(entity).insert(InstanceBuffer {
            buffer,
            length: set.data.len() as u32,
            generation: set.generation,
        });
    }
}

#[derive(Resource)]
struct InstancingPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
}

fn init_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mesh_pipeline: Res<MeshPipeline>,
) {
    commands.insert_resource(InstancingPipeline {
        shader: asset_server.load(graphics_shaders::CUSTOM_INSTANCING),
        mesh_pipeline: mesh_pipeline.clone(),
    });
}

impl SpecializedMeshPipeline for InstancingPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        descriptor.label = Some("grafix_custom_instancing_pipeline".into());
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<InstanceData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    // Locations 0-2 are position, normal and uv.
                    shader_location: 3,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: VertexFormat::Float32x4.size(),
                    shader_location: 4,
                },
            ],
        });
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
        }
        Ok(descriptor)
    }
}

fn queue_instanced(
    draw_functions: Res<DrawFunctions<Transparent3d>>,
    pipeline: Res<InstancingPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancingPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    batched_instance_buffers: Option<Res<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>>,
    instance_sets: Query<(Entity, &MainEntity), With<InstanceSet>>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<&ExtractedView>,
    view_key_cache: Res<ViewKeyCache>,
    telemetry: Res<GpuTelemetry>,
) {
    let draw_function = draw_functions.read().id::<DrawInstanced>();
    for view in &views {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };
        for (entity, main_entity) in &instance_sets {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };
            let key = view_key
                | MeshPipelineKey::from_primitive_topology_and_strip_index(
                    mesh.primitive_topology(),
                    mesh.index_format(),
                );
            let pipeline_id =
                match pipelines.specialize(&pipeline_cache, &pipeline, key, &mesh.layout) {
                    Ok(id) => id,
                    Err(err) => {
                        telemetry.report_error(format!("ShaderError custom_instancing: {err}"));
                        continue;
                    }
                };
            let mesh_center = pbr::get_mesh_instance_world_from_local(
                *main_entity,
                mesh_instance.current_uniform_index,
                &render_mesh_instances,
                batched_instance_buffers.as_deref(),
            )
            .transform_point3(mesh.aabb_center);
            phase.add_retained(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center,
                    depth_bias: 0.0,
                },
                entity: (entity, *main_entity),
                pipeline: pipeline_id,
                draw_function,
                distance: 0.0,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
    }
}

type DrawInstanced = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<InstanceBuffer>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        instance_buffer: Option<&'w InstanceBuffer>,
        (meshes, render_mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let mesh_allocator = mesh_allocator.into_inner();
        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id()) else {
            return RenderCommandResult::Skip;
        };
        let Some(instance_buffer) = instance_buffer else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id())
        else {
            return RenderCommandResult::Skip;
        };
        pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));
        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(index_slice) =
                    mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id())
                else {
                    return RenderCommandResult::Skip;
                };
                pass.set_index_buffer(index_slice.buffer.slice(..), *index_format);
                pass.draw_indexed(
                    index_slice.range.start..(index_slice.range.start + count),
                    vertex_slice.range.start as i32,
                    0..instance_buffer.length,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(vertex_slice.range, 0..instance_buffer.length);
            }
        }
        RenderCommandResult::Success
    }
}
