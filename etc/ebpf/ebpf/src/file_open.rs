use aya_ebpf::cty::c_long;
use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use aya_log_ebpf::info;
use common::File;

use crate::{access, maps, vmlinux};

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|_| 0) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, c_long> {
    let ctx_file: *const vmlinux::file = ctx.arg(0);
    let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(ctx_file))?;
    let inode_n = aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))?;

    info!(&ctx, "file_open attempt on {}", inode_n);

    // Todo: Get device ID.
    let file = File {
        inode: inode_n,
        dev: 0,
    };

    verify_permission(&ctx, &file)
}

unsafe fn verify_permission(ctx: &LsmContext, file: &File) -> Result<i32, c_long> {
    let binfile = get_current_process_binfile()?;
    let pid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
    info!(
        ctx,
        "Process {} running bin {} attempting to open file", pid, binfile.inode
    );

    if let Some(rule_list) = maps::FILE_RULES.get(&binfile) {
        if binfile.dev == file.dev {
            if let Some(rule) = rule_list.rules.iter().find(|rule| rule.inode == file.inode) {
                return Ok(rule.allow);
            }
        }
    }

    // Todo: Send event about access that was not accounted for.
    Ok(0)
}

unsafe fn get_current_process_binfile() -> Result<File, c_long> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    let file = aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))?;
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    // Get the inode number.
    let i_ino = aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))?;
    // Get the device ID from the SuperBlock obj.
    let super_block = aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_sb(f_inode))?;
    let dev = aya_ebpf::helpers::bpf_probe_read_kernel(access::super_block_s_dev(super_block))?;
    Ok(File { inode: i_ino, dev })
}
