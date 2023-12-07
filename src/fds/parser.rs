//! Load a fds input file and read all Ramps, Materials, and Surfaces

use std::path::Path;

use chumsky::{
    error::Cheap,
    prelude::*,
    text::{newline, whitespace},
};

use super::{material::MaterialList, meta::Meta, ramp::RampList, surface::SurfaceList};

use anyhow::*;

/// A [`Property`] consisting of the name and the value(s).
#[derive(Debug, Clone)]
pub struct Property {
    pub key: String,
    pub value: String,
}

/// The different supported namespaces, witch the parser search and parse for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NameSpace {
    Ramp,
    Material,
    Surface,
}

/// Reads an FDS simulation and determines the relevant data [`Meta`], [`MaterialList`] and [`SurfaceList`].
///
/// # Errors
///
/// This function will return an error if
/// - the file cannot be read.
/// - the metadata at the beginning of the file has been forgotten or cannot be converted correctly.
/// - no [`RampList`] can be created because the file is structured incorrectly
/// - no [`MaterialList`] can be created because the file is structured incorrectly
/// - no [`SurfaceList`] can be created because the file is structured incorrectly
pub fn parse_script_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<(Meta, MaterialList, SurfaceList)> {
    let path = path.as_ref();

    let script_parser = script_parser();

    let script = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file at {path:?}"))?;
    let ((dimensions, meta), namespaces) = match script_parser.parse(script) {
        std::result::Result::Ok(ok) => ok,
        Err(_) => bail!("Meta data is missing or incorrect formatted."),
    };

    let (ramps, other): (Vec<_>, Vec<_>) = namespaces
        .into_iter()
        .partition(|(n, _)| *n == NameSpace::Ramp);

    let mut ramp_list = RampList::default();
    for properties in ramps.into_iter().map(|(_, r)| r) {
        ramp_list.try_add_from_properties(properties)?;
    }

    let (materials, surfaces): (Vec<_>, Vec<_>) = other
        .into_iter()
        .partition(|(n, _)| *n == NameSpace::Material);

    let mut material_list = MaterialList::default();
    for properties in materials.into_iter().map(|(_, m)| m) {
        material_list.try_add_from_properties(properties, &ramp_list)?;
    }

    let mut surface_list = SurfaceList::default();
    for properties in surfaces.into_iter().map(|(_, s)| s) {
        // HACK Some surface do not need all properties.
        _ = surface_list.try_add_from_properties(properties, &material_list);
    }
    let meta = Meta::try_new(dimensions, meta, &surface_list)?;

    Ok((meta, material_list, surface_list))
}

/// Parses the dimension information in the meta data. 3 numbers must be specified that are separated by `,`. The last element must also end with `,`
///
/// # Panics
///
/// Panics if there are not 3 elements.
fn meta_dimension_parser() -> impl Parser<char, (String, String, String), Error = Cheap<char>> {
    filter(char::is_ascii_digit)
        .repeated()
        .at_least(1)
        .then_ignore(just(',').padded())
        .repeated()
        .exactly(3)
        .map(|dimensions| {
            let mut dimensions = dimensions.into_iter();
            if let (Some(x), Some(y), Some(z)) =
                (dimensions.next(), dimensions.next(), dimensions.next())
            {
                (
                    x.into_iter().collect::<String>(),
                    y.into_iter().collect::<String>(),
                    z.into_iter().collect::<String>(),
                )
            } else {
                panic!("No 3 elements in vector wich should not occur.")
            }
        })
}

/// Splits the meta data into surface names, which are separated with `;`. The last element must also end with `;`. There must be no spaces in the name.
fn meta_surface_parser() -> impl Parser<char, Vec<String>, Error = Cheap<char>> {
    filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_')
        .repeated()
        .at_least(1)
        .then_ignore(just(';').padded())
        .repeated()
        .at_least(1)
        .map(|surfaces| {
            surfaces
                .into_iter()
                .map(|surface| surface.into_iter().collect::<String>())
                .collect::<Vec<_>>()
        })
}

/// Determines the metha data. This must be at the very beginning of the file and start with `//META`. If it is a 3D simulation, the dimension must be defined and then only one surface. In the 1D case, several surfaces can be defined.
fn meta_parser(
) -> impl Parser<char, (Option<(String, String, String)>, Vec<String>), Error = Cheap<char>> {
    just("//")
        .then(just("META").padded())
        .ignore_then(meta_dimension_parser().or_not())
        .then(meta_surface_parser())
}

/// A property assignment can end with a `,` so that the name of the next property is not mistakenly recognized as an assignment, this function is executed.
fn ignore_parser() -> impl Parser<char, (), Error = Cheap<char>> {
    take_until(none_of(",=/ ").repeated().at_least(1))
        .then(whitespace())
        .then(just('='))
        .ignored()
}
/// Attempts to determine a property with name and assignment. If a string is assigned to the property, this string must not contain `=`, `/` and space.
pub(super) fn property_parser() -> impl Parser<char, Property, Error = Cheap<char>> {
    none_of("/").rewind().ignore_then(
        take_until(just('=').padded().ignored())
            .then(take_until(
                ignore_parser().or(just('/').padded().ignored()).rewind(),
            ))
            .then_ignore(just(',').or_not().padded().ignored())
            .padded()
            .map(|((key, _), (value, _))| Property {
                key: key.into_iter().collect::<String>(),
                value: value
                    .into_iter()
                    .filter(|c| *c != '\'' && *c != '\"')
                    .collect::<String>(),
            }),
    )
}

/// Determines the namespace. This must begin with `&`. Only the namespaces defined in [`NameSpace`] are supported.
pub(super) fn namespace_parser(
) -> impl Parser<char, (NameSpace, Vec<Property>), Error = Cheap<char>> {
    just("&RAMP")
        .to(NameSpace::Ramp)
        .or(just("&MATL").to(NameSpace::Material))
        .or(just("&SURF").to(NameSpace::Surface))
        .padded()
        .then(property_parser().repeated())
}

/// Parses the whole text to relevant data.
pub(super) fn script_parser() -> impl Parser<
    char,
    (
        (Option<(String, String, String)>, Vec<String>),
        Vec<(NameSpace, Vec<Property>)>,
    ),
    Error = Cheap<char>,
> {
    meta_parser().then(
        namespace_parser()
            .map(Some)
            .or(take_until(newline()).to(None))
            .repeated()
            .flatten(),
    )
}
