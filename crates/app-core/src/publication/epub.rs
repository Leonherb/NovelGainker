use super::{markdown, xml, BookSnapshot, PublicationIssue, MAX_PUBLICATION_OUTPUT};
use quick_xml::{events::Event, Reader};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

pub(crate) struct BuiltArchive {
    pub bytes: u64,
    pub hash: String,
    pub entries: usize,
}

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

// Stored ZIP only: no compression bombs, ZIP64, arbitrary paths, extras or encryption.
struct StoredZip<W: Write> {
    writer: W,
    offset: u64,
    hash: Sha256,
    entries: Vec<Entry>,
}

impl<W: Write> StoredZip<W> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), PublicationIssue> {
        let end = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or(PublicationIssue::LimitExceeded)?;
        if end > MAX_PUBLICATION_OUTPUT {
            return Err(PublicationIssue::LimitExceeded);
        }
        self.writer
            .write_all(bytes)
            .map_err(|_| PublicationIssue::StoreUnavailable)?;
        self.hash.update(bytes);
        self.offset = end;
        Ok(())
    }
    fn u16(&mut self, v: u16) -> Result<(), PublicationIssue> {
        self.bytes(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) -> Result<(), PublicationIssue> {
        self.bytes(&v.to_le_bytes())
    }
    fn entry(&mut self, name: &str, data: &[u8]) -> Result<(), PublicationIssue> {
        if self.entries.len() >= 704
            || name.len() > 96
            || name.is_empty()
            || name
                .bytes()
                .any(|b| !(b.is_ascii_alphanumeric() || b"/-._".contains(&b)))
            || name.split('/').any(|p| p.is_empty() || p.starts_with('.'))
            || self.entries.iter().any(|entry| entry.name == name)
            || self.entries.is_empty() && (name != "mimetype" || data != b"application/epub+zip")
        {
            return Err(PublicationIssue::LocalValidationFailed);
        }
        let entry = Entry {
            name: name.to_owned(),
            crc: crc32fast::hash(data),
            size: u32::try_from(data.len()).map_err(|_| PublicationIssue::LimitExceeded)?,
            offset: u32::try_from(self.offset).map_err(|_| PublicationIssue::LimitExceeded)?,
        };
        self.u32(0x04034b50)?;
        for value in [20, 0, 0, 0, 0x21] {
            self.u16(value)?;
        }
        self.u32(entry.crc)?;
        self.u32(entry.size)?;
        self.u32(entry.size)?;
        self.u16(name.len() as u16)?;
        self.u16(0)?;
        self.bytes(name.as_bytes())?;
        self.bytes(data)?;
        self.entries.push(entry);
        Ok(())
    }
    fn xml_entry(&mut self, name: &str, data: &str) -> Result<(), PublicationIssue> {
        validate_xml(data)?;
        self.entry(name, data.as_bytes())
    }
    fn finish(mut self) -> Result<BuiltArchive, PublicationIssue> {
        let start = self.offset as u32;
        let entries = std::mem::take(&mut self.entries);
        for entry in &entries {
            self.u32(0x02014b50)?;
            for value in [20, 20, 0, 0, 0, 0x21] {
                self.u16(value)?;
            }
            self.u32(entry.crc)?;
            self.u32(entry.size)?;
            self.u32(entry.size)?;
            self.u16(entry.name.len() as u16)?;
            for _ in 0..4 {
                self.u16(0)?;
            }
            self.u32(0)?;
            self.u32(entry.offset)?;
            self.bytes(entry.name.as_bytes())?;
        }
        let size = self.offset as u32 - start;
        self.u32(0x06054b50)?;
        self.u16(0)?;
        self.u16(0)?;
        self.u16(entries.len() as u16)?;
        self.u16(entries.len() as u16)?;
        self.u32(size)?;
        self.u32(start)?;
        self.u16(0)?;
        self.writer
            .flush()
            .map_err(|_| PublicationIssue::StoreUnavailable)?;
        Ok(BuiltArchive {
            bytes: self.offset,
            hash: format!("{:x}", self.hash.finalize()),
            entries: entries.len(),
        })
    }
}

fn validate_xml(text: &str) -> Result<(), PublicationIssue> {
    let bad = PublicationIssue::LocalValidationFailed;
    let mut reader = Reader::from_str(text);
    let mut stack = Vec::<Vec<u8>>::new();
    let mut roots = 0;
    loop {
        match reader.read_event().map_err(|_| bad)? {
            Event::Start(element) | Event::Empty(element) => {
                // Empty elements are handled separately below through is_empty detection.
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| bad)?;
                    attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Explicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|_| bad)?;
                }
                if stack.is_empty() {
                    roots += 1;
                }
                // Reader's buffer position points past '>'; '/>' denotes our generated empty form.
                let position = reader.buffer_position() as usize;
                if !text[..position].ends_with("/>") {
                    stack.push(element.name().as_ref().to_vec());
                }
                if stack.len() > 16 || roots > 1 {
                    return Err(bad);
                }
            }
            Event::End(element) => {
                if stack.pop().as_deref() != Some(element.name().as_ref()) {
                    return Err(bad);
                }
            }
            Event::Text(text) => {
                text.xml_content(quick_xml::XmlVersion::Explicit1_0)
                    .map_err(|_| bad)?;
            }
            Event::GeneralRef(reference) => {
                let reference: &[u8] = reference.as_ref();
                if !matches!(reference, b"amp" | b"lt" | b"gt" | b"quot" | b"apos") {
                    return Err(bad);
                }
            }
            Event::Decl(_) => {}
            Event::Eof => break,
            _ => return Err(bad),
        }
    }
    if roots != 1 || !stack.is_empty() {
        return Err(bad);
    }
    Ok(())
}

pub(crate) fn build(
    snapshot: &BookSnapshot,
    artifact_id: &str,
    modified: &str,
    writer: impl Write,
    mut checkpoint: impl FnMut() -> Result<(), PublicationIssue>,
) -> Result<BuiltArchive, PublicationIssue> {
    if uuid::Uuid::parse_str(artifact_id).is_err()
        || chrono::NaiveDateTime::parse_from_str(modified, "%Y-%m-%dT%H:%M:%SZ").is_err()
        || modified.len() != 20
    {
        return Err(PublicationIssue::LocalValidationFailed);
    }
    if snapshot.chapters.is_empty()
        || snapshot.chapters.len() > super::MAX_PUBLICATION_CHAPTERS
        || snapshot.assets.len() > super::MAX_PUBLICATION_ASSETS
    {
        return Err(PublicationIssue::LimitExceeded);
    }
    let mut zip = StoredZip {
        writer,
        offset: 0,
        hash: Sha256::new(),
        entries: Vec::new(),
    };
    checkpoint()?;
    zip.entry("mimetype", b"application/epub+zip")?;
    zip.xml_entry("META-INF/container.xml", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"EPUB/package.opf\" media-type=\"application/oebps-package+xml\" /></rootfiles></container>")?;
    let mut mapping = BTreeMap::new();
    let mut manifest = String::from("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\" />");
    let mut spine = String::new();
    let mut nav = String::new();
    let mut rendered = 0usize;
    for (index, (path, asset)) in snapshot.assets.iter().enumerate() {
        checkpoint()?;
        let href = format!("images/a{index}.{}", asset.extension);
        let property = if snapshot.metadata.cover.as_deref() == Some(path.as_str()) {
            " properties=\"cover-image\""
        } else {
            ""
        };
        manifest.push_str(&format!(
            "<item id=\"a{index}\" href=\"{href}\" media-type=\"{}\"{property} />",
            asset.media_type
        ));
        zip.entry(&format!("EPUB/{href}"), &asset.bytes)?;
        mapping.insert(path.clone(), href);
    }
    if snapshot
        .metadata
        .cover
        .as_ref()
        .is_some_and(|cover| !mapping.contains_key(cover))
    {
        return Err(PublicationIssue::InvalidAsset);
    }
    let mut numbers = BTreeSet::new();
    let mut previous = 0;
    for (index, chapter) in snapshot.chapters.iter().enumerate() {
        checkpoint()?;
        if chapter.number <= previous || !numbers.insert(chapter.number) {
            return Err(PublicationIssue::InvalidChapter);
        }
        previous = chapter.number;
        let title = xml(&chapter.title)?;
        let content = markdown::render(&chapter.body, &mapping)?;
        rendered = rendered
            .checked_add(content.len())
            .ok_or(PublicationIssue::LimitExceeded)?;
        if rendered > super::MAX_PUBLICATION_RENDERED {
            return Err(PublicationIssue::LimitExceeded);
        }
        zip.xml_entry(
            &format!("EPUB/c{index}.xhtml"),
            &xhtml(&title, &format!("<h1>{title}</h1>{content}")),
        )?;
        manifest.push_str(&format!(
            "<item id=\"c{index}\" href=\"c{index}.xhtml\" media-type=\"application/xhtml+xml\" />"
        ));
        spine.push_str(&format!("<itemref idref=\"c{index}\" />"));
        nav.push_str(&format!("<li><a href=\"c{index}.xhtml\">{title}</a></li>"));
    }
    zip.xml_entry(
        "EPUB/nav.xhtml",
        &xhtml(
            "目录",
            &format!("<nav epub:type=\"toc\" id=\"toc\"><h1>目录</h1><ol>{nav}</ol></nav>"),
        ),
    )?;
    let metadata = &snapshot.metadata;
    let opf = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:uuid:{artifact_id}</dc:identifier><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>{}</dc:language><meta property=\"dcterms:modified\">{modified}</meta></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>", xml(&metadata.title)?, xml(&metadata.author)?, xml(&metadata.language)?);
    zip.xml_entry("EPUB/package.opf", &opf)?;
    checkpoint()?;
    zip.finish()
}

fn xhtml(title: &str, body: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"zh-CN\" xml:lang=\"zh-CN\"><head><title>{title}</title></head><body>{body}</body></html>")
}
