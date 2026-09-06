use super::*;

fn metadata() -> BookMetadata {
    BookMetadata {
        title: "测试 & 书名".to_owned(),
        author: "合成作者".to_owned(),
        language: "zh-CN".to_owned(),
        cover: None,
    }
}

fn stored_entries(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut entries = BTreeMap::new();
    let mut position = 0;
    let u16_at = |p| u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap()) as usize;
    let u32_at = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
    while bytes[position..].starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
        assert_eq!(u16_at(position + 6), 0);
        assert_eq!(u16_at(position + 8), 0);
        let size = u32_at(position + 18) as usize;
        assert_eq!(size, u32_at(position + 22) as usize);
        let name_len = u16_at(position + 26);
        assert_eq!(u16_at(position + 28), 0);
        let name = std::str::from_utf8(&bytes[position + 30..position + 30 + name_len]).unwrap();
        let start = position + 30 + name_len;
        let data = &bytes[start..start + size];
        assert_eq!(crc32fast::hash(data), u32_at(position + 14));
        assert!(entries.insert(name.to_owned(), data.to_vec()).is_none());
        if position == 0 {
            assert_eq!(name, "mimetype");
            assert_eq!(data, b"application/epub+zip");
        }
        position = start + size;
    }
    let central_start = position;
    for _ in 0..entries.len() {
        assert_eq!(&bytes[position..position + 4], &[0x50, 0x4b, 0x01, 0x02]);
        let name_len = u16_at(position + 28);
        assert_eq!(u16_at(position + 30), 0);
        assert_eq!(u16_at(position + 32), 0);
        let local = u32_at(position + 42) as usize;
        assert_eq!(&bytes[local..local + 4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(
            &bytes[position + 46..position + 46 + name_len],
            &bytes[local + 30..local + 30 + name_len]
        );
        position += 46 + name_len;
    }
    assert_eq!(&bytes[position..position + 4], &[0x50, 0x4b, 0x05, 0x06]);
    assert_eq!(u16_at(position + 8), entries.len());
    assert_eq!(u16_at(position + 10), entries.len());
    assert_eq!(u32_at(position + 12) as usize, position - central_start);
    assert_eq!(u32_at(position + 16) as usize, central_start);
    assert_eq!(position + 22, bytes.len());
    entries
}

#[test]
fn nr505_metadata_uses_actual_author_field_and_closed_asset_paths() {
    let good = "---\ntitle: 合成测试\nauthorDisplayName: 测试作者\nwritingLanguage: zh-CN\nkind: project\nschemaVersion: 1\n---\n# 项目\n";
    assert_eq!(
        BookMetadata::parse(good.as_bytes()).unwrap().author,
        "测试作者"
    );
    for bad in [
        good.replace("authorDisplayName", "author"),
        good.replace("zh-CN", "en"),
        good.replace("测试作者", "''"),
        good.replace("schemaVersion: 1", "schemaVersion: 2"),
    ] {
        assert!(BookMetadata::parse(bad.as_bytes()).is_err());
    }
    for path in [
        "https://example.com/a.png",
        "../secrets.png",
        "assets/../a.png",
        "assets/a.svg",
        "assets/a.png?x",
        "assets/%2e/a.png",
        "assets/CON:/x.png",
        "assets/.private/a.jpg",
    ] {
        assert!(assets::asset_path(path).is_err(), "{path}");
    }
    assert_eq!(
        assets::asset_path("../assets/插图.png").unwrap(),
        "assets/插图.png"
    );
}

#[test]
fn nr505_draft_and_markdown_preserve_supported_content_and_reject_silent_loss() {
    let draft = markdown::draft(b"---\nkind: chapter\n---\n# Title\n## Draft\nBody & text\n```text\n## Not a section\n```\n## Notes\nSECRET NOTE\n").unwrap();
    assert!(draft.contains("## Not a section"));
    assert!(!draft.contains("SECRET NOTE"));
    assert!(markdown::draft(b"## Draft\none\n## Draft\ntwo").is_err());
    assert!(markdown::draft(b"## Notes\nno body").is_err());
    let rendered = markdown::render("### 标题\n\n普通 **粗体** 与 *强调* `a < b`。\n\n- 一\n- 二\n\n3. 三\n4. 四\n\n> 引用\n\n```text\n<script>只是代码</script>\n```", &BTreeMap::new()).unwrap();
    assert!(rendered.contains("<strong>粗体</strong>"));
    assert!(rendered.contains("<li value=\"3\">三</li>"));
    assert!(rendered.contains("&lt;script&gt;只是代码&lt;/script&gt;"));
    for bad in [
        "<script>alert(1)</script>",
        "[链接](https://example.com)",
        "| 表格 |",
        "[^脚注]",
        "**嵌套 *标记* **",
        "    缩进代码",
        "正文 ![图片](assets/a.png)",
        "```\n未闭合",
        "*未闭合",
        "~~~\n未闭合",
    ] {
        assert!(markdown::render(bad, &BTreeMap::new()).is_err(), "{bad}");
    }
}

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, content) in [
        (&b"IHDR"[..], vec![0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]),
        (&b"IDAT"[..], vec![120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1]),
        (&b"IEND"[..], vec![]),
    ] {
        bytes.extend((content.len() as u32).to_be_bytes());
        let start = bytes.len();
        bytes.extend(kind);
        bytes.extend(content);
        let crc = crc32fast::hash(&bytes[start..]);
        bytes.extend(crc.to_be_bytes());
    }
    bytes
}

#[test]
fn nr505_png_signature_crc_dimensions_and_jpeg_markers_are_bounded() {
    assert!(assets::validate("assets/a.png", png()).is_ok());
    let mut broken = png();
    broken[20] = 9;
    assert!(assets::validate("assets/a.png", broken).is_err());
    assert!(assets::validate("assets/a.jpg", png()).is_err());
    for bad in [
        vec![],
        b"<svg/>".to_vec(),
        vec![0xff, 0xd8, 0xff, 0xd9],
        vec![0xff, 0xd8, 0xff, 0xc0, 0, 1, 0xff, 0xd9],
    ] {
        assert!(assets::validate("assets/a.jpg", bad).is_err());
    }
    let mut trailing = png();
    trailing.extend(b"hidden payload");
    assert!(assets::validate("assets/a.png", trailing).is_err());
}

#[test]
fn nr505_epub_stored_zip_xml_navigation_spine_and_scope_match_snapshot() {
    let mut book = BookSnapshot {
        metadata: metadata(),
        chapters: vec![
            BookChapter {
                number: 1,
                title: "第一章 & 起点".into(),
                body: "正文 & 内容\n\n![插图](assets/a.png)".into(),
            },
            BookChapter {
                number: 2,
                title: "第二章".into(),
                body: "结尾。".into(),
            },
        ],
        assets: BTreeMap::new(),
    };
    book.assets.insert(
        "assets/a.png".into(),
        assets::validate("assets/a.png", png()).unwrap(),
    );
    let mut bytes = Vec::new();
    let result = epub::build(
        &book,
        &uuid::Uuid::new_v4().to_string(),
        "2026-09-05T00:00:00Z",
        &mut bytes,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(result.bytes as usize, bytes.len());
    assert_eq!(result.hash, crate::workspace::sha256_hex(&bytes));
    let entries = stored_entries(&bytes);
    assert_eq!(result.entries, entries.len());
    assert_eq!(entries.len(), 7);
    let opf = std::str::from_utf8(&entries["EPUB/package.opf"]).unwrap();
    assert!(opf.contains("测试 &amp; 书名"));
    assert!(opf.contains("media-type=\"image/png\""));
    assert!(opf.contains("<itemref idref=\"c0\" /><itemref idref=\"c1\" />"));
    assert!(!opf.contains("cover-image"));
    assert!(std::str::from_utf8(&entries["EPUB/c0.xhtml"])
        .unwrap()
        .contains("src=\"images/a0.png\""));
    assert!(!entries.keys().any(|path| path.contains(".novelgainker")
        || path.contains("project.md")
        || path.contains("..")));
    book.metadata.cover = Some("assets/a.png".into());
    let mut covered = Vec::new();
    epub::build(
        &book,
        &uuid::Uuid::new_v4().to_string(),
        "2026-09-05T00:00:00Z",
        &mut covered,
        || Ok(()),
    )
    .unwrap();
    assert!(
        std::str::from_utf8(&stored_entries(&covered)["EPUB/package.opf"])
            .unwrap()
            .contains("properties=\"cover-image\"")
    );
    // Isolated synthetic artifact for an independent .NET ZIP/XML test dependency.
    // This is not an application export path or a real manuscript.
    let evidence_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/qa/nr-505");
    std::fs::create_dir_all(&evidence_root).unwrap();
    std::fs::write(evidence_root.join("epub-structure-fixture.epub"), &covered).unwrap();
    book.chapters.reverse();
    assert!(epub::build(
        &book,
        &uuid::Uuid::new_v4().to_string(),
        "2026-09-05T00:00:00Z",
        Vec::new(),
        || Ok(())
    )
    .is_err());
    assert!(epub::build(
        &book,
        &uuid::Uuid::new_v4().to_string(),
        "2026-09-05T00:00:00Z",
        Vec::new(),
        || Err(PublicationIssue::Cancelled)
    )
    .is_err());
}

#[test]
fn nr505_container_500_chapter_limit_and_unsupported_resource_limit_are_explicit() {
    let mut book = BookSnapshot {
        metadata: metadata(),
        chapters: (1..=500)
            .map(|number| BookChapter {
                number,
                title: format!("第{number}章"),
                body: "合成正文。".to_owned(),
            })
            .collect(),
        assets: BTreeMap::new(),
    };
    let built = epub::build(
        &book,
        &uuid::Uuid::new_v4().to_string(),
        "2026-09-05T00:00:00Z",
        std::io::sink(),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(built.entries, 504);
    book.chapters.push(BookChapter {
        number: 501,
        title: "超限".into(),
        body: "合成".into(),
    });
    assert!(matches!(
        epub::build(
            &book,
            &uuid::Uuid::new_v4().to_string(),
            "2026-09-05T00:00:00Z",
            std::io::sink(),
            || Ok(())
        ),
        Err(PublicationIssue::LimitExceeded)
    ));
    book.chapters.truncate(1);
    for index in 0..201 {
        book.assets.insert(
            format!("assets/a{index}.png"),
            assets::validate("assets/a.png", png()).unwrap(),
        );
    }
    assert!(matches!(
        epub::build(
            &book,
            &uuid::Uuid::new_v4().to_string(),
            "2026-09-05T00:00:00Z",
            std::io::sink(),
            || Ok(())
        ),
        Err(PublicationIssue::LimitExceeded)
    ));
}
