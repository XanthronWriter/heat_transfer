//! This program checks whether and how one dimensional heat transport can be calculated on a graphics card. For this purpose, a CPU algorithm and 3 GPU algorithms were created. The heat transport itself was implemented according to the FDS Technical Reference Guide. This programme was created as part of the master thesis "Ausführung eines Wärmetransportalgorithmus auf einer GPU".

pub mod benchmark;
pub mod fds;
pub mod heat_transfer;
pub mod modification;
#[cfg(feature = "plot")]
pub mod plot;

use anyhow::{self, Context};
use benchmark::{run_benchmark, BenchmarkName};
use clap::Parser;
use fds::{create_simulations, run_simulations};
use heat_transfer::simulations::{SimulationKind, SimulationMethod};
#[cfg(feature = "plot")]
use plot::{plot_simulations, PlotType};

/// Run and evaluate heat transfer simulations on cpu and gpu.
#[derive(Parser)]
struct Cli {
    /// Set this flag to start the fds simulations.
    #[arg(short, long)]
    simulations: bool,

    /// The name the benchmark is assigned to. If not set no benchmark is done.
    #[arg(short, long, value_name = "NAME")]
    benchmark: Option<String>,

    /// Set witch benchmarks should be run. If not set all will run.
    #[arg(short = 'n', long, value_name = "[BENCHMARKS]", num_args = 1.., value_delimiter = ',')]
    benchmark_name: Option<Vec<BenchmarkName>>,

    /// Set the simulation method wich should be used. If empty all methods will be used.
    #[arg(short, long, value_name = "[METHOD]", value_enum, num_args = 1.., value_delimiter = ',')]
    method: Option<Vec<SimulationMethod>>,

    /// Set this flag, to create plots.
    #[cfg_attr(feature = "plot", arg(short, long, value_name = "TYP"))]
    #[cfg(feature = "plot")]
    plots: Option<PlotType>,
    #[cfg(not(feature = "plot"))]
    plots: (),

    /// Set the simulation kind wich should be used. If empty all kinds will be used.
    #[arg(short, long, value_name = "[KIND]", num_args = 1.., value_delimiter = ',')]
    kind: Option<Vec<SimulationKind>>,

    /// Set this flag, to continue even when an error occurs.
    #[arg(short, long)]
    force: bool,
}

fn evaluate_errors(errors: Result<(), Vec<anyhow::Error>>, cli: &Cli) -> bool {
    if let Err(err) = errors {
        println!("\n\n");
        for e in err {
            println!("{e:?}\n");
        }
        if !cli.force {
            return true;
        } else {
            println!("\n\nSome fds simulations failed to run. Aborting the program.\nUse -f or --force to force continuing");
        }
    }
    false
}

fn set_max_element_per_chunk(label: &str) -> anyhow::Result<()> {
    for line in std::fs::read_to_string("benchmarks.txt")
        .with_context(|| "Failed to read file at \"benchmarks.txt\"")?
        .lines()
    {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let mut splits = line.split('=');
        if let (Some(name), Some(number)) = (splits.next(), splits.next()) {
            if name.trim() == label {
                let number = number
                    .trim()
                    .parse::<usize>()
                    .with_context(|| format!("Failed to parse {number}"))?;
                heat_transfer::one_dimensional::set_max_element_per_chunk(number);
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!("No chunk size assigned to the current benchmark label \"{label}\". Insert \"{label} = [size]\" inside \"benchmark.txt\" as a new line."))
}

fn main() {
    let cli = Cli::parse();

    if cli.simulations {
        println!("\nStart creation of fds simulations from templates");
        if evaluate_errors(
            create_simulations(cli.method.as_deref(), cli.kind.as_deref()),
            &cli,
        ) {
            return;
        }

        println!("\nStart running of fds simulations");
        if evaluate_errors(
            run_simulations(cli.method.as_deref(), cli.kind.as_deref()),
            &cli,
        ) {
            return;
        }
    }

    // Because of cfg(debug_assertions) the warning below is emitted. Therefore allow(unused_assignments) attribute is set.
    #[allow(unused_assignments)]
    #[allow(unused_mut)]
    let mut release_mode = true;
    #[cfg(debug_assertions)]
    {
        release_mode = false;
    }
    if let Some(name) = &cli.benchmark {
        if let Err(err) = set_max_element_per_chunk(name) {
            println!("{}", err);
            if !&cli.force {
                return;
            }
        } else if release_mode {
            let result = run_benchmark(
                name,
                cli.method.as_deref(),
                cli.kind.as_deref(),
                cli.benchmark_name.as_deref(),
            )
            .map_err(|err| vec![err]);

            if evaluate_errors(result, &cli) {
                return;
            }
        } else {
            println!("Compile the program in release mode first to run benchmarks.");
            return;
        }
    }

    if let Ok(profile) = std::env::var("PROFILE") {
        println!("cargo:rustc-cfg=build={:?}", profile);
    }

    #[cfg(feature = "plot")]
    if let Some(plot_type) = cli.plots {
        println!("\nStart plotting");
        if evaluate_errors(
            plot_simulations(
                plot_type,
                cli.method.as_deref(),
                cli.kind.as_deref(),
                cli.benchmark_name.as_deref(),
            ),
            &cli,
        ) {
            return;
        }
    }

    println!("\nFinished without errors");
}
