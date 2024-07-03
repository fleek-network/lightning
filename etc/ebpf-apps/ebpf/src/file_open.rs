use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{
    File,
    FileCacheKey,
    FileRule,
    GlobalConfig,
    ACCESS_DENIED_EVENT,
    FILE_OPEN_PROG_ID,
    MAX_BUFFER_LEN,
};

use crate::utils::{ALLOW, DENY};
use crate::{access, maps, utils, vmlinux};

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|e| e) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let file: *const vmlinux::file = ctx.arg(0);
    let task_file = utils::get_file_from_current_task().map_err(|_| ALLOW)?;
    let task_inode = utils::read_file_inode(task_file).map_err(|_| ALLOW)?;

    if let Some(profile) = maps::PROFILES.get(&File::new(task_inode)) {
        let global_config = maps::GLOBAL_CONFIG.get(&0).ok_or(DENY)?;

        // Get the path for the target file.
        let buf = maps::BUFFERS.get_ptr_mut(&0).ok_or(DENY)?;
        let path = buf.as_mut().ok_or(DENY)?.as_mut_slice();
        utils::read_path(file, path)?;

        if global_config.mode == GlobalConfig::ENFORCE_MODE {
            // Get the inode of the target file.
            let target_inode = {
                let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))
                    .map_err(|_| DENY)?;
                aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))
                    .map_err(|_| DENY)?
            };

            // Check the cache first.
            let cache_key = &FileCacheKey {
                target: target_inode,
                task: task_inode,
            };
            if let Some(cached_rule) = maps::FILE_CACHE.get(&cache_key) {
                if utils::contains(cached_rule.path.as_slice(), path, MAX_BUFFER_LEN)
                    && cached_rule.permissions & FileRule::OPEN_MASK > 0
                {
                    return Ok(ALLOW);
                }
                // If we get here, it means that the file's inode number
                // has been assigned to a different file.
            }

            // Go through the rules in this profile and try to find a match.
            if let Some(rule) = utils::find_match(profile, path, FileRule::OPEN_MASK)? {
                let _ = maps::FILE_CACHE.insert(cache_key, rule, 0);
                return Ok(ALLOW);
            }
        }

        // The error may indicate that the ring buffer is full so there is nothing
        // left to do but let the consumer catch up.
        let _ = utils::send_event(ACCESS_DENIED_EVENT, FILE_OPEN_PROG_ID, task_file, path);

        return if global_config.mode == GlobalConfig::LEARN_MODE {
            Ok(ALLOW)
        } else {
            Ok(DENY)
        };
    }

    Ok(ALLOW)
}
