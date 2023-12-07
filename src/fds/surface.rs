use std::ops::{Deref, DerefMut};

use super::{material::MaterialList, parser::Property};

use anyhow::*;

#[derive(Debug, Clone, Copy)]
pub struct SurfaceCell {
    pub material_id: u32,
    pub size: f32,
}

/// Structure of the surface from several [`SurfaceCell`]s.
#[derive(Debug)]
pub struct Surface(Vec<SurfaceCell>);
impl Deref for Surface {
    type Target = Vec<SurfaceCell>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Surface {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// List of all [`Surface`]s inside a Simulation wich could be parsed correctly and the corresponding name.
#[derive(Debug, Default)]
pub struct SurfaceList(pub Vec<(String, Surface)>);
impl SurfaceList {
    /// Attempts to add a [`Surface`] to the list from the [`Property`]s and the [`MaterialList`].
    ///
    /// # Errors
    ///
    /// This function will return an error if no surface can be created from the [`Property`]s and the [`MaterialList`].
    pub fn try_add_from_properties(
        &mut self,
        properties: Vec<Property>,
        material_list: &MaterialList,
    ) -> Result<()> {
        self.0.push(
            try_surface_from_properties(properties, material_list)
                .with_context(|| "Failed to add properties as material.")?,
        );
        Ok(())
    }

    /// Tries to find a [`Surface`] wich a given name inside the list and returns its index.
    pub fn find_index(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|(s, _)| s as &str == id)
    }

    /// Tries to find a [`Surface`] wich a given name inside the list.
    pub fn find(&self, id: &str) -> Option<&Surface> {
        self.0
            .iter()
            .find_map(|(s, v)| if s as &str == id { Some(v) } else { None })
    }
}
impl Deref for SurfaceList {
    type Target = Vec<(String, Surface)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for SurfaceList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Attempts to create a [`Surface`] from the [`Property`]s and the [`MaterialList`].
///
/// # Errors
///
/// This function will return an error if
/// - the [`Property`] values could not be parsed.
/// - a [`Property`] is missing
/// - a requested [`Material`] could not be found.
fn try_surface_from_properties(
    properties: Vec<Property>,
    material_list: &MaterialList,
) -> Result<(String, Surface)> {
    let mut id = None;
    let mut material_ids = None;
    let mut thicknesses = None;

    for Property { key, value } in properties {
        match key.as_str() {
            "ID" => id = Some(value),
            "MATL_ID" => {
                material_ids = Some(
                    value
                        .split(',')
                        .map(|s| s.trim())
                        .map(|s| {
                            material_list
                                .find_index(s)
                                .ok_or(anyhow!("Could not find MATL wit ID = \"{value}\""))
                        })
                        .collect::<Result<Vec<usize>>>()?,
                );
            }
            "THICKNESS" => {
                thicknesses = Some(
                    value
                        .split(',')
                        .map(|s| s.trim())
                        .map(|s| {
                            s.parse::<f32>()
                                .with_context(|| format!("Failed to parse \"{s}\" to float."))
                        })
                        .collect::<Result<Vec<f32>>>()?,
                );
            }
            "HT3D" => {
                if value == ".TRUE." || value == "T" {
                    thicknesses = Some(vec![0.0])
                }
            }
            _ => {}
        }
    }

    match id.is_none() || material_ids.is_none() || thicknesses.is_none() {
        true => {
            bail!(
                "On or more properties are missing. Found ID: {}, MATL_ID: {}, THICKNESS: {}.",
                id.is_some(),
                material_ids.is_some(),
                thicknesses.is_some()
            )
        }
        false => {
            let surface_cells = cells_from_materials_and_thickness(
                material_list,
                &material_ids.unwrap(),
                &thicknesses.unwrap(),
            );

            Ok((id.unwrap(), Surface(surface_cells)))
        }
    }
}

/// Creates all cells for a Surface with smaller cells at the boarder and bigger cells in the middle for all layers
pub fn cells_from_materials_and_thickness(
    material_list: &MaterialList,
    material_ids: &[usize],
    thicknesses: &[f32],
) -> Vec<SurfaceCell> {
    let mut surface_cells = material_ids
        .iter()
        .zip(thicknesses)
        .flat_map(|(m, t)| cells_from_material_and_thickness(material_list, *m, *t))
        .collect::<Vec<_>>();
    surface_cells.insert(0, surface_cells[0]);
    surface_cells.push(surface_cells[surface_cells.len() - 1]);
    surface_cells
}

/// Creates all cells for a Surface with smaller cells at the boarder and bigger cells in the middle for a single layer.
fn cells_from_material_and_thickness(
    material_list: &MaterialList,
    material_id: usize,
    thickness: f32,
) -> impl Iterator<Item = SurfaceCell> {
    const DELTA_TIME: f32 = 1.0;
    const TEMPERATURE: f32 = 20.0;

    let material = &material_list[material_id].1;
    let specific_heat = material.specific_heat.calc(TEMPERATURE);
    let conductivity = material.conductivity.calc(TEMPERATURE);
    let density = material.density;

    let size = f32::sqrt((conductivity * DELTA_TIME) / (density * specific_heat));

    let (cell_count, start_size) = get_cell_count_and_start_size(size, thickness);
    (0..cell_count)
        .map(move |i| start_size * 2.0f32.powi(usize::min(i, cell_count - i - 1) as i32))
        .map(move |size| SurfaceCell {
            material_id: material_id as u32,
            size,
        })
}

/// Calculate the amount of cells in a single layer of a surface.
#[inline]
fn get_cell_count_and_start_size(size: f32, thickness: f32) -> (usize, f32) {
    const MAX_CELLS: usize = 999;
    let mut s = 0.0;
    for n in 1..=MAX_CELLS {
        s = 0.0;
        for i in 1..=(n) {
            s += 2.0f32.powi(usize::min(i - 1, n - i) as i32)
        }
        if thickness / s < size {
            return (n, thickness / s);
        }
    }
    (MAX_CELLS, thickness / s)
}
