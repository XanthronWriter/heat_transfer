use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

use ::anyhow::*;

use crate::{
    heat_transfer::simulations::{SimulationKind, SimulationMethod},
    modification::was_modified,
};

/// How many simulations can be run in parallel.
const MAX_PARALLEL: usize = 10;
/// Witch paths should be ignored. If the path starts wit `**` there can be an arbitrary amount of directory before.
const IGNORE_PATHS: [&str; 7] = [
    "**/template_heat_transfer.fds",
    "**/result",
    "fds/1D/AdiabaticSpeedTest",
    "fds/1D/Diabatic/multiple/1",
    "fds/1D/Diabatic/multiple/2",
    "fds/1D/Diabatic/multiple/4",
    "fds/1D/Diabatic/multiple/8",
];
/// The root path to search fo fds simulations
const ROOT_PATH: &str = "fds";

/// The status of a simulation that had run.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Status {
    Passed(PathBuf),
    Succeeded(PathBuf),
    Failed(PathBuf),
}

/// Check if a path should be ignored.
fn ignore<P: AsRef<Path>>(path: &Path, ignore_paths: &[P]) -> bool {
    for ignore_path in ignore_paths {
        let ignore_path = ignore_path.as_ref();
        if ignore_path.starts_with("**/") {
            let mut ignore_path_iter = ignore_path.iter();
            ignore_path_iter.next();
            if ignore_path_iter
                .rev()
                .zip(path.iter().rev())
                .all(|(p1, p2)| p1 == p2)
            {
                return true;
            }
        } else if path == ignore_path {
            return true;
        }
    }
    false
}

/// Find all files that end with `.fds` except for those that should be ignored.
fn find_fds_files<P1: AsRef<Path>, P2: AsRef<Path>>(
    start_path: P1,
    ignore_paths: &[P2],
    file_paths: &mut Vec<PathBuf>,
) {
    if let std::result::Result::Ok(read_dir) = fs::read_dir(start_path) {
        for path in read_dir {
            let std::result::Result::Ok(dir_entry) = path else {
                continue;
            };
            let path = dir_entry.path();
            if ignore(&path, ignore_paths) {
                continue;
            }
            if path.is_dir() {
                find_fds_files(path, ignore_paths, file_paths);
            } else if path.extension().map_or(false, |ext| ext == "fds") {
                file_paths.push(path)
            }
        }
    }
}

/// Returns the file name, the absolut result directory, and the run file path.
///
/// # Errors
///
/// This function will return an error if
/// - the result directory can not be created.
/// - the file name is unable to obtain.
fn get_run_simulation_path_data(file_path: &Path) -> Result<(String, PathBuf, PathBuf)> {
    let Some(result_dir) = file_path.parent() else {
        bail!("Failed to get parent of file {:?}", file_path)
    };
    let result_dir = result_dir.join("result");
    let run_file = result_dir.join("run");
    fs::create_dir_all(&result_dir)
        .with_context(|| format!("Failed to create directories for {:?}", result_dir))?;
    let abs_result_dir = result_dir.canonicalize().with_context(|| {
        format!(
            "Failed to get absolut path of {:?} while trying to run fds simulation at {:?}.",
            result_dir, file_path
        )
    })?;
    let Some(file_name) = file_path.file_name() else {
        bail!(
            "Failed to get name of file {:?} while trying to run fds simulation at {:?}.",
            file_path,
            file_path
        )
    };
    let Some(file_name) = file_name.to_str() else {
        bail!(
            "Failed to get string from file name for file {:?} while trying to run fds simulation at {:?}.",
            file_path,
            file_path,
        )
    };
    Ok((file_name.to_string(), abs_result_dir, run_file))
}

/// Checks if the results are newer then the simulation file, else the simulation is run.
///
/// # Errors
///
/// This function will return an error if
/// - the modification check fails.
/// - running the simulation fails.
/// - the run file can not be created.
fn run_simulation_checked(file_path: PathBuf) -> Result<Status> {
    let (file_name, abs_result_dir, run_file) = get_run_simulation_path_data(&file_path)?;
    if !was_modified(&[&file_path], &[&run_file])
        .with_context(|| format!("Failed to run fds simulation at {:?}.", file_path))?
    {
        return Ok(Status::Passed(file_path));
    }
    println!("  Start fds simulation at {:?}.", file_path);

    let result = run_simulation(file_path, file_name, abs_result_dir, 1)?;
    fs::File::create(&run_file).with_context(|| {
        format!(
            "Failed to create a run file for fds simulation at {:?}",
            run_file
        )
    })?;
    Ok(result)
}

/// Runs the FDS simulation without checking for modification date and creating a run file.
pub(super) fn run_simulation_unchecked(file_path: PathBuf, cores: usize) -> Result<Status> {
    let (file_name, abs_result_dir, _) = get_run_simulation_path_data(&file_path)?;
    run_simulation(file_path, file_name, abs_result_dir, cores)
}

/// Starts a FDS simulation.
///
/// # Errors
///
/// This function will return an error if running the simulation fails.
fn run_simulation(
    file_path: PathBuf,
    file_name: String,
    abs_result_dir: PathBuf,
    cores: usize,
) -> Result<Status> {
    let output = Command::new("mpiexec")
        .arg("-n")
        .arg(cores.to_string())
        .arg("fds")
        .arg(format!("../{}", file_name))
        .current_dir(abs_result_dir)
        .output()
        .with_context(|| format!("Failed to execute fds for {:?}.", file_path))?;
    let stderr = output.stderr;
    if stderr.len() > 150 {
        let slice = &stderr[(stderr.len() - 150)..stderr.len()];
        let end = core::str::from_utf8(slice).with_context(|| {
            format!(
                "Failed convert output to string for fds simulation at {:?}.",
                file_path
            )
        })?;
        if end.contains("ERROR") || end.contains("error") {
            return Ok(Status::Failed(file_path));
        }
    }

    Ok(Status::Succeeded(file_path))
}

/// Run all Simulations that can be found.
///
/// # Errors
///
/// This function will return an error if running the simulation fails.
pub fn run_simulations(
    method: Option<&[SimulationMethod]>,
    kind: Option<&[SimulationKind]>,
) -> Result<(), Vec<anyhow::Error>> {
    let ignore = [
        if !SimulationMethod::OneDimensional.is_simulation_type(method) {
            Some("fds/1D")
        } else {
            None
        },
        if !SimulationKind::Adiabatic.is_simulation_kind(kind) {
            Some("**/Adiabatic")
        } else {
            None
        },
        if !SimulationKind::Diabatic.is_simulation_kind(kind) {
            Some("**/Diabatic")
        } else {
            None
        },
        if !SimulationKind::DiabaticOneSide.is_simulation_kind(kind) {
            Some("**/DiabaticOneSide")
        } else {
            None
        },
    ];
    let ignore = ignore
        .iter()
        .flatten()
        .chain(IGNORE_PATHS.iter())
        .collect::<Vec<_>>();

    let mut file_paths = vec![];
    find_fds_files(ROOT_PATH, &ignore, &mut file_paths);

    let file_paths = Arc::new(Mutex::new(file_paths));

    let mut handles = Vec::with_capacity(MAX_PARALLEL);
    for _ in 0..MAX_PARALLEL {
        let file_paths = file_paths.clone();
        let handle = thread::spawn(move || -> Vec<Result<Status, anyhow::Error>> {
            let mut results = vec![];
            while let Some(file_path) = {
                match file_paths.lock() {
                    std::result::Result::Ok(mut ok) => ok.pop(),
                    Err(_) => {
                        results.push(Err(anyhow!(
                            "Thread aborted, because an other thread panicked."
                        )));
                        None
                    }
                }
            } {
                let result = run_simulation_checked(file_path);
                results.push(result);
            }
            results
        });
        handles.push(handle)
    }
    let mut run = vec![];
    let mut errors = vec![];

    handles
        .into_iter()
        .flat_map(|h| match h.join() {
            std::result::Result::Ok(ok) => ok,
            Err(err) => vec![Err(anyhow!("Error in thread. {:?}", err))],
        })
        .for_each(|f| match f {
            std::result::Result::Ok(r) => run.push(r),
            Err(e) => errors.push(e),
        });

    let mut any_failed = false;
    run.sort();
    for r in run {
        match r {
            Status::Passed(p) => {
                println!(
                    "  Result for fds simulation at {:?} is newer than the input file.",
                    p
                );
            }
            Status::Succeeded(p) => {
                println!("  Finished fds simulation at {:?} successfully.", p);
            }
            Status::Failed(p) => {
                any_failed = true;
                println!("  Failed fds simulation at {:?}.", p);
            }
        }
    }
    if any_failed {
        errors.push(anyhow!("Some fds simulations failed to run."));
    }

    if errors.is_empty() {
        std::result::Result::Ok(())
    } else {
        Err(errors)
    }
}
