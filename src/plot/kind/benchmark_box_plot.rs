//! Create a boxplot that displays the simulation time for different methods.

use super::COLORS;
use crate::{
    heat_transfer::simulations::{
        BenchmarkPathPart, BenchmarkReader, SimulationType1D, SIMULATION_RERUNS,
    },
    modification::was_modified,
    plot::Status,
};
use anyhow::*;
use plotly::{
    box_plot::BoxPoints,
    common::{Line, Title},
    layout::{self, Axis, Legend, Margin},
    BoxPlot, ImageFormat, Layout, Plot,
};
use std::{
    fmt::Display,
    fs::create_dir_all,
    path::{Path, PathBuf},
    vec,
};

const SIMULATION_NAME: &str = "benchmark_box_plot.rs";

/// Data for a graph element inside a plot.
struct BenchmarkBoxPlotSource {
    /// direction fo the simulation.
    simulation_directory: PathBuf,
    /// suffix for `simulation_directory` for the full path
    benchmark_path_part: BenchmarkPathPart,
    /// Legend name.
    legend: String,
    /// Element color.
    color: String,
}

/// All data to create a boxplot that displays the simulation time for different methods
pub struct BenchmarkBoxPlotSources {
    /// The directory the plot should be saved.
    plot_directory: PathBuf,
    /// Data for all graph elements inside a plot.
    benchmark_sources: Vec<BenchmarkBoxPlotSource>,
    /// The number of wall elements that should be displayed inside the plot.
    elements: Vec<usize>,
    /// Suffix of the plot file.
    suffix: String,
    /// Wether the Legend should be floating or on the right.
    floating_legend: bool,
}
impl BenchmarkBoxPlotSources {
    /// Create a [`BenchmarkBoxPlotSources`] for a plot that compares the different simulation methods.
    pub fn compare_mode<P: AsRef<Path>, S: Display, E: AsRef<[usize]>>(
        simulation_directory: P,
        label: S,
        elements: E,
    ) -> Self {
        let simulation_directory = simulation_directory.as_ref();
        let plot_directory = PathBuf::from("plot").join(simulation_directory);
        let benchmark_sources = [
            SimulationType1D::Cpu,
            SimulationType1D::GpuM1,
            SimulationType1D::GpuM2,
            SimulationType1D::GpuM3,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, s)| BenchmarkBoxPlotSource {
            simulation_directory: simulation_directory.to_path_buf(),
            benchmark_path_part: BenchmarkPathPart::new(None, label.to_string(), s.into()),
            color: COLORS[i].to_string(),
            legend: s.to_string(),
        })
        .collect::<Vec<_>>();
        let suffix = format!("{label}_compare_mode");
        let elements = elements.as_ref().to_vec();
        Self {
            plot_directory,
            benchmark_sources,
            elements,
            suffix,
            floating_legend: true,
        }
    }

    /// Create a [`BenchmarkBoxPlotSources`] for a plot that compares the influence of different materials.
    pub fn compare_multiple<P: AsRef<Path>, S: Display, E: AsRef<[usize]>, N: AsRef<[u8]>>(
        simulation_directory: P,
        label: S,
        elements: E,
        numbers: N,
        simulation_type: SimulationType1D,
    ) -> Self {
        let simulation_directory = simulation_directory.as_ref();
        let plot_directory = PathBuf::from("plot").join(simulation_directory);
        let benchmark_sources = numbers
            .as_ref()
            .iter()
            .enumerate()
            .map(|(i, &n)| BenchmarkBoxPlotSource {
                simulation_directory: simulation_directory.to_path_buf(),
                benchmark_path_part: BenchmarkPathPart {
                    prefix: Some(n.to_string()),
                    label: label.to_string(),
                    simulation_method: simulation_type.into(),
                },
                color: COLORS[i].to_string(),
                legend: n.to_string(),
            })
            .collect::<Vec<_>>();
        let suffix = format!("{label}_compare_multiple_{}", simulation_type.path_str());
        let elements = elements.as_ref().to_vec();
        Self {
            plot_directory,
            benchmark_sources,
            elements,
            suffix,
            floating_legend: false,
        }
    }

    /// Create a [`BenchmarkBoxPlotSources`] for a plot that compares a cpu and gpu m3 simulation with FDS.
    pub fn compare_fds<P1: AsRef<Path>, P2: AsRef<Path>, S: Display, E: AsRef<[usize]>>(
        fds_directory: P1,
        simulation_directory: P2,
        label: S,
        elements: E,
        simulation_types: &[SimulationType1D],
    ) -> Self {
        let simulation_directory = simulation_directory.as_ref();
        let fds_directory = fds_directory.as_ref();
        let plot_directory = PathBuf::from("plot").join(fds_directory);
        let benchmark_sources = simulation_types
            .iter()
            .map(|s| BenchmarkBoxPlotSource {
                simulation_directory: simulation_directory.to_path_buf(),
                benchmark_path_part: BenchmarkPathPart::new(None, label.to_string(), (*s).into()),
                color: match s {
                    SimulationType1D::Cpu => COLORS[0].to_string(),
                    SimulationType1D::GpuM1 => COLORS[1].to_string(),
                    SimulationType1D::GpuM2 => COLORS[2].to_string(),
                    SimulationType1D::GpuM3 => COLORS[3].to_string(),
                },
                legend: s.to_string(),
            })
            .chain([BenchmarkBoxPlotSource {
                benchmark_path_part: BenchmarkPathPart {
                    prefix: Some("multi_core".to_string()),
                    label: label.to_string(),
                    simulation_method:
                        crate::heat_transfer::simulations::SimulationMethod::SpeedTestFDS,
                },
                color: COLORS[4].to_string(),
                legend: "FDS max Kerne".to_string(),
                simulation_directory: fds_directory.to_path_buf(),
            }])
            .collect::<Vec<_>>();
        let suffix = format!("{label}_compare_fds");
        let elements = elements.as_ref().to_vec();
        Self {
            plot_directory,
            benchmark_sources,
            elements,
            suffix,
            floating_legend: true,
        }
    }
}

/// Create a boxplot that displays the simulation time for different methods
pub fn plot(benchmark_source: BenchmarkBoxPlotSources) -> Result<Status> {
    let BenchmarkBoxPlotSources {
        plot_directory,
        benchmark_sources,
        elements,
        suffix,
        floating_legend,
    } = benchmark_source;

    create_dir_all(&plot_directory)
        .with_context(|| format!("Failed to create directories at {plot_directory:?}."))?;
    let save_path = plot_directory.join(format!("benchmark_{}.svg", suffix));
    let paths = elements
        .iter()
        .flat_map(|e| {
            benchmark_sources
                .iter()
                .map(|b| {
                    let path = PathBuf::from("benchmark")
                        .join(&b.simulation_directory)
                        .join(b.benchmark_path_part.path_str().unwrap())
                        .join(format!("{}.bin", e));
                    if path.exists() {
                        std::result::Result::Ok(path)
                    } else {
                        Err(Status::Ignored {
                            simulation: SIMULATION_NAME,
                            reason: format!("\n     {:?} does not exist", path),
                            path: save_path.clone(),
                        })
                    }
                })
                .collect::<Vec<_>>()
        })
        .chain([std::result::Result::Ok(PathBuf::from(
            "src/plot/kind/benchmark_box_plot.rs",
        ))])
        .collect::<Result<Vec<PathBuf>, Status>>();
    let paths = match paths {
        std::result::Result::Ok(ok) => ok,
        Err(err) => return Ok(err),
    };
    if !was_modified(&paths, &[&save_path])? {
        return Ok(Status::Passed {
            simulation: SIMULATION_NAME,
            path: save_path,
        });
    };
    println!(
        "  Start \"{SIMULATION_NAME}\" plot for fds simulation at {:?}.",
        &save_path
    );

    let mut legend = Legend::new().title(Title::new("Legende"));
    if floating_legend {
        legend = legend
            .border_color("#000000")
            .border_width(1)
            .x(0.01)
            .y(0.99);
    }

    let layout = Layout::new()
        .legend(legend)
        .show_legend(true)
        .y_axis(
            Axis::new()
                .show_line(true)
                .title(Title::new("Zeit [s]"))
                .type_(plotly::layout::AxisType::Log),
        )
        .x_axis(
            Axis::new()
                .show_line(true)
                .type_(layout::AxisType::Category)
                .title(Title::new("Wandelemente"))
                .show_grid(true)
                .ticks_on(layout::TicksPosition::Boundaries),
        )
        .margin(Margin::new().top(10).left(60).right(60).bottom(60))
        .box_mode(layout::BoxMode::Group);

    let mut plot = Plot::new();
    plot.set_layout(layout);

    let mut box_plots_x =
        vec![Vec::with_capacity(SIMULATION_RERUNS * elements.len()); benchmark_sources.len()];
    let mut box_plots_y =
        vec![Vec::with_capacity(SIMULATION_RERUNS * elements.len()); benchmark_sources.len()];
    for e in elements {
        for (i, benchmark_source) in benchmark_sources.iter().enumerate() {
            let benchmark_path = benchmark_source
                .simulation_directory
                .join(benchmark_source.benchmark_path_part.path_str()?)
                .join(format!("{}.bin", e));
            let benchmark_reader =
                BenchmarkReader::try_new(PathBuf::from("benchmark").join(benchmark_path))?;
            for time in benchmark_reader {
                let time = time?;
                box_plots_y[i].push(time);
                box_plots_x[i].push(e.to_string());
            }
        }
    }

    box_plots_x
        .into_iter()
        .zip(box_plots_y)
        .enumerate()
        .for_each(|(i, (x, y))| {
            plot.add_trace(
                BoxPlot::new_xy(x, y)
                    .line(Line::new().color(benchmark_sources[i].color.clone()))
                    .name(&benchmark_sources[i].legend)
                    .box_points(BoxPoints::False),
            )
        });

    plot.write_image(&save_path, ImageFormat::SVG, 600, 350, 1.0);
    if save_path.exists() {
        Ok(Status::Succeeded {
            simulation: SIMULATION_NAME,
            path: save_path,
        })
    } else {
        Ok(Status::Failed {
            simulation: SIMULATION_NAME,
            path: save_path,
        })
    }
}
