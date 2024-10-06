use std::borrow::Cow;
use std::path::Path;

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
use deno_napi::NapiPermissions;

use crate::params::FETCH_BLACKLIST;

pub struct Permissions {}

impl Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        _api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
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
    fn check_net_url(
        &mut self,
        url: &Url,
        api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        self.check_net_url(url, api_name)
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        // Disable reading files via fetch
        Err(anyhow!("paths are disabled :("))
    }
}

impl WebSocketPermissions for Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        self.check_net_url(url, api_name)
    }
}

impl NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        host: &(T, Option<u16>),
        _api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        if FETCH_BLACKLIST.contains(&host.0.as_ref()) {
            Err(anyhow!("{} is blacklisted", host.0.as_ref()))
        } else {
            Ok(())
        }
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        // Disable reading file descriptors
        Err(anyhow!("paths are disabled :("))
    }
    fn check_write(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> anyhow::Result<(), deno_core::error::AnyError> {
        // Disable writing file descriptors
        Err(anyhow!("paths are disabled :("))
    }
}

impl FsPermissions for Permissions {
    fn check_open<'a>(
        &mut self,
        resolved: bool,
        read: bool,
        write: bool,
        path: &'a Path,
        api_name: &str,
    ) -> std::result::Result<Cow<'a, Path>, FsError> {
        todo!()
    }

    fn check_read(&mut self, path: &Path, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_read_all(&mut self, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_read_blind(
        &mut self,
        p: &Path,
        display: &str,
        api_name: &str,
    ) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_write(&mut self, path: &Path, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_write_partial(
        &mut self,
        path: &Path,
        api_name: &str,
    ) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_write_all(&mut self, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_write_blind(
        &mut self,
        p: &Path,
        display: &str,
        api_name: &str,
    ) -> std::result::Result<(), AnyError> {
        todo!()
    }
}

impl NodePermissions for Permissions {
    fn check_net_url(&mut self, url: &Url, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_read_with_api_name(
        &mut self,
        path: &Path,
        api_name: Option<&str>,
    ) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_sys(&mut self, kind: &str, api_name: &str) -> std::result::Result<(), AnyError> {
        todo!()
    }

    fn check_write_with_api_name(
        &mut self,
        path: &Path,
        api_name: Option<&str>,
    ) -> std::result::Result<(), AnyError> {
        todo!()
    }
}

impl NapiPermissions for Permissions {
    fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
        todo!()
    }
}