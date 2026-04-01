use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use crate::ui::app;

/// 命令执行结果 (FFI)
#[repr(C)]
pub struct CommandResult {
    pub exit_code: i32,
    pub output: *const c_char,
    pub error: *const c_char,
}

/// 配置数据 (FFI)
#[repr(C)]
pub struct ConfigData {
    pub json_data: *const c_char,
    pub length: i32,
}

/// 备份列表 (FFI)
#[repr(C)]
pub struct BackupList {
    pub paths: *const *const c_char,
    pub count: i32,
}

/// 解析后的命令 (FFI)
#[repr(C)]
pub struct ParsedCommand {
    pub commands: *const *const c_char,
    pub count: i32,
}

// ============================================================================
// 命令执行 FFI
// ============================================================================

/// 执行命令
#[no_mangle]
pub extern "C" fn rust_execute_command(
    program: *const c_char,
    working_dir: *const c_char,
    command_str: *const c_char,
) -> CommandResult {
    if program.is_null() {
        return CommandResult {
            exit_code: -1,
            output: CString::new("Program is null").unwrap().into_raw(),
            error: CString::new("Invalid program").unwrap().into_raw(),
        };
    }

    let program_str = match unsafe { CStr::from_ptr(program).to_str() } {
        Ok(s) => s,
        Err(_) => {
            return CommandResult {
                exit_code: -1,
                output: ptr::null(),
                error: CString::new("Invalid program encoding").unwrap().into_raw(),
            };
        }
    };

    let working_dir_str = if !working_dir.is_null() {
        match unsafe { CStr::from_ptr(working_dir).to_str() } {
            Ok(s) => s,
            Err(_) => "",
        }
    } else {
        ""
    };

    let cmd_str = if !command_str.is_null() {
        match unsafe { CStr::from_ptr(command_str).to_str() } {
            Ok(s) => s,
            Err(_) => "",
        }
    } else {
        ""
    };

    // 调用 app 层的执行函数
    let result_str = app::execute_command(program_str, working_dir_str, cmd_str);

    let output = CString::new(result_str).unwrap_or_default();
    CommandResult {
        exit_code: 0,
        output: output.into_raw(),
        error: ptr::null(),
    }
}

/// 释放命令结果内存
#[no_mangle]
pub extern "C" fn rust_free_command_result(result: CommandResult) {
    if !result.output.is_null() {
        unsafe {
            let _ = CString::from_raw(result.output as *mut c_char);
        }
    }
    if !result.error.is_null() {
        unsafe {
            let _ = CString::from_raw(result.error as *mut c_char);
        }
    }
}

// ============================================================================
// 配置管理 FFI
// ============================================================================

/// 加载配置
#[no_mangle]
pub extern "C" fn rust_load_config(
    config_path: *const c_char,
    backup_dir: *const c_char,
) -> ConfigData {
    if config_path.is_null() {
        return ConfigData {
            json_data: CString::new("{}").unwrap().into_raw(),
            length: 2,
        };
    }

    let path_str = match unsafe { CStr::from_ptr(config_path).to_str() } {
        Ok(s) => s,
        Err(_) => {
            return ConfigData {
                json_data: CString::new("{}").unwrap().into_raw(),
                length: 2,
            };
        }
    };

    let backup_str = if !backup_dir.is_null() {
        match unsafe { CStr::from_ptr(backup_dir).to_str() } {
            Ok(s) => s,
            Err(_) => "",
        }
    } else {
        ""
    };

    let json_str = app::load_config(path_str, backup_str);
    let len = json_str.len() as i32;
    let c_str = CString::new(json_str).unwrap_or_default();
    ConfigData {
        json_data: c_str.into_raw(),
        length: len,
    }
}

/// 保存配置
#[no_mangle]
pub extern "C" fn rust_save_config(
    config_path: *const c_char,
    backup_dir: *const c_char,
    json_data: *const c_char,
) -> bool {
    if config_path.is_null() || json_data.is_null() {
        return false;
    }

    let path_str = match unsafe { CStr::from_ptr(config_path).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    let backup_str = if !backup_dir.is_null() {
        match unsafe { CStr::from_ptr(backup_dir).to_str() } {
            Ok(s) => s,
            Err(_) => return false,
        }
    } else {
        return false;
    };

    let json_str = match unsafe { CStr::from_ptr(json_data).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    app::save_config(path_str, backup_str, json_str)
}

/// 释放配置数据内存
#[no_mangle]
pub extern "C" fn rust_free_config_data(config: ConfigData) {
    if !config.json_data.is_null() {
        unsafe {
            let _ = CString::from_raw(config.json_data as *mut c_char);
        }
    }
}

// ============================================================================
// 备份管理 FFI
// ============================================================================

/// 创建备份
#[no_mangle]
pub extern "C" fn rust_create_backup(
    config_path: *const c_char,
    backup_dir: *const c_char,
) -> bool {
    if config_path.is_null() || backup_dir.is_null() {
        return false;
    }

    let path_str = match unsafe { CStr::from_ptr(config_path).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    let backup_str = match unsafe { CStr::from_ptr(backup_dir).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    app::create_backup(path_str, backup_str)
}

/// 恢复备份
#[no_mangle]
pub extern "C" fn rust_restore_backup(
    config_path: *const c_char,
    backup_dir: *const c_char,
    backup_name: *const c_char,
) -> bool {
    if config_path.is_null() || backup_dir.is_null() || backup_name.is_null() {
        return false;
    }

    let config_str = match unsafe { CStr::from_ptr(config_path).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    let backup_dir_str = match unsafe { CStr::from_ptr(backup_dir).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    let backup_str = match unsafe { CStr::from_ptr(backup_name).to_str() } {
        Ok(s) => s,
        Err(_) => return false,
    };

    app::restore_backup(config_str, backup_dir_str, backup_str)
}

/// 获取备份列表
#[no_mangle]
pub extern "C" fn rust_get_backups(
    config_path: *const c_char,
    backup_dir: *const c_char,
) -> BackupList {
    if config_path.is_null() || backup_dir.is_null() {
        return BackupList {
            paths: ptr::null(),
            count: 0,
        };
    }

    let config_str = match unsafe { CStr::from_ptr(config_path).to_str() } {
        Ok(s) => s,
        Err(_) => {
            return BackupList {
                paths: ptr::null(),
                count: 0,
            };
        }
    };

    let backup_str = match unsafe { CStr::from_ptr(backup_dir).to_str() } {
        Ok(s) => s,
        Err(_) => {
            return BackupList {
                paths: ptr::null(),
                count: 0,
            };
        }
    };

    let backups = app::get_backups(config_str, backup_str);
    let mut c_strings = Vec::new();
    for backup in backups {
        if let Ok(c_str) = CString::new(backup) {
            c_strings.push(c_str.into_raw());
        }
    }

    let count = c_strings.len() as i32;
    let paths = if !c_strings.is_empty() {
        Box::leak(c_strings.into_boxed_slice()) as *const _ as *const *const c_char
    } else {
        ptr::null()
    };

    BackupList { paths, count }
}

/// 释放备份列表内存
#[no_mangle]
pub extern "C" fn rust_free_backup_list(list: BackupList) {
    if !list.paths.is_null() && list.count > 0 {
        unsafe {
            // 迭代释放每个字符串
            for i in 0..(list.count as usize) {
                let path_ptr = *list.paths.add(i);
                if !path_ptr.is_null() {
                    let _ = CString::from_raw(path_ptr as *mut c_char);
                }
            }
            // 释放数组本身
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                list.paths as *mut *mut c_char,
                list.count as usize,
            ));
        }
    }
}

// ============================================================================
// 命令解析 FFI
// ============================================================================

/// 解析命令
#[no_mangle]
pub extern "C" fn rust_parse_command(input: *const c_char) -> ParsedCommand {
    if input.is_null() {
        return ParsedCommand {
            commands: ptr::null(),
            count: 0,
        };
    }

    let input_str = match unsafe { CStr::from_ptr(input).to_str() } {
        Ok(s) => s,
        Err(_) => {
            return ParsedCommand {
                commands: ptr::null(),
                count: 0,
            };
        }
    };

    let parsed_commands = app::parse_command(input_str);
    let mut c_strings = Vec::new();
    for cmd in parsed_commands {
        if let Ok(c_str) = CString::new(cmd) {
            c_strings.push(c_str.into_raw());
        }
    }

    let count = c_strings.len() as i32;
    let commands = if !c_strings.is_empty() {
        Box::leak(c_strings.into_boxed_slice()) as *const _ as *const *const c_char
    } else {
        ptr::null()
    };

    ParsedCommand { commands, count }
}

/// 释放解析后的命令内存
#[no_mangle]
pub extern "C" fn rust_free_parsed_command(cmd: ParsedCommand) {
    if !cmd.commands.is_null() && cmd.count > 0 {
        unsafe {
            // 迭代释放每个字符串
            for i in 0..(cmd.count as usize) {
                let cmd_ptr = *cmd.commands.add(i);
                if !cmd_ptr.is_null() {
                    let _ = CString::from_raw(cmd_ptr as *mut c_char);
                }
            }
            // 释放数组本身
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                cmd.commands as *mut *mut c_char,
                cmd.count as usize,
            ));
        }
    }
}

// ============================================================================
// 日志 FFI
// ============================================================================

/// 初始化日志
#[no_mangle]
pub extern "C" fn rust_init_logger() {
    let _ = crate::utils::Logger::init();
}

/// 记录日志
#[no_mangle]
pub extern "C" fn rust_log(level: *const c_char, message: *const c_char) {
    if level.is_null() || message.is_null() {
        return;
    }

    let level_str = match unsafe { CStr::from_ptr(level).to_str() } {
        Ok(s) => s,
        Err(_) => return,
    };

    let msg_str = match unsafe { CStr::from_ptr(message).to_str() } {
        Ok(s) => s,
        Err(_) => return,
    };

    match level_str {
        "debug" => log::debug!("{}", msg_str),
        "info" => log::info!("{}", msg_str),
        "warn" => log::warn!("{}", msg_str),
        "error" => log::error!("{}", msg_str),
        _ => log::info!("{}", msg_str),
    }
}
