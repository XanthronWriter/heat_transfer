//! Create a bar chart witch displays the ratio beten two simulations.

use super::COLORS;
use crate::{
    benchmark::{BENCHMARK_CHUNK, BENCHMARK_ELEMENTS},
    heat_transfer::simulations::{BenchmarkReader, SIMULATION_RERUNS},
    modification::was_modified,
    plot::Status,
};
use anyhow::*;
use plotly::{
    common::{Anchor, DashType, Marker, Title},
    layout::{self, Axis, BarMode, Legend, Margin, Shape, ShapeLine, ShapeType},
    Bar, ImageFormat, Layout, Plot,
};
use std::{
    fmt::Display,
    fs::create_dir_all,
    path::{Path, PathBuf},
    vec,
};

const SIMULATION_NAME: &str = "benchmark_factor_plot.rs";

/// Data for a graph element inside a plot.
struct BenchmarkFactorPlotSource {
    /// Path to the benchmark data.
    benchmark_directory: PathBuf,
    /// Legend name.
    legend: String,
    /// Element color.
    color: String,
}

/// Enum to determent the position of the legend.
enum LegendPos {
    TopLeft,
    BottomRight,
}

/// All data to create a bar chart witch displays the ratio beten two simulations.
pub struct BenchmarkFactorPlotSources {
    /// The directory the plot should be saved.
    plot_directory: PathBuf,
    /// Data for all graph elements inside a plot.
    benchmark_sources: Vec<BenchmarkFactorPlotSource>,
    /// suffix for the `benchmark_sources` for the full path
    benchmark_compare_path: [PathBuf; 2],
    /// The number of wall elements that should be displayed inside the plot.
    elements: Vec<usize>,
    /// Suffix of the plot file.
    suffix: String,
    /// Lable of the y-Axis.
    y_axis: String,
    /// the position of the legend.
    legend_pos: LegendPos,
}
impl BenchmarkFactorPlotSources {
    /// Create a [`BenchmarkFactorPlotSources`] for a plot that compares the impact of different thicknesses on the simulation time between cpu and gpu m3.
    pub fn thickness_mode<P: AsRef<Path>, S: Display>(simulation_directory: P, label: S) -> Self {
        let elements = &BENCHMARK_ELEMENTS;
        let benchmark_directory = PathBuf::from("benchmark").join(simulation_directory.as_ref());
        let plot_directory = PathBuf::from("plot").join(simulation_directory.as_ref());
        let benchmark_sources = [
            ("005cm", "0,05 m (6 Zellen)"),
            ("010cm", "0,10 m (8 Zellen)"),
            ("050cm", "0,50 m (13 Zellen)"),
            ("100cm", "1,00 m (14 Zellen)"),
            ("500cm", "5,00 m (19 Zellen)"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (path_part, legend))| BenchmarkFactorPlotSource {
            benchmark_directory: benchmark_directory.join(path_part).join(label.to_string()),
            color: COLORS[i].to_string(),
            legend: legend.to_string(),
        })
        .collect::<Vec<_>>();
        let suffix = format!("{label}_thickness_mode");
        let elements = elements.as_ref().to_vec();
        Self {
            plot_directory,
            benchmark_sources,
            elements,
            suffix,
            y_axis: "Verhältnis CPU / GPU M3".to_string(),
            benchmark_compare_path: [PathBuf::from("cpu"), PathBuf::from("gpu_m3")],
            legend_pos: LegendPos::TopLeft,
        }
    }

    /// Create a [`BenchmarkFactorPlotSources`] for a plot that compares the impact of different thicknesses on the simulation time with and without the chunk adjustment.
    pub fn compare_chunk_thickness_mode<P: AsRef<Path>>(
        simulation_directory: P,
        computer_1: &str,
        computer_2: &str,
    ) -> Self {
        let elements = BENCHMARK_ELEMENTS[(BENCHMARK_ELEMENTS.len() - 3)..]
            .iter()
            .chain(BENCHMARK_CHUNK.iter())
            .copied()
            .collect::<Vec<_>>();
        let benchmark_directory = PathBuf::from("benchmark").join(simulation_directory.as_ref());
        let plot_directory = PathBuf::from("plot").join(simulation_directory.as_ref());
        let benchmark_sources = [
            ("005cm", "0,05 m (6 Zellen)"),
            ("010cm", "0,10 m (8 Zellen)"),
            ("050cm", "0,50 m (13 Zellen)"),
            ("100cm", "1,00 m (14 Zellen)"),
            ("500cm", "5,00 m (19 Zellen)"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (path_part, legend))| BenchmarkFactorPlotSource {
            benchmark_directory: benchmark_directory.join(path_part),
            color: COLORS[i].to_string(),
            legend: legend.to_string(),
        })
        .collect::<Vec<_>>();
        let suffix = format!("compare_{computer_1}_to_{computer_2}_thickness_mode");
        Self {
            plot_directory,
            benchmark_sources,
            elements,
            suffix,
            y_axis: "Verhältnis ohne/mit".to_string(),
            benchmark_compare_path: [
                PathBuf::from(computer_1).join("gpu_m3"),
                PathBuf::from(computer_2).join("gpu_m3"),
            ],
            legend_pos: LegendPos::BottomRight,
        }
    }

    /// Create a [`BenchmarkFactorPlotSources`] for a plot that compares the impact of the chunk adjustment.
    pub fn compare_chunk_mode<P: AsRef<Path>>(
        simulation_directory: P,
        computer_1: &str,
        computer_2: &str,
    ) -> Self {
        let elements = BENCHMARK_ELEMENTS[(BENCHMARK_ELEMENTS.len() - 3)..]
            .iter()
            .chain(BENCHMARK_CHUNK.iter())
            .copied()
            .collect::<Vec<_>>();
        let benchmark_directory = PathBuf::from("benchmark").join(simulation_directory.as_ref());
        let plot_directory = PathBuf::from("plot").join(simulation_directory.as_ref());
        let benchmark_sources = vec![BenchmarkFactorPlotSource {
            benchmark_directory,
            legend: "Verhältnis".to_string(),
            color: COLORS[3].to_string(),
        }];
        let suffix = format!("compare_{computer_1}_to_{computer_2}_mode");
        Self {
            y_axis: "Verhältnis ohne/mit".to_string(),
            plot_directory,
            benchmark_compare_path: [
                PathBuf::from(computer_1).join("gpu_m3"),
                PathBuf::from(computer_2).join("gpu_m3"),
            ],
            benchmark_sources,
            elements,
            suffix,
            legend_pos: LegendPos::BottomRight,
        }
    }
}

/// Create a bar chart witch displays the ratio beten two simulations.
pub fn plot(benchmark_source: BenchmarkFactorPlotSources) -> Result<Status> {
    let BenchmarkFactorPlotSources {
        plot_directory,
        benchmark_sources,
        elements,
        suffix,
        benchmark_compare_path,
        y_axis,
        legend_pos,
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
                    let path_benchmark_compare_1 = &b
                        .benchmark_directory
                        .join(&benchmark_compare_path[0])
                        .join(format!("{}.bin", e));
                    let path_benchmark_compare_2 = &b
                        .benchmark_directory
                        .join(&benchmark_compare_path[1])
                        .join(format!("{}.bin", e));

                    if path_benchmark_compare_1.exists() && path_benchmark_compare_2.exists() {
                        std::result::Result::Ok(b.benchmark_directory.clone())
                    } else {
                        Err(Status::Ignored {
                            simulation: SIMULATION_NAME,
                            reason: format!(
                                "\n     {:?} or {:?} does not exist",
                                path_benchmark_compare_1, path_benchmark_compare_2
                            ),
                            path: save_path.clone(),
                        })
                    }
                })
                .collect::<Vec<_>>()
        })
        .chain([std::result::Result::Ok(PathBuf::from(
            "src/plot/kind/benchmark_factor_plot.rs",
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

    let mut legend = Legend::new()
        .title(Title::new("Legende"))
        .border_color("#000000")
        .border_width(1);
    match legend_pos {
        LegendPos::TopLeft => {
            legend = legend.x(0.01).y(0.99);
        }
        LegendPos::BottomRight => {
            legend = legend
                .x_anchor(Anchor::Right)
                .y_anchor(Anchor::Bottom)
                .x(0.99)
                .y(0.01);
        }
    }

    let mut layout = Layout::new()
        .legend(legend)
        .show_legend(true)
        .y_axis(
            Axis::new().show_line(true).title(Title::new(&y_axis)), //.type_(plotly::layout::AxisType::Log),
        )
        .x_axis(
            Axis::new()
                .show_line(true)
                .type_(layout::AxisType::Category)
                .title(Title::new("Wandelemente"))
                .show_grid(true)
                .ticks_on(layout::TicksPosition::Boundaries),
        )
        .bar_gap(0.2)
        .bar_group_gap(0.2)
        .margin(Margin::new().top(10).left(60).right(60).bottom(60))
        .bar_mode(BarMode::Group);
    layout.add_shape(
        Shape::new()
            .shape_type(ShapeType::Line)
            .x_ref("paper")
            .x0(0)
            .y0(1)
            .x1(1)
            .y1(1)
            .line(
                ShapeLine::new()
                    .dash(DashType::Dash)
                    .width(1.0)
                    .color("000000AA"),
            ),
    );

    let mut plot = Plot::new();
    plot.set_layout(layout);

    for benchmark_source in benchmark_sources.into_iter() {
        let mut x = vec![];
        let mut y = vec![];
        for e in elements.iter() {
            let path_benchmark_cpu = benchmark_source
                .benchmark_directory
                .join(&benchmark_compare_path[0])
                .join(format!("{}.bin", *e));
            let mut compare_1_times = BenchmarkReader::try_new(path_benchmark_cpu)?
                .collect::<Result<Vec<f64>, Error>>()?;
            compare_1_times.sort_by(|a, b| a.total_cmp(b));
            let compare_1_time = (compare_1_times[SIMULATION_RERUNS / 2]
                + compare_1_times[SIMULATION_RERUNS / 2 + 1])
                / 2.0;
            let compare_1_path_benchmark = benchmark_source
                .benchmark_directory
                .join(&benchmark_compare_path[1])
                .join(format!("{}.bin", *e));
            let mut compare_2_times = BenchmarkReader::try_new(compare_1_path_benchmark)?
                .collect::<Result<Vec<f64>, Error>>()?;
            compare_2_times.sort_by(|a, b| a.total_cmp(b));
            let compare_2_time = (compare_2_times[SIMULATION_RERUNS / 2]
                + compare_2_times[SIMULATION_RERUNS / 2 + 1])
                / 2.0;
            x.push(*e);
            y.push(compare_1_time / compare_2_time)
        }
        plot.add_trace(
            Bar::new(x, y)
                .name(benchmark_source.legend)
                .marker(Marker::new().color(benchmark_source.color)),
        )
    }

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
