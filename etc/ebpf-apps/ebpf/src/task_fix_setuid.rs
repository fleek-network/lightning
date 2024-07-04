use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{
    File,
    GlobalConfig,
    LEARNING_MODE_EVENT,
    MAX_BUFFER_LEN,
    TASK_FIX_SETUID_PROG_ID,
};

use crate::utils::{ALLOW, DENY};
use crate::vmlinux::cred;
use crate::{access, maps, utils};

#[lsm(hook = "task_fix_setuid")]
pub fn task_fix_setuid(ctx: LsmContext) -> i32 {
    unsafe { try_task_fix_setuid(ctx).unwrap_or_else(|e| e) }
}

pub unsafe fn try_task_fix_setuid(ctx: LsmContext) -> Result<i32, i32> {
    // Get the current process's binary file information.
    let file = utils::get_file_from_current_task().map_err(|_| ALLOW)?;
    let inode = utils::read_file_inode(file).map_err(|_| ALLOW)?;

    // Check the current mode that we're running on.
    let global_config = maps::GLOBAL_CONFIG.get(&0).ok_or(ALLOW)?;
    if maps::PROFILES.get(&File::new(inode)).is_some() {
        if global_config.mode == GlobalConfig::LEARN_MODE {
            let buf = maps::BUFFERS.get_ptr_mut(&1).ok_or(DENY)?;
            let scratch_buf = buf.as_mut().ok_or(ALLOW)?.as_mut_slice();

            // Send event about this operation.
            // New credentials.
            let new: *const cred = ctx.arg(0);
            let uid = access::cred_uid_val(new);
            let gid = access::cred_gid_val(new);
            write_uint(scratch_buf, uid, 0)?;
            write_uint(scratch_buf, gid, 4)?;

            utils::send_event(
                LEARNING_MODE_EVENT,
                TASK_FIX_SETUID_PROG_ID,
                file,
                scratch_buf,
            );
            return Ok(ALLOW);
        } else {
            return Ok(DENY);
        }
    }

    Ok(ALLOW)
}

fn write_uint(dst: &mut [u8], src: u32, start: usize) -> Result<(), i32> {
    if start + 4 > MAX_BUFFER_LEN {
        return Err(DENY);
    }

    let bytes = src.to_le_bytes();
    for i in start..start + 4 {
        dst[i] = bytes[i];
    }

    Ok(())
}
