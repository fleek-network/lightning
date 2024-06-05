use aya_ebpf::cty::c_long;
use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use aya_log_ebpf::info;
use lightning_ebpf_common::{File, FileRule, MAX_FILE_RULES};

use crate::{access, maps, vmlinux};

pub const ALLOW: i32 = 0;
pub const DENY: i32 = -1;
pub const MAX_PARENT_SEARCH_DEPTH: u8 = 2;

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|_| ALLOW) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, c_long> {
    let target_file: *const vmlinux::file = ctx.arg(0);

    let target_inode = {
        let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(target_file))?;
        aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))?
    };
    let task_inode = get_inode_from_current_task()?;

    if let Some(rule_list) = maps::FILE_RULES.get(&File::new(task_inode)) {
        info!(
            &ctx,
            "file_open attempt on {} by {}", target_inode, task_inode
        );

        // Todo: let's put this log behind a flag as it's for debugging.
        let pid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
        info!(
            &ctx,
            "Process {} running bin {} attempting to open file", pid, task_inode
        );

        if rule_list
            .rules
            .iter()
            .find(|rule| rule.inode == target_inode)
            .map(|rule| rule.permissions & FileRule::OPEN_MASK > 0)
            .unwrap_or(false)
            || verify_parent(target_file, &rule_list.rules, FileRule::OPEN_MASK)?
        {
            return Ok(ALLOW);
        } else {
            // Todo: Send event about access that was not accounted for.
            return Ok(DENY);
        }
    }

    Ok(ALLOW)
}

/// Returns true if there is a matching rule for
/// one of the target file's parents, and false otherwise.
unsafe fn verify_parent(
    file: *const vmlinux::file,
    rules: &[FileRule; MAX_FILE_RULES],
    mask: u32,
) -> Result<bool, c_long> {
    let dentry = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_dentry(file))?;
    let mut next_parent_dentry =
        aya_ebpf::helpers::bpf_probe_read_kernel(access::dentry_d_parent(dentry))?;

    for _ in 0..MAX_PARENT_SEARCH_DEPTH {
        if next_parent_dentry.is_null() {
            break;
        }

        let parent_inode = {
            let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::dentry_d_inode(
                next_parent_dentry,
            ))?;
            aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))?
        };

        if rules
            .iter()
            .find(|rule| rule.inode == parent_inode)
            .map(|rule| rule.permissions & mask > 0)
            .unwrap_or(false)
        {
            return Ok(true);
        }

        next_parent_dentry =
            aya_ebpf::helpers::bpf_probe_read_kernel(access::dentry_d_parent(next_parent_dentry))?;
    }

    Ok(false)
}

/// Get the inode number of the current process's binary file.
unsafe fn get_inode_from_current_task() -> Result<u64, c_long> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    let file = aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))?;
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))
}
