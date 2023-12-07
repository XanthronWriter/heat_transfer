use std::{fs::create_dir_all, path::PathBuf};

use crate::plot::{kind::COLORS, Status};
use anyhow::*;
use plotly::{
    color::NamedColor,
    common::{Anchor, Line, Marker, MarkerSymbol, Mode, Title},
    layout::{Axis, Legend, Margin},
    ImageFormat, Layout, Plot, Scatter,
};

#[derive(Debug, Clone, Copy)]
enum PlotMode {
    SpecificHeat,
    Conductivity,
}
#[derive(Debug, Clone, Copy)]
enum PlotType {
    Concrete,
    Steel,
}

/// The function for the steel specific heat progression.
fn steel_c(values: impl Iterator<Item = f64>) -> impl Iterator<Item = f64> {
    values.map(|v| {
        if v < 600.0 {
            425.0 + 0.773 * v - 0.00169 * v.powi(2) + 2.22e-6 * v.powi(3)
        } else if v < 735.0 {
            666.0 + 13002.0 / (738.0 - v)
        } else if v < 900.0 {
            545.0 + 17820.0 / (v - 731.0)
        } else {
            650.0
        }
    })
}

/// The function for the concrete specific heat progression.
fn concrete_c(values: impl Iterator<Item = f64>) -> impl Iterator<Item = f64> {
    values.map(|v| {
        if v < 100.0 {
            900.0
        } else if v < 200.0 {
            900.0 + (v - 100.0)
        } else if v < 400.0 {
            1000.0 + (v - 200.0) / 2.0
        } else {
            1100.0
        }
    })
}

/// The function for the steel specific conductivity.
fn steel_k(values: impl Iterator<Item = f64>) -> impl Iterator<Item = f64> {
    values.map(|v| {
        if v < 800.0 {
            54.0 - 3.33 * 10.0f64.powi(-2) * v
        } else {
            27.3
        }
    })
}

/// The function for the steel concrete conductivity.
fn concrete_k(values: impl Iterator<Item = f64>) -> impl Iterator<Item = f64> {
    values.map(|v| 1.36 - 0.136 * (v / 100.0) + 0.0057 * (v / 100.0).powi(2))
}

/// Create the plot for the concrete conductivity
pub fn plot_concrete_k() -> Result<Status> {
    plot_one(PlotType::Concrete, PlotMode::Conductivity)
}
/// Create the plot for the concrete specific heat
pub fn plot_concrete_c() -> Result<Status> {
    plot_one(PlotType::Concrete, PlotMode::SpecificHeat)
}
/// Create the plot for the steel conductivity
pub fn plot_steel_k() -> Result<Status> {
    plot_one(PlotType::Steel, PlotMode::Conductivity)
}
/// Create the plot for the steel specific heat
pub fn plot_steel_c() -> Result<Status> {
    plot_one(PlotType::Steel, PlotMode::SpecificHeat)
}

/// Create the plots for the different materials and
fn plot_one(plot_type: PlotType, plot_mode: PlotMode) -> Result<Status> {
    let (path_type_name, color) = match plot_type {
        PlotType::Concrete => ("concrete", COLORS[0]),
        PlotType::Steel => ("steel", COLORS[1]),
    };
    let path_mode_name = match plot_mode {
        PlotMode::SpecificHeat => "c",
        PlotMode::Conductivity => "k",
    };
    let (mut plot, precise_x, precise_y, approximate_x, approximate_y) =
        match (plot_type, plot_mode) {
            (PlotType::Concrete, PlotMode::SpecificHeat) => {
                let plot = plot_canvas(plot_mode, vec![0.0, 1200.0], vec![0.0, 2200.0]);
                let precise_x = (20..1200).map(|v| v as f64).collect::<Vec<_>>();
                let precise_y = concrete_c(precise_x.iter().copied()).collect::<Vec<_>>();
                let approximate_x = vec![20.0, 100.0, 200.0, 400.0, 1200.0];
                let approximate_y = concrete_c(approximate_x.iter().copied()).collect::<Vec<_>>();
                (plot, precise_x, precise_y, approximate_x, approximate_y)
            }
            (PlotType::Concrete, PlotMode::Conductivity) => {
                let plot = plot_canvas(plot_mode, vec![0.0, 1200.0], vec![0.0, 2.0]);
                let precise_x = (0..1200).map(|v| v as f64).collect::<Vec<_>>();
                let precise_y = concrete_k(precise_x.iter().copied()).collect::<Vec<_>>();
                let approximate_x = (0..1201).step_by(100).map(|v| v as f64).collect::<Vec<_>>();
                let approximate_y = concrete_k(approximate_x.iter().copied()).collect::<Vec<_>>();
                (plot, precise_x, precise_y, approximate_x, approximate_y)
            }
            (PlotType::Steel, PlotMode::SpecificHeat) => {
                let plot = plot_canvas(plot_mode, vec![0.0, 1200.0], vec![0.0, 5500.0]);
                let precise_x = (20..1200).map(|v| v as f64).collect::<Vec<_>>();
                let precise_y = steel_c(precise_x.iter().copied()).collect::<Vec<_>>();
                let approximate_x = vec![
                    20.0, 400.0, 630.0, 690.0, 720.0, 735.0, 750.0, 780.0, 830.0, 900.0, 1200.0,
                ];
                let approximate_y = steel_c(approximate_x.iter().copied()).collect::<Vec<_>>();
                (plot, precise_x, precise_y, approximate_x, approximate_y)
            }
            (PlotType::Steel, PlotMode::Conductivity) => {
                let plot = plot_canvas(plot_mode, vec![0.0, 1200.0], vec![0.0, 60.0]);
                let precise_x = (20..1200).map(|v| v as f64).collect::<Vec<_>>();
                let precise_y = steel_k(precise_x.iter().copied()).collect::<Vec<_>>();
                let approximate_x = vec![20.0, 800.0, 1200.0];
                let approximate_y = steel_k(approximate_x.iter().copied()).collect::<Vec<_>>();
                (plot, precise_x, precise_y, approximate_x, approximate_y)
            }
        };
    let dir = PathBuf::from("plot/helper/ramp");
    let path = dir.join(format!("{}_{}.svg", path_type_name, path_mode_name));

    create_dir_all(&dir).with_context(|| format!("Failed to create directories at {dir:?}."))?;

    plot.add_trace(
        Scatter::new(precise_x, precise_y)
            .name("Genau")
            .mode(Mode::Lines)
            .line(Line::new().color(NamedColor::LightGray).width(4.0)),
    );
    plot.add_trace(
        Scatter::new(approximate_x, approximate_y)
            .name("Approximiert")
            .mode(Mode::LinesMarkers)
            .line(
                Line::new()
                    .color(color)
                    .dash(plotly::common::DashType::LongDashDot),
            )
            .marker(Marker::new().symbol(MarkerSymbol::X).size(8)),
    );

    plot.write_image(&path, ImageFormat::SVG, 400, 300, 1.0);

    Ok(Status::Succeeded {
        simulation: "helper",
        path,
    })
}

/// Create the canvas of the plot
fn plot_canvas(plot_mode: PlotMode, range_x: Vec<f64>, range_y: Vec<f64>) -> Plot {
    let legend = Legend::new()
        .title(Title::new("Legende"))
        .border_color("#000000")
        .border_width(1)
        .x_anchor(Anchor::Right)
        .x(1.0)
        .y(1.0);

    let layout = Layout::new()
        .legend(legend)
        .show_legend(true)
        .x_axis(
            Axis::new()
                .title(Title::new("Temperatur [°C]"))
                .show_line(true)
                .range(range_x),
        )
        .y_axis(
            Axis::new()
                .title(Title::new(match plot_mode {
                    PlotMode::SpecificHeat => "Spezifische Wärme [J/(kg⋅K)]",
                    PlotMode::Conductivity => "Wärmeleitfähigkeit [W/(m⋅K)]",
                }))
                .range(range_y),
        )
        .margin(Margin::new().top(10).left(60).right(20).bottom(60));
    let mut plot = Plot::new();
    plot.set_layout(layout);
    plot
}
