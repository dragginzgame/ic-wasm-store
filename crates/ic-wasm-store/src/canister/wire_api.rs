//! Generate Wire contracts from the mounted endpoints and verify their release Wasm exports.

use ic_wire::{
    Direction, Encoding, Limits,
    typescript::{Method, PayloadLimits, SchemaLimits, SchemaRegistry, Wire},
};
use std::{collections::BTreeSet, env, fs, path::PathBuf};
use wasmparser::{ExternalKind, Parser, Payload};

pub(super) struct Endpoint {
    name: &'static str,
    mode: &'static str,
    manifest: String,
}

impl Endpoint {
    pub(super) fn new<Request: Wire, Reply: Wire>(
        name: &'static str,
        mode: &'static str,
        id: u64,
        request: Limits,
        reply: Limits,
    ) -> Self {
        let mut registry = SchemaRegistry::new(SchemaLimits::default()).expect("schema budget");
        registry.register::<Request>("Request").expect(name);
        registry.register::<Reply>("Reply").expect(name);
        // Prefix codec identifiers to keep them valid JavaScript names.
        let codec = format!("wire_{name}");
        let method = Method {
            export: &codec,
            id,
            request: "Request",
            response: "Reply",
            request_limits: payload_limits(request, id, Direction::Request),
            response_limits: payload_limits(reply, id, Direction::Response),
        };
        let mut documents = registry
            .finish()
            .expect(name)
            .api_manifests(&[(name, method)])
            .expect(name);

        Self {
            name,
            mode,
            manifest: documents.remove(name).expect(name),
        }
    }
}

fn payload_limits(limits: Limits, method: u64, direction: Direction) -> PayloadLimits {
    let header = ic_wire::encoded_len(&(), Encoding::Frame { method, direction }, limits)
        .expect("an empty frame fits the endpoint budget");
    PayloadLimits {
        bytes: (limits.bytes - header) as u64,
        nodes: limits.nodes as u64,
        depth: limits.depth as u64,
    }
}

fn canister_exports(wasm: &[u8]) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.expect("parse release Wasm") {
            Payload::Version { encoding, .. } => assert_eq!(encoding, wasmparser::Encoding::Module),
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.expect("parse Wasm export");
                    if export.name.starts_with("canister_") {
                        assert_eq!(export.kind, ExternalKind::Func, "{}", export.name);
                        assert!(exports.insert(export.name.to_owned()), "{}", export.name);
                    }
                }
            }
            _ => {}
        }
    }
    exports
}

#[test]
#[ignore = "requires the release Wasm and an empty output directory"]
fn write_manifests() {
    let wasm = fs::read(env::var_os("STORE_WIRE_API_WASM").expect("release Wasm path"))
        .expect("read release Wasm");
    let output = PathBuf::from(env::var_os("STORE_WIRE_API_OUTPUT").expect("output directory"));
    assert!(
        fs::read_dir(&output)
            .expect("read output directory")
            .next()
            .is_none()
    );

    let endpoints = super::wire_endpoints();
    // The only canister exports that do not have Wire contracts are the IC inspection and lifecycle hooks.
    let mut expected = BTreeSet::from([
        "canister_init".to_owned(),
        "canister_inspect_message".to_owned(),
        "canister_post_upgrade".to_owned(),
    ]);
    for endpoint in &endpoints {
        let export = format!("canister_{} {}", endpoint.mode, endpoint.name);
        assert!(
            expected.insert(export),
            "duplicate export {}",
            endpoint.name
        );
    }

    let actual = canister_exports(&wasm);
    let unexpected: Vec<_> = actual.difference(&expected).collect();
    let missing: Vec<_> = expected.difference(&actual).collect();
    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "release Wasm export mismatch\nUnexpected: {unexpected:?}\nMissing: {missing:?}"
    );
    for endpoint in endpoints {
        let file = output.join(format!("{}-{}.json", endpoint.mode, endpoint.name));
        fs::write(file, endpoint.manifest).expect("write Wire manifest");
    }
}
