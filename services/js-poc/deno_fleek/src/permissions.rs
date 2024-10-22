use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_fetch::FetchPermissions;
use deno_fs::FsPermissions;
use deno_io::fs::FsError;
use deno_net::NetPermissions;
use deno_node::NodePermissions;
use deno_web::TimersPermission;
use deno_websocket::WebSocketPermissions;
use tracing::debug;

pub const FETCH_BLACKLIST: &[&str] = &["localhost", "127.0.0.1", "::1"];
pub const BUILTIN_API_NOT_ALLOWED: &[&str] = &["node:os"];

pub struct Permissions {}

impl Permissions {
    fn check_net_url(&mut self, url: &Url, _api_name: &str) -> anyhow::Result<(), AnyError> {
        if let Some(host) = url.host_str() {
            if FETCH_BLACKLIST.contains(&host) {
                return Err(anyhow!("{host} is blacklisted"));
            }
        }
        Ok(())
    }
}

impl TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        false
    }
}

impl FetchPermissions for Permissions {
    fn check_net_url(&mut self, url: &Url, api_name: &str) -> anyhow::Result<(), AnyError> {
        self.check_net_url(url, api_name)
    }
    fn check_read<'a>(&mut self, _p: &'a Path, _api_name: &str) -> Result<Cow<'a, Path>, AnyError> {
        // Disable reading files via fetch
        Err(anyhow!("paths are disabled :("))
    }
}

impl WebSocketPermissions for Permissions {
    fn check_net_url(&mut self, url: &Url, api_name: &str) -> anyhow::Result<(), AnyError> {
        self.check_net_url(url, api_name)
    }
}

impl NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        host: &(T, Option<u16>),
        _api_name: &str,
    ) -> anyhow::Result<(), AnyError> {
        if FETCH_BLACKLIST.contains(&host.0.as_ref()) {
            Err(anyhow!("{} is blacklisted", host.0.as_ref()))
        } else {
            Ok(())
        }
    }
    fn check_read(&mut self, _p: &str, _api_name: &str) -> anyhow::Result<PathBuf, AnyError> {
        // Disable reading file descriptors
        Err(anyhow!("paths are disabled :("))
    }
    fn check_write(&mut self, _p: &str, _api_name: &str) -> anyhow::Result<PathBuf, AnyError> {
        // Disable writing file descriptors
        Err(anyhow!("paths are disabled :("))
    }

    fn check_write_path<'a>(
        &mut self,
        _p: &'a Path,
        _api_name: &str,
    ) -> Result<Cow<'a, Path>, AnyError> {
        Err(anyhow!("paths are disabled :("))
    }
}

impl FsPermissions for Permissions {
    fn check_open<'a>(
        &mut self,
        _resolved: bool,
        _read: bool,
        _write: bool,
        _path: &'a Path,
        _api_name: &str,
    ) -> Result<Cow<'a, Path>, FsError> {
        unimplemented!()
    }

    fn check_read(&mut self, _path: &str, _api_name: &str) -> Result<PathBuf, AnyError> {
        unimplemented!()
    }

    fn check_read_path<'a>(
        &mut self,
        _path: &'a Path,
        _api_name: &str,
    ) -> Result<Cow<'a, Path>, AnyError> {
        unimplemented!()
    }

    fn check_read_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
        unimplemented!()
    }

    fn check_read_blind(
        &mut self,
        _p: &Path,
        _display: &str,
        _api_name: &str,
    ) -> Result<(), AnyError> {
        unimplemented!()
    }

    fn check_write(&mut self, _path: &str, _api_name: &str) -> Result<PathBuf, AnyError> {
        unimplemented!()
    }

    fn check_write_path<'a>(
        &mut self,
        _path: &'a Path,
        _api_name: &str,
    ) -> Result<Cow<'a, Path>, AnyError> {
        unimplemented!()
    }

    fn check_write_partial(&mut self, _path: &str, _api_name: &str) -> Result<PathBuf, AnyError> {
        unimplemented!()
    }

    fn check_write_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
        unimplemented!()
    }

    fn check_write_blind(
        &mut self,
        _p: &Path,
        _display: &str,
        _api_name: &str,
    ) -> Result<(), AnyError> {
        unimplemented!()
    }
}

impl NodePermissions for Permissions {
    fn check_net_url(&mut self, _url: &Url, _api_name: &str) -> Result<(), AnyError> {
        unimplemented!()
    }

    fn check_read_with_api_name(
        &mut self,
        _path: &str,
        _api_name: Option<&str>,
    ) -> Result<PathBuf, AnyError> {
        unimplemented!()
    }

    fn check_read_path<'a>(&mut self, _path: &'a Path) -> Result<Cow<'a, Path>, AnyError> {
        unimplemented!()
    }

    fn query_read_all(&mut self) -> bool {
        unimplemented!()
    }

    fn check_sys(&mut self, kind: &str, api_name: &str) -> Result<(), AnyError> {
        debug!("checking sys for kind={kind} and api_name={api_name}");

        if BUILTIN_API_NOT_ALLOWED
            .iter()
            .any(|api| api_name.starts_with(*api))
        {
            return Err(anyhow!("{kind} is not allowed"));
        }

        Ok(())
    }

    fn check_write_with_api_name(
        &mut self,
        _path: &str,
        _api_name: Option<&str>,
    ) -> Result<PathBuf, AnyError> {
        unimplemented!()
    }
}
