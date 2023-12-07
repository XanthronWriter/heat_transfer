use crate::fds::{Material, Ramp};
use std::fmt::Write;

/// Transforms a [`Ramp`] to the inner of a ramp function for a shader.
fn ramp_to_body(ramp: &Ramp) -> String {
    let mut body = String::new();
    if ramp.len() == 1 {
        body += &format!("return {:.32};", ramp[0].1);
    } else {
        let mut last_t = 0.0;
        let mut last_f = 0.0;
        for (i, (t, f)) in ramp.iter().enumerate() {
            if i == 0 {
                body +=
                    &format!("if (temperature <= {t:.32}) {{\n\t\t\t\treturn {f:.32};\n\t\t\t}}");
            } else {
                body += &format!(
                    " else if (temperature <= {t:.32}) {{\n\t\t\t\treturn {last_f:.32} + ({f:.32} - {last_f:.32}) / ({t:.32} - {last_t:.32}) * (temperature - {last_t:.32});\n\t\t\t}}"
                );
            }
            last_t = *t;
            last_f = *f;
        }
        body += &format!(" else {{\n\t\t\t\treturn {last_f:.32};\n\t\t\t}}");
    }
    body
}

/// Insert the material data inside a shader.
pub fn insert_material_data(shader: &str, materials: &[Material]) -> String {
    let mut specific_heat_body = String::from("switch id {\n");
    let mut conductivity_body = String::from("switch id {\n");
    let mut density_body = String::from("switch id {\n");
    let mut emissivity_body = String::from("switch id {\n");

    for (id, material) in materials.iter().enumerate() {
        let Material {
            specific_heat,
            conductivity,
            density,
            emissivity,
        } = material;
        if id == 0 {
            specific_heat_body += &format!(
                "\t\tdefault: {{\n\t\t\t{}\n\t\t}}",
                ramp_to_body(specific_heat)
            );
            conductivity_body += &format!(
                "\t\tdefault: {{\n\t\t\t{}\n\t\t}}",
                ramp_to_body(conductivity)
            );
            density_body += &format!("\t\tdefault: {{\n\t\t\treturn {density:.32};\n\t\t}}");
            emissivity_body += &format!("\t\tdefault: {{\n\t\t\treturn {emissivity:.32};\n\t\t}}");
        } else {
            specific_heat_body += &format!(
                "\t\tcase {id}u: {{\n\t\t\t{}\n\t\t}}",
                ramp_to_body(specific_heat)
            );
            conductivity_body += &format!(
                "\t\tcase {id}u: {{\n\t\t\t{}\n\t\t}}",
                ramp_to_body(conductivity)
            );
            density_body += &format!("\t\tcase {id}u: {{\n\t\t\treturn {density:.32};\n\t\t}}");
            emissivity_body +=
                &format!("\t\tcase {id}u: {{\n\t\t\treturn {emissivity:.32};\n\t\t}}");
        }
    }
    specific_heat_body += "\n\t}";
    conductivity_body += "\n\t}";
    density_body += "\n\t}";
    emissivity_body += "\n\t}";
    shader
        .replace("//! specific_heat", &specific_heat_body)
        .replace("//! conductivity", &conductivity_body)
        .replace("//! density", &density_body)
        .replace("//! emissivity", &emissivity_body)
}

/// Insert the additional GPU M2 data to a shader.
pub fn insert_gpu_m2_data(shader: &str, sizes: &[f32], materials: &[u32]) -> String {
    let cell_length = sizes.len();

    let cell_length = format!(
        "const CELL_LENGTH = {}u;\n const N = {}u; //",
        cell_length,
        cell_length - 2
    );
    let sizes = format!(
        "var<private> cell_sizes: array<f32, CELL_LENGTH> = array<f32, CELL_LENGTH>({}); //",
        sizes.iter().fold(String::new(), |mut output, s| {
            let _ = write!(output, "{s:.32}, ");
            output
        })
    );
    let material_ids = format!(
        "var<private> cell_materials: array<u32, CELL_LENGTH> = array<u32, CELL_LENGTH>({}); //",
        materials.iter().fold(String::new(), |mut output, m| {
            let _ = write!(output, "{m}u, ");
            output
        })
    );

    shader
        .replace("//! cell_length\n", &cell_length)
        .replace("//! cell_sizes\n", &sizes)
        .replace("//! cell_materials\n", &material_ids)
}

/// Insert the additional GPU M3 data to a shader.
pub fn insert_gpu_m3_data(shader: &str, max_cell_count: usize) -> String {
    let max_cell_count = format!(
        "const MAX_CELL_COUNT = {}u;\n const N = {}u; //",
        max_cell_count,
        max_cell_count - 2
    );
    shader.replace("//! max_cell_count\n", &max_cell_count)
}
