use super::ModuleKey;

#[derive(Debug, Clone)]
pub struct ModuleAttendance {
    pub module: ModuleKey,
    pub title: String,
    pub attended: u32,
    pub voided: u32,
    pub pending_voids: u32,
    pub conducted: u32,
    pub class_limit: Option<u32>,
}

impl ModuleAttendance {
    /// Adjustments are applied before flooring at zero and applying the class limit.
    pub fn denominator(&self) -> u32 {
        let adjusted = (i64::from(self.conducted) - i64::from(self.voided)
            + i64::from(self.pending_voids))
        .clamp(0, i64::from(u32::MAX)) as u32;
        self.class_limit
            .filter(|limit| *limit > 0 && *limit < adjusted)
            .unwrap_or(adjusted)
    }

    /// Statistical difference, not a count of absent class records.
    pub fn unattended(&self) -> u32 {
        self.denominator().saturating_sub(self.attended)
    }

    pub fn rate(&self) -> f32 {
        let denominator = self.denominator();
        if denominator == 0 {
            0.0
        } else {
            ((self.attended as f32 * 100.0) / denominator as f32).min(100.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttendanceSummary {
    pub modules: Vec<ModuleAttendance>,
    pub attended: u32,
    /// Sum of adjusted denominators, not raw conducted class counts.
    pub conducted: u32,
    /// Sum of per-module statistical differences.
    pub unattended: u32,
    pub rate: f32,
}

impl AttendanceSummary {
    pub(crate) fn from_modules(modules: Vec<ModuleAttendance>) -> Self {
        let attended = modules.iter().map(|module| module.attended).sum();
        let conducted = modules.iter().map(ModuleAttendance::denominator).sum();
        let unattended = modules.iter().map(ModuleAttendance::unattended).sum();
        let rate = if conducted == 0 {
            0.0
        } else {
            ((attended as f32 * 100.0) / conducted as f32).min(100.0)
        };
        Self {
            modules,
            attended,
            conducted,
            unattended,
            rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjustments_are_applied_before_flooring() {
        let mut module = ModuleAttendance {
            module: ModuleKey {
                id: "X".into(),
                course_type: "A".into(),
                academic_year: "26-27".into(),
            },
            title: String::new(),
            attended: 0,
            conducted: 2,
            voided: 5,
            pending_voids: 2,
            class_limit: None,
        };
        assert_eq!(module.denominator(), 0);
        module.pending_voids = 7;
        assert_eq!(module.denominator(), 4);
        module.class_limit = Some(3);
        module.attended = 1;
        assert_eq!(module.unattended(), 2);
    }
}
