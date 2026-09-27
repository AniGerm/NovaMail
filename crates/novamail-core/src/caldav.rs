//! CalDAV client: REPORT sync, PROPFIND discovery, PUT/DELETE write-back.

use std::collections::HashMap;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::{CoreError, CoreResult};

#[derive(Debug, Clone)]
pub struct CalDavCollection {
    pub href: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct ParsedEvent {
    pub uid: String,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub all_day: bool,
    pub status: String,
    /// Reminder offsets in minutes before start.
    pub reminder_minutes: Vec<i64>,
    pub organizer: Option<String>,
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

/// Discover calendar collections from a CalDAV base / well-known URL (Basic auth).
pub async fn discover_calendars(
    url: &str,
    username: &str,
    password: &str,
) -> CoreResult<Vec<CalDavCollection>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CoreError::Message(e.to_string()))?;

    let base = url.trim_end_matches('/');
    let principal_xml = caldav_propfind(
        &client,
        base,
        username,
        password,
        0,
        r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#,
    )
    .await
    .unwrap_or_default();

    let principal = extract_href_after(&principal_xml, "current-user-principal")
        .map(|h| resolve_href(base, &h))
        .unwrap_or_else(|| base.to_string());

    let home_xml = caldav_propfind(
        &client,
        &principal,
        username,
        password,
        0,
        r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-home-set/></D:prop>
</D:propfind>"#,
    )
    .await
    .unwrap_or_default();

    let home = extract_href_after(&home_xml, "calendar-home-set")
        .map(|h| resolve_href(&principal, &h))
        .unwrap_or_else(|| principal.clone());

    let list_xml = caldav_propfind(
        &client,
        &home,
        username,
        password,
        1,
        r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#,
    )
    .await?;

    let mut collections = parse_calendar_collections(&list_xml, &home);
    if collections.is_empty() {
        // Treat the provided URL itself as a calendar collection.
        collections.push(CalDavCollection {
            href: home,
            display_name: "Calendar".into(),
        });
    }
    Ok(collections)
}

pub async fn put_vevent(
    collection_url: &str,
    username: &str,
    password: &str,
    ical_uid: &str,
    ics: &str,
) -> CoreResult<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CoreError::Message(e.to_string()))?;
    let href = object_href(collection_url, ical_uid);
    let response = client
        .put(&href)
        .header(AUTHORIZATION, basic_auth(username, password))
        .header(CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(ics.to_string())
        .send()
        .await
        .map_err(|e| CoreError::Message(format!("CalDAV PUT failed: {e}")))?;
    if !(response.status().is_success() || response.status().as_u16() == 201) {
        return Err(CoreError::Message(format!(
            "CalDAV PUT HTTP {}",
            response.status()
        )));
    }
    Ok(())
}

pub async fn delete_object(
    collection_url: &str,
    username: &str,
    password: &str,
    ical_uid: &str,
) -> CoreResult<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CoreError::Message(e.to_string()))?;
    let href = object_href(collection_url, ical_uid);
    let response = client
        .delete(&href)
        .header(AUTHORIZATION, basic_auth(username, password))
        .send()
        .await
        .map_err(|e| CoreError::Message(format!("CalDAV DELETE failed: {e}")))?;
    if !(response.status().is_success() || response.status().as_u16() == 404) {
        return Err(CoreError::Message(format!(
            "CalDAV DELETE HTTP {}",
            response.status()
        )));
    }
    Ok(())
}

pub fn build_vevent_ics(
    uid: &str,
    title: &str,
    starts_at: i64,
    ends_at: Option<i64>,
    location: Option<&str>,
    description: Option<&str>,
    all_day: bool,
    status: &str,
    reminder_minutes: &[i64],
) -> String {
    let end = ends_at.unwrap_or(starts_at + 3600);
    let (dtstart, dtend) = if all_day {
        (
            format_ical_date(starts_at),
            format_ical_date(end),
        )
    } else {
        (format_ical_utc(starts_at), format_ical_utc(end))
    };
    let mut lines = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//NovaMail//EN".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{}", format_ical_utc(chrono::Utc::now().timestamp())),
        if all_day {
            format!("DTSTART;VALUE=DATE:{dtstart}")
        } else {
            format!("DTSTART:{dtstart}")
        },
        if all_day {
            format!("DTEND;VALUE=DATE:{dtend}")
        } else {
            format!("DTEND:{dtend}")
        },
        format!("SUMMARY:{}", escape_ical_text(title)),
        format!("STATUS:{}", status.to_ascii_uppercase()),
    ];
    if let Some(loc) = location.filter(|s| !s.is_empty()) {
        lines.push(format!("LOCATION:{}", escape_ical_text(loc)));
    }
    if let Some(desc) = description.filter(|s| !s.is_empty()) {
        lines.push(format!("DESCRIPTION:{}", escape_ical_text(desc)));
    }
    for minutes in reminder_minutes {
        lines.push("BEGIN:VALARM".into());
        lines.push("ACTION:DISPLAY".into());
        lines.push(format!("DESCRIPTION:{}", escape_ical_text(title)));
        lines.push(format!("TRIGGER:-PT{minutes}M"));
        lines.push("END:VALARM".into());
    }
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());
    lines.join("\r\n") + "\r\n"
}

fn object_href(collection_url: &str, ical_uid: &str) -> String {
    let base = collection_url.trim_end_matches('/');
    let safe: String = ical_uid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{base}/{safe}.ics")
}

fn format_ical_utc(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_else(|| "19700101T000000Z".into())
}

fn format_ical_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y%m%d").to_string())
        .unwrap_or_else(|| "19700101".into())
}

fn escape_ical_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn resolve_href(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(joined) = base_url.join(href) {
            return joined.to_string();
        }
    }
    if href.starts_with('/') {
        if let Ok(base_url) = url::Url::parse(base) {
            let mut origin = base_url.origin().ascii_serialization();
            if !origin.ends_with('/') && !href.starts_with('/') {
                origin.push('/');
            }
            return format!("{origin}{href}");
        }
    }
    format!("{}/{}", base.trim_end_matches('/'), href.trim_start_matches('/'))
}

fn extract_href_after(xml: &str, marker: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let marker = marker.to_ascii_lowercase();
    let start = lower.find(&marker)?;
    let after = &xml[start..];
    let after_lower = &lower[start..];
    let href_rel = after_lower.find("<d:href>")
        .or_else(|| after_lower.find("<href>"))?;
    let href_start = after_lower[href_rel..]
        .find('>')
        .map(|i| href_rel + i + 1)?;
    let href_end_rel = after_lower[href_start..]
        .find("</")
        .map(|i| href_start + i)?;
    Some(after[href_start..href_end_rel].trim().to_string())
}

fn parse_calendar_collections(xml: &str, home: &str) -> Vec<CalDavCollection> {
    let mut out = Vec::new();
    let lower = xml.to_ascii_lowercase();
    let mut search = 0;
    while let Some(resp_rel) = lower[search..].find("<d:response")
        .or_else(|| lower[search..].find("<response"))
    {
        let resp_start = search + resp_rel;
        let resp_end = lower[resp_start..]
            .find("</d:response>")
            .or_else(|| lower[resp_start..].find("</response>"))
            .map(|i| resp_start + i)
            .unwrap_or(lower.len());
        let chunk = &xml[resp_start..resp_end];
        let chunk_lower = chunk.to_ascii_lowercase();
        if !chunk_lower.contains("calendar") || chunk_lower.contains("calendar-home-set") {
            search = resp_end + 1;
            continue;
        }
        // skip if not a calendar resourcetype
        if !(chunk_lower.contains("<c:calendar")
            || chunk_lower.contains("<calendar/>")
            || chunk_lower.contains(":calendar/>")
            || chunk_lower.contains("<calendar "))
        {
            search = resp_end + 1;
            continue;
        }
        let href = extract_href_after(chunk, "href")
            .map(|h| resolve_href(home, &h))
            .unwrap_or_default();
        if href.is_empty() {
            search = resp_end + 1;
            continue;
        }
        let display_name = extract_tag_text(chunk, "displayname").unwrap_or_else(|| {
            href.trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("Calendar")
                .to_string()
        });
        out.push(CalDavCollection { href, display_name });
        search = resp_end + 1;
    }
    out
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{tag}");
    let open_alt = format!(":d:{tag}");
    let start = lower
        .find(&open)
        .or_else(|| lower.find(&format!("<d:{tag}")))
        .or_else(|| lower.find(&open_alt))?;
    let after = lower[start..].find('>')? + start + 1;
    let end = lower[after..].find("</")? + after;
    let text = xml[after..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

async fn caldav_propfind(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
    depth: u8,
    body: &str,
) -> CoreResult<String> {
    let response = client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(AUTHORIZATION, basic_auth(username, password))
        .header("Depth", depth.to_string())
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| CoreError::Message(format!("CalDAV PROPFIND failed: {e}")))?;
    if !(response.status().is_success() || response.status().as_u16() == 207) {
        return Err(CoreError::Message(format!(
            "CalDAV PROPFIND HTTP {}",
            response.status()
        )));
    }
    response
        .text()
        .await
        .map_err(|e| CoreError::Message(e.to_string()))
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
        let status = props
            .get("STATUS")
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "confirmed".into());
        let organizer = props.get("ORGANIZER").map(|v| {
            v.trim_start_matches("mailto:")
                .trim_start_matches("MAILTO:")
                .to_string()
        });
        let mut reminder_minutes = Vec::new();
        for alarm in split_components(&block, "VALARM") {
            let alarm_props = parse_props(&alarm);
            if let Some(trigger) = alarm_props.get("TRIGGER") {
                if let Some(mins) = parse_trigger_minutes(trigger) {
                    reminder_minutes.push(mins);
                }
            }
        }
        events.push(ParsedEvent {
            uid,
            title,
            starts_at,
            ends_at: if ends_at > 0 { Some(ends_at) } else { None },
            location: props.get("LOCATION").cloned(),
            description: props.get("DESCRIPTION").cloned(),
            all_day,
            status,
            reminder_minutes,
            organizer,
        });
    }
    events
}

fn parse_trigger_minutes(trigger: &str) -> Option<i64> {
    // -PT15M / -P1DT2H
    let t = trigger.trim().to_ascii_uppercase();
    if !t.starts_with('-') {
        return Some(0);
    }
    let t = t.trim_start_matches('-').trim_start_matches('P');
    let mut minutes = 0i64;
    if let Some(day_split) = t.split_once('D') {
        minutes += day_split.0.parse::<i64>().ok()? * 24 * 60;
        let rest = day_split.1.trim_start_matches('T');
        if let Some(h) = rest.split_once('H') {
            minutes += h.0.parse::<i64>().unwrap_or(0) * 60;
            if let Some(m) = h.1.split_once('M') {
                minutes += m.0.parse::<i64>().unwrap_or(0);
            }
        } else if let Some(m) = rest.split_once('M') {
            minutes += m.0.parse::<i64>().unwrap_or(0);
        }
        return Some(minutes);
    }
    let t = t.trim_start_matches('T');
    if let Some(h) = t.split_once('H') {
        minutes += h.0.parse::<i64>().ok()? * 60;
        if let Some(m) = h.1.split_once('M') {
            minutes += m.0.parse::<i64>().unwrap_or(0);
        }
        return Some(minutes);
    }
    if let Some(m) = t.split_once('M') {
        return m.0.parse().ok();
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_vevent_ics_with_uid() {
        let ics = build_vevent_ics(
            "abc-123",
            "Standup",
            1_700_000_000,
            Some(1_700_003_600),
            Some("Zoom"),
            None,
            false,
            "confirmed",
            &[15],
        );
        assert!(ics.contains("UID:abc-123"));
        assert!(ics.contains("SUMMARY:Standup"));
        assert!(ics.contains("LOCATION:Zoom"));
        assert!(ics.contains("BEGIN:VEVENT"));
        assert!(ics.contains("BEGIN:VALARM"));
        assert!(ics.contains("TRIGGER:-PT15M"));
    }
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
