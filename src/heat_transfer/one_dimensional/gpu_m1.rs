use std::borrow::Cow;

use futures::{
    executor::block_on,
    future::{join, join_all},
};
use futures_channel::oneshot::{channel, Receiver};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, Queue,
};

use crate::{fds::Material, heat_transfer::shader::insert_material_data};

use super::{
    super::gpu::get_gpu_device_and_queue, get_max_element_per_chunk, update_bind_group,
    DeviceFuture, HeatTransfer1D, WallCell, WallElement,
};
use anyhow::*;

/// The whole base shader for method 1.
pub const SHADER: &str = include_str!("gpu_m1.wgsl");

/// All relevant data for the heat transfer algorithm on the GPU with method 1.
pub struct GPUSetupData {
    device: Device,
    queue: Queue,
    compute_pipeline: ComputePipeline,
    chunks: Vec<Chunk>,
}

impl HeatTransfer1D for GPUSetupData {
    fn setup(materials: Vec<Material>, wall_elements: Vec<WallElement>) -> Result<Self> {
        let (device, queue) = block_on(get_gpu_device_and_queue())
            .with_context(|| "Failed to get device and queue.")?;

        let shader = insert_material_data(SHADER, &materials);
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

        let chunks = Chunk::build(&device, &compute_pipeline, wall_elements);
        let gpu_setup_data = GPUSetupData {
            device,
            queue,
            compute_pipeline,
            chunks,
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

/// All data for a single chunk.
struct Chunk {
    setup_bind_group: BindGroup,
    matrix_bind_group: BindGroup,
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
    ) -> Vec<Self> {
        let mut chunks = vec![];
        let mut cell_indices = vec![];
        let mut wall_cells = vec![];
        for wall_elements_chunk in wall_elements
            .into_iter()
            .map(Option::Some)
            .collect::<Vec<Option<WallElement>>>()
            .chunks_mut(get_max_element_per_chunk())
        {
            let mut last_size = 0;
            for wall_element in wall_elements_chunk {
                match wall_element.take() {
                    Some(mut wall_element) => {
                        last_size += wall_element.len() as u32;
                        cell_indices.push(last_size);
                        wall_cells.append(&mut wall_element.0);
                    }
                    None => unreachable!(),
                }
            }

            let wall_element_count = cell_indices.len();
            let cell_count = cell_indices[cell_indices.len() - 1] as usize;

            let setup_bind_group =
                setup_bind_group(device, compute_pipeline, &cell_indices, &wall_cells);
            cell_indices.clear();
            wall_cells.clear();

            let matrix_bind_group = matrix_bind_group(device, compute_pipeline, cell_count);
            let (
                update_bind_group,
                wall_heat_transfer_coefficients_buffer,
                wall_q_in_buffer,
                delta_time_buffer,
            ) = update_bind_group(device, compute_pipeline, wall_element_count, 2);

            let groups = (wall_element_count as f32 / 256.0).ceil() as u32;

            let chunk = Chunk {
                setup_bind_group,
                matrix_bind_group,
                update_bind_group,
                wall_heat_transfer_coefficients_buffer,
                wall_q_in_buffer,
                delta_time_buffer,
                groups,
            };
            chunks.push(chunk);
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
            update_compute_pass.set_bind_group(1, &self.matrix_bind_group, &[]);
            update_compute_pass.set_bind_group(2, &self.update_bind_group, &[]);
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
    cell_indices: &[u32],
    wall_cells: &[WallCell],
) -> BindGroup {
    let cell_indices_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Cell Indices Buffer"),
        contents: bytemuck::cast_slice(cell_indices),
        usage: BufferUsages::STORAGE,
    });
    let wall_cells_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Cell Sizes Buffer"),
        contents: bytemuck::cast_slice(wall_cells),
        usage: BufferUsages::STORAGE,
    });

    let setup_bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let setup_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Setup Bind Group"),
        layout: &setup_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: cell_indices_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: wall_cells_buffer.as_entire_binding(),
            },
        ],
    });
    setup_bind_group
}

/// Create the matrix [`BindGroup`] with all the [`Buffer`]s.
#[inline]
fn matrix_bind_group(
    device: &Device,
    compute_pipeline: &ComputePipeline,
    cell_count: usize,
) -> BindGroup {
    let solve_matrix_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("Solve Matrix Buffer"),
        size: (std::mem::size_of::<f32>() * cell_count) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let solve_matrix_buffer_d = device.create_buffer(&BufferDescriptor {
        label: Some("Solve Matrix Buffer"),
        size: (std::mem::size_of::<f32>() * cell_count) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let solve_matrix_buffer_a = device.create_buffer(&BufferDescriptor {
        label: Some("Solve Matrix Buffer"),
        size: (std::mem::size_of::<f32>() * cell_count) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let solve_matrix_buffer_c = device.create_buffer(&BufferDescriptor {
        label: Some("Solve Matrix Buffer"),
        size: (std::mem::size_of::<f32>() * cell_count) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let matrix_bind_group_layout = compute_pipeline.get_bind_group_layout(1);
    let matrix_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Matrix Bind Group"),
        layout: &matrix_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: solve_matrix_buffer_b.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: solve_matrix_buffer_d.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: solve_matrix_buffer_a.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: solve_matrix_buffer_c.as_entire_binding(),
            },
        ],
    });
    matrix_bind_group
}
