use rayon::prelude::*;

use super::{HeatTransfer1D, WallElement};
use crate::fds::Material;

/// The maximum temperature difference that may be between neighboring cells before the time step is reduced.
pub const MAX_DELTA_TEMPERATURE: f32 = 10.0;
/// The maximum number of times the time step may be reduced.
pub const MAX_TIME_SUBDIVISIONS: usize = 4;
///  Stefan Boltzmann constant
pub const SIGMA: f32 = 0.0000000567;

/// Value for `h_f` and / o`h_b`_b to indicate that the wall side has adiabatic properties.
pub const ADIABATIC_H: f32 = -100000.0;
/// Value for `h_f` and / or `h_b` to indicate that the wall side has a constant temperature. Note `q_f` and / or `q_b` has to be set to the constant temperature.
pub const CONST_TEMP_H: f32 = -100001.0;

/// All relevant data for the heat transfer algorithm on the CPU.
pub struct CPUSetupData {
    materials: Vec<Material>,
    wall_elements: Vec<WallElement>,
}

impl HeatTransfer1D for CPUSetupData {
    fn setup(materials: Vec<Material>, wall_elements: Vec<WallElement>) -> anyhow::Result<Self> {
        Ok(Self {
            materials,
            wall_elements,
        })
    }

    fn update(
        &mut self,
        delta_time: f32,
        wall_heat_transfer_coefficients: &[[f32; 2]],
        wall_q_in: &[[f32; 2]],
        wall_temperature: &mut [[f32; 2]],
    ) -> anyhow::Result<()> {
        let materials = &self.materials;

        let mut_iter = self
            .wall_elements
            .par_iter_mut()
            .zip_eq(wall_temperature.par_iter_mut());
        let iter = wall_heat_transfer_coefficients
            .par_iter()
            .zip_eq(wall_q_in.par_iter());

        mut_iter.zip_eq(iter).for_each(
            |((wall_element, wall_temperature), (wall_heat_transfer_coefficient, wall_q_in))| {
                heat_transfer(
                    wall_element,
                    materials,
                    *wall_heat_transfer_coefficient,
                    *wall_q_in,
                    delta_time,
                );
                let len = wall_element.len();
                wall_temperature[0] =
                    (wall_element[0].temperature + wall_element[1].temperature) / 2.0;
                wall_temperature[1] =
                    (wall_element[len - 1].temperature + wall_element[len - 2].temperature) / 2.0;
            },
        );
        Ok(())
    }
}

/// Calculation of the highest temperature between two neighboring cells.
#[inline]
pub fn max_delta_temperature(
    wall_element: &WallElement,
    materials: &[Material],
    delta_time: f32,
) -> f32 {
    let len = wall_element.len();
    let mut delta_temperature = 0.0;

    let material_b = &materials[wall_element[0].material as usize];
    let t_b = wall_element[0].temperature;
    let x_b = wall_element[0].size;
    let k_b = material_b.conductivity.calc(t_b);

    let material_c = &materials[wall_element[1].material as usize];
    let t_c = wall_element[1].temperature;
    let mut x_c = wall_element[1].size;
    let k_c = material_c.conductivity.calc(t_c);
    let c_c = material_c.specific_heat.calc(t_c);
    let rho_c = material_c.density;

    let k_m_b = (k_c + k_b) / 2.0;
    let mut before = k_m_b * (t_c - t_b) / ((x_c + x_b) / 2.0);

    let mut f1 = delta_time * (rho_c * c_c);

    for i in 1..(len - 1) {
        let material_a = &materials[wall_element[i + 1].material as usize];
        let t_a = wall_element[i + 1].temperature;
        let x_a = wall_element[i + 1].size;
        let k_a = material_a.conductivity.calc(t_a);
        let c_a = material_a.specific_heat.calc(t_a);
        let rho_a = material_a.density;

        let k_m_a = (k_c + k_a) / 2.0;

        let after = k_m_a * (t_a - t_c) / ((x_a + x_c) / 2.0);

        delta_temperature = ((f1 * (after - before) / x_c).abs()).max(delta_temperature);

        x_c = x_a;
        before = after;
        f1 = delta_time * (rho_a * c_a);
    }

    delta_temperature
}

/// Calculations of repetitions / divisions due to large temperature difference between two cells.
#[inline]
pub fn repeats(max_delta_temperature: f32) -> usize {
    if max_delta_temperature < MAX_DELTA_TEMPERATURE {
        return 1;
    }
    let eta = max_delta_temperature / MAX_DELTA_TEMPERATURE;
    2_usize
        .pow((eta.ln() / 2.0f32.ln()).ceil() as u32)
        .clamp(1, MAX_TIME_SUBDIVISIONS)
}

/// Calculation of the gas interaction variables.
#[inline]
pub fn calc_rfac2_and_qdxk_no_radiation(
    wall_element: &WallElement,
    materials: &[Material],
    wall_heat_transfer_coefficient: [f32; 2],
    wall_q_in: [f32; 2],
) -> [f32; 4] {
    let h_f = wall_heat_transfer_coefficient[0];
    let (rfac2_f, qdxk_f) = if h_f == ADIABATIC_H {
        (1.0, 0.0)
    } else if h_f == CONST_TEMP_H {
        let q2_f = wall_q_in[0];
        (-1.0, 2.0 * q2_f)
    } else {
        let q2_f = wall_q_in[0];

        let temperature_f = (wall_element[0].temperature + wall_element[1].temperature) / 2.0;
        let material_f = &materials[wall_element[0].material as usize];
        let dx_f = wall_element[0].size;

        let emission_rfac_f = 2.0 * material_f.emissivity * SIGMA * temperature_f.powf(3.0);
        let emission_qdxk_f = 3.0 * material_f.emissivity * SIGMA * temperature_f.powf(4.0);

        let rfac_f = 0.5 * h_f + emission_rfac_f;
        let k_f = material_f.conductivity.calc(temperature_f);
        let rfac2_f = (k_f / dx_f - rfac_f) / (k_f / dx_f + rfac_f);
        let qdxk_f = (q2_f + emission_qdxk_f) / (k_f / dx_f + rfac_f);
        (rfac2_f, qdxk_f)
    };

    let h_b = wall_heat_transfer_coefficient[1];
    let (rfac2_b, qdxk_b) = if h_b == ADIABATIC_H {
        (1.0, 0.0)
    } else if h_b == CONST_TEMP_H {
        let q2_b = wall_q_in[1];
        (-1.0, 2.0 * q2_b)
    } else {
        let q2_b = wall_q_in[1];

        let len = wall_element.len();
        let temperature_b =
            (wall_element[len - 1].temperature + wall_element[len - 2].temperature) / 2.0;
        let material_b = &materials[wall_element[len - 1].material as usize];
        let dx_b = wall_element[len - 1].size;

        let emission_rfac_b = 2.0 * material_b.emissivity * SIGMA * temperature_b.powf(3.0);
        let emission_qdxk_b = 3.0 * material_b.emissivity * SIGMA * temperature_b.powf(4.0);

        let rfac_b = 0.5 * h_b + emission_rfac_b;
        let k_b = material_b.conductivity.calc(temperature_b);
        let rfac2_b = (k_b / dx_b - rfac_b) / (k_b / dx_b + rfac_b);
        let qdxk_b = (q2_b + emission_qdxk_b) / (k_b / dx_b + rfac_b);

        (rfac2_b, qdxk_b)
    };

    [rfac2_f, qdxk_f, rfac2_b, qdxk_b]
}

/// Filling the solution matrix.
#[inline]
pub fn populate_solve_matrix(
    wall_element: &WallElement,
    materials: &[Material],
    delta_time: f32,
) -> Vec<[f32; 4]> {
    let mut matrix = Vec::with_capacity(wall_element.len() - 2);

    let mut temperature_d = wall_element[1].temperature;
    let mut material_d = &materials[wall_element[1].material as usize];
    let mut dx_d = wall_element[1].size;

    let mut f1 = 2.0 * material_d.density * material_d.specific_heat.calc(temperature_d);

    // B
    let temperature_b = wall_element[0].temperature;
    let material_b = &materials[wall_element[0].material as usize];
    let dx_b = wall_element[0].size;

    let k_b = (material_d.conductivity.calc(temperature_d)
        + material_b.conductivity.calc(temperature_b))
        / 2.0;
    let mut b = -delta_time * k_b / (f1 * dx_d * (dx_d + dx_b) / 2.0);
    let mut c_b = b * (temperature_d - temperature_b);

    for i in 1..(wall_element.len() - 1) {
        // A
        let temperature_a = wall_element[i + 1].temperature;
        let material_a = &materials[wall_element[i + 1].material as usize];
        let dx_a = wall_element[i + 1].size;

        let k_a = (material_d.conductivity.calc(temperature_d)
            + material_a.conductivity.calc(temperature_a))
            / 2.0;
        let a = -delta_time * k_a / (f1 * dx_d * (dx_d + dx_a) / 2.0);
        let c_a = a * (temperature_a - temperature_d);

        // D
        let d = 1.0 - a - b;

        // C
        let c = temperature_d - c_a + c_b;
        matrix.push([b, d, a, c]);

        f1 = 2.0 * material_a.density * material_a.specific_heat.calc(temperature_a);
        let k_b = (material_a.conductivity.calc(temperature_a)
            + material_a.conductivity.calc(temperature_d))
            / 2.0;
        b = -delta_time * k_b / (f1 * dx_a * (dx_a + dx_d) / 2.0);
        c_b = b * (temperature_a - temperature_d);

        temperature_d = temperature_a;
        material_d = material_a;
        dx_d = dx_a;
    }

    matrix
}

/// Solving the solution matrix with the Thomas algorithm.
#[inline]
pub fn solve_heat_transfer(
    wall_element: &mut WallElement,
    materials: &[Material],
    rfac2_qdxk: [f32; 4],
    delta_time: f32,
) {
    let len = wall_element.len();
    let n = len - 2;
    let mut matrix = populate_solve_matrix(wall_element, materials, delta_time);

    let [rfac2_f, qdxk_f, rfac2_b, qdxk_b] = rfac2_qdxk;

    // 0: b, 1: d, 2: a, 3: c
    matrix[0][3] -= matrix[0][0] * qdxk_f;
    matrix[n - 1][3] -= matrix[n - 1][2] * qdxk_b;

    matrix[0][1] += matrix[0][0] * rfac2_f;
    matrix[n - 1][1] += matrix[n - 1][2] * rfac2_b;

    for i in 1..n {
        let r = matrix[i][0] / matrix[i - 1][1];
        matrix[i][1] -= r * matrix[i - 1][2];
        matrix[i][3] -= r * matrix[i - 1][3];
    }

    matrix[n - 1][3] /= matrix[n - 1][1];
    for i in (0..(n - 1)).rev() {
        matrix[i][3] = (matrix[i][3] - matrix[i][2] * matrix[i + 1][3]) / matrix[i][1]
    }

    for i in 0..n {
        wall_element[i + 1].temperature = matrix[i][3];
    }
    wall_element[0].temperature = wall_element[1].temperature * rfac2_f + qdxk_f;
    wall_element[len - 1].temperature = wall_element[len - 2].temperature * rfac2_b + qdxk_b;
}

//Calculation of the total heat transfer with reduction of the time step if necessary.
#[inline]
pub fn heat_transfer(
    wall_element: &mut WallElement,
    materials: &[Material],
    wall_heat_transfer_coefficient: [f32; 2],
    wall_q_in: [f32; 2],
    delta_time: f32,
) {
    let max_delta_temperature = max_delta_temperature(wall_element, materials, delta_time);
    let repeats = repeats(max_delta_temperature);

    let new_delta_time = delta_time / (repeats as f32);
    for _ in 0..repeats {
        let rfac2_qdxk = calc_rfac2_and_qdxk_no_radiation(
            wall_element,
            materials,
            wall_heat_transfer_coefficient,
            wall_q_in,
        );
        solve_heat_transfer(wall_element, materials, rfac2_qdxk, new_delta_time);
    }
}
