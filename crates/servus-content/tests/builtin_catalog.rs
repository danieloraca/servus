use servus_content::ContentCatalog;

#[test]
fn builtin_catalog_has_human_readable_content() {
    let catalog = ContentCatalog::builtin();
    assert!(catalog.services().iter().all(|service| {
        !service.display_name.trim().is_empty() && !service.description.trim().is_empty()
    }));
}
