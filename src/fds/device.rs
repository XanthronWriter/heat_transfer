use anyhow::*;
use std::{
    fs::File,
    io::{BufRead, BufReader, Lines},
    path::Path,
};

/// Structure that has a reader for the `heat_transfer_devc.csv` file and can read line by line. When reading, the selected entries are returned as [`Vec<f32>`].
pub struct Devices {
    /// Indexes of the selected entries
    indexes: Vec<usize>,
    /// reader to the device file
    lines: Lines<BufReader<File>>,
}

impl Devices {
    /// Attempts to create a [`Devices`] from the path to the device file and the selected entries.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the file could not be read
    /// - one passed entry could not be found inside the file.
    pub fn try_new<P: AsRef<Path>, S: AsRef<str>>(path: P, devices: &[S]) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).with_context(|| format!("Failed to open file at {path:?}."))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut indexes = vec![None; devices.len()];

        lines.next();

        if let Some(line) = lines.next() {
            let line = line.with_context(|| format!("Failed to read line in file at {path:?}."))?;
            for (i, s) in line.split(',').map(|s| s.replace('\"', "")).enumerate() {
                for (j, device) in devices.iter().enumerate() {
                    if device.as_ref() == s {
                        indexes[j] = Some(i);
                    }
                }
            }
        }

        if let Some(indexes) = indexes.into_iter().collect::<Option<Vec<_>>>() {
            if indexes.len() == devices.len() {
                return Ok(Self { indexes, lines });
            }
        };

        Err(anyhow!(
            "On or multiple devices are missing. {:?}",
            devices.iter().map(|s| s.as_ref()).collect::<Vec<_>>()
        ))
    }
}

impl Iterator for Devices {
    type Item = Result<Vec<f32>>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = match self.lines.next()? {
            std::result::Result::Ok(ok) => ok,
            Err(err) => {
                return Some(Err(err).with_context(|| "Failed to read line."));
            }
        };
        let splits = line.split(',').map(|s| s.trim()).collect::<Vec<_>>();
        let len = self.indexes.len();
        let mut devices = Vec::with_capacity(len);

        for i in 0..len {
            let device = match splits[self.indexes[i]].parse::<f32>() {
                std::result::Result::Ok(ok) => ok,
                Err(err) => {
                    return Some(Err(err.into()));
                }
            };
            devices.push(device);
        }

        Some(Ok(devices))
    }
}
