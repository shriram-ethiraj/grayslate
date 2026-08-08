#[derive(serde::Deserialize)]
struct FileAssociationManifest {
    extensions: Vec<String>,
}

#[test]
fn installer_extensions_match_the_language_detector() {
    let manifest: FileAssociationManifest =
        serde_json::from_str(include_str!("../../packaging/file-associations.json"))
            .expect("file association manifest must be valid JSON");
    let detector_extensions = grayslate_langdetect::supported_extensions()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(manifest.extensions, detector_extensions);
}
