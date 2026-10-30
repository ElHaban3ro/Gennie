//! Calendario de ocurrencias, separado de la autorización y ejecución de efectos.
use crate::{Document, Frequency, Kind};
use gennie_core::Result;
use jiff::{
    Span, Timestamp,
    tz::{AmbiguousOffset, Disambiguation, TimeZone},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Delivery,
    Preparation,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepeatedTime {
    Earlier,
    Later,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissingTime {
    Skip,
    ShiftForward,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DstPolicy {
    pub repeated: RepeatedTime,
    pub missing: MissingTime,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Calendar {
    Once {
        at: i64,
    },
    Weekly {
        zone: String,
        local_time: String,
        weekdays: Vec<i8>,
        dst: DstPolicy,
    },
    Interval {
        anchor: i64,
        seconds: i64,
    },
}
impl Calendar {
    pub fn from_document(
        document: &Document,
        trigger: Trigger,
        dst: DstPolicy,
        now: i64,
    ) -> Result<Self> {
        Timestamp::from_second(now)?;
        if trigger == Trigger::Preparation {
            let minutes = document
                .preparation
                .interval_minutes
                .ok_or("El encargo no tiene una cadencia de preparación")?;
            if !(1..=525600).contains(&minutes) {
                return Err("Cadencia de preparación inválida".into());
            }
            return Ok(Self::Interval {
                anchor: now,
                seconds: i64::from(minutes) * 60,
            });
        }
        if document.kind == Kind::Recurring {
            let recurrence = document
                .recurrence
                .as_ref()
                .ok_or("Falta la recurrencia del encargo")?;
            let weekdays = match recurrence.frequency {
                Frequency::Daily => (1..=7).collect(),
                Frequency::Weekly => recurrence.weekdays.iter().map(|d| *d as i8).collect(),
            };
            let calendar = Self::Weekly {
                zone: document.timezone.clone(),
                local_time: recurrence.local_time.clone(),
                weekdays,
                dst,
            };
            calendar.validate()?;
            Ok(calendar)
        } else {
            let instant = document
                .notify_at
                .as_deref()
                .ok_or("El encargo necesita una fecha de entrega o cadencia de preparación")?
                .parse::<Timestamp>()?;
            Ok(Self::Once {
                at: instant.as_second(),
            })
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Once { at } => {
                Timestamp::from_second(*at)?;
            }
            Self::Interval { anchor, seconds } => {
                Timestamp::from_second(*anchor)?;
                if !(60..=525600 * 60).contains(seconds) {
                    return Err("Intervalo no válido".into());
                }
            }
            Self::Weekly {
                zone,
                local_time,
                weekdays,
                ..
            } => {
                TimeZone::get(zone)?;
                parse_time(local_time)?;
                if weekdays.is_empty()
                    || weekdays.len() > 7
                    || weekdays.iter().any(|d| !(1..=7).contains(d))
                    || weekdays.iter().collect::<BTreeSet<_>>().len() != weekdays.len()
                {
                    return Err("Días de recurrencia no válidos".into());
                }
            }
        }
        Ok(())
    }
    pub fn next_after(&self, after: i64) -> Result<Option<i64>> {
        self.bound(after, true)
    }
    pub fn latest_at(&self, now: i64) -> Result<Option<i64>> {
        self.bound(now, false)
    }
    fn bound(&self, instant: i64, next: bool) -> Result<Option<i64>> {
        self.validate()?;
        Timestamp::from_second(instant)?;
        Ok(match self {
            Self::Once { at } => {
                if (next && *at > instant) || (!next && *at <= instant) {
                    Some(*at)
                } else {
                    None
                }
            }
            Self::Interval { anchor, seconds } => {
                if instant < *anchor {
                    if next { Some(*anchor) } else { None }
                } else {
                    let steps = (instant - *anchor) / seconds + i64::from(next);
                    Some(
                        anchor
                            .checked_add(
                                steps
                                    .checked_mul(*seconds)
                                    .ok_or("Intervalo fuera de rango")?,
                            )
                            .ok_or("Fecha fuera de rango")?,
                    )
                }
            }
            Self::Weekly {
                zone,
                local_time,
                weekdays,
                dst,
            } => {
                let timezone = TimeZone::get(zone)?;
                let date = Timestamp::from_second(instant)?
                    .to_zoned(timezone.clone())
                    .date();
                let (hour, minute) = parse_time(local_time)?;
                let mut instants = BTreeSet::new();
                // Dos semanas cubren una ocurrencia semanal omitida por un salto civil.
                // Incluir días adyacentes también deduplica saltos de un día completo.
                for delta in -15..=15 {
                    let Ok(day) = date.checked_add(Span::new().days(delta)) else {
                        continue;
                    };
                    if !weekdays.contains(&day.weekday().to_monday_one_offset()) {
                        continue;
                    }
                    let ambiguous = timezone.to_ambiguous_zoned(day.at(hour, minute, 0, 0));
                    let strategy = match ambiguous.offset() {
                        AmbiguousOffset::Gap { .. } if dst.missing == MissingTime::Skip => continue,
                        AmbiguousOffset::Gap { .. } => Disambiguation::Compatible,
                        _ if dst.repeated == RepeatedTime::Later => Disambiguation::Later,
                        _ => Disambiguation::Earlier,
                    };
                    instants.insert(ambiguous.disambiguate(strategy)?.timestamp().as_second());
                }
                if next {
                    instants.into_iter().find(|t| *t > instant)
                } else {
                    instants.into_iter().rev().find(|t| *t <= instant)
                }
            }
        })
    }
}
fn parse_time(value: &str) -> Result<(i8, i8)> {
    let b = value.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![b[0], b[1], b[3], b[4]].iter().all(u8::is_ascii_digit) {
        return Err("Usa una hora HH:mm".into());
    }
    let hour = value[..2].parse::<i8>()?;
    let minute = value[3..].parse::<i8>()?;
    if hour > 23 || minute > 59 {
        return Err("Hora fuera de rango".into());
    }
    Ok((hour, minute))
}
pub fn utc(instant: i64) -> Result<String> {
    Ok(Timestamp::from_second(instant)?.to_string())
}
pub fn epoch(instant: &str) -> Result<i64> {
    Ok(instant.parse::<Timestamp>()?.as_second())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn calendar(zone: &str, time: &str) -> Calendar {
        Calendar::Weekly {
            zone: zone.into(),
            local_time: time.into(),
            weekdays: (1..=7).collect(),
            dst: DstPolicy {
                repeated: RepeatedTime::Earlier,
                missing: MissingTime::Skip,
            },
        }
    }
    fn next(c: &Calendar, s: &str) -> String {
        utc(c.next_after(epoch(s).unwrap()).unwrap().unwrap()).unwrap()
    }
    #[test]
    fn bogota_preserves_local_clock_across_month_and_year() {
        let c = calendar("America/Bogota", "08:00");
        assert_eq!(next(&c, "2026-12-31T13:00:00Z"), "2027-01-01T13:00:00Z");
        assert_eq!(next(&c, "2026-10-29T12:59:59Z"), "2026-10-29T13:00:00Z");
    }
    #[test]
    fn missing_and_repeated_local_times_follow_the_persisted_choice_once() {
        let c = calendar("America/New_York", "02:30");
        assert_eq!(next(&c, "2026-03-08T00:00:00Z"), "2026-03-09T06:30:00Z");
        let mut shifted = c.clone();
        if let Calendar::Weekly { dst, .. } = &mut shifted {
            dst.missing = MissingTime::ShiftForward;
        }
        assert_eq!(
            next(&shifted, "2026-03-08T00:00:00Z"),
            "2026-03-08T07:30:00Z"
        );
        let mut repeated = calendar("America/New_York", "01:30");
        assert_eq!(
            next(&repeated, "2026-11-01T00:00:00Z"),
            "2026-11-01T05:30:00Z"
        );
        assert_eq!(
            next(&repeated, "2026-11-01T05:30:00Z"),
            "2026-11-02T06:30:00Z"
        );
        if let Calendar::Weekly { dst, .. } = &mut repeated {
            dst.repeated = RepeatedTime::Later;
        }
        assert_eq!(
            next(&repeated, "2026-11-01T00:00:00Z"),
            "2026-11-01T06:30:00Z"
        );
    }
    #[test]
    fn weekly_skipped_day_and_interval_do_not_accumulate_occurrences() {
        let mut c = calendar("America/New_York", "02:30");
        if let Calendar::Weekly { weekdays, .. } = &mut c {
            *weekdays = vec![7];
        }
        assert_eq!(next(&c, "2026-03-07T00:00:00Z"), "2026-03-15T06:30:00Z");
        let interval = Calendar::Interval {
            anchor: 1000,
            seconds: 60,
        };
        assert_eq!(interval.next_after(1120).unwrap(), Some(1180));
        assert_eq!(interval.latest_at(1181).unwrap(), Some(1180));
        let apia = calendar("Pacific/Apia", "08:00");
        assert!(
            apia.next_after(epoch("2011-12-29T18:00:00Z").unwrap())
                .unwrap()
                .unwrap()
                > epoch("2011-12-29T18:00:00Z").unwrap()
        );
    }
}
