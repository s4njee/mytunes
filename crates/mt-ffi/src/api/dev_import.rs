//! Development-only library seeding from file picker paths.

use anyhow::Result;

use super::device::DeviceLibrary;

/// Outcome of copying selected audio into a development library.
pub struct DevImportReport {
    pub imported: u32,
    pub unchanged: u32,
    pub errors: Vec<String>,
}

impl DeviceLibrary {
    /// Import audio chosen by the debug build's native picker.
    pub fn import_dev_files(&self, paths: Vec<String>) -> Result<DevImportReport> {
        #[cfg(all(debug_assertions, feature = "dev-import"))]
        {
            let mut db = self.connection()?;
            let sources = paths
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            let report = mt_db::seed::seed_device(&mut db, &self.root, &sources)?;
            Ok(DevImportReport {
                imported: report.imported,
                unchanged: report.unchanged,
                errors: report.errors,
            })
        }
        #[cfg(not(all(debug_assertions, feature = "dev-import")))]
        {
            let _ = paths;
            anyhow::bail!("Development importing requires a debug build")
        }
    }
}
