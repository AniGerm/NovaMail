//! LDAP filter parsing/evaluation for the embedded hub server (RFC 4511 subset).
//!
//! Supports and/or/not, equalityMatch, substrings, and present — enough for
//! Ricoh MFP / generic directory clients and NovaMail's own client.

use std::collections::HashMap;

use lber::common::TagClass;
use lber::structure::{StructureTag, PL};

/// Parsed LDAP filter (subset).
#[derive(Debug, Clone)]
pub enum LdapFilter {
    And(Vec<LdapFilter>),
    Or(Vec<LdapFilter>),
    Not(Box<LdapFilter>),
    Equality { attr: String, value: String },
    Present { attr: String },
    Substring {
        attr: String,
        initial: Option<String>,
        any: Vec<String>,
        final_: Option<String>,
    },
    /// Unsupported / unparsed filter — treat as match-all so clients still work.
    MatchAll,
}

pub fn parse_filter(tag: &StructureTag) -> LdapFilter {
    // Filters use context-specific tags (RFC 4511 §4.5.1).
    match (tag.class, tag.id) {
        (TagClass::Context, 0) => {
            // and
            let parts = tag.clone().expect_constructed().unwrap_or_default();
            LdapFilter::And(parts.iter().map(parse_filter).collect())
        }
        (TagClass::Context, 1) => {
            let parts = tag.clone().expect_constructed().unwrap_or_default();
            LdapFilter::Or(parts.iter().map(parse_filter).collect())
        }
        (TagClass::Context, 2) => {
            // not — single nested filter
            let parts = tag.clone().expect_constructed().unwrap_or_default();
            if let Some(inner) = parts.first() {
                LdapFilter::Not(Box::new(parse_filter(inner)))
            } else {
                LdapFilter::MatchAll
            }
        }
        (TagClass::Context, 3) => {
            // equalityMatch AttributeValueAssertion
            let parts = tag.clone().expect_constructed().unwrap_or_default();
            let attr = octet(parts.first()).unwrap_or_default();
            let value = octet(parts.get(1)).unwrap_or_default();
            LdapFilter::Equality { attr, value }
        }
        (TagClass::Context, 4) => {
            // substrings
            let parts = tag.clone().expect_constructed().unwrap_or_default();
            let attr = octet(parts.first()).unwrap_or_default();
            let mut initial = None;
            let mut any = Vec::new();
            let mut final_ = None;
            if let Some(subs) = parts.get(1).and_then(|t| t.clone().expect_constructed()) {
                for sub in subs {
                    match (sub.class, sub.id) {
                        (TagClass::Context, 0) => initial = octet_from_tag(&sub),
                        (TagClass::Context, 1) => {
                            if let Some(v) = octet_from_tag(&sub) {
                                any.push(v);
                            }
                        }
                        (TagClass::Context, 2) => final_ = octet_from_tag(&sub),
                        _ => {}
                    }
                }
            }
            LdapFilter::Substring {
                attr,
                initial,
                any,
                final_,
            }
        }
        (TagClass::Context, 7) => {
            // present
            let attr = octet_from_tag(tag).unwrap_or_default();
            LdapFilter::Present { attr }
        }
        _ => LdapFilter::MatchAll,
    }
}

fn octet(tag: Option<&StructureTag>) -> Option<String> {
    tag.and_then(octet_from_tag)
}

fn octet_from_tag(tag: &StructureTag) -> Option<String> {
    match &tag.payload {
        PL::P(bytes) => String::from_utf8(bytes.clone()).ok(),
        PL::C(parts) => parts.first().and_then(octet_from_tag),
    }
}

fn attr_values<'a>(attrs: &'a HashMap<String, Vec<String>>, name: &str) -> Vec<&'a str> {
    let key = name.to_ascii_lowercase();
    attrs
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case(&key) || k.to_ascii_lowercase() == key)
        .flat_map(|(_, vals)| vals.iter().map(String::as_str))
        .collect()
}

fn value_matches_ci(attr: &str, left: &str, right: &str) -> bool {
    if attr.eq_ignore_ascii_case("objectClass") || attr.eq_ignore_ascii_case("objectclass") {
        left.eq_ignore_ascii_case(right)
    } else {
        // DirectoryString matching: caseIgnoreMatch for typical person attrs
        left.eq_ignore_ascii_case(right)
    }
}

fn substring_match(attr: &str, haystack: &str, filter: &LdapFilter) -> bool {
    let LdapFilter::Substring {
        initial,
        any,
        final_,
        ..
    } = filter
    else {
        return false;
    };
    let hay = if attr.eq_ignore_ascii_case("objectClass") {
        haystack.to_ascii_lowercase()
    } else {
        haystack.to_ascii_lowercase()
    };
    let mut pos = 0usize;
    if let Some(init) = initial {
        let needle = init.to_ascii_lowercase();
        if !hay.starts_with(&needle) {
            return false;
        }
        pos = needle.len();
    }
    for part in any {
        let needle = part.to_ascii_lowercase();
        if let Some(found) = hay[pos..].find(&needle) {
            pos += found + needle.len();
        } else {
            return false;
        }
    }
    if let Some(fin) = final_ {
        let needle = fin.to_ascii_lowercase();
        if !hay.ends_with(&needle) {
            return false;
        }
        if hay.len() < pos + needle.len() {
            // overlapping edge case already covered by ends_with if prefix consumed
        }
    }
    true
}

pub fn filter_matches(filter: &LdapFilter, attrs: &HashMap<String, Vec<String>>) -> bool {
    match filter {
        LdapFilter::MatchAll => true,
        LdapFilter::And(parts) => parts.iter().all(|p| filter_matches(p, attrs)),
        LdapFilter::Or(parts) => parts.is_empty() || parts.iter().any(|p| filter_matches(p, attrs)),
        LdapFilter::Not(inner) => !filter_matches(inner, attrs),
        LdapFilter::Present { attr } => !attr_values(attrs, attr).is_empty(),
        LdapFilter::Equality { attr, value } => attr_values(attrs, attr)
            .iter()
            .any(|v| value_matches_ci(attr, v, value)),
        LdapFilter::Substring { attr, .. } => attr_values(attrs, attr)
            .iter()
            .any(|v| substring_match(attr, v, filter)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> HashMap<String, Vec<String>> {
        let mut m = HashMap::new();
        for (k, v) in pairs {
            m.entry((*k).into()).or_insert_with(Vec::new).push((*v).into());
        }
        m
    }

    #[test]
    fn equality_and_present() {
        let a = attrs(&[
            ("objectClass", "inetOrgPerson"),
            ("cn", "Ada Lovelace"),
            ("facsimileTelephoneNumber", "030999"),
        ]);
        assert!(filter_matches(
            &LdapFilter::Equality {
                attr: "objectClass".into(),
                value: "inetOrgPerson".into()
            },
            &a
        ));
        assert!(filter_matches(
            &LdapFilter::Present {
                attr: "facsimileTelephoneNumber".into()
            },
            &a
        ));
        assert!(!filter_matches(
            &LdapFilter::Present {
                attr: "mail".into()
            },
            &a
        ));
    }

    #[test]
    fn substring_cn() {
        let a = attrs(&[("cn", "Ada Lovelace")]);
        assert!(filter_matches(
            &LdapFilter::Substring {
                attr: "cn".into(),
                initial: None,
                any: vec!["love".into()],
                final_: None,
            },
            &a
        ));
        assert!(filter_matches(
            &LdapFilter::Substring {
                attr: "cn".into(),
                initial: Some("ada".into()),
                any: vec![],
                final_: Some("lace".into()),
            },
            &a
        ));
    }
}
