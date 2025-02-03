use deno_node::ExtNodeSys;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::{InNpmPackageChecker, NpmPackageFolderResolver};
/// Re-export mock memory sys bindings
pub use sys_traits::impls::InMemorySys;
use sys_traits::{BaseFsCanonicalize, BaseFsMetadata, BaseFsRead, EnvCurrentDir};

/// Simple implementation to disable npm package resolution
pub struct DisabledNpmChecker {}

impl NpmPackageFolderResolver for DisabledNpmChecker {
    fn resolve_package_folder_from_package(
        &self,
        specifier: &str,
        referrer: &deno_core::url::Url,
    ) -> Result<std::path::PathBuf, node_resolver::errors::PackageFolderResolveError> {
        Err(PackageFolderResolveError(Box::new(
            node_resolver::errors::PackageFolderResolveErrorKind::PackageNotFound(
                node_resolver::errors::PackageNotFoundError {
                    package_name: specifier.into(),
                    referrer: referrer.clone(),
                    referrer_extra: None,
                },
            ),
        )))
    }
}

impl InNpmPackageChecker for DisabledNpmChecker {
    fn in_npm_package(&self, _specifier: &deno_core::url::Url) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub struct InMemorySysWrapper(InMemorySys);

impl ExtNodeSys for InMemorySysWrapper {}

impl BaseFsCanonicalize for InMemorySysWrapper {
    fn base_fs_canonicalize(&self, path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        self.0.base_fs_canonicalize(path)
    }
}

impl BaseFsMetadata for InMemorySysWrapper {
    type Metadata = <InMemorySys as BaseFsMetadata>::Metadata;

    fn base_fs_metadata(&self, path: &std::path::Path) -> std::io::Result<Self::Metadata> {
        self.0.base_fs_metadata(path)
    }

    fn base_fs_symlink_metadata(&self, path: &std::path::Path) -> std::io::Result<Self::Metadata> {
        self.0.base_fs_symlink_metadata(path)
    }
}

impl BaseFsRead for InMemorySysWrapper {
    fn base_fs_read(
        &self,
        path: &std::path::Path,
    ) -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
        self.0.base_fs_read(path)
    }
}

impl EnvCurrentDir for InMemorySysWrapper {
    fn env_current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        self.0.env_current_dir()
    }
}
