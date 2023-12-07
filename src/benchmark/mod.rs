//! The benchmarks are defined and executed in this module.

use crate::{
    fds::{self},
    heat_transfer::simulations::{
        one_dimensional_by_simulation_type, SimulationKind, SimulationMethod, SimulationType1D,
    },
};
use anyhow::*;
use clap::ValueEnum;

/// The different quantities of wall elements that are tested.
pub const BENCHMARK_ELEMENTS: [usize; 8] = [256, 512, 1024, 2048, 4096, 8192, 16384, 32768];
/// The different quantities of wall elements that are additionally tested in order to check the adjustment using chunks.
pub const BENCHMARK_CHUNK: [usize; 3] = [32768 * 2, 32768 * 4, 32768 * 8];

/// All possible benchmarks that can be performed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BenchmarkName {
    /// Diabatic simulation with 1 material.
    #[clap(name = "d1kc")]
    Diabatic1MaterialKC = 1 << 0,
    /// Diabatic simulation with 2 material.
    #[clap(name = "d2kc")]
    Diabatic2MaterialsKC = 1 << 1,
    /// Diabatic simulation with 4 material.
    #[clap(name = "d4kc")]
    Diabatic4MaterialsKC = 1 << 2,
    /// Diabatic simulation with 8 material.
    #[clap(name = "d8kc")]
    Diabatic8MaterialsKC = 1 << 3,
    /// Diabatic simulation with 16 material.
    #[clap(name = "d16kc")]
    Diabatic16MaterialsKC = 1 << 4,
    /// All diabatic simulations that test the influence of multiple materials.
    #[clap(name = "dxkc")]
    DiabaticAllMaterialsKC = (1 << 0) + (1 << 1) + (1 << 2) + (1 << 3) + (1 << 4),

    /// Diabatic simulation with concrete material.
    #[clap(name = "concrete")]
    DiabaticConcreteKC = 1 << 5,
    /// Diabatic simulation with steel material.
    #[clap(name = "steel")]
    DiabaticSteelKC = 1 << 6,
    /// Both diabatic simulations with concrete and steel.
    #[clap(name = "compare")]
    DiabaticCompareKC = (1 << 5) + (1 << 6),

    /// Diabatic simulation with steel material and 5cm wall thickness.
    #[clap(name = "thickness_005")]
    DiabaticThickness005 = 1 << 7,
    /// Diabatic simulation with steel material and 10cm wall thickness.
    #[clap(name = "thickness_010")]
    DiabaticThickness010 = 1 << 8,
    /// Diabatic simulation with steel material and 50cm wall thickness.
    #[clap(name = "thickness_050")]
    DiabaticThickness050 = 1 << 9,
    /// Diabatic simulation with steel material and 100cm wall thickness.
    #[clap(name = "thickness_100")]
    DiabaticThickness100 = 1 << 10,
    /// Diabatic simulation with steel material and 500cm wall thickness.
    #[clap(name = "thickness_500")]
    DiabaticThickness500 = 1 << 11,
    /// All diabatic simulations with steel material and different thicknesses.
    #[clap(name = "thickness")]
    DiabaticThickness = (1 << 7) + (1 << 8) + (1 << 9) + (1 << 10) + (1 << 11),

    /// Adiabatic simulation to compare with FDS.
    #[clap(name = "adiabatic")]
    Adiabatic = 1 << 12,
    /// Adiabatic FDS simulation.
    #[clap(name = "fds_speed_test")]
    SpeedTestFDS = 1 << 13,
}

impl BenchmarkName {
    /// All diabatic simulations that test the influence of multiple materials.
    pub const MATERIAL: [BenchmarkName; 7] = [
        BenchmarkName::Diabatic1MaterialKC,
        BenchmarkName::Diabatic2MaterialsKC,
        BenchmarkName::Diabatic4MaterialsKC,
        BenchmarkName::Diabatic8MaterialsKC,
        BenchmarkName::Diabatic16MaterialsKC,
        BenchmarkName::DiabaticConcreteKC,
        BenchmarkName::DiabaticSteelKC,
    ];
    /// All diabatic simulations with steel material and different thicknesses.
    pub const THICKNESS: [BenchmarkName; 5] = [
        BenchmarkName::DiabaticThickness005,
        BenchmarkName::DiabaticThickness010,
        BenchmarkName::DiabaticThickness050,
        BenchmarkName::DiabaticThickness100,
        BenchmarkName::DiabaticThickness500,
    ];

    pub fn is_benchmark(self, benchmark_names: Option<&[BenchmarkName]>) -> bool {
        match benchmark_names {
            Some(benchmark_names) => benchmark_names
                .iter()
                .any(|b| (self as u32 & *b as u32) > 0),
            None => true,
        }
    }

    /// Returns a reference to the path of this [`BenchmarkName`].
    ///
    /// # Panics
    /// Panics if this [`BenchmarkName`] is a collection of benchmarks.
    pub fn path_str(&self) -> &str {
        match self {
            BenchmarkName::Diabatic1MaterialKC => "fds/1D/Diabatic/multiple/1",
            BenchmarkName::Diabatic2MaterialsKC => "fds/1D/Diabatic/multiple/2",
            BenchmarkName::Diabatic4MaterialsKC => "fds/1D/Diabatic/multiple/4",
            BenchmarkName::Diabatic8MaterialsKC => "fds/1D/Diabatic/multiple/8",
            BenchmarkName::Diabatic16MaterialsKC => "fds/1D/Diabatic/multiple/16",

            BenchmarkName::DiabaticConcreteKC => "fds/1D/Diabatic/concrete_k_c",
            BenchmarkName::DiabaticSteelKC => "fds/1D/Diabatic/steel_k_c",

            BenchmarkName::DiabaticThickness005 => "fds/1D/Diabatic/thickness_steel_k_c/005cm",
            BenchmarkName::DiabaticThickness010 => "fds/1D/Diabatic/thickness_steel_k_c/010cm",
            BenchmarkName::DiabaticThickness050 => "fds/1D/Diabatic/thickness_steel_k_c/050cm",
            BenchmarkName::DiabaticThickness100 => "fds/1D/Diabatic/thickness_steel_k_c/100cm",
            BenchmarkName::DiabaticThickness500 => "fds/1D/Diabatic/thickness_steel_k_c/500cm",

            BenchmarkName::Adiabatic => "fds/1D/Adiabatic/concrete_k_c",

            BenchmarkName::DiabaticAllMaterialsKC
            | BenchmarkName::DiabaticThickness
            | BenchmarkName::SpeedTestFDS
            | BenchmarkName::DiabaticCompareKC => {
                panic!("{self:?} is a collection, therefore there is no path.")
            }
        }
    }
}

/// This function executes all benchmarks that are defined via `simulation_methods`, `sumulation_kinds` and `benchmark_names`.
/// - If `sumulation_methods` == [`None`], all simulation methods are checked.
/// - If `sumulation_kinds` == [`None`], all simulation types are checked.
/// - If `benchmark_names` == [`None`], all benchmarks are performed.
/// # Errors
///
/// This function will return an error if the simulations can not be started.
pub fn run_benchmark(
    name: &str,
    simulation_methods: Option<&[SimulationMethod]>,
    simulation_kinds: Option<&[SimulationKind]>,
    benchmark_names: Option<&[BenchmarkName]>,
) -> Result<()> {
    println!("Benchmarks");
    for benchmark_name in BenchmarkName::MATERIAL {
        if benchmark_name.is_benchmark(benchmark_names) {
            for simulation_type in [
                SimulationType1D::Cpu,
                SimulationType1D::GpuM1,
                SimulationType1D::GpuM2,
                SimulationType1D::GpuM3,
            ] {
                if simulation_type.is_simulation_type(simulation_methods)
                    && SimulationKind::Diabatic.is_simulation_kind(simulation_kinds)
                {
                    println!(" {:?} with {:?}.", benchmark_name, simulation_type);
                    one_dimensional_by_simulation_type(
                        benchmark_name.path_str(),
                        name.to_string(),
                        &BENCHMARK_ELEMENTS,
                        SimulationKind::Diabatic,
                        simulation_type,
                    )?;
                }
            }
        }
    }
    for benchmark_name in BenchmarkName::MATERIAL {
        if benchmark_name.is_benchmark(benchmark_names) {
            let simulation_type = SimulationType1D::GpuM3;
            if simulation_type.is_simulation_type(simulation_methods)
                && SimulationKind::Diabatic.is_simulation_kind(simulation_kinds)
            {
                println!(" {:?} with {:?}.", benchmark_name, simulation_type);
                one_dimensional_by_simulation_type(
                    benchmark_name.path_str(),
                    name.to_string(),
                    &BENCHMARK_CHUNK,
                    SimulationKind::Diabatic,
                    simulation_type,
                )?;
            }
        }
    }

    for benchmark_name in BenchmarkName::THICKNESS {
        if benchmark_name.is_benchmark(benchmark_names) {
            for simulation_type in [SimulationType1D::Cpu, SimulationType1D::GpuM3] {
                if simulation_type.is_simulation_type(simulation_methods)
                    && SimulationKind::Diabatic.is_simulation_kind(simulation_kinds)
                {
                    println!(" {:?} with {:?}.", benchmark_name, simulation_type);
                    one_dimensional_by_simulation_type(
                        benchmark_name.path_str(),
                        name.to_string(),
                        &BENCHMARK_ELEMENTS,
                        SimulationKind::Diabatic,
                        simulation_type,
                    )?;
                }
            }
        }
    }
    for benchmark_name in BenchmarkName::THICKNESS {
        if benchmark_name.is_benchmark(benchmark_names) {
            let simulation_type = SimulationType1D::GpuM3;
            if simulation_type.is_simulation_type(simulation_methods)
                && SimulationKind::Diabatic.is_simulation_kind(simulation_kinds)
            {
                println!(" {:?} with {:?}.", benchmark_name, simulation_type);
                one_dimensional_by_simulation_type(
                    benchmark_name.path_str(),
                    name.to_string(),
                    &BENCHMARK_CHUNK,
                    SimulationKind::Diabatic,
                    simulation_type,
                )?;
            }
        }
    }

    if BenchmarkName::Adiabatic.is_benchmark(benchmark_names) {
        for simulation_type in [
            SimulationType1D::Cpu,
            SimulationType1D::GpuM1,
            SimulationType1D::GpuM2,
            SimulationType1D::GpuM3,
        ] {
            if simulation_type.is_simulation_type(simulation_methods)
                && SimulationKind::Adiabatic.is_simulation_kind(simulation_kinds)
            {
                println!(
                    " {:?} with {:?}.",
                    BenchmarkName::Adiabatic,
                    simulation_type
                );
                one_dimensional_by_simulation_type(
                    BenchmarkName::Adiabatic.path_str(),
                    name.to_string(),
                    &BENCHMARK_ELEMENTS,
                    SimulationKind::Adiabatic,
                    simulation_type,
                )?;
            }
        }
    }

    if BenchmarkName::SpeedTestFDS.is_benchmark(benchmark_names) {
        println!("Speed Test");
        fds::benchmark(name)?
    }

    Ok(())
}
