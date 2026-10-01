// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::*;

#[test]
#[expect(
    clippy::used_underscore_binding,
    reason = "inspect the lifetime-owned extracted payload after public loading"
)]
fn signed_resources_round_trip() {
    let stage = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(stage.path().join("resources/drawable")).unwrap();
    std::fs::write(
        stage.path().join("manifest.toml"),
        "[bundle]\nname = 'example'\nformat_version = 1\n",
    )
    .unwrap();
    let resource = vec![0x5a; 200_000];
    std::fs::write(stage.path().join("resources/drawable/icon.png"), &resource).unwrap();
    let out = stage.path().join("example.reseam");
    pack(stage.path(), &SigningKey::from_bytes(&[42; 32]), &out).unwrap();
    let archive = BundleArchive::open(&out).unwrap();
    assert_eq!(
        archive.files().collect::<Vec<_>>(),
        ["resources/drawable/icon.png"]
    );
    let bundle = archive.load().unwrap();
    assert_eq!(
        std::fs::read(bundle._extracted.path().join("resources/drawable/icon.png")).unwrap(),
        resource
    );
}

fn resource_archive(path: &Path, name: &str, declared: &[u8], actual: &[u8]) {
    let key = SigningKey::from_bytes(&[42; 32]);
    let manifest = BundleManifest {
        bundle: BundleInfo {
            name: "example".into(),
            author: String::new(),
            description: String::new(),
            format_version: BUNDLE_FORMAT_VERSION,
            engine: ENGINE_VERSION.into(),
        },
        files: BTreeMap::from([(name.into(), hex::encode(Sha256::digest(declared)))]),
    };
    let manifest = toml::to_string(&manifest).unwrap();
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in [
        ("mimetype", BUNDLE_MIMETYPE.as_bytes()),
        ("manifest.toml", manifest.as_bytes()),
        ("manifest.pubkey", &key.verifying_key().to_bytes()),
        ("manifest.sig", &key.sign(manifest.as_bytes()).to_bytes()),
        (name, actual),
    ] {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn rejects_unverified_or_unsafe_resources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("example.reseam");
    for name in [
        "resources/../escape",
        "resources//icon",
        "resources/C:icon",
        "resources/CON.png",
        "resources/a\\b",
        "resources/a.",
    ] {
        resource_archive(&path, name, b"resource", b"resource");
        assert!(
            BundleArchive::open(&path).unwrap().load().is_err(),
            "{name}"
        );
    }
    resource_archive(
        &path,
        "resources/drawable/icon.png",
        b"expected",
        b"tampered",
    );
    assert!(BundleArchive::open(&path).unwrap().load().is_err());
}

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
mod indexed {
    use super::index::{
        Declaration, MemberKind,
        MemberKind::{Field, Method},
    };
    use super::*;
    use std::process::Command;

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        #[expect(
            clippy::large_include_file,
            reason = "compile declarations against the embedded engine runtime"
        )]
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let runtime = dir.path().join("runtime.jar");
            std::fs::write(
                &runtime,
                include_bytes!(concat!(env!("OUT_DIR"), "/runtime.jar")),
            )
            .unwrap();
            let source = dir.path().join("Declarations.java");
            std::fs::write(&source, r#"
package sample;
import app.reseam.patch.ReseamPatch;
import app.reseam.patch.PatchRuntime;
public final class Declarations {
    private static class TestPatch implements ReseamPatch {
        public String getName() { return "Indexed patch"; }
        public void execute(PatchRuntime runtime) {}
    }
    private static final ReseamPatch shared = new TestPatch();
    public static final ReseamPatch zAlias = shared;
    public static final ReseamPatch aAlias = shared;
    public static ReseamPatch getGood() { return shared; }
    public static ReseamPatch getGood(String unused) { throw new AssertionError("non-declaration overload invoked"); }
    public static ReseamPatch getNull() { return null; }
    public static ReseamPatch getThrows() { throw new IllegalStateException("declaration failure"); }
    public static ReseamPatch getBadMetadata() {
        return new TestPatch() {
            public String getName() { throw new IllegalStateException("metadata failure"); }
        };
    }
    public static ReseamPatch getNullMetadata() {
        return new TestPatch() {
            public java.util.List<ReseamPatch> getDependencies() { return null; }
        };
    }
    public static ReseamPatch getFresh() { return new TestPatch(); }
    public static ReseamPatch getNullElement() {
        return new TestPatch() {
            public java.util.List<ReseamPatch> getDependencies() { return java.util.Collections.singletonList(null); }
        };
    }
    public static String getUnindexed() { throw new AssertionError("unindexed member initialized"); }
}
"#).unwrap();
            let javac = std::env::var_os("JAVA_HOME").map_or_else(
                || PathBuf::from("javac"),
                |home| {
                    PathBuf::from(home).join("bin").join(if cfg!(windows) {
                        "javac.exe"
                    } else {
                        "javac"
                    })
                },
            );
            let status = Command::new(javac)
                .args(["--release", "17", "-classpath"])
                .arg(&runtime)
                .arg("-d")
                .arg(dir.path())
                .arg(source)
                .status()
                .unwrap();
            assert!(status.success(), "compile indexed bundle fixture");
            Self { dir }
        }

        fn archive(&self, members: &[(MemberKind, &str)]) -> BundleArchive {
            let stage = self.dir.path().join("stage");
            std::fs::create_dir_all(&stage).unwrap();
            let index = members
                .iter()
                .map(|(kind, name)| Declaration {
                    class_name: "sample.Declarations".into(),
                    owner: "sample.Declarations".into(),
                    member: (*name).into(),
                    kind: *kind,
                    id: format!("sample.{name}"),
                })
                .collect::<Vec<_>>();
            let mut jar = ZipWriter::new(File::create(stage.join("patches.jar")).unwrap());
            for file in std::fs::read_dir(self.dir.path().join("sample")).unwrap() {
                let path = file.unwrap().path();
                jar.start_file(
                    format!("sample/{}", path.file_name().unwrap().to_str().unwrap()),
                    SimpleFileOptions::default(),
                )
                .unwrap();
                std::io::copy(&mut File::open(path).unwrap(), &mut jar).unwrap();
            }
            jar.start_file("META-INF/reseam/patches.json", SimpleFileOptions::default())
                .unwrap();
            jar.write_all(&serde_json::to_vec(&index).unwrap()).unwrap();
            jar.start_file("classes.dex", SimpleFileOptions::default())
                .unwrap();
            jar.finish().unwrap();
            std::fs::write(
                stage.join("manifest.toml"),
                "[bundle]\nname = 'example'\nformat_version = 1\n",
            )
            .unwrap();
            let out = self.dir.path().join("example.reseam");
            pack(&stage, &SigningKey::from_bytes(&[42; 32]), &out).unwrap();
            BundleArchive::open(&out).unwrap()
        }
    }

    #[test]
    #[expect(
        clippy::used_underscore_binding,
        reason = "checks the lifetime of bundle payload after moving patches out"
    )]
    fn indexed_declarations_are_atomic_and_keep_aliases_alive() {
        let fixture = Fixture::new();
        let bundle = fixture
            .archive(&[(Field, "zAlias"), (Field, "aAlias"), (Method, "getGood")])
            .load()
            .unwrap();
        assert_eq!(bundle.patches().len(), 1);
        assert_eq!(bundle.patches()[0].spec().id, "sample.aAlias");
        let path = bundle._extracted.path().to_path_buf();
        let patches = bundle.into_patches();
        assert!(path.join("patches.jar").is_file());
        drop(patches);
        assert!(!path.exists());
        let repeated = fixture
            .archive(&[(Method, "getFresh"), (Method, "getFresh")])
            .load()
            .unwrap();
        assert_eq!(repeated.patches().len(), 1);
        for member in [
            "getNull",
            "getThrows",
            "getBadMetadata",
            "getNullMetadata",
            "getNullElement",
            "getMissing",
        ] {
            let archive = fixture.archive(&[(Method, "getGood"), (Method, member)]);
            assert!(archive.load().is_err(), "{member}");
        }
    }
}
