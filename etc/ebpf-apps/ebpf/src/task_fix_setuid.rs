use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{File, GlobalConfig};

use crate::utils::{ALLOW, DENY};
use crate::{maps, utils};

#[lsm(hook = "task_fix_setuid")]
pub fn task_fix_setuid(ctx: LsmContext) -> i32 {
    unsafe { try_task_fix_setuid(ctx).unwrap_or_else(|e| e) }
}

pub unsafe fn try_task_fix_setuid(_ctx: LsmContext) -> Result<i32, i32> {
    let file = utils::get_file_from_current_task().map_err(|_| ALLOW)?;
    let inode = utils::read_file_inode(file).map_err(|_| ALLOW)?;
    let global_config = maps::GLOBAL_CONFIG.get(&0).ok_or(ALLOW)?;
    if maps::PROFILES.get(&File::new(inode)).is_some() {
        if global_config.mode == GlobalConfig::LEARN_MODE {
            // Send event.
            return Ok(ALLOW);
        } else {
            return Ok(DENY);
        }
    }

    Ok(ALLOW)
}
