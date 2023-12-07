use std::{borrow::Cow, path::Path};

use anyhow::*;
use wgpu::{
    Backends, Device, DeviceDescriptor, Features, InstanceDescriptor, Limits, Queue,
    RequestAdapterOptions, ShaderModule,
};

/// Get the [`Device`] and [`Queue`] of the GPU.
///
/// # Errors
///
/// This function will return an error if either the [`Device`] ore the [`Queue`] can not be obtained.
pub async fn get_gpu_device_and_queue() -> Result<(Device, Queue)> {
    // Instantiates instance of WebGPU
    let instance = wgpu::Instance::new(InstanceDescriptor {
        backends: Backends::VULKAN | Backends::DX12,
        ..Default::default()
    });

    // `request_adapter` instantiates the general connection to the GPU in high Performance mode
    let Some(adapter) = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
    else {
        bail!("No adapter found.");
    };

    let (
        //Open connection to a graphics and/or compute device.
        device,
        //Handle to a command queue on a device.
        queue,
    ) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("Heat Transfer"),
                features: Features::MAPPABLE_PRIMARY_BUFFERS | Features::BUFFER_BINDING_ARRAY,
                limits: Limits::default(),
            },
            None, // No tracing.
        )
        .await
        .with_context(|| "No device found.")?;

    Ok((device, queue))
}

/// Loads a shader from a file.
///
/// # Errors
///
/// This function will return an error if the passed file can not be read.
pub fn load_shader_module<P: AsRef<Path>>(
    path: P,
    label: &str,
    device: &Device,
) -> Result<ShaderModule> {
    let shader_string = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read shader from {:?}", path.as_ref()))?;

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(Cow::from(shader_string)),
    });

    Ok(shader_module)
}
