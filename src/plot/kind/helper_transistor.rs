use std::{fs::create_dir_all, path::PathBuf};

use anyhow::*;
use plotly::{
    box_plot::BoxPoints,
    common::{Anchor, Line, Marker, Title},
    layout::{Axis, AxisType, Legend, Margin},
    BoxPlot, ImageFormat, Layout, Plot,
};

use crate::plot::Status;

use super::COLORS;

/// Create a plot for the transistor count for the different years.
pub fn plot() -> Result<Status> {
    let path = PathBuf::from("plot/helper/transistors.svg");

    let amd_cpu = vec![
        (4300000u64, 1996u16),
        (8800000, 1997),
        (21300000, 1999),
        (22000000, 1999),
        (54300000, 2003),
        (105900000, 2003),
        (463000000, 2007),
        (758000000, 2008),
        (904000000, 2009),
        (1200000000, 2012),
        (1303000000, 2012),
        (4800000000, 2017),
        (4800000000, 2017),
        (4800000000, 2017),
        (19200000000, 2017),
    ];
    let intel_cpu = vec![
        (2300u64, 1971u16),
        (3500, 1972),
        (4500, 1974),
        (6500, 1976),
        (29000, 1978),
        (29000, 1979),
        (55000, 1982),
        (134000, 1982),
        (275000, 1985),
        (250000, 1988),
        (1180235, 1989),
        (3100000, 1993),
        (5500000, 1995),
        (7500000, 1997),
        (7500000, 1998),
        (9500000, 1999),
        (21000000, 2000),
        (27400000, 1999),
        (45000000, 2001),
        (42000000, 2000),
        (55000000, 2002),
        (112000000, 2004),
        (169000000, 2005),
        (184000000, 2006),
        (228000000, 2005),
        (362000000, 2006),
        (47000000, 2008),
        (220000000, 2002),
        (291000000, 2006),
        (169000000, 2007),
        (410000000, 2003),
        (432000000, 2012),
        (230000000, 2008),
        (592000000, 2004),
        (411000000, 2007),
        (731000000, 2008),
        (1160000000, 2011),
        (1170000000, 2010),
        (1400000000, 2012),
        (1400000000, 2014),
        (1700000000, 2006),
        (1750000000, 2015),
        (1860000000, 2013),
        (1900000000, 2015),
        (1900000000, 2008),
        (2000000000, 2010),
        (2270000000, 2011),
        (2300000000, 2010),
        (2600000000, 2014),
        (2600000000, 2011),
        (3100000000, 2012),
        (3200000000, 2016),
        (4310000000, 2014),
        (5000000000, 2012),
        (5560000000, 2014),
        (8000000000, 2017),
        (7200000000, 2016),
        (8000000000, 2016),
    ];
    let amd_gpu = vec![
        (8000000u64, 1999u16),
        (30000000, 2000),
        (60000000, 2001),
        (107000000, 2002),
        (117000000, 2003),
        (160000000, 2004),
        (242000000, 2008),
        (292000000, 2010),
        (321000000, 2005),
        (370000000, 2011),
        (384000000, 2006),
        (514000000, 2008),
        (627000000, 2010),
        (666000000, 2008),
        (700000000, 2007),
        (716000000, 2011),
        (826000000, 2009),
        (956000000, 2008),
        (959000000, 2008),
        (1040000000, 2009),
        (1040000000, 2013),
        (1500000000, 2012),
        (1700000000, 2010),
        (2080000000, 2013),
        (2154000000, 2009),
        (2200000000, 2017),
        (2640000000, 2010),
        (2800000000, 2012),
        (3000000000, 2016),
        (4312711873, 2011),
        (5000000000, 2014),
        (5700000000, 2016),
        (6300000000, 2013),
        (8900000000, 2015),
        (12500000000, 2017),
        (13280000000, 2018),
    ];
    let nvidia_gpu = vec![
        (3500000u64, 1997u16),
        (15000000, 1999),
        (23000000, 1999),
        (20000000, 2000),
        (25000000, 2000),
        (57000000, 2001),
        (63000000, 2002),
        (135000000, 2003),
        (210000000, 2007),
        (210000000, 2008),
        (222000000, 2004),
        (260000000, 2009),
        (289000000, 2007),
        (292000000, 2011),
        (303000000, 2005),
        (314000000, 2008),
        (486000000, 2009),
        (505000000, 2008),
        (585000000, 2011),
        (681000000, 2006),
        (727000000, 2009),
        (754000000, 2007),
        (1170000000, 2010),
        (1270000000, 2012),
        (1400000000, 2008),
        (1400000000, 2008),
        (1850000000, 2017),
        (1870000000, 2014),
        (1950000000, 2011),
        (2540000000, 2012),
        (2940000000, 2014),
        (3200000000, 2010),
        (3000000000, 2010),
        (3300000000, 2017),
        (3540000000, 2012),
        (4400000000, 2016),
        (5200000000, 2014),
        (7080000000, 2012),
        (7200000000, 2016),
        (11800000000, 2017),
        (8000000000, 2015),
        (15300000000, 2016),
        (18600000000, 2018),
        (21100000000, 2017),
    ];

    let legend = Legend::new()
        .title(Title::new("Legende"))
        .border_color("#000000")
        .border_width(1)
        .x_anchor(Anchor::Left)
        .x(0.02)
        .y(1.0);

    let layout = Layout::new()
        .legend(legend)
        .show_legend(true)
        .x_axis(
            Axis::new()
                .title(Title::new("Jahr"))
                .show_line(true)
                .type_(AxisType::Date),
        )
        .y_axis(
            Axis::new()
                .title(Title::new("Transistoren"))
                .type_(AxisType::Log),
        )
        .margin(Margin::new().top(10).left(60).right(20).bottom(60));
    let mut plot = Plot::new();
    plot.set_layout(layout);

    create_dir_all("plot/helper")
        .with_context(|| format!("Failed to create directories at {path:?}."))?;

    let (amd_cpu_t, amd_cpu_j): (Vec<u64>, Vec<u16>) = amd_cpu.into_iter().unzip();
    plot.add_trace(
        BoxPlot::new_xy(amd_cpu_j, amd_cpu_t)
            .name("AMD CPU")
            .box_points(BoxPoints::All)
            .point_pos(-1.5)
            .fill_color("00000000")
            .line(Line::new().color("00000000"))
            .marker(Marker::new().color(COLORS[0])),
    );
    let (intel_cpu_t, intel_cpu_j): (Vec<u64>, Vec<u16>) = intel_cpu.into_iter().unzip();
    plot.add_trace(
        BoxPlot::new_xy(intel_cpu_j, intel_cpu_t)
            .name("Intel CPU")
            .box_points(BoxPoints::All)
            .point_pos(-0.5)
            .fill_color("00000000")
            .line(Line::new().color("00000000"))
            .marker(Marker::new().color(COLORS[1])),
    );
    let (nvidia_gpu_t, nvidia_gpu_j): (Vec<u64>, Vec<u16>) = nvidia_gpu.into_iter().unzip();
    plot.add_trace(
        BoxPlot::new_xy(nvidia_gpu_j, nvidia_gpu_t)
            .name("NVIDIA GPU")
            .box_points(BoxPoints::All)
            .point_pos(0.5)
            .fill_color("00000000")
            .line(Line::new().color("00000000"))
            .marker(Marker::new().color(COLORS[2])),
    );
    let (amd_gpu_t, amd_gpu_j): (Vec<u64>, Vec<u16>) = amd_gpu.into_iter().unzip();
    plot.add_trace(
        BoxPlot::new_xy(amd_gpu_j, amd_gpu_t)
            .name("AMD GPU")
            .box_points(BoxPoints::All)
            .point_pos(1.5)
            .fill_color("00000000")
            .line(Line::new().color("00000000"))
            .marker(Marker::new().color(COLORS[3])),
    );

    plot.write_image(&path, ImageFormat::SVG, 600, 300, 1.0);

    Ok(Status::Succeeded {
        simulation: "helper",
        path,
    })
}
