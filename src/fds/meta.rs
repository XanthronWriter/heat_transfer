use super::SurfaceList;
use anyhow::*;

/// Meta data for a simulation, wether it is 1D or 3D. The fist line of a simulation must be set with a meta definition.
/// 1D supports multiple materials. A definition could look like the following.
/// `//META SURF_STEEL; SURF_STEEL;`
/// 3D only supports one material and must be defined like the following.
/// `//META 8,8,8, SURF_STEEL;`
pub enum Meta {
    OneDimensional {
        surface_ids: Vec<usize>,
    },
    ThreeDimensional {
        x: usize,
        y: usize,
        z: usize,
        surface_id: usize,
    },
}
impl Meta {
    /// Attempts to create a [`Meta`] struct from the passed meta information.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the dimension values cannot be converted to numbers.
    /// - the specified surfaces do not exist.
    /// - when there is 3D data initialized, but multiple surface are defined.
    pub fn try_new(
        dimensions: Option<(String, String, String)>,
        meta: Vec<String>,
        surface_list: &SurfaceList,
    ) -> Result<Self> {
        let meta = match dimensions {
            Some((x, y, z)) => {
                if meta.len() > 1 {
                    bail!("Multiple surfaces in 3D are not supported.")
                } else {
                    let name = &meta[0];
                    let x: usize = x
                        .parse()
                        .with_context(|| format!("Failed to parse x value \"{x}\"."))?;
                    let y: usize = y
                        .parse()
                        .with_context(|| format!("Failed to parse y value \"{y}\"."))?;
                    let z: usize = z
                        .parse()
                        .with_context(|| format!("Failed to parse z value \"{z}\"."))?;
                    let surface_id = surface_list
                        .find_index(name)
                        .ok_or(anyhow!("Could not find SURF wit ID = \"{name}\"."))?;
                    Meta::ThreeDimensional {
                        x,
                        y,
                        z,
                        surface_id,
                    }
                }
            }
            None => {
                let surface_ids = meta
                    .into_iter()
                    .map(|name| {
                        let surface_id = surface_list
                            .find_index(&name)
                            .ok_or(anyhow!("Could not find SURF wit ID = \"{name}\"."))?;
                        Ok(surface_id)
                    })
                    .collect::<Result<Vec<usize>>>()?;
                Meta::OneDimensional { surface_ids }
            }
        };
        Ok(meta)
    }
}
