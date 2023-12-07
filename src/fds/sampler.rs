use std::{
    fmt::Write as FmtWrite,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

use crate::{
    benchmark::BENCHMARK_ELEMENTS,
    heat_transfer::simulations::{SimulationKind, SimulationMethod},
    modification::was_modified,
};

use anyhow::*;
use rayon::prelude::*;

/// The name of a template simulation, witch should not be started.
const TEMPLATE_NAME: &str = "template_heat_transfer.fds";
/// The name of a simulation that is created out of a template.
const SIMULATION_NAME: &str = "heat_transfer.fds";

/// The directories of the one dimensional templates.
pub(super) const TEMPLATE_ONE_DIMENSIONAL_DIRECTORY_PATHS: [&str; 3] = [
    "fds/1D/Diabatic",
    "fds/1D/DiabaticOneSide",
    "fds/1D/Adiabatic",
];

/// The directory of the fds speed test template.
pub(super) const SPEED_TEST_DIRECTORY_PATHS: [&str; 1] = ["fds/1D/AdiabaticSpeedTest"];

#[derive(Debug, Clone)]
struct Replace(Vec<(String, String)>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplaceType {
    OneDimensional,
    SpeedTest { size: usize, threads: usize },
}

#[derive(Debug, Clone, Copy)]
enum Material {
    Concrete,
    Steel,
}

struct SimulationSettings {
    material: Material,
    simulation_kind: SimulationKind,
    k_ramp: bool,
    c_ramp: bool,
    replace_type: ReplaceType,
}
impl SimulationSettings {
    /// Returns the template path of this [`SimulationSettings`].
    fn template_path(&self) -> PathBuf {
        PathBuf::from(self.template_dir()).join(TEMPLATE_NAME)
    }

    /// Returns a reference to the template dir of this [`SimulationSettings`].
    /// # Panics
    /// Panics if the path for a FDS speed test simulation with [`SimulationKind::Diabatic`] or [`SimulationKind::DiabaticOneSide`] are requested since they are not supported.
    fn template_dir(&self) -> &'static str {
        match (self.replace_type, self.simulation_kind) {
            (ReplaceType::OneDimensional, SimulationKind::Diabatic) => {
                TEMPLATE_ONE_DIMENSIONAL_DIRECTORY_PATHS[0]
            }
            (ReplaceType::OneDimensional, SimulationKind::DiabaticOneSide) => {
                TEMPLATE_ONE_DIMENSIONAL_DIRECTORY_PATHS[1]
            }
            (ReplaceType::OneDimensional, SimulationKind::Adiabatic) => {
                TEMPLATE_ONE_DIMENSIONAL_DIRECTORY_PATHS[2]
            }

            (ReplaceType::SpeedTest { .. }, SimulationKind::Adiabatic) => {
                SPEED_TEST_DIRECTORY_PATHS[0]
            }

            (ReplaceType::SpeedTest { .. }, SimulationKind::Diabatic)
            | (ReplaceType::SpeedTest { .. }, SimulationKind::DiabaticOneSide) => {
                unimplemented!()
            }
        }
    }

    /// Returns the simulation dir of this [`SimulationSettings`].
    fn simulation_dir(&self) -> PathBuf {
        let mut path_string = String::new();
        path_string += self.template_dir();
        path_string += match self.material {
            Material::Concrete => "/concrete",
            Material::Steel => "/steel",
        };
        path_string += match (self.k_ramp, self.c_ramp) {
            (true, true) => "_k_c",
            (true, false) => "_k",
            (false, true) => "_c",
            (false, false) => "_simple",
        };
        match self.replace_type {
            ReplaceType::OneDimensional => {}
            ReplaceType::SpeedTest {
                size,
                threads: cores,
            } => {
                path_string += &format!("_{size}cells_{cores}cores");
            }
        }

        PathBuf::from(path_string)
    }

    /// Generates the build replace of this [`SimulationSettings`].
    ///
    /// # Panics
    ///
    /// Panics if `replace_type` is [`ReplaceType::SpeedTest`] and the size is not a multiple of 4.
    fn build_replace(&self) -> Replace {
        const DENSITY: &str = "#DENSITY#";
        const EMISSIVITY: &str = "#EMISSIVITY#";
        const CONDUCTIVITY: &str = "#CONDUCTIVITY#";
        const CONDUCTIVITY_RAMP: &str = "#CONDUCTIVITY_RAMP#";
        const SPECIFIC_HEAT: &str = "#SPECIFIC_HEAT#";
        const SPECIFIC_HEAT_RAMP: &str = "#SPECIFIC_HEAT_RAMP#";

        let mut replace = vec![];

        match self.material {
            Material::Concrete => {
                replace.push((DENSITY.to_string(), String::from("DENSITY=2300")));
                match self.simulation_kind {
                    SimulationKind::Diabatic | SimulationKind::DiabaticOneSide => {
                        replace.push((EMISSIVITY.to_string(), String::from("EMISSIVITY=0.70")));
                    }
                    SimulationKind::Adiabatic => {
                        replace.push((EMISSIVITY.to_string(), String::from("EMISSIVITY=0.0")));
                    }
                }
                if self.k_ramp {
                    replace.push((
                        CONDUCTIVITY.to_string(),
                        String::from("CONDUCTIVITY_RAMP=\"ramp_k\""),
                    ));
                    replace.push((
                        CONDUCTIVITY_RAMP.to_string(),
                        String::from(
                            r#"&RAMP ID = "ramp_k" T = 0.000 F =  1.4 /
&RAMP ID = "ramp_k" T = 100.000 F =  1.2 /
&RAMP ID = "ramp_k" T = 200.000 F =  1.1 /
&RAMP ID = "ramp_k" T = 300.000 F =  1.0 /
&RAMP ID = "ramp_k" T = 400.000 F =  0.9 /
&RAMP ID = "ramp_k" T = 500.000 F =  0.8 /
&RAMP ID = "ramp_k" T = 600.000 F =  0.7 /
&RAMP ID = "ramp_k" T = 700.000 F =  0.7 /
&RAMP ID = "ramp_k" T = 800.000 F =  0.6 /
&RAMP ID = "ramp_k" T = 900.000 F =  0.6 /
&RAMP ID = "ramp_k" T = 1000.000 F =  0.6 /
&RAMP ID = "ramp_k" T = 1100.000 F =  0.6 /
&RAMP ID = "ramp_k" T = 1200.000 F =  0.5 /"#,
                        ),
                    ));
                } else {
                    replace.push((CONDUCTIVITY.to_string(), String::from("CONDUCTIVITY=1.2")));
                }
                if self.c_ramp {
                    replace.push((
                        SPECIFIC_HEAT.to_string(),
                        String::from("SPECIFIC_HEAT_RAMP=\"ramp_c\""),
                    ));
                    replace.push((
                        SPECIFIC_HEAT_RAMP.to_string(),
                        String::from(
                            r#"&RAMP ID = "ramp_c" T = 20.000 F = 0.9000 /
&RAMP ID = "ramp_c" T = 100.000 F = 0.9000 /
&RAMP ID = "ramp_c" T = 200.000 F = 1.0000 /
&RAMP ID = "ramp_c" T = 400.000 F = 1.1000 /
&RAMP ID = "ramp_c" T = 1200.000 F = 1.1000 /"#,
                        ),
                    ));
                } else {
                    replace.push((
                        SPECIFIC_HEAT.to_string(),
                        String::from("SPECIFIC_HEAT=0.9000"),
                    ));
                }
            }
            Material::Steel => {
                replace.push((DENSITY.to_string(), String::from("DENSITY=7850")));
                match self.simulation_kind {
                    SimulationKind::Diabatic | SimulationKind::DiabaticOneSide => {
                        replace.push((EMISSIVITY.to_string(), String::from("EMISSIVITY=0.79")));
                    }
                    SimulationKind::Adiabatic => {
                        replace.push((EMISSIVITY.to_string(), String::from("EMISSIVITY=0.0")));
                    }
                }
                if self.k_ramp {
                    replace.push((
                        CONDUCTIVITY.to_string(),
                        String::from("CONDUCTIVITY_RAMP=\"ramp_k\""),
                    ));
                    replace.push((
                        CONDUCTIVITY_RAMP.to_string(),
                        String::from(
                            r#"&RAMP ID = "ramp_k" T = 20.000 F = 53.3 /
&RAMP ID = "ramp_k" T = 800.000 F = 27.3 /
&RAMP ID = "ramp_k" T = 1200.000 F = 27.3 /"#,
                        ),
                    ));
                } else {
                    replace.push((CONDUCTIVITY.to_string(), String::from("CONDUCTIVITY=53.3")));
                }
                if self.c_ramp {
                    replace.push((
                        SPECIFIC_HEAT.to_string(),
                        String::from("SPECIFIC_HEAT_RAMP=\"ramp_c\""),
                    ));
                    replace.push((
                        SPECIFIC_HEAT_RAMP.to_string(),
                        String::from(
                            r#"&RAMP ID = "ramp_c" T = 20.000 F = 0.4398 /
&RAMP ID = "ramp_c" T = 400.000 F = 0.6059 /
&RAMP ID = "ramp_c" T = 630.000 F = 0.7864 /
&RAMP ID = "ramp_c" T = 690.000 F = 0.9369 /
&RAMP ID = "ramp_c" T = 720.000 F = 1.3883 /
&RAMP ID = "ramp_c" T = 735.000 F = 5.0000 /
&RAMP ID = "ramp_c" T = 750.000 F = 1.4829 /
&RAMP ID = "ramp_c" T = 780.000 F = 0.9087 /
&RAMP ID = "ramp_c" T = 830.000 F = 0.7250 /
&RAMP ID = "ramp_c" T = 900.000 F = 0.6500 /
&RAMP ID = "ramp_c" T = 1200.000 F = 0.6500 /"#,
                        ),
                    ));
                } else {
                    replace.push((
                        SPECIFIC_HEAT.to_string(),
                        String::from("SPECIFIC_HEAT=0.4398"),
                    ));
                }
            }
        }

        match self.replace_type {
            ReplaceType::OneDimensional => {}
            ReplaceType::SpeedTest {
                size,
                threads: cores,
            } => {
                if size % 4 != 0 {
                    panic!("Size {size} musst be a multiple of 4.")
                }
                let row = size / 4;

                let mut mesh = String::new();
                for i in 0..cores {
                    let start = (row as f64 / cores as f64 * i as f64).floor();
                    let end = if i + 1 == cores {
                        row as f64
                    } else {
                        (row as f64 / cores as f64 * (i + 1) as f64).floor()
                    };
                    let cells = (end - start).round() as usize;
                    _ = writeln!(
                        mesh,
                        "&MESH IJK=3,{cells},4, XB=0.0,0.3,{},{},0.0,0.4 MPI_PROCESS={i} /",
                        start / 10.0,
                        end / 10.0
                    )
                }
                let y = (row as f64 / 10.0).to_string();
                replace.push((String::from("#MESH#"), mesh));
                replace.push((String::from("#Y#"), y));
            }
        }

        Replace(replace)
    }

    /// Create a simulation from this [`SimulationSettings`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the simulation folder can not be created.
    /// - the template file can not be read.
    fn create(self) -> Result<PathBuf> {
        let template_path = self.template_path();
        let simulation_dir = self.simulation_dir();
        std::fs::create_dir_all(&simulation_dir)
            .with_context(|| format!("Failed to create directories {:?}.", simulation_dir))?;
        let simulation_path = simulation_dir.join(SIMULATION_NAME);
        if !was_modified(&[&template_path], &[&simulation_path])
            .with_context(|| "Failed to get modification date.")?
        {
            println!(
                "  Simulation at {:?} is newer than the template.",
                simulation_path
            );
            return Ok(simulation_path);
        }

        let replace = self.build_replace();

        let mut file_writer = File::create(&simulation_path)
            .with_context(|| format!("Failed to create file at {:?}", simulation_path))?;

        let source_reader = File::open(&template_path)
            .with_context(|| format!("Failed to open file at {:?}", template_path))?;
        let source_reader = BufReader::new(source_reader);
        for line in source_reader.lines() {
            let mut line = line
                .with_context(|| format!("Failed to read line of file at {:?}.", template_path))?;

            for (name, value) in replace.0.iter() {
                line = line.replace(name, value);
            }

            writeln!(file_writer, "{line}").with_context(|| {
                format!("Failed to write line to file at {:?}", simulation_path)
            })?;
        }

        println!("  Created simulation at {:?}.", simulation_path);
        Ok(simulation_path)
    }
}

/// Creates all simulations from the templates.
///
/// # Errors
///
/// This function will return an error if the simulations can not be created.
pub fn create_simulations(
    method: Option<&[SimulationMethod]>,
    kind: Option<&[SimulationKind]>,
) -> Result<(), Vec<anyhow::Error>> {
    let one_dimensional = [
        SimulationKind::Adiabatic,
        SimulationKind::Diabatic,
        SimulationKind::DiabaticOneSide,
    ]
    .iter()
    .filter_map(|s| {
        if s.is_simulation_kind(kind) && SimulationMethod::OneDimensional.is_simulation_type(method)
        {
            Some([Material::Concrete, Material::Steel].iter().flat_map(|m| {
                [true, false].iter().map(|b| SimulationSettings {
                    simulation_kind: *s,
                    material: *m,
                    k_ramp: *b,
                    c_ramp: *b,
                    replace_type: ReplaceType::OneDimensional,
                })
            }))
        } else {
            None
        }
    })
    .flatten();

    let simulations = one_dimensional.map(Some).collect::<Vec<_>>();
    create_simulations_from_settings(simulations)
}

/// Creates all FDS speed test simulations from the template.
///
/// # Panics
///
/// Panics if the available thread can not be determent.
///
/// # Errors
///
/// This function will return an error if the simulations can not be created.
pub fn create_simulation_for_speed_test() -> std::result::Result<Vec<(PathBuf, usize, usize)>, Error>
{
    BENCHMARK_ELEMENTS
        .iter()
        .map(|size| {
            let threads: usize = std::thread::available_parallelism().unwrap().into();

            let simulation_settings = SimulationSettings {
                simulation_kind: SimulationKind::Adiabatic,
                material: Material::Concrete,
                k_ramp: true,
                c_ramp: true,
                replace_type: ReplaceType::SpeedTest {
                    size: *size,
                    threads,
                },
            };
            let path = simulation_settings.create()?;
            Ok((path, *size, threads))
        })
        .collect()
}

/// The simulations are created from the passed [`Vec<Option<SimulationSettings>>`].
///
/// # Errors
///
/// This function will return an error if the simulations can not be created.
fn create_simulations_from_settings(
    mut simulations: Vec<Option<SimulationSettings>>,
) -> std::result::Result<(), Vec<Error>> {
    let errors = simulations
        .par_iter_mut()
        .filter_map(|s: &mut Option<SimulationSettings>| s.take().map(|s| s.create()))
        .filter_map(|r| match r {
            std::result::Result::Ok(_) => None,
            Err(err) => Some(err),
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        std::result::Result::Ok(())
    } else {
        Err(errors)
    }
}
