use chrono::{Datelike, NaiveDate, NaiveTime};

use super::Globals;
use crate::api::firestore::Document;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleKey {
    pub id: String,
    pub course_type: String,
    pub academic_year: String,
}

impl ModuleKey {
    pub(crate) fn parse(raw: &str) -> Result<Self> {
        let mut parts = raw.split('_');
        let id = parts.next().filter(|value| !value.is_empty());
        let course_type = parts.next().filter(|value| !value.is_empty());
        let academic_year = parts.next().filter(|value| !value.is_empty());

        if parts.next().is_some()
            || id.is_none()
            || course_type.is_none()
            || academic_year.is_none()
        {
            return Err(Error::InvalidField {
                field: "moduleKey",
                message: "expected id_courseType_academicYear",
            });
        }

        Ok(Self {
            id: id.unwrap().to_owned(),
            course_type: course_type.unwrap().to_owned(),
            academic_year: academic_year.unwrap().to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attendance {
    Void,
    Absent,
    Present,
    Ecf,
    Unknown(i64),
}

impl Attendance {
    pub(crate) fn from_raw(value: i64) -> Self {
        match value {
            -1 => Self::Void,
            0 => Self::Absent,
            1 => Self::Present,
            2 => Self::Ecf,
            other => Self::Unknown(other),
        }
    }

    pub fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }

    pub fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassStatus {
    Upcoming,
    Conducted,
    Cancelled,
    Unknown(i64),
}

#[derive(Debug, Clone)]
pub struct Class {
    pub id: String,
    pub module: ModuleKey,
    pub title: Option<String>,
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub venue: String,
    pub class_type: Option<i64>,
    pub status_code: Option<i64>,
    pub group: Option<i64>,
    pub link_id: Option<String>,
    pub attendance: Attendance,
}

impl Class {
    pub(crate) fn from_document(document: &Document) -> Result<Self> {
        Ok(Self {
            id: document.id().to_owned(),
            module: ModuleKey::parse(required_str(document, "moduleKey")?.as_str())?,
            title: document.get_str("moduleName").map(str::to_owned),
            date: date_from_wire(required_i64(document, "classDate")?)?,
            start: time_from_wire(required_i64(document, "startTime")?)?,
            end: time_from_wire(required_i64(document, "endTime")?)?,
            venue: required_str(document, "venue")?,
            class_type: document.get_i64("classType"),
            status_code: document.get_i64("classStatus"),
            group: document.get_i64("classGroup"),
            link_id: document.get_str("classLinkID").map(str::to_owned),
            attendance: Attendance::from_raw(required_i64(document, "attended")?),
        })
    }

    pub fn status(&self, globals: &Globals) -> ClassStatus {
        match self.status_code {
            Some(value) if Some(value) == globals.class_status.cancelled => ClassStatus::Cancelled,
            Some(value) if Some(value) == globals.class_status.conducted => ClassStatus::Conducted,
            Some(value) if Some(value) == globals.class_status.upcoming => ClassStatus::Upcoming,
            Some(value) => ClassStatus::Unknown(value),
            None => ClassStatus::Unknown(-1),
        }
    }
}

pub(crate) fn date_from_wire(value: i64) -> Result<NaiveDate> {
    let value: i32 = value.try_into().map_err(|_| Error::InvalidField {
        field: "classDate",
        message: "does not fit in i32",
    })?;

    NaiveDate::from_ymd_opt(
        value / 10_000,
        ((value / 100) % 100) as u32,
        (value % 100) as u32,
    )
    .ok_or(Error::InvalidField {
        field: "classDate",
        message: "invalid yyyyMMdd date",
    })
}

pub(crate) fn date_to_wire(date: NaiveDate) -> i32 {
    date.year() * 10_000 + date.month() as i32 * 100 + date.day() as i32
}

pub(crate) fn time_from_wire(value: i64) -> Result<NaiveTime> {
    let value: i32 = value.try_into().map_err(|_| Error::InvalidField {
        field: "time",
        message: "does not fit in i32",
    })?;

    if !(0..=2359).contains(&value) {
        return Err(Error::InvalidField {
            field: "time",
            message: "invalid HHmm time",
        });
    }

    NaiveTime::from_hms_opt((value / 100) as u32, (value % 100) as u32, 0).ok_or(
        Error::InvalidField {
            field: "time",
            message: "invalid HHmm time",
        },
    )
}

fn required_str(document: &Document, field: &'static str) -> Result<String> {
    document
        .get_str(field)
        .map(str::to_owned)
        .ok_or(Error::MissingField(field))
}

fn required_i64(document: &Document, field: &'static str) -> Result<i64> {
    document.get_i64(field).ok_or(Error::MissingField(field))
}
