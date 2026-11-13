//! 独立课表：按学期、周次、星期和节假日，把课程排到某一天。
//!
//! JSON 格式见 `plugins/desktop_tools/README.md`；这里只做纯计算，方便单测。
use chrono::{Datelike, NaiveDate};
use serde_json::Value;

/// 某天的课程。每一项是「08:10 · 1-2节」再加课程、地点、老师。
pub(crate) fn course_day(data: &Value, day: NaiveDate) -> Vec<String> {
    let Some(term_start) = data["term_start"].as_str().and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()) else {
        return Vec::new();
    };
    let key = day.to_string();
    if data["holidays"].as_array().is_some_and(|days| days.iter().any(|value| value.as_str() == Some(&key))) {
        return Vec::new();
    }
    let days = (day - term_start).num_days();
    if days < 0 || data["term_end"].as_str().is_some_and(|end| key.as_str() > end) {
        return Vec::new();
    }
    let week = days / 7 + 1;
    data["rules"].as_array().map(|rules| {
        rules.iter()
            .filter(|rule| rule["weekday"].as_u64() == Some(day.weekday().number_from_monday() as u64)
                && rule["weeks"].as_array().is_some_and(|weeks| weeks.iter().any(|value| value.as_i64() == Some(week))))
            .map(|rule| format!(
                "{} · {}节\n{}\n{}\n{}",
                rule["start"].as_str().unwrap_or(""),
                rule["periods"].as_str().unwrap_or(""),
                rule["course"].as_str().unwrap_or(""),
                rule["location"].as_str().unwrap_or(""),
                rule["teacher"].as_str().unwrap_or(""),
            ))
            .collect()
    }).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rules_respect_weeks_and_holidays() {
        let data = json!({
            "term_start": "2026-09-07",
            "term_end": "2027-01-17",
            "holidays": ["2026-09-14"],
            "rules": [{ "weekday": 1, "weeks": [1, 2], "course": "课程", "start": "08:10", "periods": "1-2节" }],
        });
        assert_eq!(course_day(&data, NaiveDate::from_ymd_opt(2026, 9, 7).unwrap()).len(), 1);
        assert!(course_day(&data, NaiveDate::from_ymd_opt(2026, 9, 14).unwrap()).is_empty());
        assert!(course_day(&data, NaiveDate::from_ymd_opt(2026, 9, 21).unwrap()).is_empty());
    }

    #[test]
    fn a_day_outside_the_term_has_no_courses() {
        let data = json!({ "term_start": "2026-09-07", "term_end": "2027-01-17", "rules": [] });
        assert!(course_day(&data, NaiveDate::from_ymd_opt(2026, 8, 31).unwrap()).is_empty(), "开学前");
        assert!(course_day(&data, NaiveDate::from_ymd_opt(2027, 2, 1).unwrap()).is_empty(), "学期结束后");
        assert!(course_day(&Value::Null, NaiveDate::from_ymd_opt(2026, 9, 7).unwrap()).is_empty());
    }
}
