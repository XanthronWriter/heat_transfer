use std::ops::{Deref, DerefMut};

use super::{
    parser::Property,
    ramp::{Ramp, RampList},
};

use anyhow::*;

/// All relevant and supported data of a material.
#[derive(Debug, Clone)]
pub struct Material {
    pub specific_heat: Ramp,
    pub conductivity: Ramp,
    pub density: f32,
    pub emissivity: f32,
}

/// List of all [`Material`]s inside a Simulation wich could be parsed correctly and the corresponding name.
#[derive(Debug, Default)]
pub struct MaterialList(Vec<(String, Material)>);
impl MaterialList {
    /// Attempts to add a [`Material`] to the list from the [`Property`]s and the existing [`Ramp`]s inside the [`RampList`].
    ///
    /// # Errors
    ///
    /// This function will return an error if no material can be created from the [`Property`]s and the [`RampList`].
    pub fn try_add_from_properties(
        &mut self,
        properties: Vec<Property>,
        ramp_list: &RampList,
    ) -> Result<()> {
        self.0.push(
            try_material_from_properties(properties, ramp_list)
                .with_context(|| "Failed to add properties as material.")?,
        );
        Ok(())
    }

    /// Tries to find a [`Material`] wich a given name inside the list.
    pub fn find_index(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|(s, _)| s as &str == id)
    }

    /// Removes the associated name to a [`Material`] and converts this [`MaterialList`] to a [`Vec<Material>`].
    pub fn into_materials(self) -> Vec<Material> {
        self.0.into_iter().map(|(_, m)| m).collect()
    }
}
impl Deref for MaterialList {
    type Target = Vec<(String, Material)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for MaterialList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Attempts to create a [`Material`] from the [`Property`]s and the existing [`Ramp`]s inside the [`RampList`].
///
/// # Errors
///
/// This function will return an error if
/// - the [`Property`] values could not be parsed.
/// - a [`Property`] is missing
/// - a requested [`Ramp`] could not be found.
fn try_material_from_properties(
    properties: Vec<Property>,
    ramp_list: &RampList,
) -> Result<(String, Material)> {
    let mut id = None;
    let mut specific_heat = None;
    let mut conductivity = None;
    let mut density = None;
    let mut emissivity = None;

    for Property { key, value } in properties {
        match key.as_str() {
            "ID" => id = Some(value),
            "SPECIFIC_HEAT_RAMP" => {
                specific_heat = match ramp_list.find(&value) {
                    Some(some) => Some(some.clone().multiply(1000.0)),
                    None => {
                        bail!("Could not find RAMP wit ID = \"{value}\"")
                    }
                }
            }
            "SPECIFIC_HEAT" => {
                specific_heat = Some(
                    (value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?
                        * 1000.0)
                        .into(),
                )
            }
            "CONDUCTIVITY_RAMP" => {
                conductivity = match ramp_list.find(&value) {
                    Some(some) => Some(some.clone()),
                    None => {
                        bail!("Could not find RAMP wit ID = \"{value}\"")
                    }
                }
            }
            "CONDUCTIVITY" => {
                conductivity = Some(
                    value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?
                        .into(),
                )
            }
            "DENSITY" => {
                density = Some(
                    value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?,
                )
            }
            "EMISSIVITY" => {
                emissivity = Some(
                    value
                        .parse::<f32>()
                        .with_context(|| format!("Failed to parse \"{value}\" to float."))?,
                )
            }
            _ => {}
        }
    }

    match id.is_none()
            || specific_heat.is_none()
            || conductivity.is_none()
            || density.is_none()
            || emissivity.is_none() {
        true => bail!("On or more properties are missing. Found ID: {}, SPECIFIC_HEAT(_RAMP): {}, CONDUCTIVITY(_RAMP): {}, DENSITY: {}, EMISSIVITY: {}.", id.is_some(), specific_heat.is_some(), conductivity.is_some(), density.is_some(), emissivity.is_some()),
        false => {
            std::result::Result::Ok((
                id.unwrap(),
                Material {
                    specific_heat: specific_heat.unwrap(),
                    conductivity: conductivity.unwrap(),
                    density: density.unwrap(),
                    emissivity: emissivity.unwrap(),
                },
            ))
        }
    }
}
