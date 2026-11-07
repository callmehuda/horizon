//! The pinned Minecraft's files, as the browser can download them: the version JSON and the
//! client jar and the logging config and the asset index — the parts of
//! launcher/src/mojang.rs's `download` whose Mojang hosts let a browser through
//! (piston-meta.mojang.com and piston-data.mojang.com allow cross-origin requests;
//! libraries.minecraft.net and resources.download.minecraft.net, the libraries and the
//! sounds, do not, so the desktop launcher's *Download directly* finishes those).
//!
//! Every file is checked against the sha1 Mojang lists, as the desktop launcher checks them,
//! and written whole (the File System Access API's writable stream replaces the file only
//! when it closes): a download stopped half way keeps the files it finished, and the next one
//! fetches only the rest.

use serde_json::Value;

/// Where the pinned version JSON lives, as `fetch_manifest` builds its URL (sources.rs): the
/// pin's sha1 is the path.
pub fn version_file_url(version: &str, sha1: &str) -> String {
    format!("https://piston-meta.mojang.com/v1/packages/{sha1}/{version}.json")
}

/// One file to download: its url, the sha1 Mojang lists for it, its size when it lists one,
/// and its path in the official launcher's layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadFile {
    pub url: String,
    pub sha1: String,
    pub size: Option<u64>,
    pub path: String,
}

impl DownloadFile {
    /// The file with the version JSON's own url and sha1 (the first sha1 `Pins` holds).
    pub fn version_json(version: &str, manifests: &[&str]) -> DownloadFile {
        DownloadFile {
            url: version_file_url(version, manifests[0]),
            sha1: manifests[0].to_string(),
            size: None,
            path: format!("versions/{version}/{version}.json"),
        }
    }
}

/// The pinned version's other files, from its version JSON: the client jar (its sha1 the pin
/// holds, checked as `download` checks it), the logging config the game is started with, and
/// the asset index. `libraries` and the asset `objects` stay for the desktop launcher: Mojang
/// allows a browser neither.
pub fn plan(version_json: &str, client_sha1: &str) -> Result<Vec<DownloadFile>, String> {
    let manifest: Value =
        serde_json::from_str(version_json).map_err(|e| format!("the version file: {e}"))?;
    let version = relative(
        manifest["id"]
            .as_str()
            .ok_or("the version file has no id")?,
    )?;
    let client = &manifest["downloads"]["client"];
    let jar = DownloadFile {
        url: text(client, "url", "downloads.client")?,
        // The pin's sha1 for the pinned version, as `download` takes it.
        sha1: if client_sha1.is_empty() {
            text(client, "sha1", "downloads.client")?
        } else {
            client_sha1.to_string()
        },
        size: client["size"].as_u64(),
        path: format!("versions/{version}/{version}.jar"),
    };
    // The logging setup the game is started with (`logging.client.argument`): for 1.7 to 1.18,
    // Mojang's fixed one is what keeps Log4Shell out.
    let logging = &manifest["logging"]["client"]["file"];
    let log_config = if logging.is_null() {
        None
    } else {
        Some(DownloadFile {
            url: text(logging, "url", "logging.client.file")?,
            sha1: text(logging, "sha1", "logging.client.file")?,
            size: logging["size"].as_u64(),
            path: format!(
                "assets/log_configs/{}",
                relative(&text(logging, "id", "logging.client.file")?)?
            ),
        })
    };
    let index = &manifest["assetIndex"];
    let index = DownloadFile {
        url: text(index, "url", "assetIndex")?,
        sha1: text(index, "sha1", "assetIndex")?,
        size: index["size"].as_u64(),
        path: format!(
            "assets/indexes/{}.json",
            relative(&text(index, "id", "assetIndex")?)?
        ),
    };
    Ok(match log_config {
        Some(config) => vec![jar, config, index],
        None => vec![jar, index],
    })
}

fn text(v: &Value, key: &str, where_: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("the version file has no {where_}.{key}"))
}

/// A path from the version file, refused if it could leave the folder — `relative` (mojang.rs)
/// for the browser, where every path is written inside the folder the player picked. `/` and
/// `\` are both separators (the player's folder may be opened on any system), and a name with
/// a `:` (a drive, an NTFS stream) never becomes a directory.
pub fn relative(path: &str) -> Result<String, String> {
    let refuse = || format!("unsafe path {path:?} in the version file");
    if path.starts_with(['/', '\\']) {
        return Err(refuse());
    }
    let mut parts = Vec::new();
    let mut rest = path;
    while !rest.is_empty() {
        let (head, tail) = match rest.split_once(['/', '\\']) {
            Some((head, tail)) => (head, tail),
            None => (rest, ""),
        };
        match head {
            "" | "." => {}
            ".." => return Err(refuse()),
            part if part.contains(':') => return Err(refuse()),
            part => parts.push(part),
        }
        rest = tail;
    }
    (!parts.is_empty())
        .then(|| parts.join("/"))
        .ok_or_else(refuse)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
      "id": "26.3",
      "downloads": {"client": {
        "url": "https://piston-data.mojang.com/v1/objects/e877b6a07acd633fb3bb475002175cec036e7b87/client.jar",
        "sha1": "wrong-by-the-manifest", "size": 40459595
      }},
      "logging": {"client": {"file": {
        "url": "https://piston-data.mojang.com/v1/objects/7aa46c519a3d42dc1b80e339a7b3a/colors.json",
        "sha1": "7aa46c519a3d42dc1b80e339a7b3a", "id": "client-1.12.xml"
      }}},
      "assetIndex": {
        "url": "https://piston-meta.mojang.com/v1/packages/331e/34.json",
        "sha1": "331e5c80937c9cd79647ab34bdc1cbb5ef09f456", "id": "34"
      }
    }"#;

    #[test]
    fn the_pin_decides_the_client_jars_sha1_and_the_paths_follow_the_official_layout() {
        let files = plan(MANIFEST, "e877b6a07acd633fb3bb475002175cec036e7b87").unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "versions/26.3/26.3.jar",
                "assets/log_configs/client-1.12.xml",
                "assets/indexes/34.json"
            ]
        );
        // The pin's sha1, not the version file's own (download checks it the same way).
        assert_eq!(files[0].sha1, "e877b6a07acd633fb3bb475002175cec036e7b87");
        assert_eq!(
            files[0].url,
            "https://piston-data.mojang.com/v1/objects/e877b6a07acd633fb3bb475002175cec036e7b87/client.jar"
        );
        assert_eq!(files[0].size, Some(40_459_595));
        assert_eq!(files[2].sha1, "331e5c80937c9cd79647ab34bdc1cbb5ef09f456");
    }

    #[test]
    fn a_version_without_logging_leaves_its_config_out() {
        let without = MANIFEST.replace("logging", "unused");
        let files = plan(&without, "").unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["versions/26.3/26.3.jar", "assets/indexes/34.json"]);
        // No pin: the version file's own sha1 for the client jar.
        assert_eq!(files[0].sha1, "wrong-by-the-manifest");
    }

    #[test]
    fn the_paths_the_version_file_names_stay_inside_the_folder() {
        assert_eq!(relative("client-1.12.xml").unwrap(), "client-1.12.xml");
        assert_eq!(relative("a/b/c.json").unwrap(), "a/b/c.json");
        assert_eq!(relative("./x").unwrap(), "x");
        assert_eq!(relative("a\\b.txt").unwrap(), "a/b.txt");
        for bad in [
            "..",
            "../x",
            "a/../../x",
            "/abs",
            "\\abs",
            "C:/x",
            "C:",
            "a/b:c",
            "",
        ] {
            assert!(relative(bad).is_err(), "{bad:?} should be refused");
        }
        // What the version file names is refused too, not just the ids on their own.
        for hostile in [r#"{"id":"26.3"}"#, r#"{"id":".."}"#] {
            assert!(plan(hostile, "").is_err());
        }
        let bad_index = r#"{
          "id": "26.3",
          "downloads": {"client": {"url": "u", "sha1": "s"}},
          "assetIndex": {"url": "u", "sha1": "s", "id": "../evil"}
        }"#;
        assert!(plan(bad_index, "").unwrap_err().contains("unsafe path"));
    }

    #[test]
    fn the_version_files_url_is_pinned_by_its_sha1() {
        assert_eq!(
            version_file_url("26.3", "abc"),
            "https://piston-meta.mojang.com/v1/packages/abc/26.3.json"
        );
        let file = DownloadFile::version_json("26.3", &["abc", "def"]);
        assert_eq!(file.path, "versions/26.3/26.3.json");
        assert_eq!(file.sha1, "abc");
    }

    #[test]
    fn a_version_file_without_the_client_names_the_missing_part() {
        let broken = r#"{"id": "26.3"}"#;
        assert!(plan(broken, "")
            .unwrap_err()
            .contains("downloads.client.url"));
    }
}
