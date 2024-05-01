#![allow(unused)]
use aya_bpf::cty::{c_int, c_uint, c_ulong, c_ushort};

use crate::vmlinux::{
    cred,
    dentry,
    file,
    inode,
    linux_binprm,
    mm_struct,
    sockaddr,
    sockaddr_in,
    super_block,
    task_struct,
};

#[allow(improper_ctypes)]
extern "C" {
    pub fn task_struct_mm(target: *const task_struct) -> *const *const mm_struct;
    pub fn cred_gid_val(target: *const cred) -> c_uint;
    pub fn cred_uid_val(target: *const cred) -> c_uint;
    pub fn dentry_i_ino(target: *const dentry) -> c_ulong;
    pub fn file_inode(target: *const file) -> *const *const inode;
    pub fn file_f_path_dentry_inode(target: *const file) -> c_ulong;
    pub fn file_dentry(target: *const file) -> *const dentry;
    pub fn inode_i_ino(inode: *const inode) -> *const c_ulong;
    pub fn inode_i_sb(inode: *const inode) -> *const *const super_block;
    pub fn linux_binprm_argc(task: *const linux_binprm) -> c_int;
    pub fn mm_exe_file(target: *const mm_struct) -> *const *const file;
    pub fn sockaddr_in_sin_addr_s_addr(task: *const sockaddr_in) -> c_uint;
    pub fn sockaddr_in_sin_port(target: *const sockaddr_in) -> c_ushort;
    pub fn sockaddr_sa_family(task: *const sockaddr) -> c_ushort;
    pub fn super_block_s_dev(sb: *const super_block) -> *const c_uint;
}
