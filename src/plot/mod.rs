mod kind;

use std::{path::PathBuf, thread};

use anyhow::*;
use clap::ValueEnum;
use rayon::prelude::*;

use self::kind::{benchmark_box_plot, helper_cell_count, helper_ramps_plot, helper_transistor};
use crate::{
    benchmark::{BenchmarkName, BENCHMARK_ELEMENTS},
    heat_transfer::simulations::{SimulationKind, SimulationMethod, SimulationType1D},
    plot::kind::{
        benchmark_box_plot::BenchmarkBoxPlotSources,
        benchmark_factor_plot::{self, BenchmarkFactorPlotSources},
        temperature_diff,
    },
};

/// All possible plot types.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum PlotType {
    Benchmark = 0b0001,
    Temperature = 0b0010,
    Helper = 0b0100,
    All = 0b0111,
}
impl PlotType {
    fn is_plot_type(self, plot_type: PlotType) -> bool {
        self as u8 & plot_type as u8 != 0
    }
}

/// All possible return stati a simulation can return.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Passed {
        simulation: &'static str,
        path: PathBuf,
    },
    Ignored {
        simulation: &'static str,
        reason: String,
        path: PathBuf,
    },
    Succeeded {
        simulation: &'static str,
        path: PathBuf,
    },
    Failed {
        simulation: &'static str,
        path: PathBuf,
    },
}

/// Print the state of all simulations and return `true`, if an error occurred.
pub fn print_plot_state(
    mut results: Vec<Result<Status, Error>>,
    errors: &mut Vec<anyhow::Error>,
) -> bool {
    let mut s = vec![];

    while let Some(result) = results.pop() {
        match result {
            std::result::Result::Ok(ok) => {
                s.push(ok);
            }
            Err(err) => {
                errors.push(err);
            }
        }
    }

    s.sort();

    let mut any_failed = false;
    for r in s {
        match r {
            Status::Ignored {
                simulation,
                reason,
                path,
            } => println!(
                "  \"{simulation}\" plot for fds simulation at {:?} was ignored because of {reason}.",
                path
            ),
            Status::Passed { simulation, path } => {
                println!(
                    "  \"{simulation}\" plot for fds simulation at {:?} is newer than the input file.",
                    path
                );
            }
            Status::Succeeded { simulation, path } => {
                println!(
                    "  Finished \"{simulation}\" plot for fds simulation at {:?} successfully.",
                    path
                );
            }
            Status::Failed { simulation, path } => {
                any_failed = true;
                println!(
                    "  Failed \"{simulation}\" plot for fds simulation at {:?}.",
                    path
                );
            }
        }
    }

    any_failed
}

/// Determine all names of all computers on which the benchmarks were performed.
///
/// # Errors
///
/// This function will return an error if the `benchmarks.txt` can not be read.
fn plot_benchmark_files() -> Result<Vec<String>> {
    let labels = std::fs::read_to_string("benchmarks.txt")
        .with_context(|| "Failed to read file at \"benchmarks.txt\"")?
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.starts_with('#'))
        .filter_map(|s| s.split('=').next())
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    Ok(labels)
}

/// Starts the temperature plot of all simulations.
///
/// # Panics
///
/// Panics if a panic occurred inside a thread.
///
/// # Errors
///
/// This function will return an error if any simulation failed to run.
pub fn plot_simulations(
    plot_type: PlotType,
    method: Option<&[SimulationMethod]>,
    kind: Option<&[SimulationKind]>,
    benchmark_names: Option<&[BenchmarkName]>,
) -> Result<(), Vec<anyhow::Error>> {
    let mut any_failed = false;
    let mut errors = vec![];

    if PlotType::Helper.is_plot_type(plot_type) {
        let results = [
            helper_cell_count::plot,
            helper_transistor::plot,
            helper_ramps_plot::plot_concrete_c,
            helper_ramps_plot::plot_concrete_k,
            helper_ramps_plot::plot_steel_c,
            helper_ramps_plot::plot_steel_k,
        ]
        .par_iter()
        .map(|v| v())
        .collect::<Vec<_>>();
        if print_plot_state(results, &mut errors) {
            any_failed = true;
        }
    }

    if PlotType::Benchmark.is_plot_type(plot_type) {
        let mut handles = vec![];
        println!("\n Plot Benchmarks");
        plot_benchmark_files()
            .map_err(|err| vec![err])?
            .into_iter()
            .for_each(|l| {
                BenchmarkName::MATERIAL.iter().for_each(|&b| {
                    if b.is_benchmark(benchmark_names) {
                        let l = l.clone();
                        let handle = thread::spawn(move || {
                            benchmark_box_plot::plot(BenchmarkBoxPlotSources::compare_mode(
                                b.path_str(),
                                l,
                                BENCHMARK_ELEMENTS,
                            ))
                        });
                        handles.push(handle);
                    }
                });
                SimulationType1D::ALL_1D.into_iter().for_each(|s| {
                    if s.is_simulation_type(method) {
                        let l = l.clone();
                        let handle = thread::spawn(move || {
                            benchmark_box_plot::plot(BenchmarkBoxPlotSources::compare_multiple(
                                "fds/1D/Diabatic/multiple",
                                l,
                                BENCHMARK_ELEMENTS,
                                [1, 2, 4, 8, 16],
                                s,
                            ))
                        });
                        handles.push(handle);
                    }
                });
                if SimulationMethod::SpeedTestFDS.is_simulation_type(method) {
                    let l = l.clone();
                    let handle = thread::spawn(|| {
                        benchmark_box_plot::plot(BenchmarkBoxPlotSources::compare_fds(
                            "fds/1D/AdiabaticSpeedTest",
                            "fds/1D/Adiabatic/concrete_k_c",
                            l,
                            BENCHMARK_ELEMENTS,
                            &[SimulationType1D::Cpu, SimulationType1D::GpuM3],
                        ))
                    });
                    handles.push(handle);
                }
                if BenchmarkName::DiabaticThickness.is_benchmark(benchmark_names) {
                    let l = l.clone();
                    let handle = thread::spawn(move || {
                        benchmark_factor_plot::plot(BenchmarkFactorPlotSources::thickness_mode(
                            "fds/1D/Diabatic/thickness_steel_k_c",
                            l,
                        ))
                    });
                    handles.push(handle);
                }
            });
        for [c1, c2] in [["desktop_l_n", "desktop_l"], ["laptop_l_n", "laptop_l"]] {
            if BenchmarkName::DiabaticConcreteKC.is_benchmark(benchmark_names) {
                let handle = thread::spawn(move || {
                    benchmark_factor_plot::plot(BenchmarkFactorPlotSources::compare_chunk_mode(
                        "fds/1D/Diabatic/concrete_k_c",
                        c1,
                        c2,
                    ))
                });
                handles.push(handle);
            }
            if BenchmarkName::DiabaticSteelKC.is_benchmark(benchmark_names) {
                let handle = thread::spawn(move || {
                    benchmark_factor_plot::plot(BenchmarkFactorPlotSources::compare_chunk_mode(
                        "fds/1D/Diabatic/steel_k_c",
                        c1,
                        c2,
                    ))
                });
                handles.push(handle);
            }
            if BenchmarkName::DiabaticThickness.is_benchmark(benchmark_names) {
                let handle = thread::spawn(move || {
                    benchmark_factor_plot::plot(
                        BenchmarkFactorPlotSources::compare_chunk_thickness_mode(
                            "fds/1D/Diabatic/thickness_steel_k_c",
                            c1,
                            c2,
                        ),
                    )
                });
                handles.push(handle);
            }
        }
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        if print_plot_state(results, &mut errors) {
            any_failed = true;
        }
    }
    if PlotType::Temperature.is_plot_type(plot_type) {
        println!("\n Plot Simulations");
        let mut handles = vec![];
        SimulationType1D::ALL_1D
            .into_iter()
            .for_each(|simulation_type| {
                if simulation_type.is_simulation_type(method) {
                    if SimulationKind::Adiabatic.is_simulation_kind(kind) {
                        for path in [
                            "fds/1D/Adiabatic/steel_simple",
                            "fds/1D/Adiabatic/steel_k_c",
                            "fds/1D/Adiabatic/concrete_simple",
                            "fds/1D/Adiabatic/concrete_k_c",
                        ] {
                            let handle = thread::spawn(move || {
                                temperature_diff::plot_one_dimensional_by_type(
                                    path,
                                    SimulationKind::Adiabatic,
                                    simulation_type,
                                )
                            });
                            handles.push(handle)
                        }
                    }
                    if SimulationKind::DiabaticOneSide.is_simulation_kind(kind) {
                        for path in [
                            "fds/1D/DiabaticOneSide/steel_simple",
                            "fds/1D/DiabaticOneSide/steel_k_c",
                            "fds/1D/DiabaticOneSide/concrete_simple",
                            "fds/1D/DiabaticOneSide/concrete_k_c",
                        ] {
                            let handle = thread::spawn(move || {
                                temperature_diff::plot_one_dimensional_by_type(
                                    path,
                                    SimulationKind::DiabaticOneSide,
                                    simulation_type,
                                )
                            });
                            handles.push(handle)
                        }
                    }
                    if SimulationKind::Diabatic.is_simulation_kind(kind) {
                        for path in [
                            "fds/1D/Diabatic/steel_simple",
                            "fds/1D/Diabatic/steel_k_c",
                            "fds/1D/Diabatic/concrete_simple",
                            "fds/1D/Diabatic/concrete_k_c",
                            // Thickness
                            "fds/1D/Diabatic/thickness_steel_k_c/005cm",
                            "fds/1D/Diabatic/thickness_steel_k_c/010cm",
                            "fds/1D/Diabatic/thickness_steel_k_c/050cm",
                            "fds/1D/Diabatic/thickness_steel_k_c/100cm",
                            "fds/1D/Diabatic/thickness_steel_k_c/500cm",
                        ] {
                            let handle = thread::spawn(move || {
                                temperature_diff::plot_one_dimensional_by_type(
                                    path,
                                    SimulationKind::Diabatic,
                                    simulation_type,
                                )
                            });
                            handles.push(handle)
                        }
                    }
                }
            });

        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        if print_plot_state(results, &mut errors) {
            any_failed = true;
        }
    }

    if any_failed {
        errors.push(anyhow!("Some fds simulations failed to run."));
    }
    if errors.is_empty() {
        std::result::Result::Ok(())
    } else {
        Err(errors)
    }
}
