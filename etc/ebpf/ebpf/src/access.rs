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
    task_struct,
};

#[allow(improper_ctypes)]
extern "C" {
    fn task_struct_mm(target: *const task_struct) -> *const *const mm_struct;
    fn cred_gid_val(target: *const cred) -> c_uint;
    fn cred_uid_val(target: *const cred) -> c_uint;
    fn dentry_i_ino(target: *const dentry) -> c_ulong;
    fn exe_file_inode(target: *const file) -> *const *const inode;
    fn file_dentry(target: *const file) -> *const dentry;
    fn file_inode(target: *const file) -> c_ulong;
    fn inode_i_ino(inode: *const inode) -> *const c_ulong;
    fn linux_binprm_argc(task: *const linux_binprm) -> c_int;
    fn mm_exe_file(target: *const mm_struct) -> *const *const file;
    fn sockaddr_in_sin_addr_s_addr(task: *const sockaddr_in) -> c_uint;
    fn sockaddr_in_sin_port(target: *const sockaddr_in) -> c_ushort;
    fn sockaddr_sa_family(task: *const sockaddr) -> c_ushort;
}
