use std::{
    fs::File,
    io::{BufRead, BufReader, LineWriter, Lines, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::*;

use crate::{
    fds::Devices,
    heat_transfer::{
        one_dimensional::{
            cpu::{CPUSetupData, ADIABATIC_H, CONST_TEMP_H},
            gpu_m1, gpu_m2, gpu_m3, HeatTransfer1D,
        },
        simulations::duplication,
    },
};

use super::{
    load_fds_simulation_one_dimensional, SimulationKind, SimulationMethod, SimulationType1D,
    DELTA_TIME_SOLID_FACTOR,
};

/// The amount a simulation is rerun in order to determent the median simulation time.
pub const SIMULATION_RERUNS: usize = 100;
/// The amount of simulation steps that should be done.
pub const SIMULATION_STEPS: usize = 100;

/// An helper struct for reading the simulation data for a benchmark test line by line witch means simulation step by simulation step.
pub struct SimulationBenchmarkDevice {
    last_time: f32,
    buffer_wall_heat_transfer_coefficient: Vec<[f32; 2]>,
    buffer_wall_q_in: Vec<[f32; 2]>,
    simulation_kind: SimulationKind,
    device: Devices,
}
impl SimulationBenchmarkDevice {
    /// Attempts to create a [`SimulationBenchmarkDevice`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the transmitted device file cannot be read.
    /// - the transmitted device file does not match the requested devices.
    /// - the number of wall elements is 0.
    pub fn try_new<P: AsRef<Path>>(
        simulation_kind: SimulationKind,
        path: P,
        wall_element_count: usize,
    ) -> Result<Self> {
        const DIABATIC: [&str; 6] = [
            "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_WEST",
            "DEVC_GAS_TEMPERATURE_WEST",
            "DEVC_WALL_RADIATIVE_HEAT_FLUX_WEST",
            "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_EAST",
            "DEVC_GAS_TEMPERATURE_EAST",
            "DEVC_WALL_RADIATIVE_HEAT_FLUX_EAST",
        ];

        const DIABATIC_NO_RADIATION: [&str; 4] = [
            "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_WEST",
            "DEVC_GAS_TEMPERATURE_WEST",
            "DEVC_WALL_HEAT_TRANSFER_COEFFICIENT_EAST",
            "DEVC_GAS_TEMPERATURE_EAST",
        ];

        const ADIABATIC: [&str; 0] = [];

        let device_path = path.as_ref().join("result/heat_transfer_devc.csv");

        if wall_element_count == 0 {
            bail!("Count should be at least 1.")
        } else if wall_element_count > 1 {
            let mut devices = (0..wall_element_count)
                .flat_map(|i| {
                    match simulation_kind {
                        SimulationKind::Diabatic => DIABATIC.iter(),
                        SimulationKind::DiabaticOneSide => DIABATIC_NO_RADIATION.iter(),
                        SimulationKind::Adiabatic => ADIABATIC.iter(),
                    }
                    .map(move |s| format!("{s}_{}", i + 1))
                })
                .collect::<Vec<String>>();
            devices.insert(0, "Time".to_string());
            let device = Devices::try_new(device_path, &devices)?;
            std::result::Result::Ok(Self {
                last_time: 0.0,
                buffer_wall_heat_transfer_coefficient: vec![[0.0, 0.0]; wall_element_count],
                buffer_wall_q_in: vec![[0.0, 0.0]; wall_element_count],
                simulation_kind,
                device,
            })
        } else {
            let mut devices = match simulation_kind {
                SimulationKind::Diabatic => DIABATIC.to_vec(),
                SimulationKind::DiabaticOneSide => DIABATIC_NO_RADIATION.to_vec(),
                SimulationKind::Adiabatic => ADIABATIC.to_vec(),
            };
            devices.insert(0, "Time");
            let device = Devices::try_new(device_path, &devices)?;
            std::result::Result::Ok(Self {
                last_time: 0.0,
                buffer_wall_heat_transfer_coefficient: vec![[0.0, 0.0]],
                buffer_wall_q_in: vec![[0.0, 0.0]],
                simulation_kind,
                device,
            })
        }
    }

    /// Returns the kind of this [`SimulationBenchmarkDevice`].
    pub fn kind(&self) -> SimulationKind {
        self.simulation_kind
    }

    /// Returns the buffers of this [`SimulationBenchmarkDevice`].
    pub fn buffers(&self) -> (&[[f32; 2]], &[[f32; 2]]) {
        (
            &self.buffer_wall_heat_transfer_coefficient,
            &self.buffer_wall_q_in,
        )
    }
}
impl Iterator for SimulationBenchmarkDevice {
    type Item = Result<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        for _ in 0..(DELTA_TIME_SOLID_FACTOR - 1) {
            if let Err(err) = self.device.next()? {
                return Some(Err(err));
            }
        }
        let data = match self.device.next()? {
            std::result::Result::Ok(ok) => ok,
            Err(err) => return Some(Err(err)),
        };

        let time = data[0];
        let delta_time = time - self.last_time;
        self.last_time = time;

        for i in 0..self.buffer_wall_heat_transfer_coefficient.len() {
            match self.simulation_kind {
                SimulationKind::Diabatic => {
                    let n = i * 6;
                    self.buffer_wall_heat_transfer_coefficient[i] = [data[n + 1], data[n + 4]];
                    self.buffer_wall_q_in[i] = [
                        data[1] * data[n + 2] + data[n + 3] * 1000.0,
                        data[n + 4] * data[n + 5] + data[n + 6] * 1000.0,
                    ];
                }
                SimulationKind::DiabaticOneSide => {
                    let n = i * 4;
                    self.buffer_wall_q_in[i] = [data[n + 1], data[n + 3]];
                    self.buffer_wall_q_in[i] =
                        [data[n + 1] * data[n + 2], data[n + 3] * data[n + 4]];
                }
                SimulationKind::Adiabatic => {
                    self.buffer_wall_heat_transfer_coefficient[i] = [CONST_TEMP_H, ADIABATIC_H];
                    self.buffer_wall_q_in[i] = [200.0, 0.0];
                }
            }
        }

        Some(std::result::Result::Ok(delta_time))
    }
}

/// Helper to write the benchmarks to disk.
pub struct BenchmarkWriter {
    pub size: usize,
    line_writer: LineWriter<File>,
}
impl BenchmarkWriter {
    /// Trys to create a [`BenchmarkWriter`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the path can not be determent.
    /// - the destination directory can not be created.
    /// - the data can not be written to the file.
    pub fn try_new<P: AsRef<Path>>(
        path: P,
        benchmark_path_part: &BenchmarkPathPart,
        size: usize,
    ) -> Result<Self> {
        let dir_path = path.as_ref().join(benchmark_path_part.path_str()?);
        std::fs::create_dir_all(&dir_path)
            .with_context(|| format!("Failed to create directories at path {dir_path:?}"))?;
        let write_path = dir_path.join(format!("{size}.bin"));
        let file = std::fs::File::create(write_path)
            .with_context(|| "Failed to create file at {write_path:?}.")?;
        let mut line_writer = LineWriter::new(file);
        writeln!(line_writer, "Size: {size}")
            .with_context(|| "Failed to write size inside buffer")?;
        writeln!(line_writer, "Steps: {SIMULATION_STEPS}")
            .with_context(|| "Failed to write size inside buffer")?;
        writeln!(line_writer, "Reruns: {SIMULATION_RERUNS}")
            .with_context(|| "Failed to write size inside buffer")?;

        std::result::Result::Ok(Self { size, line_writer })
    }

    /// Writes the simulation time to the buffer.
    ///
    /// # Errors
    ///
    /// This function will return an error if the write fails.
    pub fn write(&mut self, time: f64) -> std::result::Result<(), std::io::Error> {
        writeln!(self.line_writer, "{}", time)
    }
}

/// Helper to read the benchmark from disk.
pub struct BenchmarkReader {
    lines: Lines<BufReader<File>>,
    _size: usize,
    _steps: usize,
    _reruns: usize,
}
impl BenchmarkReader {
    /// Tries to create a [`BenchmarkReader`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the passed file does not exist.
    /// - the file can not be read.
    pub fn try_new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Failed to open file at {:?}.", path))?;
        let mut lines = BufReader::new(file).lines();
        let size = lines
            .next()
            .ok_or(anyhow!("Line is missing."))??
            .split(':')
            .last()
            .ok_or(anyhow!("Failed to split line."))?
            .trim()
            .parse::<usize>()?;
        let steps = lines
            .next()
            .ok_or(anyhow!("Line is missing."))??
            .split(':')
            .last()
            .ok_or(anyhow!("Failed to split line."))?
            .trim()
            .parse::<usize>()?;
        let reruns = lines
            .next()
            .ok_or(anyhow!("Line is missing."))??
            .split(':')
            .last()
            .ok_or(anyhow!("Failed to split line."))?
            .trim()
            .parse::<usize>()?;

        Ok(Self {
            lines,
            _size: size,
            _steps: steps,
            _reruns: reruns,
        })
    }
}
impl Iterator for BenchmarkReader {
    type Item = Result<f64>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.next()?;

        match line {
            std::result::Result::Ok(s) => match s.parse::<f64>() {
                std::result::Result::Ok(ok) => Some(Ok(ok)),
                Err(err) => Some(Err(anyhow::Error::from(err))),
            },
            Err(err) => Some(Err(anyhow::Error::from(err))),
        }
    }
}

/// Helper to create the path to the benchmark.
#[derive(Debug, Clone)]
pub struct BenchmarkPathPart {
    pub prefix: Option<String>,
    pub label: String,
    pub simulation_method: SimulationMethod,
}

impl BenchmarkPathPart {
    /// Creates a new [`BenchmarkPathPart`].
    pub fn new(prefix: Option<String>, label: String, simulation_method: SimulationMethod) -> Self {
        Self {
            prefix,
            label,
            simulation_method,
        }
    }

    /// Returns the path str of this [`BenchmarkPathPart`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the path can not be determent.
    pub fn path_str(&self) -> Result<String> {
        let simulation_method_path_str = self.simulation_method.path_str()?;

        match &self.prefix {
            Some(prefix) => Ok(format!(
                "{}/{}/{}",
                prefix, self.label, simulation_method_path_str,
            )),
            None => Ok(format!("{}/{}", self.label, simulation_method_path_str,)),
        }
    }
}

/// Executes a benchmark simulation.
///
/// # Errors
///
/// This function will return an error if
/// - the fds simulation file can not be loaded.
/// - the [`WallElement`]s can not be duplicated correctly.
/// - a [`BenchmarkWriter`] can not be created.
/// - it failed to flush the stdout for status information.
/// - it failed to initialize the simulation.
/// - it failed to update the simulation.
/// - the [`BenchmarkWriter`] failed to read the next time step.
fn one_dimensional<P: AsRef<Path>, S: HeatTransfer1D>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
    simulation_type: SimulationType1D,
) -> Result<()> {
    let path = path.as_ref();
    let (materials, wall_elements) = load_fds_simulation_one_dimensional(path)
        .with_context(|| format!("Failed to build simulation for file at {:?}", path))?;

    let benchmark_path_part = BenchmarkPathPart::new(None, label, simulation_type.into());
    for &e in elements {
        let duplication = duplication(e, wall_elements.len())?;

        println!("  Start benchmark with size {}.", e);
        let mut benchmark_writer = BenchmarkWriter::try_new(
            PathBuf::from("benchmark").join(path),
            &benchmark_path_part,
            e,
        )
        .with_context(|| {
            format!(
                "Failed to create benchmark writer for simulation at {:?}",
                path
            )
        })?;
        let mut time = 10.0;
        print!("  Simulation 0/{SIMULATION_RERUNS}");
        std::io::stdout()
            .flush()
            .with_context(|| "Failed to flush stdout.")?;
        for i in 0..SIMULATION_RERUNS {
            if time >= 0.25 {
                print!("\r   Simulation {}/{SIMULATION_RERUNS}", i + 1);
                std::io::stdout().flush().unwrap();
                time = 0.0;
            }
            let mut device =
                SimulationBenchmarkDevice::try_new(simulation_kind, path, wall_elements.len())?;
            // Wall elements are mapped to the new length. If there are more then 1 type, the types are cloned with an equal amount one after another.
            let wall_elements = wall_elements
                .iter()
                .flat_map(|w| vec![w.clone(); duplication])
                .collect::<Vec<_>>();

            let mut gpu_setup_data = S::setup(materials.clone(), wall_elements)
                .with_context(|| "Failed to setup shader.")?;

            let mut wall_temperature_buffer = vec![[0.0; 2]; e];
            let mut elapsed = 0.0;
            let mut i = 0;
            while let Some(delta_time) = device.next() {
                let delta_time = delta_time?;
                if i > SIMULATION_STEPS {
                    break;
                }
                i += 1;

                let (wall_heat_transfer_coefficients, wall_q_in) = device.buffers();
                let wall_heat_transfer_coefficients = wall_heat_transfer_coefficients
                    .iter()
                    .flat_map(|&v| vec![v; duplication])
                    .collect::<Vec<_>>();
                let wall_q_in = wall_q_in
                    .iter()
                    .flat_map(|&v| vec![v; duplication])
                    .collect::<Vec<_>>();

                let start = Instant::now();

                gpu_setup_data
                    .update(
                        delta_time,
                        &wall_heat_transfer_coefficients,
                        &wall_q_in,
                        &mut wall_temperature_buffer,
                    )
                    .with_context(|| "Failed update")?;
                elapsed += start.elapsed().as_secs_f64();
                time += elapsed;
            }
            benchmark_writer.write(elapsed).with_context(|| {
                format!(
                    "Failed to write to the benchmark writer for simulation at {:?}",
                    path
                )
            })?;
        }
        println!("\r   Simulation {SIMULATION_RERUNS}/{SIMULATION_RERUNS}");
    }
    Ok(())
}

/// Start the CPU benchmark simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_cpu<P: AsRef<Path>>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
) -> Result<()> {
    one_dimensional::<P, CPUSetupData>(
        path,
        label,
        elements,
        simulation_kind,
        SimulationType1D::Cpu,
    )
}

/// Start the GPU M1 benchmark simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m1<P: AsRef<Path>>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
) -> Result<()> {
    one_dimensional::<P, gpu_m1::GPUSetupData>(
        path,
        label,
        elements,
        simulation_kind,
        SimulationType1D::GpuM1,
    )
}

/// Start the GPU M2 benchmark simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m2<P: AsRef<Path>>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
) -> Result<()> {
    one_dimensional::<P, gpu_m2::GPUSetupData>(
        path,
        label,
        elements,
        simulation_kind,
        SimulationType1D::GpuM2,
    )
}

/// Start the GPU M3 benchmark simulation.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_gpu_m3<P: AsRef<Path>>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
) -> Result<()> {
    one_dimensional::<P, gpu_m3::GPUSetupData>(
        path,
        label,
        elements,
        simulation_kind,
        SimulationType1D::GpuM3,
    )
}

/// Start the benchmark simulation for a given simulation method.
///
/// # Errors
///
/// This function will return an error if the simulation can not be started.
pub fn one_dimensional_by_simulation_type<P: AsRef<Path>>(
    path: P,
    label: String,
    elements: &[usize],
    simulation_kind: SimulationKind,
    simulation_type: SimulationType1D,
) -> Result<()> {
    match simulation_type {
        SimulationType1D::Cpu => one_dimensional_cpu(path, label, elements, simulation_kind),
        SimulationType1D::GpuM1 => one_dimensional_gpu_m1(path, label, elements, simulation_kind),
        SimulationType1D::GpuM2 => one_dimensional_gpu_m2(path, label, elements, simulation_kind),
        SimulationType1D::GpuM3 => one_dimensional_gpu_m3(path, label, elements, simulation_kind),
    }
}
