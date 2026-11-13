//! 个人 CalDAV 日历的写入：带版本检查的新建、修改和删除。
//!
//! 上游日历只读；这里沿用它的发现、认证和解析代码，只补上 `PUT` / `DELETE`。
//! 凭据只发给配置里的同一个来源，条件写入用 `If-Match` / `If-None-Match` 防止覆盖
//! 其他设备刚做的修改。
use crate::calendar::{self, Config, Provider};
use crate::mail::Meeting;
use chrono::NaiveDate;

/// 一条可以编辑的日历对象；`readonly` 表示只展示（重复日程或缺少版本信息）。
#[derive(Clone)]
pub(crate) struct Item {
    pub config: Config,
    pub event: Meeting,
    pub href: String,
    pub etag: String,
    pub raw: String,
    pub readonly: bool,
}

/// 某一天的所有个人日历日程；跨多个日历时按开始时间排序。
pub(crate) fn list(config: &Config, password: &str, day: NaiveDate) -> Result<Vec<Item>, String> {
    let agent = calendar::agent();
    let (collections, _) = calendar::discover_calendars(&agent, config, password).map_err(|error| error.message)?;
    let (lo, hi) = day_window(day);
    let mut items = Vec::new();
    for href in collections {
        items.extend(list_collection(&agent, config, password, &href, lo, hi)?);
    }
    items.sort_by_key(|item| item.event.start);
    Ok(items)
}

fn list_collection(
    agent: &ureq::Agent,
    config: &Config,
    password: &str,
    href: &str,
    lo: chrono::DateTime<chrono::Utc>,
    hi: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Item>, String> {
    let body = calendar::report_body(config.provider, &stamp(lo), &stamp(hi));
    // 原始对象要留下来做冲突检测和保留未修改属性，所以这里不能用展开后的数据。
    let body = regex::Regex::new(r"<c:calendar-data>.*?</c:calendar-data>")
        .map_err(|error| error.to_string())?
        .replace(&body, "<c:calendar-data/>");
    let (xml, base) = calendar::dav_response(agent, "REPORT", href, "1", &body, config, password)
        .map_err(|error| error.message)?;
    if let Some(status) = calendar::report_status_error(&xml) {
        return Err(human_error(config.provider, status));
    }
    parse_items(config, &base, &xml, lo, hi)
}

fn parse_items(
    config: &Config,
    base: &str,
    xml: &str,
    lo: chrono::DateTime<chrono::Utc>,
    hi: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Item>, String> {
    let block = regex::Regex::new(r"(?is)<(?:[a-z0-9]+:)?response\b[^>]*>(.*?)</(?:[a-z0-9]+:)?response>")
        .map_err(|error| error.to_string())?;
    let mut items = Vec::new();
    for capture in block.captures_iter(xml) {
        let block = &capture[1];
        let Some(raw) = calendar::calendar_data(block).into_iter().next() else { continue; };
        let href = property(block, "href").ok_or("服务器没有返回日程地址")?;
        let href = trusted_url(config, &calendar::join_url(base, &href))?;
        let etag = property(block, "getetag").unwrap_or_default();
        let readonly = is_recurring(&raw) || raw.matches("BEGIN:VEVENT").count() != 1 || etag.is_empty() || etag.starts_with("W/");
        for event in calendar::events_from_ics(&raw, lo, hi).map_err(|error| error)? {
            items.push(Item { config: config.clone(), event, href: href.clone(), etag: etag.clone(), raw: raw.clone(), readonly });
        }
    }
    Ok(items)
}

/// 新建（`old = None`）或修改、删除一条日程。删除时用原对象做条件检查。
pub(crate) fn save(config: &Config, password: &str, event: &Meeting, old: Option<&Item>, delete: bool) -> Result<(), String> {
    if old.is_some_and(|item| item.readonly) {
        return Err("这条日程只可展示，暂不支持编辑或删除".into());
    }
    let agent = calendar::agent();
    let href = match old {
        Some(item) => trusted_url(config, &item.href)?,
        None => {
            if delete { return Err("日程不存在".into()); }
            let collection = match config.href.clone() {
                Some(href) => href,
                None => calendar::discover_calendars(&agent, config, password)
                    .map_err(|error| error.message)?
                    .0
                    .into_iter()
                    .next()
                    .ok_or("没有找到可以写入的个人日历")?,
            };
            trusted_url(config, &calendar::join_url(&collection, &format!("karma-{}-{}.ics", std::process::id(), unique_suffix())))?
        }
    };
    let method = if delete { "DELETE" } else { "PUT" };
    let mut request = agent.request(method, &href).set("Authorization", &calendar::basic_auth(&config.username, password));
    match old {
        // 没有强 ETag 就不能做冲突检查，宁可让用户先刷新。
        Some(item) if item.etag.is_empty() || item.etag.starts_with("W/") => {
            return Err("服务器没有提供可用于冲突检查的版本，请刷新后再试".into());
        }
        Some(item) => request = request.set("If-Match", &item.etag),
        None => request = request.set("If-None-Match", "*"),
    }
    let result = if delete {
        request.call()
    } else {
        request.set("Content-Type", "text/calendar; charset=utf-8").send_string(&content(event, old.map(|item| item.raw.as_str()))?)
    };
    match result {
        Ok(response) if (200..300).contains(&response.status()) => Ok(()),
        Ok(response) => Err(human_error(config.provider, response.status())),
        Err(ureq::Error::Status(409 | 412, _)) => Err("日程已被其他设备修改或删除，请刷新后重新编辑".into()),
        Err(ureq::Error::Status(status, _)) => Err(human_error(config.provider, status)),
        Err(_) => Err("写入结果未确认，请刷新日历核对，避免重复创建".into()),
    }
}

/// 生成要写入的 iCalendar。修改时保留原来的地点、说明、提醒等未编辑字段。
pub(crate) fn content(event: &Meeting, original: Option<&str>) -> Result<String, String> {
    let end = event.end.ok_or("这条日程没有结束时间")?;
    if event.title.trim().is_empty() || event.title.chars().any(|character| character == '\0') || end <= event.start {
        return Err("标题或时间无效".into());
    }
    let utc = |timestamp: i64| {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|time| time.format("%Y%m%dT%H%M%SZ").to_string())
            .ok_or_else(|| "时间无效".to_string())
    };
    let date = |timestamp: i64| {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|time| time.with_timezone(&chrono::Local).format("%Y%m%d").to_string())
            .ok_or_else(|| "日期无效".to_string())
    };
    let times = if event.all_day {
        format!("DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}", date(event.start)?, date(end)?)
    } else {
        format!("DTSTART:{}\r\nDTEND:{}", utc(event.start)?, utc(end)?)
    };
    let now = utc(chrono::Utc::now().timestamp())?;
    let fields = format!("SUMMARY:{}\r\n{times}\r\nDTSTAMP:{now}\r\nLAST-MODIFIED:{now}\r\n", escape(&event.title));
    let Some(original) = original else {
        return Ok(format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//KarMa//Daozai//ZH\r\nBEGIN:VEVENT\r\nUID:{}\r\n{fields}END:VEVENT\r\nEND:VCALENDAR\r\n",
            escape(&event.uid)
        ));
    };
    if is_recurring(original) || original.matches("BEGIN:VEVENT").count() != 1 {
        return Err("重复日程暂不支持编辑或删除".into());
    }
    let unfolded = original.replace("\r\n", "\n").replace("\n ", "").replace("\n\t", "");
    let mut output = String::new();
    let mut depth = 0;
    for line in unfolded.lines() {
        if line == "BEGIN:VEVENT" {
            depth = 1;
            output.push_str(line);
            output.push_str("\r\n");
            output.push_str(&fields);
            continue;
        }
        if depth > 0 && line.starts_with("BEGIN:") { depth += 1; }
        let name = line.split([';', ':']).next().unwrap_or("");
        if depth == 1 && matches!(name, "SUMMARY" | "DTSTART" | "DTEND" | "DURATION" | "DTSTAMP" | "LAST-MODIFIED") {
            continue;
        }
        if depth == 1 && name == "SEQUENCE" {
            let next = line.split_once(':').and_then(|(_, value)| value.parse::<u64>().ok()).unwrap_or(0).saturating_add(1);
            output.push_str(&format!("SEQUENCE:{next}\r\n"));
            continue;
        }
        if line.starts_with("END:") && depth > 0 { depth -= 1; }
        output.push_str(line);
        output.push_str("\r\n");
    }
    Ok(output)
}

fn is_recurring(raw: &str) -> bool {
    raw.replace("\r\n", "\n").lines()
        .any(|line| matches!(line.split([';', ':']).next(), Some("RRULE" | "RDATE" | "RECURRENCE-ID")))
}

fn property(xml: &str, name: &str) -> Option<String> {
    let pattern = format!(r"(?is)<(?:[a-z0-9]+:)?{name}\b[^>]*>(.*?)</(?:[a-z0-9]+:)?{name}>");
    let capture = regex::Regex::new(&pattern).ok()?.captures(xml)?;
    Some(unescape_xml(&capture[1]))
}

fn unescape_xml(value: &str) -> String {
    value.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#13;", "\r").replace("&apos;", "'").replace("&amp;", "&")
}

/// 只接受配置里同一个来源的地址，避免把日历凭据发给别的服务器。
fn trusted_url(config: &Config, url: &str) -> Result<String, String> {
    let base = url::Url::parse(&config.base()).map_err(|_| "日历地址无效".to_string())?;
    let target = url::Url::parse(url).map_err(|_| "日程地址无效".to_string())?;
    if target.origin() != base.origin() || !target.username().is_empty() || target.password().is_some() || target.fragment().is_some() {
        return Err("拒绝向其他服务器发送日历凭据".into());
    }
    Ok(target.into())
}

/// 和核心一样的错误文案，界面不用区分来源。
fn human_error(provider: Provider, status: u16) -> String {
    calendar::human_error(provider, Some(status))
}

/// 当天 0 点到次日 0 点的 UTC 区间，和核心的查询窗口一致。
fn day_window(day: NaiveDate) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    let local = |date: NaiveDate| date.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(chrono::Local).earliest().unwrap();
    (local(day).with_timezone(&chrono::Utc), local(day.succ_opt().unwrap()).with_timezone(&chrono::Utc))
}

fn stamp(time: chrono::DateTime<chrono::Utc>) -> String {
    time.format("%Y%m%dT%H%M%SZ").to_string()
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or_default()
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\r', "").replace('\n', "\\n").replace(';', "\\;").replace(',', "\\,")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting() -> Meeting {
        Meeting {
            uid: "test".into(),
            title: "标题,一;二\n三".into(),
            start: 1_793_844_000,
            end: Some(1_793_847_600),
            all_day: false,
            at: String::new(),
        }
    }

    #[test]
    fn update_preserves_unedited_fields_and_nested_alarm() {
        let raw = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test\r\nSUMMARY:old\r\nDTSTART:20261105T100000Z\r\nDURATION:PT1H\r\nLOCATION:保留地点\r\nDESCRIPTION:保留说明\r\nSEQUENCE:4\r\nBEGIN:VALARM\r\nDTSTART:20261105T090000Z\r\nDESCRIPTION:提醒\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let output = content(&meeting(), Some(raw)).unwrap();
        assert!(output.contains("LOCATION:保留地点\r\nDESCRIPTION:保留说明"));
        assert!(output.contains("SEQUENCE:5"));
        assert!(output.contains("BEGIN:VALARM\r\nDTSTART:20261105T090000Z"));
        assert!(!output.contains("DURATION:"));
        assert!(output.contains("SUMMARY:标题\\,一\\;二\\n三"));
        assert!(content(&meeting(), Some(&raw.replace("SEQUENCE:4", "RRULE:FREQ=DAILY"))).is_err());
    }

    #[test]
    fn new_event_is_created_with_a_stable_uid() {
        let output = content(&meeting(), None).unwrap();
        assert!(output.starts_with("BEGIN:VCALENDAR"));
        assert!(output.contains("UID:test"));
        assert!(output.contains("PRODID:-//KarMa//Daozai//ZH"));
    }

    #[test]
    fn credentials_never_leave_the_configured_origin() {
        let config = Config { provider: Provider::Custom, username: "karma".into(), href: None, server: Some("https://calendar.example/dav/".into()) };
        assert!(trusted_url(&config, "https://calendar.example/dav/item.ics").is_ok());
        assert!(trusted_url(&config, "https://other.example/item.ics").is_err());
        assert!(trusted_url(&config, "https://calendar.example/dav/item.ics#x").is_err());
    }
}
