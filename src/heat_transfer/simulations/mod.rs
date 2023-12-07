use crate::{
    fds::{parse_script_from_file, Material, Meta, SurfaceCell},
    heat_transfer::one_dimensional::WallElement,
};
use anyhow::*;
use clap::ValueEnum;
use std::fmt::Display;
use std::{path::Path, vec};

mod benchmark;
pub mod temperature;
pub use benchmark::*;

use super::one_dimensional;

/// The factor the gas delta time is multiplied to get the solid delta time.
pub const DELTA_TIME_SOLID_FACTOR: u8 = 2;

/// All supported simulation methods.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SimulationMethod {
    #[clap(name = "1d_cpu")]
    OneDimensionalCpu = 0b00000001,
    #[clap(name = "1d_gpu_m1")]
    OneDimensionalGpuM1 = 0b00000010,
    #[clap(name = "1d_gpu_m2")]
    OneDimensionalGpuM2 = 0b00000100,
    #[clap(name = "1d_gpu_m3")]
    OneDimensionalGpuM3 = 0b00001000,
    #[clap(name = "1d")]
    OneDimensional = 0b00001111,
    #[clap(name = "fds")]
    SpeedTestFDS = 0b10000000,
}
impl SimulationMethod {
    pub fn is_simulation_type(&self, simulation_types: Option<&[SimulationMethod]>) -> bool {
        match simulation_types {
            Some(simulation_types) => simulation_types
                .iter()
                .any(|s| (*self) as u16 & (*s) as u16 != 0),
            None => true,
        }
    }

    /// Returns the path str of this [`SimulationMethod`].
    ///
    /// # Errors
    ///
    /// This function will return an error if this is [`SimulationMethod::OneDimensional`].
    pub fn path_str(&self) -> Result<&'static str> {
        match self {
            SimulationMethod::OneDimensionalCpu => Ok(SimulationType1D::Cpu.path_str()),
            SimulationMethod::OneDimensionalGpuM1 => Ok(SimulationType1D::GpuM1.path_str()),
            SimulationMethod::OneDimensionalGpuM2 => Ok(SimulationType1D::GpuM2.path_str()),
            SimulationMethod::OneDimensionalGpuM3 => Ok(SimulationType1D::GpuM3.path_str()),
            SimulationMethod::SpeedTestFDS => Ok("fds"),
            SimulationMethod::OneDimensional => {
                bail!("{self:?} is a collection, therefore has no distinct path.")
            }
        }
    }
}
impl From<SimulationType1D> for SimulationMethod {
    fn from(value: SimulationType1D) -> Self {
        match value {
            SimulationType1D::Cpu => Self::OneDimensionalCpu,
            SimulationType1D::GpuM1 => Self::OneDimensionalGpuM1,
            SimulationType1D::GpuM2 => Self::OneDimensionalGpuM2,
            SimulationType1D::GpuM3 => Self::OneDimensionalGpuM3,
        }
    }
}

/// All simulation methods for the one dimensional heat transfer.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimulationType1D {
    Cpu,
    GpuM1,
    GpuM2,
    GpuM3,
}
impl SimulationType1D {
    pub const ALL_1D: [SimulationType1D; 4] = [
        SimulationType1D::Cpu,
        SimulationType1D::GpuM1,
        SimulationType1D::GpuM2,
        SimulationType1D::GpuM3,
    ];
    pub fn path_str(&self) -> &'static str {
        match self {
            SimulationType1D::Cpu => "cpu",
            SimulationType1D::GpuM1 => "gpu_m1",
            SimulationType1D::GpuM2 => "gpu_m2",
            SimulationType1D::GpuM3 => "gpu_m3",
        }
    }
    pub fn is_simulation_type(&self, simulation_types: Option<&[SimulationMethod]>) -> bool {
        match simulation_types {
            Some(simulation_types) => {
                let left: SimulationMethod = (*self).into();
                simulation_types
                    .iter()
                    .any(|s| left as u16 & (*s) as u16 != 0)
            }
            None => true,
        }
    }
}
impl Display for SimulationType1D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SimulationType1D::Cpu => "CPU",
            SimulationType1D::GpuM1 => "GPU M1",
            SimulationType1D::GpuM2 => "GPU M2",
            SimulationType1D::GpuM3 => "GPU M3",
        };
        write!(f, "{s}")
    }
}

/// Calculate how many time a single [`WallElement`] is duplicate
///
/// # Errors
///
/// This function will return an error if in the simulation are multiple surfaces defined and the amount of surfaces, witch are equal to the amount of measured wall elements inside a fds simulation, and is not completely divisible the desired element count.
fn duplication(elements: usize, wall_elements: usize) -> Result<usize> {
    if elements % wall_elements > 0 {
        bail!(
            "Element count ({elements}) must be divisible without residue by wall element count {}.",
            wall_elements
        );
    }
    Ok(elements / wall_elements)
}

/// Loads the FDS simulation for a one dimensional simulation.
///
/// # Errors
///
/// This function will return an error if
/// - the passed file can not be parsed.
/// - the file is defined as 3D inside the meta data
pub fn load_fds_simulation_one_dimensional<P: AsRef<Path>>(
    path: P,
) -> Result<(Vec<Material>, Vec<WallElement>)> {
    let path = path.as_ref();
    let simulation_file_path = path.join("heat_transfer.fds");

    let (meta, material_list, surface_list) = parse_script_from_file(simulation_file_path)
        .with_context(|| format!("Failed to parse script at {path:?}."))?;
    match meta {
        Meta::OneDimensional { surface_ids } => {
            let mut wall_elements = vec![];
            for surface_id in surface_ids {
                let wall_cells = surface_list[surface_id]
                    .1
                    .iter()
                    .map(
                        |SurfaceCell { material_id, size }| one_dimensional::WallCell {
                            material: *material_id,
                            size: *size,
                            temperature: 20.0,
                        },
                    )
                    .collect::<Vec<_>>();
                wall_elements.push(WallElement::new(wall_cells))
            }

            let materials = material_list.into_materials();

            Ok((materials, wall_elements))
        }
        Meta::ThreeDimensional { .. } => {
            bail!("{path:?} is a 3D simulation.")
        }
    }
}

/// All kinds of simulations that can be run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SimulationKind {
    Diabatic = 0b001,
    DiabaticOneSide = 0b010,
    Adiabatic = 0b100,
}
impl SimulationKind {
    /// Checks if this [`SimulationKind`] is defined in the parsed argument.
    pub fn is_simulation_kind(&self, simulation_kind: Option<&[SimulationKind]>) -> bool {
        match simulation_kind {
            Some(simulation_types) => simulation_types
                .iter()
                .any(|k| (*self) as u8 & (*k) as u8 != 0),
            None => true,
        }
    }
}
