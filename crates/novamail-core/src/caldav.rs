//! Minimal CalDAV client: REPORT calendar-query + basic iCalendar parse.

use std::collections::HashMap;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::{CoreError, CoreResult};

#[derive(Debug, Clone)]
pub struct ParsedEvent {
    pub uid: String,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub all_day: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedTask {
    pub uid: String,
    pub title: String,
    pub due_at: Option<i64>,
    pub completed: bool,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct CalDavSyncResult {
    pub events: Vec<ParsedEvent>,
    pub tasks: Vec<ParsedTask>,
}

pub async fn sync_calendar(
    url: &str,
    username: &str,
    password: &str,
) -> CoreResult<CalDavSyncResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CoreError::Message(e.to_string()))?;

    let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT"/>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#;

    let event_xml = caldav_report(&client, url, username, password, body).await?;
    let mut events = Vec::new();
    for ical in extract_calendar_data(&event_xml) {
        events.extend(parse_vevents(&ical));
    }

    let task_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VTODO"/>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#;

    let task_xml = caldav_report(&client, url, username, password, task_body).await?;
    let mut tasks = Vec::new();
    for ical in extract_calendar_data(&task_xml) {
        tasks.extend(parse_vtodos(&ical));
    }

    Ok(CalDavSyncResult { events, tasks })
}

async fn caldav_report(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
    body: &str,
) -> CoreResult<String> {
    let response = client
        .request(reqwest::Method::from_bytes(b"REPORT").unwrap(), url)
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(
            AUTHORIZATION,
            basic_auth(username, password),
        )
        .header("Depth", "1")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| CoreError::Message(format!("CalDAV request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(CoreError::Message(format!(
            "CalDAV REPORT HTTP {}",
            response.status()
        )));
    }
    response
        .text()
        .await
        .map_err(|e| CoreError::Message(e.to_string()))
}

fn basic_auth(user: &str, pass: &str) -> String {
    use base64::Engine;
    let raw = format!("{user}:{pass}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
    )
}

fn extract_calendar_data(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = xml.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(start_rel) = lower[search_from..].find("calendar-data") {
        let start = search_from + start_rel;
        let after_tag = match lower[start..].find('>') {
            Some(i) => start + i + 1,
            None => break,
        };
        let end_marker = "</";
        let Some(end_rel) = lower[after_tag..].find(end_marker) else {
            break;
        };
        let end = after_tag + end_rel;
        let chunk = xml[after_tag..end]
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&#13;", "\n");
        if chunk.contains("BEGIN:VCALENDAR") || chunk.contains("BEGIN:VEVENT") {
            out.push(chunk);
        }
        search_from = end + 2;
    }
    out
}

fn parse_vevents(ical: &str) -> Vec<ParsedEvent> {
    let mut events = Vec::new();
    for block in split_components(ical, "VEVENT") {
        let props = parse_props(&block);
        let uid = props.get("UID").cloned().unwrap_or_else(|| {
            format!("local-{}", props.get("SUMMARY").cloned().unwrap_or_default())
        });
        let title = props
            .get("SUMMARY")
            .cloned()
            .unwrap_or_else(|| "(no title)".into());
        let (starts_at, all_day) = parse_ical_time(props.get("DTSTART").map(String::as_str));
        let ends_at = parse_ical_time(props.get("DTEND").map(String::as_str)).0;
        events.push(ParsedEvent {
            uid,
            title,
            starts_at,
            ends_at: if ends_at > 0 { Some(ends_at) } else { None },
            location: props.get("LOCATION").cloned(),
            description: props.get("DESCRIPTION").cloned(),
            all_day,
        });
    }
    events
}

fn parse_vtodos(ical: &str) -> Vec<ParsedTask> {
    let mut tasks = Vec::new();
    for block in split_components(ical, "VTODO") {
        let props = parse_props(&block);
        let uid = props.get("UID").cloned().unwrap_or_else(|| {
            format!("task-{}", props.get("SUMMARY").cloned().unwrap_or_default())
        });
        let title = props
            .get("SUMMARY")
            .cloned()
            .unwrap_or_else(|| "(no title)".into());
        let due_at = {
            let t = parse_ical_time(props.get("DUE").map(String::as_str)).0;
            if t > 0 {
                Some(t)
            } else {
                None
            }
        };
        let status = props
            .get("STATUS")
            .map(|s| s.to_ascii_uppercase())
            .unwrap_or_default();
        let completed = status == "COMPLETED" || props.contains_key("COMPLETED");
        tasks.push(ParsedTask {
            uid,
            title,
            due_at,
            completed,
            notes: props.get("DESCRIPTION").cloned().unwrap_or_default(),
        });
    }
    tasks
}

fn split_components(ical: &str, name: &str) -> Vec<String> {
    let begin = format!("BEGIN:{name}");
    let end = format!("END:{name}");
    let mut out = Vec::new();
    let mut rest = ical;
    while let Some(start) = rest.find(&begin) {
        let after = &rest[start + begin.len()..];
        if let Some(end_at) = after.find(&end) {
            out.push(after[..end_at].to_string());
            rest = &after[end_at + end.len()..];
        } else {
            break;
        }
    }
    out
}

fn parse_props(block: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let unfolded = block.replace("\r\n ", "").replace("\n ", "");
    for line in unfolded.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (key_part, value) = match line.split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        let key = key_part
            .split(';')
            .next()
            .unwrap_or(key_part)
            .to_ascii_uppercase();
        map.insert(key, value.to_string());
    }
    map
}

fn parse_ical_time(raw: Option<&str>) -> (i64, bool) {
    let Some(raw) = raw else {
        return (0, false);
    };
    let compact: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if compact.len() == 8 {
        // YYYYMMDD all-day
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&compact, "%Y%m%d") {
            let dt = date
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp();
            return (dt, true);
        }
    }
    if compact.ends_with('Z') && compact.len() >= 15 {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&compact[..15], "%Y%m%dT%H%M%S")
        {
            return (dt.and_utc().timestamp(), false);
        }
    }
    if compact.len() >= 15 {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&compact[..15], "%Y%m%dT%H%M%S")
        {
            return (dt.and_utc().timestamp(), false);
        }
    }
    (0, false)
}
