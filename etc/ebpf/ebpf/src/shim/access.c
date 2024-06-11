#include "vmlinux.h"

struct mm_struct ** task_struct_mm(struct task_struct *task) {
	return __builtin_preserve_access_index(&task->mm);
}

pid_t task_struct_pid(struct task_struct *task) {
	return __builtin_preserve_access_index(task->pid);
}

pid_t task_struct_tgid(struct task_struct *task) {
	return __builtin_preserve_access_index(task->tgid);
}

struct file ** mm_exe_file(struct mm_struct *target) {
	return __builtin_preserve_access_index(&target->exe_file);
}

struct inode ** file_inode(struct file *target) {
	return __builtin_preserve_access_index(&target->f_inode);
}

struct path * file_f_path(struct file *target) {
	return __builtin_preserve_access_index(&target->f_path);
}

struct dentry ** file_dentry(struct file *target) {
	return __builtin_preserve_access_index(&target->f_path.dentry);
}

struct dentry ** dentry_d_parent(struct dentry *target) {
	return __builtin_preserve_access_index(&target->d_parent);
}

struct inode ** dentry_d_inode(struct dentry *target) {
	return __builtin_preserve_access_index(&target->d_inode);
}

const unsigned char ** dentry_d_name_name(struct dentry *target) {
	return __builtin_preserve_access_index(&target->d_name.name);
}

//struct qstr * dentry_d_name(struct dentry *target) {
//	return __builtin_preserve_access_index(&target->d_name);
//}

uint64_t * inode_i_ino(struct inode *target) {
	return __builtin_preserve_access_index(&target->i_ino);
}

struct super_block * inode_i_sb(struct inode *target) {
	return __builtin_preserve_access_index(&target->i_sb);
}

int32_t linux_binprm_argc(struct linux_binprm *target) {
	return __builtin_preserve_access_index(target->argc);
}

int16_t sockaddr_sa_family(struct sockaddr *target) {
	return __builtin_preserve_access_index(target->sa_family);
}

uint32_t sockaddr_in_sin_addr_s_addr(struct sockaddr_in *target) {
	return __builtin_preserve_access_index(target->sin_addr.s_addr);
}

uint16_t sockaddr_in_sin_port(struct sockaddr_in *target) {
	return __builtin_preserve_access_index(target->sin_port);
}

uid_t cred_uid_val(struct cred *target) {
	return __builtin_preserve_access_index(target->uid.val);
}

uid_t cred_gid_val(struct cred *target) {
	return __builtin_preserve_access_index(target->gid.val);
}

uint32_t * super_block_s_dev(struct super_block *target) {
    return __builtin_preserve_access_index(&target->s_dev);
}
