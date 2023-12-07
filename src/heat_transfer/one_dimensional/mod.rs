use crate::fds::Material;
use anyhow::*;
use bytemuck::{Pod, Zeroable};
use futures::Future;
use std::{
    ops::{Deref, DerefMut},
    task::Poll,
};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferDescriptor, BufferUsages,
    ComputePipeline, Device, MaintainBase,
};

pub mod cpu;
pub mod gpu_m1;
pub mod gpu_m2;
pub mod gpu_m3;

/// The default maximal wall elements fo one chunk. This value is replaced at the start of the program.
static mut MAX_ELEMENTS_PER_CHUNK: usize = 16384;

/// Set the maximal wall element count per chunk. This should only run once!
/// If this is run other then the start of the program, the wohle simulation might not work.
///
/// # Unsafe
///
/// It sets the static value of [`MAX_ELEMENTS_PER_CHUNK`] without checking if the value is read at the same time.
pub fn set_max_element_per_chunk(max_elements_per_chunk: usize) {
    unsafe {
        MAX_ELEMENTS_PER_CHUNK = max_elements_per_chunk;
    }
    println!("Set max element per chunk to {max_elements_per_chunk}")
}

/// Set the maximal wall element count per chunk. This should be used after the value was set.
///
/// # Unsafe
///
/// It gets the static value of [`MAX_ELEMENTS_PER_CHUNK`] without checking if the value is written at the same time.
/// Since the value should only be written at the start of the program, no problem should occur.
#[inline]
pub fn get_max_element_per_chunk() -> usize {
    unsafe { MAX_ELEMENTS_PER_CHUNK }
}

/// The data of a single [`WallCell`]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WallCell {
    pub size: f32,
    pub material: u32,
    pub temperature: f32,
}
unsafe impl Zeroable for WallCell {}
unsafe impl Pod for WallCell {}

/// The data of a single one dimensional [`WallElement`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WallElement(pub Vec<WallCell>);
impl WallElement {
    pub fn new(inner: Vec<WallCell>) -> Self {
        Self(inner)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn into_iter(self) -> std::vec::IntoIter<WallCell> {
        self.0.into_iter()
    }
}

impl Deref for WallElement {
    type Target = Vec<WallCell>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for WallElement {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct DeviceFuture<'a>(&'a Device);
impl<'a> Future for DeviceFuture<'a> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if !self.0.poll(MaintainBase::Poll) {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// Trait for initializing and updating a heat transfer simulation.
pub trait HeatTransfer1D {
    /// Initialize the data for a heat transfer simulation for a fixed number of [`WallElement`]s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the initialization fails.
    fn setup(materials: Vec<Material>, wall_elements: Vec<WallElement>) -> Result<Self>
    where
        Self: Sized;

    /// Updates the heat transfer with the next time step.
    ///
    /// # Errors
    ///
    /// This function will return an error if the update fails.
    fn update(
        &mut self,
        delta_time: f32,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
        wall_temperature: &mut [[f32; 2]],
    ) -> Result<()>;
}

/// Create the update [`BindGroup`] with all the [`Buffer`]s.
#[inline]
fn update_bind_group(
    device: &Device,
    compute_pipeline: &ComputePipeline,
    wall_element_count: usize,
    index: u32,
) -> (BindGroup, Buffer, Buffer, Buffer) {
    let wall_heat_transfer_coefficients_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Wall Heat Transfer Coefficients"),
        size: (std::mem::size_of::<[f32; 2]>() * wall_element_count) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let wall_q_in_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Wall Energy Insertions"),
        size: (std::mem::size_of::<[f32; 2]>() * wall_element_count) as u64,
        usage: BufferUsages::STORAGE
            | BufferUsages::MAP_READ
            | BufferUsages::MAP_WRITE
            | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let delta_time_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Delta Time"),
        size: std::mem::size_of::<f32>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let update_bind_group_layout = compute_pipeline.get_bind_group_layout(index);
    let update_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Update Bind Group"),
        layout: &update_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: wall_heat_transfer_coefficients_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: wall_q_in_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: delta_time_buffer.as_entire_binding(),
            },
        ],
    });
    (
        update_bind_group,
        wall_heat_transfer_coefficients_buffer,
        wall_q_in_buffer,
        delta_time_buffer,
    )
}
