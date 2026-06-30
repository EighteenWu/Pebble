//! Phase 0 probe that the `ews` crate's API is actually *usable*, not merely
//! resolvable in the dependency tree.
//!
//! The dependency review flagged that declaring `ews` as a dep only proves it
//! resolves. Decision #3 (use the `ews` crate for the SOAP/type layer) depends
//! on the crate's request/response types and its `quick-xml`/`xml_struct`-backed
//! serialization actually working for us. The test below exercises one real EWS
//! operation end to end:
//!   * builds a real `GetFolder` request (the same operation the spike probes),
//!   * wraps it in a SOAP [`ews::soap::Envelope`] with a `RequestServerVersion`
//!     header pinned to `Exchange2010_SP2`,
//!   * serializes the whole envelope to XML via the crate's own
//!     `Envelope::as_xml_document()` (which drives `xml_struct` + `quick-xml`),
//!   * asserts the produced XML carries the expected EWS element/value names.
//!
//! There is no runtime code here yet — this module exists purely to lock in the
//! `ews` API shape we intend to build the real `EwsProvider` on.

#[cfg(test)]
mod tests {
    use ews::get_folder::GetFolder;
    use ews::server_version::ExchangeServerVersion;
    use ews::soap::{Envelope, Header};
    use ews::{BaseFolderId, BaseShape, FolderShape};

    /// Proves the `ews` crate can build and serialize a real `GetFolder`
    /// operation, end to end, through its SOAP/`xml_struct`/`quick-xml` layer.
    #[test]
    fn ews_get_folder_request_serializes_to_xml() {
        // Build a real GetFolder request for the distinguished `inbox` folder,
        // asking only for the folder id (BaseShape=IdOnly) — exactly the request
        // the connectivity spike issues, but via the typed `ews` API instead of
        // a hand-written SOAP string.
        let request = GetFolder {
            folder_shape: FolderShape {
                base_shape: BaseShape::IdOnly,
            },
            folder_ids: vec![BaseFolderId::DistinguishedFolderId {
                id: "inbox".to_string(),
                change_key: None,
            }],
        };

        // Wrap it in a SOAP envelope with the schema version pinned, then drive
        // the crate's own serialization machinery to produce a full document.
        let envelope = Envelope {
            headers: vec![Header::RequestServerVersion {
                version: ExchangeServerVersion::Exchange2010_SP2,
            }],
            body: request,
        };

        let document = envelope
            .as_xml_document()
            .expect("ews should serialize a GetFolder envelope to an XML document");
        let xml = String::from_utf8(document).expect("serialized EWS XML should be valid UTF-8");

        // Assert the real EWS element/value names the type layer is responsible
        // for emitting. If the `ews` API or its serialization regressed, these
        // would not be present.
        assert!(
            xml.contains("GetFolder"),
            "serialized XML should contain the GetFolder operation element, got:\n{xml}"
        );
        assert!(
            xml.contains("IdOnly"),
            "serialized XML should contain the BaseShape value IdOnly, got:\n{xml}"
        );
        assert!(
            xml.contains("DistinguishedFolderId"),
            "serialized XML should contain a DistinguishedFolderId element, got:\n{xml}"
        );
        assert!(
            xml.contains("inbox"),
            "serialized XML should reference the inbox distinguished folder, got:\n{xml}"
        );
        assert!(
            xml.contains("Exchange2010_SP2"),
            "serialized XML should pin RequestServerVersion to Exchange2010_SP2, got:\n{xml}"
        );
    }
}
