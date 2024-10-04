// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_core::error::AnyError;
use deno_core::url::Url;

mod ops;

pub trait NodePermissions {
    fn check_net_url(&mut self, url: &Url, api_name: &str) -> Result<(), AnyError>;
    #[inline(always)]
    fn check_read(&mut self, path: &Path) -> Result<(), AnyError> {
        self.check_read_with_api_name(path, None)
    }
    fn check_read_with_api_name(
        &mut self,
        path: &Path,
        api_name: Option<&str>,
    ) -> Result<(), AnyError>;
    fn check_sys(&mut self, kind: &str, api_name: &str) -> Result<(), AnyError>;
    fn check_write_with_api_name(
        &mut self,
        path: &Path,
        api_name: Option<&str>,
    ) -> Result<(), AnyError>;
}
