use super::{assets::asset_path, xml, PublicationIssue};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn draft(bytes: &[u8]) -> Result<String, PublicationIssue> {
    let raw = std::str::from_utf8(bytes).map_err(|_| PublicationIssue::InvalidChapter)?;
    let text = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    if text.contains('\r') {
        return Err(PublicationIssue::InvalidChapter);
    }
    let mut inside = false;
    let mut found = false;
    let mut fence: Option<&str> = None;
    let mut body = String::new();
    for line in text.lines() {
        if let Some(marker) = fence {
            if line == marker {
                fence = None;
            }
            if inside {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if let Some(marker) = fence_start(line) {
            fence = Some(marker);
        }
        if line.trim_end() == "## Draft" {
            if found {
                return Err(PublicationIssue::InvalidChapter);
            }
            found = true;
            inside = true;
            continue;
        }
        if line.starts_with("# ") || line.starts_with("## ") {
            inside = false;
        }
        if inside {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !found || body.trim().is_empty() || fence.is_some() {
        return Err(PublicationIssue::InvalidChapter);
    }
    Ok(body)
}

fn fence_start(line: &str) -> Option<&'static str> {
    ["```", "~~~"]
        .into_iter()
        .find(|marker| line.starts_with(marker))
}

fn image(line: &str) -> Result<Option<(String, String)>, PublicationIssue> {
    if !line.starts_with("![") {
        return Ok(None);
    }
    let (alt, target) = line
        .strip_prefix("![")
        .and_then(|v| v.strip_suffix(')'))
        .and_then(|v| v.split_once("]("))
        .ok_or(PublicationIssue::UnsupportedMarkdown)?;
    if alt.contains(['[', ']', '\n']) || target.contains(['(', ')', '"', '\'']) {
        return Err(PublicationIssue::UnsupportedMarkdown);
    }
    Ok(Some((alt.to_owned(), asset_path(target)?)))
}

pub(crate) fn asset_references(body: &str) -> Result<BTreeSet<String>, PublicationIssue> {
    let mut result = BTreeSet::new();
    let mut fence = None;
    for line in body.lines() {
        if let Some(marker) = fence {
            if line == marker {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = fence_start(line) {
            fence = Some(marker);
            continue;
        }
        if let Some((_, path)) = image(line)? {
            result.insert(path);
        }
    }
    Ok(result)
}

// Deliberately closed, nonrecursive inline grammar. Unsupported syntax fails visibly.
fn inline(text: &str) -> Result<String, PublicationIssue> {
    let bad = PublicationIssue::UnsupportedMarkdown;
    let mut output = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix('\\') {
            let c = next.chars().next().ok_or(bad)?;
            if !c.is_ascii_punctuation() {
                return Err(bad);
            }
            output.push_str(&xml(&c.to_string())?);
            rest = &next[c.len_utf8()..];
            continue;
        }
        if let Some((marker, tag)) = [
            ("**", "strong"),
            ("__", "strong"),
            ("*", "em"),
            ("_", "em"),
            ("`", "code"),
        ]
        .into_iter()
        .find(|(marker, _)| rest.starts_with(marker))
        {
            let next = &rest[marker.len()..];
            let end = next.find(marker).ok_or(bad)?;
            let value = &next[..end];
            if value.is_empty()
                || marker != "`" && value.contains(['*', '_', '`', '[', ']', '<', '>'])
            {
                return Err(bad);
            }
            output.push_str(&format!("<{tag}>{}</{tag}>", xml(value)?));
            rest = &next[end + marker.len()..];
            continue;
        }
        let c = rest.chars().next().ok_or(bad)?;
        if matches!(c, '[' | ']' | '<' | '>' | '|' | '~') {
            return Err(bad);
        }
        output.push_str(&xml(&c.to_string())?);
        rest = &rest[c.len_utf8()..];
    }
    Ok(output)
}

pub(crate) fn render(
    body: &str,
    assets: &BTreeMap<String, String>,
) -> Result<String, PublicationIssue> {
    let bad = PublicationIssue::UnsupportedMarkdown;
    let mut output = String::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut list: Option<&str> = None;
    let mut fence: Option<&str> = None;
    let flush = |output: &mut String,
                 paragraph: &mut Vec<&str>,
                 list: &mut Option<&str>|
     -> Result<(), PublicationIssue> {
        if !paragraph.is_empty() {
            output.push_str(&format!("<p>{}</p>\n", inline(&paragraph.join("\n"))?));
            paragraph.clear();
        }
        if let Some(tag) = list.take() {
            output.push_str(&format!("</{tag}>\n"));
        }
        Ok(())
    };
    for line in body.lines() {
        if let Some(marker) = fence {
            if line == marker {
                output.push_str("</code></pre>\n");
                fence = None;
            } else {
                output.push_str(&xml(line)?);
                output.push('\n');
            }
            continue;
        }
        if let Some(marker) = fence_start(line) {
            flush(&mut output, &mut paragraph, &mut list)?;
            let language = &line[3..];
            if language.len() > 32
                || !language
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Err(bad);
            }
            output.push_str("<pre><code>");
            fence = Some(marker);
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut output, &mut paragraph, &mut list)?;
            continue;
        }
        if line.starts_with([' ', '\t'])
            || line.starts_with("---")
            || line.starts_with("===")
            || line.contains("![") && !line.starts_with("![")
        {
            return Err(bad);
        }
        if let Some((alt, path)) = image(line)? {
            flush(&mut output, &mut paragraph, &mut list)?;
            let entry = assets.get(&path).ok_or(PublicationIssue::InvalidAsset)?;
            output.push_str(&format!(
                "<p><img src=\"{}\" alt=\"{}\" /></p>\n",
                xml(entry)?,
                xml(&alt)?
            ));
            continue;
        }
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        if hashes > 0 {
            if hashes > 6 || line.as_bytes().get(hashes) != Some(&b' ') {
                return Err(bad);
            }
            flush(&mut output, &mut paragraph, &mut list)?;
            output.push_str(&format!(
                "<h{hashes}>{}</h{hashes}>\n",
                inline(&line[hashes + 1..])?
            ));
            continue;
        }
        if let Some(quote) = line.strip_prefix("> ") {
            flush(&mut output, &mut paragraph, &mut list)?;
            output.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>\n",
                inline(quote)?
            ));
            continue;
        }
        let unordered = ["- ", "+ ", "* "]
            .into_iter()
            .find_map(|prefix| line.strip_prefix(prefix));
        let ordered = line.split_once(". ").filter(|(n, _)| {
            !n.is_empty() && n.len() <= 4 && n.bytes().all(|b| b.is_ascii_digit())
        });
        let item = unordered
            .map(|value| ("ul", value, None))
            .or_else(|| ordered.map(|(number, value)| ("ol", value, Some(number))));
        if let Some((tag, value, number)) = item {
            if list != Some(tag) {
                flush(&mut output, &mut paragraph, &mut list)?;
                output.push_str(&format!("<{tag}>\n"));
                list = Some(tag);
            }
            let attribute = number
                .map(|n| format!(" value=\"{n}\""))
                .unwrap_or_default();
            output.push_str(&format!("<li{attribute}>{}</li>\n", inline(value)?));
            continue;
        }
        if list.is_some() {
            return Err(bad);
        }
        paragraph.push(line);
    }
    if fence.is_some() {
        return Err(bad);
    }
    flush(&mut output, &mut paragraph, &mut list)?;
    if output.len() > super::MAX_PUBLICATION_RENDERED {
        return Err(PublicationIssue::LimitExceeded);
    }
    Ok(output)
}
