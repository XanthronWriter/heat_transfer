use std::borrow::Cow;

use anyhow::*;
use futures::{
    executor::block_on,
    future::{join, join_all},
};
use futures_channel::oneshot::{channel, Receiver};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferUsages, CommandEncoderDescriptor,
    ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device, Queue,
};

use crate::heat_transfer::{
    gpu::get_gpu_device_and_queue,
    shader::{insert_gpu_m3_data, insert_material_data},
};

use super::{
    get_max_element_per_chunk, update_bind_group, DeviceFuture, HeatTransfer1D, WallCell,
    WallElement,
};

/// The whole base shader for method 3.
pub const SHADER: &str = include_str!("gpu_m3.wgsl");

/// All relevant data for the heat transfer algorithm on the GPU with method 3.
pub struct GPUSetupData {
    device: Device,
    queue: Queue,
    compute_pipeline: ComputePipeline,
    chunks: Vec<Chunk>,
}
impl HeatTransfer1D for GPUSetupData {
    fn setup(
        materials: Vec<crate::fds::Material>,
        wall_elements: Vec<WallElement>,
    ) -> anyhow::Result<Self> {
        let max_cell_count = wall_elements
            .iter()
            .map(|w| w.len())
            .max()
            .unwrap_or_default();

        let (device, queue) = block_on(get_gpu_device_and_queue())
            .with_context(|| "Failed to get device and queue.")?;

        let shader = insert_material_data(SHADER, &materials);
        let shader = insert_gpu_m3_data(&shader, max_cell_count);
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader Module"),
            source: wgpu::ShaderSource::Wgsl(Cow::from(shader)),
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: "compute",
        });

        let chunks = Chunk::build(&device, &compute_pipeline, wall_elements, max_cell_count);
        Ok(Self {
            device,
            queue,
            compute_pipeline,
            chunks,
        })
    }

    fn update(
        &mut self,
        delta_time: f32,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
        wall_temperature: &mut [[f32; 2]],
    ) -> anyhow::Result<()> {
        let receivers = self
            .chunks
            .iter()
            .zip(wall_temperature.chunks_mut(get_max_element_per_chunk()))
            .zip(
                wall_heat_transfer_coefficients
                    .chunks(get_max_element_per_chunk())
                    .zip(wall_q_in.chunks(get_max_element_per_chunk())),
            )
            .map(
                |(
                    (chunk, wall_temperature_buffer),
                    (wall_heat_transfer_coefficients, wall_q_in),
                )| {
                    let receiver = chunk.submit_update_to_queue(
                        &self.device,
                        &self.queue,
                        delta_time,
                        &self.compute_pipeline,
                        wall_heat_transfer_coefficients,
                        wall_q_in,
                    );
                    chunk.receive_update(receiver, wall_temperature_buffer)
                },
            )
            .collect::<Vec<_>>();
        _ = block_on(join(join_all(receivers), DeviceFuture(&self.device)));
        Ok(())
    }
}

/// All data for a single chunk
struct Chunk {
    setup_bind_group: BindGroup,
    update_bind_group: BindGroup,
    wall_heat_transfer_coefficients_buffer: Buffer,
    wall_q_in_buffer: Buffer,
    delta_time_buffer: Buffer,
    groups: u32,
}
impl Chunk {
    /// Create all [`Chunk`]s for all passed [`WallElement`]s
    fn build(
        device: &Device,
        compute_pipeline: &ComputePipeline,
        wall_elements: Vec<WallElement>,
        max_cell_count: usize,
    ) -> Vec<Self> {
        let mut chunks = vec![];

        for wall_elements_chunk in wall_elements
            .into_iter()
            .map(Option::Some)
            .collect::<Vec<_>>()
            .chunks_mut(get_max_element_per_chunk())
        {
            let wall_element_count = wall_elements_chunk.len();
            let mut flattened_wall_elements: Vec<u8> = vec![];
            for wall_element in wall_elements_chunk {
                if let Some(mut wall_element) = wall_element.take() {
                    let cell_count = wall_element.len() as u32;
                    for _ in 0..(max_cell_count - wall_element.len()) {
                        wall_element.push(WallCell::default());
                    }
                    let mut bytes = bytemuck::cast_slice::<_, u8>(&[cell_count])
                        .iter()
                        .chain(bytemuck::cast_slice::<_, u8>(wall_element.as_slice()))
                        .copied()
                        .collect::<Vec<u8>>();
                    flattened_wall_elements.append(&mut bytes);
                }
            }
            let setup_bind_group =
                setup_bind_group(device, compute_pipeline, &flattened_wall_elements);
            let (
                update_bind_group,
                wall_heat_transfer_coefficients_buffer,
                wall_q_in_buffer,
                delta_time_buffer,
            ) = update_bind_group(device, compute_pipeline, wall_element_count, 1);
            let groups = (wall_element_count as f32 / 256.0).ceil() as u32;
            let chunk = Chunk {
                setup_bind_group,
                update_bind_group,
                wall_heat_transfer_coefficients_buffer,
                wall_q_in_buffer,
                delta_time_buffer,
                groups,
            };
            chunks.push(chunk)
        }
        chunks
    }

    /// Start the calculation for this [`Chunk`]
    #[inline]
    fn submit_update_to_queue(
        &self,
        device: &Device,
        queue: &Queue,
        delta_time: f32,
        compute_pipeline: &ComputePipeline,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
    ) -> Receiver<std::result::Result<(), wgpu::BufferAsyncError>> {
        queue.write_buffer(
            &self.wall_heat_transfer_coefficients_buffer,
            0,
            bytemuck::cast_slice(wall_heat_transfer_coefficients),
        );
        queue.write_buffer(&self.wall_q_in_buffer, 0, bytemuck::cast_slice(wall_q_in));
        queue.write_buffer(
            &self.delta_time_buffer,
            0,
            bytemuck::cast_slice(&[delta_time]),
        );

        let mut update_command_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Update Command Encode"),
        });
        {
            let mut update_compute_pass =
                update_command_encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Setup Compute Pass"),
                });
            update_compute_pass.set_pipeline(compute_pipeline);
            update_compute_pass.set_bind_group(0, &self.setup_bind_group, &[]);
            update_compute_pass.set_bind_group(1, &self.update_bind_group, &[]);
            update_compute_pass.dispatch_workgroups(self.groups, 1, 1);
        }
        queue.submit(Some(update_command_encoder.finish()));

        let (sender, receiver) = channel();
        self.wall_q_in_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| {
                let _ = sender.send(result);
            });
        receiver
    }

    /// Receive the calculated data for this [`Chunk`].
    #[inline]
    async fn receive_update(
        &self,
        receiver: Receiver<std::result::Result<(), wgpu::BufferAsyncError>>,
        wall_temperature_buffer: &mut [[f32; 2]],
    ) {
        _ = receiver.await;
        {
            let data = self.wall_q_in_buffer.slice(..).get_mapped_range();
            wall_temperature_buffer.copy_from_slice(bytemuck::cast_slice(&data));
        }
        self.wall_q_in_buffer.unmap();
    }
}

/// Create the setup [`BindGroup`] with all the [`Buffer`]s.
#[inline]
fn setup_bind_group(
    device: &Device,
    compute_pipeline: &ComputePipeline,
    wall_elements: &[u8],
) -> BindGroup {
    let wall_elements_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Wall Elements Buffer"),
        contents: wall_elements,
        usage: BufferUsages::STORAGE,
    });

    let setup_bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let setup_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Setup Bind Group"),
        layout: &setup_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: wall_elements_buffer.as_entire_binding(),
        }],
    });
    setup_bind_group
}
