use std::{io::Write, num::ParseFloatError, path::PathBuf};

use anyhow::Context;

use crate::heat_transfer::simulations::{
    BenchmarkPathPart, BenchmarkWriter, SimulationMethod, SIMULATION_RERUNS,
};

use super::{
    sampler::create_simulation_for_speed_test,
    simulations::{run_simulation_unchecked, Status},
};

pub const PATH: &str = "fds/1D/AdiabaticSpeedTest";

/// Executes the adiabatic FDS simulations that serve as a benchmark. Each simulation is repeated [`SIMULATION_RERUNS`] times. The time to calculate the walls is selected from the created `heat_transfer_cpu.csv` file. If several threads are executed, the time required is saved for each individual thread. The median is therefore selected from the values and written to the benchmark file.
///
/// # Panics
///
/// Panics if the executed simulation returns the status [`Status::Passed`], which should not happen.
///
/// # Errors
///
/// This function will return an error if
/// - an error occurs during the simulation.
/// - This function will return an error if `heat_transfer_cpu.csv` cannot be read or the values cannot be determined from the file.
pub fn benchmark(label: &str) -> Result<(), anyhow::Error> {
    for (simulation_path, size, cores) in create_simulation_for_speed_test()? {
        println!("\n Run FDS simulation at {simulation_path:?}");
        print!("  0/{SIMULATION_RERUNS}");
        std::io::stdout().flush().unwrap();
        let parent = simulation_path.parent().unwrap();
        let mut benchmark_writer = BenchmarkWriter::try_new(
            PathBuf::from("benchmark").join(parent.parent().unwrap()),
            &BenchmarkPathPart::new(
                if cores == 1 {
                    Some("single_core".to_string())
                } else {
                    Some("multi_core".to_string())
                },
                label.to_string(),
                SimulationMethod::SpeedTestFDS,
            ),
            size,
        )?;
        let read_file = parent.join("result/heat_transfer_cpu.csv");
        for i in 0..SIMULATION_RERUNS {
            match run_simulation_unchecked(simulation_path.clone(), cores)? {
                Status::Passed(_) => unreachable!(),
                Status::Succeeded(_) => {
                    let mut time = std::fs::read_to_string(&read_file)
                        .with_context(|| format!("Failed to read file at {read_file:?}."))?
                        .lines()
                        .skip(1)
                        .filter_map(|l| l.split(',').nth(6))
                        .map(|s| s.trim().parse::<f64>())
                        .collect::<std::result::Result<Vec<f64>, ParseFloatError>>()?
                        .into_iter()
                        .collect::<Vec<_>>();
                    time.sort_by(|a, b| a.total_cmp(b));
                    let mean_time = if time.len() % 2 == 0 {
                        (time[time.len() / 2 - 1] + time[time.len() / 2]) / 2.0
                    } else {
                        time[time.len() / 2]
                    };
                    benchmark_writer.write(mean_time)?;
                    print!("\r  {}/{SIMULATION_RERUNS}", i + 1);
                    std::io::stdout().flush().unwrap();
                }
                Status::Failed(path) => {
                    anyhow::bail!("Failed to run simulation at path {path:?}")
                }
            }
        }
    }

    Ok(())
}
