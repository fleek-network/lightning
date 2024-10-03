// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Serialize;

#[derive(Debug, Default, Serialize, Clone)]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub sys: u64,
    pub idle: u64,
    pub irq: u64,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct CpuInfo {
    pub model: String,
    /* in MHz */
    pub speed: u64,
    pub times: CpuTimes,
}

impl CpuInfo {
    pub fn new() -> Self {
        Self::default()
    }
}
