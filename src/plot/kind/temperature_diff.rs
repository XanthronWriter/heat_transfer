use super::COLORS;
use crate::{
    heat_transfer::simulations::{
        temperature::{one_dimensional_by_type, Diff, Temperatures},
        SimulationKind, SimulationType1D,
    },
    modification::was_modified,
    plot::Status,
};
use anyhow::*;
use plotly::{
    color::{NamedColor, Rgba},
    common::{AxisSide, DashType, Font, Line, Mode, Title},
    layout::{Axis, Legend, Margin},
    ImageFormat, Layout, Plot, Scatter,
};
use std::path::{Path, PathBuf};

/// Start and plot the temperature of a 1D simulation with a comparison between FDS and this program for the different simulation methods.
pub fn plot_one_dimensional_by_type<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
    simulation_type: SimulationType1D,
) -> Result<Status> {
    match simulation_type {
        SimulationType1D::Cpu => plot_one_dimensional(
            path,
            simulation_kind,
            simulation_type,
            &[
                "src/plot/kind/temperature_diff.rs",
                "src/heat_transfer/one_dimensional/cpu.rs",
                "src/heat_transfer/simulations/temperature.rs",
            ],
        ),
        SimulationType1D::GpuM1 => plot_one_dimensional(
            path,
            simulation_kind,
            simulation_type,
            &[
                "src/plot/kind/temperature_diff.rs",
                "src/heat_transfer/one_dimensional/gpu_m1.rs",
                "src/heat_transfer/one_dimensional/gpu_m1.wgsl",
                "src/heat_transfer/simulations/temperature.rs",
            ],
        ),
        SimulationType1D::GpuM2 => plot_one_dimensional(
            path,
            simulation_kind,
            simulation_type,
            &[
                "src/plot/kind/temperature_diff.rs",
                "src/heat_transfer/one_dimensional/gpu_m2.rs",
                "src/heat_transfer/one_dimensional/gpu_m2.wgsl",
                "src/heat_transfer/simulations/temperature.rs",
            ],
        ),
        SimulationType1D::GpuM3 => plot_one_dimensional(
            path,
            simulation_kind,
            simulation_type,
            &[
                "src/plot/kind/temperature_diff.rs",
                "src/heat_transfer/one_dimensional/gpu_m3.rs",
                "src/heat_transfer/one_dimensional/gpu_m3.wgsl",
                "src/heat_transfer/simulations/temperature.rs",
            ],
        ),
    }
}

/// Plot the temperature of a 1D simulation with a comparison between FDS and this program.
fn plot(
    temperatures: Temperatures,
    plot_path: PathBuf,
    plot_path_f: PathBuf,
    plot_path_b: PathBuf,
    simulation_type_str: &'static str,
) -> Result<Status> {
    let Diff {
        front: diff_front,
        back: diff_back,
    } = temperatures.diff();
    let Temperatures {
        time,
        fds_front,
        fds_back,
        sim_front,
        sim_back,
    } = temperatures;

    std::fs::create_dir_all(&plot_path)
        .with_context(|| format!("Failed to create directories {:?}.", plot_path))?;
    plot_temperature_time_diff(&plot_path_f, time.clone(), fds_front, sim_front, diff_front);
    plot_temperature_time_diff(&plot_path_b, time, fds_back, sim_back, diff_back);

    if plot_path_f.exists() && plot_path_b.exists() {
        Ok(Status::Succeeded {
            simulation: simulation_type_str,
            path: plot_path,
        })
    } else {
        Ok(Status::Failed {
            simulation: simulation_type_str,
            path: plot_path,
        })
    }
}

/// Start and plot the temperature of a 1D simulation with a comparison between FDS and this program.
fn plot_one_dimensional<P: AsRef<Path>>(
    path: P,
    simulation_kind: SimulationKind,
    simulation_type: SimulationType1D,
    modification_paths: &'static [&'static str],
) -> Result<Status> {
    let simulation_type_str = simulation_type.path_str();
    let path = path.as_ref();
    let plot_path = PathBuf::from("plot").join(path);
    let plot_path_f = plot_path.join(format!("{simulation_type_str}_f.svg"));
    let plot_path_b = plot_path.join(format!("{simulation_type_str}_b.svg"));

    if !was_modified(
        &modification_paths
            .iter()
            .map(PathBuf::from)
            .chain([path.join("heat_transfer.fds")])
            .collect::<Vec<_>>(),
        &[&plot_path_f, &plot_path_b],
    )? {
        return Ok(Status::Passed {
            simulation: simulation_type_str,
            path: plot_path,
        });
    }
    println!(
        "  Start \"{}\" plot for fds simulation at {:?}.",
        simulation_type_str, path
    );

    let temperatures = one_dimensional_by_type(path, simulation_kind, simulation_type)
        .with_context(|| format!("Failed fds simulation at {:?}.", path))?;
    plot(
        temperatures,
        plot_path,
        plot_path_f,
        plot_path_b,
        simulation_type_str,
    )
}

/// Plot the temperature of a 1D simulation with a comparison between FDS and this program.
fn plot_temperature_time_diff(
    path: &Path,
    time: Vec<f32>,
    fds: Vec<f32>,
    sim: Vec<f32>,
    diff: Vec<f32>,
) {
    let diff_plot = Scatter::new(time.clone(), diff)
        .mode(Mode::Lines)
        .line(Line::new().dash(DashType::Dot).color(COLORS[2]))
        .name("Differenz")
        .y_axis("y2");
    let fds_plot = Scatter::new(time.clone(), fds)
        .mode(Mode::Lines)
        .line(Line::new().color(COLORS[1]))
        .name("FDS");
    let sim_plot = Scatter::new(time, sim)
        .mode(Mode::Lines)
        .line(Line::new().dash(DashType::Dash).color(COLORS[0]))
        .name("Programm");

    let mut plot = plot_canvas();
    plot.add_trace(diff_plot);
    plot.add_trace(fds_plot);
    plot.add_trace(sim_plot);
    plot.write_image(path, ImageFormat::SVG, 600, 350, 1.0);
}

/// create the canvas of the plot.
fn plot_canvas() -> Plot {
    let legend = Legend::new()
        .title(Title::new("Legende"))
        .border_color("#000000")
        .border_width(1)
        .x(0.77)
        .y(0.02);

    let light_red = Rgba::new(42, 205, 62, 0.5);

    let layout = Layout::new()
        .legend(legend)
        .show_legend(true)
        .y_axis(Axis::new().title(Title::new("Temperatur [°C]")))
        .x_axis(Axis::new().title(Title::new("Zeit [s]")).show_line(true))
        .margin(Margin::new().top(10).left(60).right(60).bottom(60))
        .y_axis2(
            Axis::new()
                .title(Title::new("Differenz [K]").font(Font::new().color(NamedColor::Black)))
                .overlaying("y")
                .side(AxisSide::Right)
                .zero_line(true)
                .zero_line_color(light_red)
                .auto_range(false)
                .range(vec![-2.0, 2.0])
                .show_line(true)
                .show_grid(false)
                .tick_font(Font::new().color(NamedColor::Black))
                .color(COLORS[2]),
        );

    let mut plot = Plot::new();
    plot.set_layout(layout);
    plot
}
