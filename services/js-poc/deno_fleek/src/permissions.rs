use std::borrow::Cow;
use std::path::Path;

use deno_core::url::Url;
use deno_fetch::FetchPermissions;
use deno_fs::FsPermissions;
use deno_io::fs::FsError;
use deno_net::NetPermissions;
use deno_node::NodePermissions;
use deno_permissions::PermissionCheckError;
use deno_web::TimersPermission;
use deno_websocket::WebSocketPermissions;

pub const FETCH_BLACKLIST: &[&str] = &["localhost", "127.0.0.1", "::1"];

pub struct Permissions {}

impl Permissions {
    fn check_net(&mut self, host: &str) -> Result<(), PermissionCheckError> {
        if FETCH_BLACKLIST.contains(&host) {
            Err(PermissionCheckError::PermissionDenied(
                deno_permissions::PermissionDeniedError::Fatal {
                    access: "blacklisted".into(),
                },
            ))
        } else {
            Ok(())
        }
    }

    fn check_net_url(&mut self, url: &Url, _api_name: &str) -> Result<(), PermissionCheckError> {
        if let Some(host) = url.host_str() {
            self.check_net(host)?;
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
    fn check_net_url(&mut self, url: &Url, api_name: &str) -> Result<(), PermissionCheckError> {
        self.check_net_url(url, api_name)
    }
    fn check_read<'a>(
        &mut self,
        _p: &'a Path,
        _api_name: &str,
    ) -> Result<Cow<'a, std::path::Path>, PermissionCheckError> {
        // Disable reading files via fetch
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }
}

impl WebSocketPermissions for Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        self.check_net_url(url, api_name)
    }
}

impl NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        host: &(T, Option<u16>),
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        // Blacklist hosts
        if FETCH_BLACKLIST.contains(&host.0.as_ref()) {
            Err(PermissionCheckError::PermissionDenied(
                deno_permissions::PermissionDeniedError::Fatal {
                    access: format!("{} not allowed", host.0.as_ref()),
                },
            ))
        } else {
            Ok(())
        }
    }
    fn check_read(
        &mut self,
        _p: &str,
        _api_name: &str,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        // Disable reading file descriptors
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }
    fn check_write(
        &mut self,
        _p: &str,
        _api_name: &str,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        // Disable writing file descriptors
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_path<'a>(
        &mut self,
        _p: &'a Path,
        _api_name: &str,
    ) -> std::result::Result<
        std::borrow::Cow<'a, std::path::Path>,
        deno_permissions::PermissionCheckError,
    > {
        // Disable all write paths
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
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

    fn check_read(
        &mut self,
        _path: &str,
        _api_name: &str,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_read_path<'a>(
        &mut self,
        _path: &'a Path,
        _api_name: &str,
    ) -> std::result::Result<
        std::borrow::Cow<'a, std::path::Path>,
        deno_permissions::PermissionCheckError,
    > {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_read_all(
        &mut self,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_read_blind(
        &mut self,
        _p: &Path,
        _display: &str,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write(
        &mut self,
        _path: &str,
        _api_name: &str,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_path<'a>(
        &mut self,
        _path: &'a Path,
        _api_name: &str,
    ) -> std::result::Result<
        std::borrow::Cow<'a, std::path::Path>,
        deno_permissions::PermissionCheckError,
    > {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_partial(
        &mut self,
        _path: &str,
        _api_name: &str,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_all(
        &mut self,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_blind(
        &mut self,
        _p: &Path,
        _display: &str,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }
}

impl NodePermissions for Permissions {
    fn check_net_url(
        &mut self,
        _url: &Url,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_read_with_api_name(
        &mut self,
        _path: &str,
        _api_name: Option<&str>,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_read_path<'a>(
        &mut self,
        _path: &'a Path,
    ) -> std::result::Result<
        std::borrow::Cow<'a, std::path::Path>,
        deno_permissions::PermissionCheckError,
    > {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn query_read_all(&mut self) -> bool {
        unimplemented!()
    }

    fn check_sys(
        &mut self,
        _kind: &str,
        _api_name: &str,
    ) -> std::result::Result<(), deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_write_with_api_name(
        &mut self,
        _path: &str,
        _api_name: Option<&str>,
    ) -> std::result::Result<std::path::PathBuf, deno_permissions::PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError::Fatal {
                access: "not allowed".into(),
            },
        ))
    }

    fn check_net(
        &mut self,
        (host, _port): (&str, Option<u16>),
        _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
        self.check_net(host)
    }
}
