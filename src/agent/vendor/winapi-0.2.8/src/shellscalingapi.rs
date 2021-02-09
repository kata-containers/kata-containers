// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
// ShellScalingApi.h
ENUM!{enum PROCESS_DPI_AWARENESS {
    Process_DPI_Unaware = 0,
    Process_System_DPI_Aware  = 1,
    Process_Per_Monitor_DPI_Aware = 2,
}}
ENUM!{enum SHELL_UI_COMPONENT {
    SHELL_UI_COMPONENT_TASKBARS = 0,
    SHELL_UI_COMPONENT_NOTIFICATIONAREA = 1,
    SHELL_UI_COMPONENT_DESKBAND = 2,
}}
ENUM!{enum MONITOR_DPI_TYPE {
    MDT_EFFECTIVE_DPI = 0,
    MDT_ANGULAR_DPI = 1,
    MDT_RAW_DPI = 2,
}}
pub const MDT_DEFAULT: MONITOR_DPI_TYPE = MDT_EFFECTIVE_DPI;
