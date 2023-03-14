use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Result, Write};
use std::path::Path;

/// Temporary structure, created when reading a file containing OID declarations
#[derive(Debug)]
pub struct LoadedEntry {
    /// Name of the global constant for this entry.
    ///
    /// If `name` is "", then no global constant is defined
    pub name: String,
    /// Textual representation of OID (ex: 2.5.4.3)
    pub oid: String,
    /// A short name to describe OID. Should be unique (no check is done)
    pub sn: String,
    /// A description for this entry
    pub description: String,
}

/// Temporary structure, created when reading a file containing OID declarations
pub type LoadedMap = BTreeMap<String, Vec<LoadedEntry>>;

/// Load a file to an OID description map
///
/// format of the file: tab-separated values
/// <pre>
/// feature   name   oid   short_name   description (until end of line)
/// </pre>
///
/// `name` is used to declare a global constant when creating output file (see `generate_file`).
/// If `name` is "" then no constant will be written
///
pub fn load_file<P: AsRef<Path>>(path: P) -> Result<LoadedMap> {
    let mut map = BTreeMap::new();

    let file = File::open(path)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // split by tabs
        let mut iter = line.splitn(5, '\t');
        let feature = iter.next().expect("invalid oid_db format: missing feature").replace('-', "_");
        let name = iter.next().expect("invalid oid_db format: missing name").to_string();
        let oid = iter.next().expect("invalid oid_db format: missing OID").to_string();
        let sn = iter.next().expect("invalid oid_db format: missing short name").to_string();
        let description = iter.next().expect("invalid oid_db format: missing description").to_string();

        let entry = LoadedEntry {
            name,
            oid,
            sn,
            description,
        };

        let v = map.entry(feature.to_string()).or_insert_with(Vec::new);

        v.push(entry);
    }
    Ok(map)
}

/// Generate a file containing a `with_<feat>` method for OidRegistry
pub fn generate_file<P: AsRef<Path>>(map: &LoadedMap, dest_path: P) -> Result<()> {
    let mut out_file = File::create(&dest_path)?;
    for feat_entries in map.values() {
        for v in feat_entries {
            if v.name != "\"\"" {
                writeln!(out_file, "/// {}", v.oid)?;
                writeln!(out_file, "pub const {}: Oid<'static> = oid!({});", v.name, v.oid)?;
            }
        }
    }
    writeln!(out_file)?;
    writeln!(out_file, r#"#[cfg(feature = "registry")]"#)?;
    writeln!(out_file, r#"#[cfg_attr(docsrs, doc(cfg(feature = "registry")))]"#)?;
    writeln!(out_file, "impl<'a> OidRegistry<'a> {{")?;
    for (k, v) in map {
        writeln!(out_file, r#"    #[cfg(feature = "{}")]"#, k)?;
        writeln!(out_file, r#"    #[cfg_attr(docsrs, doc(cfg(feature = "{}")))]"#, k)?;
        writeln!(
            out_file,
            r#"    #[doc = "Load all known OIDs for feature `{}` in the registry."]"#,
            k
        )?;
        writeln!(out_file, "    pub fn with_{}(mut self) -> Self {{", k)?;
        for item in v {
            writeln!(
                out_file,
                r#"        self.insert(oid!({}), OidEntry::new("{}", "{}"));"#,
                item.oid, item.sn, item.description
            )?;
        }
        writeln!(out_file, "        self")?;
        writeln!(out_file, "    }}")?;
        writeln!(out_file)?;
    }
    writeln!(out_file, "}}")?;
    Ok(())
}
