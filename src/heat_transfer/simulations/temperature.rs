use crate::{
    fds::Devices,
    heat_transfer::one_dimensional::{
        cpu::{CPUSetupData, ADIABATIC_H, CONST_TEMP_H},
        gpu_m1, gpu_m2, gpu_m3, HeatTransfer1D,
    },
};
use anyhow::*;
use std::path::Path;

use super::{
    load_fds_simulation_one_dimensional, SimulationKind, SimulationType1D, DELTA_TIME_SOLID_FACTOR,
};

/// An helper struct for reading the simulation data for a temperature plot line by line witch means simulation step by simulation step.
pub struct SimulationTemperatureDevice1D {
    simulation_kind: SimulationKind,
    last_time: f32,
    device: Devices,
}
impl SimulationTemperatureDevice1D {
    /// Attempts to create a [`SimulationBenchmarkDevice`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the transmitted device file cannot be read.
    /// - the transmitted device file does not match the requested devices.
    pub fn try_new<P: AsRef<Path>>(simulation_kind: SimulationKind, path: P) -> Result<Self> {
        let path = path.as_ref();
        let device_path = path.join("result/heat_transfer_devc.csv");
        let device = match simulation_kind {
            SimulationKind::Diabatic => Devices::try_new(
                device_path,
                &[
                    "Time",
                    "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_WEST",
                    "DEVC_GAS_TEMPERATURE_WEST",
                    "DEVC_WALL_RADIATIVE_HEAT_FLUX_WEST",
                    "DEVC_WALL_TEMPERATURE_WEST",
                    "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_EAST",
                    "DEVC_GAS_TEMPERATURE_EAST",
                    "DEVC_WALL_RADIATIVE_HEAT_FLUX_EAST",
                    "DEVC_WALL_TEMPERATURE_EAST",
                ],
            )?,
            SimulationKind::DiabaticOneSide => Devices::try_new(
                device_path,
                &[
                    "Time",
                    "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_WEST",
                    "DEVC_GAS_TEMPERATURE_WEST",
                    "DEVC_WALL_RADIATIVE_HEAT_FLUX_WEST",
                    "DEVC_WALL_TEMPERATURE_WEST",
                    "DEVC_WALL_TEMPERATURE_EAST",
                ],
            )?,
            SimulationKind::Adiabatic => {
                Devices::try_new(device_path, &["Time", "DEVC_WALL_TEMPERATURE_WEST"])?
            }
        };
        std::result::Result::Ok(Self {
            simulation_kind,
            last_time: 0.0,
            device,
        })
    }
}
impl Iterator for SimulationTemperatureDevice1D {
    type Item = Result<(f32, [f32; 2], [f32; 2], [f32; 2])>;

    fn next(&mut self) -> Option<Self::Item> {
        for _ in 0..(DELTA_TIME_SOLID_FACTOR - 1) {
            if let Err(err) = self.device.next()? {
                return Some(Err(err));
            }
        }
        let data = match self.device.next()? {
            std::result::Result::Ok(ok) => ok,
            Err(err) => return Some(Err(err)),
        };

        let time = data[0];
        let delta_time = time - self.last_time;
        self.last_time = time;
        match self.simulation_kind {
            SimulationKind::Diabatic => {
                let wall_heat_transfer_coefficient = [data[1], data[5]];
                let wall_q_in = [
                    data[1] * data[2] + data[3] * 1000.0,
                    data[5] * data[6] + data[7] * 1000.0,
                ];
                let fds = [data[4], data[8]];
                Some(std::result::Result::Ok((
                    delta_time,
                    wall_heat_transfer_coefficient,
                    wall_q_in,
                    fds,
                )))
            }
            SimulationKind::DiabaticOneSide => {
                let wall_heat_transfer_coefficient = [data[1], ADIABATIC_H];
                let wall_q_in = [data[1] * data[2] + data[3] * 1000.0, 0.0];
                let fds = [data[4], data[5]];
                Some(std::result::Result::Ok((
                    delta_time,
                    wall_heat_transfer_coefficient,
                    wall_q_in,
                    fds,
                )))
            }
            SimulationKind::Adiabatic => {
                let wall_heat_transfer_coefficient = [CONST_TEMP_H, ADIABATIC_H];
                let wall_q_in = [200.0, 0.0];
                let fds = [200.0, data[1]];
                Some(std::result::Result::Ok((
                    delta_time,
                    wall_heat_transfer_coefficient,
                    wall_q_in,
                    fds,
                )))
            }
        }
    }
}

/// Wrapper to store the difference data of two simulations.
pub struct Diff {
    pub front: Vec<f32>,
    pub back: Vec<f32>,
}
/// Wrapper to store the temperature simulation data.
#[derive(Debug)]
pub struct Temperatures {
    pub time: Vec<f32>,
    pub fds_front: Vec<f32>,
    pub fds_back: Vec<f32>,
    pub sim_front: Vec<f32>,
    pub sim_back: Vec<f32>,
}

impl Temperatures {
    /// Calculates the difference of this [`Temperatures`] for FDS and this program.
    pub fn diff(&self) -> Diff {
        let front = self
            .fds_front
            .iter()
            .zip(self.sim_front.iter())
            .map(|(fds, sim)| fds - sim)
            .collect();
        let back = self
            .fds_back
            .iter()
            .zip(self.sim_back.iter())
            .map(|(fds, sim)| fds - sim)
            .collect();
        Diff { front, back }
    }
}

/// Execute a simulation to validate with FDS
///
/// # Errors
///
/// This function will return an error if
/// - the fds simulation file can not be loaded.
/// - the fds simulation defines multiple materials inside the meta data.
/// - it failed to initialize the simulation.
/// - it failed to update the simulation.
fn one_dimensional<P: AsRef<Path>, H: HeatTransfer1D>(
    path: P,
    simulation_kind: SimulationKind,
) -> Result<Temperatures> {
    let device: SimulationTemperatureDevice1D =
        SimulationTemperatureDevice1D::try_new(simulation_kind, &path).with_context(|| {
            format!(
                "Failed to build SimulationTemperatureDevice for file at {:?}",
                path.as_ref()
            )
        })?;
    let (materials, wall_elements) = load_fds_simulation_one_dimensional(&path)
        .with_context(|| format!("Failed to build simulation for file at {:?}", path.as_ref()))?;
    if wall_elements.len() > 1 {
        bail!("Multiple wall elements in meta defined, wich is not supported in simulate_collect_temperature.");
    }

    let mut heat_transfer =
        H::setup(materials, wall_elements).with_context(|| "Failed to setup heat transfer.")?;

    let mut time = vec![];
    let mut fds_front = vec![];
    let mut fds_back = vec![];
    let mut sim_front = vec![];
    let mut sim_back = vec![];

    let mut wall_temperature_buffer = [[0.0f32; 2]];
    let mut elapsed_time = 0.0;
    for data in device.skip(1) {
        let (delta_time, wall_heat_transfer_coefficient, wall_q_in, fds) = data?;
        fds_front.push(fds[0]);
        fds_back.push(fds[1]);

        heat_transfer
            .update(
                delta_time,
                &[wall_heat_transfer_coefficient],
                &[wall_q_in],
                &mut wall_temperature_buffer,
            )
            .with_context(|| "Failed to update heat transfer.")?;
        elapsed_time += delta_time;

        time.push(elapsed_time);
        sim_front.push(wall_temperature_buffer[0][0]);
        sim_back.push(wall_temperature_buffer[0][1]);
    }

    Ok(Temperatures {
        time,
        fds_front,
        fds_back,
        sim_front,
        sim_back,
    })
}

/// Start the CPU simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_cpu<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
) -> Result<Temperatures> {
    one_dimensional::<P, CPUSetupData>(path, simulation_kind)
}

/// Start the GPU M1 simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m1<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
) -> Result<Temperatures> {
    one_dimensional::<P, gpu_m1::GPUSetupData>(path, simulation_kind)
}

/// Start the GPU M2 simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m2<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
) -> Result<Temperatures> {
    one_dimensional::<P, gpu_m2::GPUSetupData>(path, simulation_kind)
}

/// Start the GPU M2 simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m3<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
) -> Result<Temperatures> {
    one_dimensional::<P, gpu_m3::GPUSetupData>(path, simulation_kind)
}

/// Start the simulation for a given simulation method.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_by_type<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
    simulation_type: SimulationType1D,
) -> Result<Temperatures> {
    match simulation_type {
        SimulationType1D::Cpu => one_dimensional_cpu(path, simulation_kind),
        SimulationType1D::GpuM1 => one_dimensional_gpu_m1(path, simulation_kind),
        SimulationType1D::GpuM2 => one_dimensional_gpu_m2(path, simulation_kind),
        SimulationType1D::GpuM3 => one_dimensional_gpu_m3(path, simulation_kind),
    }
}
