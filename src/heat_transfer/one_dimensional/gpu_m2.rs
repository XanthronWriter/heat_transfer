use std::borrow::Cow;

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

use crate::{
    fds::Material,
    heat_transfer::shader::{insert_gpu_m2_data, insert_material_data},
};

use super::{
    super::gpu::get_gpu_device_and_queue, update_bind_group, DeviceFuture, HeatTransfer1D,
    WallElement,
};

use anyhow::*;

/// The whole base shader for method 2.
pub const SHADER: &str = include_str!("gpu_m2.wgsl");

/// All relevant data for the heat transfer algorithm on the GPU with method 2.
pub struct GPUSetupData {
    device: Device,
    queue: Queue,
    shader_chunks: Vec<ShaderChunk>,
}

impl HeatTransfer1D for GPUSetupData {
    fn setup(materials: Vec<Material>, wall_elements: Vec<WallElement>) -> Result<Self> {
        let (device, queue) = block_on(get_gpu_device_and_queue())
            .with_context(|| "Failed to get device and queue.")?;
        let shader = insert_material_data(SHADER, &materials);
        let shader_chunks = ShaderChunk::build(&device, shader, wall_elements);

        let gpu_setup_data = GPUSetupData {
            device,
            queue,
            shader_chunks,
        };

        Ok(gpu_setup_data)
    }

    fn update(
        &mut self,
        delta_time: f32,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
        wall_temperature: &mut [[f32; 2]],
    ) -> Result<()> {
        let mut receivers = Vec::with_capacity(self.shader_chunks.len());
        let mut wall_temperature_buffer_chunk = wall_temperature;
        for (s, receiver) in self.shader_chunks.iter().map(|s| {
            let receiver = s.submit_update_to_queue(
                &self.device,
                &self.queue,
                delta_time,
                wall_heat_transfer_coefficients,
                wall_q_in,
            );
            (s, receiver)
        }) {
            let (split1, split2) = wall_temperature_buffer_chunk.split_at_mut(s.end - s.start);
            wall_temperature_buffer_chunk = split2;
            let receiver = s.receive_update(receiver, split1);
            receivers.push(receiver)
        }

        let _ = block_on(join(join_all(receivers), DeviceFuture(&self.device)));
        Ok(())
    }
}

/// All data fo a single shader.
struct ShaderChunk {
    start: usize,
    end: usize,
    compute_pipeline: ComputePipeline,
    setup_bind_group: BindGroup,
    update_bind_group: BindGroup,
    wall_heat_transfer_coefficients_buffer: Buffer,
    wall_q_in_buffer: Buffer,
    delta_time_buffer: Buffer,
    groups: u32,
}
impl ShaderChunk {
    /// Creates a new [`ShaderChunk`].
    fn new(
        end: usize,
        start: usize,
        device: &Device,
        shader: &str,
        cell_sizes: &[f32],
        cell_materials: &[u32],
        cell_temperatures: &[f32],
    ) -> ShaderChunk {
        let wall_element_count = end - start;
        let groups = (wall_element_count as f32 / 256.0).ceil() as u32;
        let (compute_pipeline, setup_bind_group) = setup_bind_group(
            device,
            shader,
            cell_sizes,
            cell_materials,
            cell_temperatures,
        );
        let (
            update_bind_group,
            wall_heat_transfer_coefficients_buffer,
            wall_q_in_buffer,
            delta_time_buffer,
        ) = update_bind_group(device, &compute_pipeline, wall_element_count, 1);

        ShaderChunk {
            start,
            end,
            compute_pipeline,
            setup_bind_group,
            update_bind_group,
            wall_heat_transfer_coefficients_buffer,
            wall_q_in_buffer,
            delta_time_buffer,
            groups,
        }
    }

    /// Build all [`ShaderChunk`]s for all [`WallElement`]s.
    fn build(device: &Device, shader: String, wall_elements: Vec<WallElement>) -> Vec<Self> {
        let mut shader_chunks = vec![];
        let (mut cell_sizes, mut cell_materials): (Vec<f32>, Vec<u32>) = wall_elements[0]
            .iter()
            .map(|cell| (cell.size, cell.material))
            .unzip();
        let mut cell_temperatures = vec![];
        let mut start = 0;
        let mut end = 0;
        for wall_element in wall_elements.into_iter() {
            if cell_sizes.len() != wall_element.len()
                || wall_element
                    .iter()
                    .zip(cell_sizes.iter().zip(cell_materials.iter()))
                    .any(|(c, (&s, &m))| c.size != s || c.material != m)
            {
                let shader_chunk = ShaderChunk::new(
                    end,
                    start,
                    device,
                    &shader,
                    &cell_sizes,
                    &cell_materials,
                    &cell_temperatures,
                );
                shader_chunks.push(shader_chunk);

                cell_sizes.clear();
                cell_materials.clear();
                cell_temperatures.clear();
                for cell in wall_element.iter() {
                    cell_sizes.push(cell.size);
                    cell_materials.push(cell.material);
                }
                start = end;
            }
            end += 1;
            for cell in wall_element.iter() {
                cell_temperatures.push(cell.temperature);
            }
        }
        let shader_chunk = ShaderChunk::new(
            end,
            start,
            device,
            &shader,
            &cell_sizes,
            &cell_materials,
            &cell_temperatures,
        );
        shader_chunks.push(shader_chunk);
        shader_chunks
    }

    /// Start the calculation for this [`Chunk`]
    #[inline]
    fn submit_update_to_queue(
        &self,
        device: &Device,
        queue: &Queue,
        delta_time: f32,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
    ) -> Receiver<std::result::Result<(), wgpu::BufferAsyncError>> {
        let wall_heat_transfer_coefficients =
            &wall_heat_transfer_coefficients[self.start..self.end];
        let wall_q_in = &wall_q_in[self.start..self.end];
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
            update_compute_pass.set_pipeline(&self.compute_pipeline);
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
    shader: &str,
    cell_sizes: &[f32],
    cell_materials: &[u32],
    cell_temperatures: &[f32],
) -> (ComputePipeline, BindGroup) {
    let shader = insert_gpu_m2_data(shader, cell_sizes, cell_materials);
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

    let cell_temperatures_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Cell Temperatures Buffer"),
        contents: bytemuck::cast_slice(cell_temperatures),
        usage: BufferUsages::STORAGE,
    });

    let setup_bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let setup_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Setup Bind Group"),
        layout: &setup_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: cell_temperatures_buffer.as_entire_binding(),
        }],
    });
    (compute_pipeline, setup_bind_group)
}
