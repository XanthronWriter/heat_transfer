use std::fs::create_dir_all;

use crate::{
    fds::{cells_from_materials_and_thickness, parse_script_from_file},
    plot::{kind::COLORS, Status},
};
use anyhow::*;
use plotly::{
    common::{Marker, MarkerSymbol, Mode, Title},
    layout::{Axis, AxisType, Legend, Margin},
    ImageFormat, Layout, Plot, Scatter,
};

pub fn plot() -> Result<Status> {
    const THICKNESSES: [f32; 10] = [0.01, 0.02, 0.05, 0.10, 0.20, 0.50, 1.0, 2.0, 5.0, 10.0];

    let (_, materials_c, _) =
        parse_script_from_file("fds/1D/Diabatic/concrete_k_c/heat_transfer.fds").with_context(
            || "Failed to parse script at \"fds/1D/Diabatic/concrete_k_c/heat_transfer.fds\".",
        )?;
    let (_, materials_s, _) = parse_script_from_file("fds/1D/Diabatic/steel_k_c/heat_transfer.fds")
        .with_context(|| {
            "Failed to parse script at \"fds/1D/Diabatic/steel_k_c/heat_transfer.fds\"."
        })?;

    if let (Some(material_c), Some(material_s)) = (
        materials_c.find_index("MATL_WALL"),
        materials_s.find_index("MATL_WALL"),
    ) {
        let cells_c = THICKNESSES
            .iter()
            .map(|n| cells_from_materials_and_thickness(&materials_c, &[material_c], &[*n]).len())
            .collect::<Vec<_>>();
        let cells_s = THICKNESSES
            .iter()
            .map(|n| {
                cells_from_materials_and_thickness(&materials_s, &[material_s], &[*n]).len() - 2
            })
            .collect::<Vec<_>>();

        let legend = Legend::new()
            .title(Title::new("Legende"))
            .border_color("#000000")
            .border_width(1)
            .x(0.02)
            .y(0.98);

        let layout = Layout::new()
            .legend(legend)
            .show_legend(true)
            .y_axis(Axis::new().title(Title::new("Anzahl Zellen")))
            .x_axis(
                Axis::new()
                    .title(Title::new("Wanddicke [m]"))
                    .show_line(true)
                    .type_(AxisType::Log)
                    .tick_values(THICKNESSES.iter().map(|t| *t as f64).collect()),
            )
            .margin(Margin::new().top(10).left(60).right(60).bottom(60));
        let mut plot = Plot::new();
        plot.set_layout(layout);

        plot.add_trace(
            Scatter::new(THICKNESSES.to_vec(), cells_c)
                .name("Beton")
                .mode(Mode::Markers)
                .marker(Marker::new().symbol(MarkerSymbol::Diamond).color(COLORS[0])),
        );
        plot.add_trace(
            Scatter::new(THICKNESSES.to_vec(), cells_s)
                .name("Stahl")
                .mode(Mode::Markers)
                .marker(Marker::new().symbol(MarkerSymbol::Circle).color(COLORS[1])),
        );
        create_dir_all("plot/helper")
            .with_context(|| "Failed to create directories at \"plot/helper/\".")?;
        plot.write_image(
            "plot/helper/cell_count.svg",
            ImageFormat::SVG,
            600,
            300,
            1.0,
        );
    } else {
        bail!("Failed to find material in FDS simulation.");
    }

    Ok(Status::Succeeded {
        simulation: "helper",
        path: "plot/helper/cell_count.svg".into(),
    })
}
