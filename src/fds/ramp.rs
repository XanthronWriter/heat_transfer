use std::ops::{Deref, DerefMut};

use super::parser::Property;
use anyhow::*;

/// All interpolation values of a ramp as a list of tuples with temperature and value.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp(Vec<(f32, f32)>);
impl Ramp {
    /// Calculate the value for a given temperature.
    pub fn calc(&self, temperature: f32) -> f32 {
        if temperature <= self.0[0].0 {
            return self.0[0].1;
        }
        for i in 1..self.0.len() {
            let (t1, f1) = self.0[i];
            if temperature < t1 {
                let (t0, f0) = self.0[i - 1];
                return f0 + (f1 - f0) / (t1 - t0) * (temperature - t0);
            }
        }
        self.0[self.0.len() - 1].1
    }

    /// Multiply all values and return a new [`Ramp`].
    pub fn multiply(mut self, value: f32) -> Self {
        for i in 0..self.len() {
            self[i].1 *= value
        }
        self
    }
}
impl From<f32> for Ramp {
    fn from(value: f32) -> Self {
        Ramp(vec![(20.0, value)])
    }
}
impl Deref for Ramp {
    type Target = Vec<(f32, f32)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Ramp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Default)]
pub struct RampList(Vec<(String, Ramp)>);
impl RampList {
    /// Attempts to add a [`Ramp`] interpolation tuple from [`Property`]s to a [`Ramp`] in the list.
    ///
    /// # Errors
    ///
    /// This function will return an error if no interpolation tuple could be created from the [`Property`]s.
    pub fn try_add_from_properties(&mut self, properties: Vec<Property>) -> Result<()> {
        let (id, t, f) = try_ramp_line_from_properties(properties)
            .with_context(|| "Failed to add properties as ramp.")?;
        if let Some(last_entry) = self.0.last_mut() {
            if last_entry.0 == id {
                last_entry.1 .0.push((t, f));
                return Ok(());
            }
        }

        let vec = vec![(t, f)];
        self.0.push((id, Ramp(vec)));
        Ok(())
    }

    /// Tries to find a [`Ramp`] wich a given name inside the list.
    pub fn find(&self, id: &str) -> Option<&Ramp> {
        self.0
            .iter()
            .find_map(|(s, r)| if s as &str == id { Some(r) } else { None })
    }
}
impl Deref for RampList {
    type Target = Vec<(String, Ramp)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Attempts to create a [`Ramp`] interpolation tuple from [`Property`]s and the name of the ramp.
///
/// # Errors
///
/// This function will return an error if
/// - a Property is missing.
/// - a property can not be parsed.
fn try_ramp_line_from_properties(properties: Vec<Property>) -> Result<(String, f32, f32)> {
    let mut t = None;
    let mut f = None;
    let mut id = None;

    for Property { key, value } in properties {
        match key.as_str() {
            "T" => {
                t = Some(
                    value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?,
                )
            }
            "F" => {
                f = Some(
                    value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?,
                )
            }
            "ID" => id = Some(value),
            _ => {}
        }
    }

    match id.is_none() || t.is_none() || f.is_none() {
        true => {
            bail!(
                "On or more properties are missing. Found ID: {}, T: {}, F: {}.",
                id.is_some(),
                t.is_some(),
                f.is_some()
            )
        }
        false => Ok((id.unwrap(), t.unwrap(), f.unwrap())),
    }
}
