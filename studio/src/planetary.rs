//! Validated, project-local planetary calendar and circular-orbit sunlight.
//! These are user-authored assumptions and top-of-atmosphere geometry; they
//! do not calculate air temperature, clouds, weather, or precipitation.
use crate::world::Grid;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const INSOLATION_ID: &str = "planet.monthly_toa_insolation";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanetCalendar {
    /// Length of the mean solar day, not a sidereal rotation period.
    pub solar_day_hours: f64,
    /// Twelve user-defined periods measured in the world's solar days.
    pub month_days: [f64; 12],
    pub axial_tilt_deg: f64,
    /// Solar longitude at the beginning of month 1. Zero = northern spring equinox.
    pub start_solar_longitude_deg: f64,
    /// Assumed irradiance at the top of the atmosphere for a circular orbit.
    pub stellar_flux_w_m2: f64,
}

impl Default for PlanetCalendar {
    fn default() -> Self {
        Self {
            solar_day_hours: 24.0,
            month_days: [30.0; 12],
            axial_tilt_deg: 23.44,
            start_solar_longitude_deg: -90.0,
            stellar_flux_w_m2: 1361.0,
        }
    }
}

impl PlanetCalendar {
    pub fn year_days(&self) -> f64 {
        self.month_days.iter().sum()
    }

    pub fn year_hours(&self) -> f64 {
        self.year_days() * self.solar_day_hours
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.solar_day_hours.is_finite() || !(0.1..=10_000.0).contains(&self.solar_day_hours) {
            return Err("Mean solar day must be 0.1–10,000 hours".into());
        }
        if self
            .month_days
            .iter()
            .any(|&days| !days.is_finite() || !(0.01..=10_000.0).contains(&days))
            || self.year_days() > 100_000.0
        {
            return Err("Each of the 12 months needs a finite positive duration; year <= 100,000 solar days".into());
        }
        if !self.axial_tilt_deg.is_finite() || !(0.0..=90.0).contains(&self.axial_tilt_deg) {
            return Err("Axial tilt must be between 0 and 90 degrees".into());
        }
        if !self.start_solar_longitude_deg.is_finite()
            || !(-360.0..=360.0).contains(&self.start_solar_longitude_deg)
        {
            return Err("Solar longitude at year start must be within -360° to +360°".into());
        }
        if !self.stellar_flux_w_m2.is_finite()
            || !(0.0..=100_000.0).contains(&self.stellar_flux_w_m2)
        {
            return Err("Incident stellar flux must be between 0 and 100,000 W/m²".into());
        }
        Ok(())
    }

    pub fn month_window(&self, month: u8) -> Result<(f64, f64), String> {
        if !(1..=12).contains(&month) {
            return Err("Month must be in 1..=12".into());
        }
        let start = self.month_days[..(month as usize - 1)].iter().sum();
        Ok((start, self.month_days[month as usize - 1]))
    }

    pub fn declination_rad(&self, year_day: f64) -> f64 {
        let solar_longitude =
            self.start_solar_longitude_deg.to_radians() + TAU * year_day / self.year_days();
        (self.axial_tilt_deg.to_radians().sin() * solar_longitude.sin()).asin()
    }

    fn daily_mean_toa_w_m2(&self, latitude_rad: f64, declination_rad: f64) -> f64 {
        let horizon = -latitude_rad.tan() * declination_rad.tan();
        let hour_angle = if horizon <= -1.0 {
            PI // continuous daylight
        } else if horizon >= 1.0 {
            0.0 // polar night
        } else {
            horizon.acos()
        };
        let irradiance = self.stellar_flux_w_m2 / PI
            * (hour_angle * latitude_rad.sin() * declination_rad.sin()
                + latitude_rad.cos() * declination_rad.cos() * hour_angle.sin());
        irradiance.max(0.0)
    }

    /// Calendar-month mean of daily-mean TOA irradiance in W/m², evaluated
    /// with sixteen midpoint quadrature samples for the assumed circular orbit.
    /// Depends only on latitude. No terrain, atmospheric, cloud or ocean model.
    pub fn monthly_insolation(
        &self,
        width: usize,
        height: usize,
        month: u8,
    ) -> Result<Grid, String> {
        self.validate()?;
        if width == 0 || height == 0 {
            return Err("Solar map needs nonzero raster dimensions".into());
        }
        let (start, duration) = self.month_window(month)?;
        const STEPS: usize = 16;
        let declinations: [f64; STEPS] = std::array::from_fn(|i| {
            self.declination_rad(start + duration * (i as f64 + 0.5) / STEPS as f64)
        });
        let mut map = Grid::new(width, height, 0.0);
        for y in 0..height {
            let latitude = -FRAC_PI_2 + PI * (y as f64 + 0.5) / height as f64;
            let watts = declinations
                .iter()
                .map(|&declination| self.daily_mean_toa_w_m2(latitude, declination))
                .sum::<f64>()
                / STEPS as f64;
            for value in &mut map.cells[y * width..(y + 1) * width] {
                *value = watts as f32;
            }
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_is_explicit_and_accepts_variable_length_months() {
        let mut calendar = PlanetCalendar::default();
        assert_eq!(calendar.year_days(), 360.0);
        assert_eq!(calendar.year_hours(), 8640.0);
        assert_eq!(calendar.month_window(7).unwrap(), (180.0, 30.0));
        calendar.month_days[1] = 48.0;
        assert_eq!(calendar.month_window(3).unwrap(), (78.0, 30.0));
        assert_eq!(calendar.year_days(), 378.0);
        assert!(calendar.month_window(0).is_err());
        assert!(calendar.month_window(13).is_err());
    }

    #[test]
    fn rejects_non_finite_nonphysical_and_overlong_calendar_values() {
        let default = PlanetCalendar::default();
        for (field, value) in [
            ("day", -1.0),
            ("day", f64::NAN),
            ("tilt", 91.0),
            ("phase", f64::INFINITY),
            ("flux", -1.0),
            ("month", 0.0),
        ] {
            let mut bad = default.clone();
            match field {
                "day" => bad.solar_day_hours = value,
                "tilt" => bad.axial_tilt_deg = value,
                "phase" => bad.start_solar_longitude_deg = value,
                "flux" => bad.stellar_flux_w_m2 = value,
                _ => bad.month_days[2] = value,
            }
            assert!(bad.validate().is_err(), "{field} should fail");
        }
    }

    #[test]
    fn solar_geometry_reverses_hemispheres_and_is_longitude_independent() {
        let calendar = PlanetCalendar::default();
        let winter = calendar.monthly_insolation(24, 120, 1).unwrap();
        let summer = calendar.monthly_insolation(24, 120, 7).unwrap();
        assert!(winter.at(0, 100) < summer.at(0, 100));
        assert!(winter.at(0, 20) > summer.at(0, 20));
        assert_eq!(winter.at(0, 60), winter.at(23, 60));
        assert!(winter.cells.iter().all(|v| v.is_finite() && *v >= 0.0));
        assert!((calendar.declination_rad(90.0)).abs() < 1e-12);
        assert!((calendar.declination_rad(180.0).to_degrees() - 23.44).abs() < 1e-10);
    }

    #[test]
    fn zero_tilt_yields_same_insolation_every_month_and_polar_night() {
        let mut calendar = PlanetCalendar {
            axial_tilt_deg: 0.0,
            ..PlanetCalendar::default()
        };
        let january = calendar.monthly_insolation(4, 100, 1).unwrap();
        let july = calendar.monthly_insolation(4, 100, 7).unwrap();
        assert_eq!(january.cells, july.cells);
        assert!(january.at(0, 0) < january.at(0, 50));
        calendar.stellar_flux_w_m2 = 0.0;
        assert!(calendar
            .monthly_insolation(4, 100, 3)
            .unwrap()
            .cells
            .iter()
            .all(|&v| v == 0.0));
    }
}
