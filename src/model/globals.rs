use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};

#[derive(Debug, Clone, Default)]
pub struct ClassStatusCodes {
    pub upcoming: Option<i64>,
    pub cancelled: Option<i64>,
    pub conducted: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Globals {
    pub class_types: HashMap<i64, String>,
    pub class_status: ClassStatusCodes,
    pub course_year: Option<String>,
    pub absent_threshold: Option<u32>,
    pub ssid_filters: Vec<String>,
}

/// A fresh read of the backend business clock; never cached by the client.
#[derive(Debug, Clone, Copy)]
pub struct ServerTime {
    pub date: NaiveDate,
    pub time: NaiveTime,
}

impl Globals {
    pub fn class_type_name(&self, class_type: i64) -> Option<&str> {
        self.class_types.get(&class_type).map(String::as_str)
    }
}
