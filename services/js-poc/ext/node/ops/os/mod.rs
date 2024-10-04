// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod cpus;

use deno_core::error::AnyError;
use deno_core::{op2, OpState};

use crate::ext::node::NodePermissions;

#[op2(fast)]
pub fn op_node_os_get_priority<P>(state: &mut OpState, pid: u32) -> Result<i32, AnyError>
where
    P: NodePermissions + 'static,
{
    unimplemented!()
}

#[op2(fast)]
pub fn op_node_os_set_priority<P>(
    state: &mut OpState,
    pid: u32,
    priority: i32,
) -> Result<(), AnyError>
where
    P: NodePermissions + 'static,
{
    unimplemented!()
}

#[op2]
#[string]
pub fn op_node_os_username<P>(state: &mut OpState) -> Result<String, AnyError>
where
    P: NodePermissions + 'static,
{
    unimplemented!()
}

#[op2(fast)]
pub fn op_geteuid(_state: &mut OpState) -> Result<u32, AnyError> {
    unimplemented!()
}

#[op2(fast)]
pub fn op_getegid(_state: &mut OpState) -> Result<u32, AnyError> {
    unimplemented!()
}

#[op2]
#[serde]
pub fn op_cpus<P>(state: &mut OpState) -> Result<Vec<cpus::CpuInfo>, AnyError>
where
    P: NodePermissions + 'static,
{
    unimplemented!()
}

#[op2(fast)]
pub fn op_homedir(_state: &mut OpState) {
    unimplemented!()
}
