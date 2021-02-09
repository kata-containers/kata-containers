// Copyright © 2015, Brian Vincent
// Licensed under the MIT License <LICENSE.md>
//! VSS Writer header file
ENUM!{enum VSS_USAGE_TYPE {
    VSS_UT_UNDEFINED = 0,
    VSS_UT_BOOTABLESYSTEMSTATE = 1,
    VSS_UT_SYSTEMSERVICE = 2,
    VSS_UT_USERDATA = 3,
    VSS_UT_OTHER = 4,
}}
ENUM!{enum VSS_SOURCE_TYPE {
    VSS_ST_UNDEFINED = 0,
    VSS_ST_TRANSACTEDDB = 1,
    VSS_ST_NONTRANSACTEDDB = 2,
    VSS_ST_OTHER = 3,
}}
ENUM!{enum VSS_RESTOREMETHOD_ENUM {
    VSS_RME_UNDEFINED = 0,
    VSS_RME_RESTORE_IF_NOT_THERE = 1,
    VSS_RME_RESTORE_IF_CAN_REPLACE = 2,
    VSS_RME_STOP_RESTORE_START = 3,
    VSS_RME_RESTORE_TO_ALTERNATE_LOCATION = 4,
    VSS_RME_RESTORE_AT_REBOOT = 5,
    VSS_RME_RESTORE_AT_REBOOT_IF_CANNOT_REPLACE = 6,
    VSS_RME_CUSTOM = 7,
    VSS_RME_RESTORE_STOP_START = 8,
}}
ENUM!{enum VSS_WRITERRESTORE_ENUM {
    VSS_WRE_UNDEFINED = 0,
    VSS_WRE_NEVER = 1,
    VSS_WRE_IF_REPLACE_FAILS = 2,
    VSS_WRE_ALWAYS = 3,
}}
ENUM!{enum VSS_COMPONENT_TYPE {
    VSS_CT_UNDEFINED = 0,
    VSS_CT_DATABASE = 1,
    VSS_CT_FILEGROUP = 2,
}}
ENUM!{enum VSS_ALTERNATE_WRITER_STATE {
    VSS_AWS_UNDEFINED = 0,
    VSS_AWS_NO_ALTERNATE_WRITER = 1,
    VSS_AWS_ALTERNATE_WRITER_EXISTS = 2,
    VSS_AWS_THIS_IS_ALTERNATE_WRITER = 3,
}}
pub type VSS_SUBSCRIBE_MASK = ::DWORD;
pub const VSS_SM_POST_SNAPSHOT_FLAG: ::DWORD = 0x00000001;
pub const VSS_SM_BACKUP_EVENTS_FLAG: ::DWORD = 0x00000002;
pub const VSS_SM_RESTORE_EVENTS_FLAG: ::DWORD = 0x00000004;
pub const VSS_SM_IO_THROTTLING_FLAG: ::DWORD = 0x00000008;
pub const VSS_SM_ALL_FLAGS: ::DWORD = 0xffffffff;
ENUM!{enum VSS_RESTORE_TARGET {
    VSS_RT_UNDEFINED = 0,
    VSS_RT_ORIGINAL = 1,
    VSS_RT_ALTERNATE = 2,
    VSS_RT_DIRECTED = 3,
    VSS_RT_ORIGINAL_LOCATION = 4,
}}
ENUM!{enum VSS_FILE_RESTORE_STATUS {
    VSS_RS_UNDEFINED = 0,
    VSS_RS_NONE = 1,
    VSS_RS_ALL = 2,
    VSS_RS_FAILED = 3,
}}
pub type VSS_COMPONENT_FLAGS = ::DWORD;
pub const VSS_CF_BACKUP_RECOVERY: ::DWORD = 0x00000001;
pub const VSS_CF_APP_ROLLBACK_RECOVERY: ::DWORD = 0x00000002;
pub const VSS_CF_NOT_SYSTEM_STATE: ::DWORD = 0x00000004;
RIDL!(
interface IVssWMFiledesc(IVssWMFiledescVtbl): IUnknown(IUnknownVtbl) {
    fn GetPath(&mut self, pbstrPath: *mut ::BSTR) -> ::HRESULT,
    fn GetFilespec(&mut self, pbstrFilespec: *mut ::BSTR) -> ::HRESULT,
    fn GetRecursive(&mut self, pbRecursive: *mut bool) -> ::HRESULT,
    fn GetAlternateLocation(&mut self, pbstrAlternateLocation: *mut ::BSTR) -> ::HRESULT,
    fn GetBackupTypeMask(&mut self, pdwTypeMask: *mut ::DWORD) -> ::HRESULT
}
);
RIDL!(
interface IVssWMDependency(IVssWMDependencyVtbl): IUnknown(IUnknownVtbl) {
    fn GetWriterId(&mut self, pWriterId: *mut ::VSS_ID) -> ::HRESULT,
    fn GetLogicalPath(&mut self, pbstrLogicalPath: *mut ::BSTR) -> ::HRESULT,
    fn GetComponentName(&mut self, pbstrComponentName: *mut ::BSTR) -> ::HRESULT
}
);
RIDL!(
interface IVssComponent(IVssComponentVtbl): IUnknown(IUnknownVtbl) {
    fn GetLogicalPath(&mut self, pbstrPath: *mut ::BSTR) -> ::HRESULT,
    fn GetComponentType(&mut self, pct: *mut ::VSS_COMPONENT_TYPE) -> ::HRESULT,
    fn GetComponentName(&mut self, pbstrName: *mut ::BSTR) -> ::HRESULT,
    fn GetBackupSucceeded(&mut self, pbSucceeded: *mut bool) -> ::HRESULT,
    fn GetAlternateLocationMappingCount(&mut self, pcMappings: *mut ::UINT) -> ::HRESULT,
    fn GetAlternateLocationMapping(
        &mut self, iMapping: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn SetBackupMetadata(&mut self, wszData: ::LPCWSTR) -> ::HRESULT,
    fn GetBackupMetadata(&mut self, pbstrData: *mut ::BSTR) -> ::HRESULT,
    fn AddPartialFile(
        &mut self, wszPath: ::LPCWSTR, wszFilename: ::LPCWSTR, wszRanges: ::LPCWSTR,
        wszMetadata: ::LPCWSTR
    ) -> ::HRESULT,
    fn GetPartialFileCount(&mut self, pcPartialFiles: *mut ::UINT) -> ::HRESULT,
    fn GetPartialFile(
        &mut self, iPartialFile: ::UINT, pbstrPath: *mut ::BSTR, pbstrFilename: *mut ::BSTR,
        pbstrRange: *mut ::BSTR, pbstrMetadata: *mut ::BSTR
    ) -> ::HRESULT,
    fn IsSelectedForRestore(&mut self, pbSelectedForRestore: *mut bool) -> ::HRESULT,
    fn GetAdditionalRestores(&mut self, pbAdditionalRestores: *mut bool) -> ::HRESULT,
    fn GetNewTargetCount(&mut self, pcNewTarget: *mut ::UINT) -> ::HRESULT,
    fn GetNewTarget(
        &mut self, iNewTarget: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn AddDirectedTarget(
        &mut self, wszSourcePath: ::LPCWSTR, wszSourceFilename: ::LPCWSTR,
        wszSourceRangeList: ::LPCWSTR, wszDestinationPath: ::LPCWSTR,
        wszDestinationFilename: ::LPCWSTR, wszDestinationRangeList: ::LPCWSTR
    ) -> ::HRESULT,
    fn GetDirectedTargetCount(&mut self, pcDirectedTarget: *mut ::UINT) -> ::HRESULT,
    fn GetDirectedTarget(
        &mut self, iDirectedTarget: ::UINT, pbstrSourcePath: *mut ::BSTR,
        pbstrSourceFileName: *mut ::BSTR, pbstrSourceRangeList: *mut ::BSTR,
        pbstrDestinationPath: *mut ::BSTR, pbstrDestinationFilename: *mut ::BSTR,
        pbstrDestinationRangeList: *mut ::BSTR
    ) -> ::HRESULT,
    fn SetRestoreMetadata(&mut self, wszRestoreMetadata: ::LPCWSTR) -> ::HRESULT,
    fn GetRestoreMetadata(&mut self, pbstrRestoreMetadata: *mut ::BSTR) -> ::HRESULT,
    fn SetRestoreTarget(&mut self, target: ::VSS_RESTORE_TARGET) -> ::HRESULT,
    fn GetRestoreTarget(&mut self, pTarget: *mut ::VSS_RESTORE_TARGET) -> ::HRESULT,
    fn SetPreRestoreFailureMsg(&mut self, wszPreRestoreFailureMsg: ::LPCWSTR) -> ::HRESULT,
    fn GetPreRestoreFailureMsg(&mut self, pbstrPreRestoreFailureMsg: *mut ::BSTR) -> ::HRESULT,
    fn SetPostRestoreFailureMsg(&mut self, wszPostRestoreFailureMsg: ::LPCWSTR) -> ::HRESULT,
    fn GetPostRestoreFailureMsg(&mut self, pbstrPostRestoreFailureMsg: *mut ::BSTR) -> ::HRESULT,
    fn SetBackupStamp(&mut self, wszBackupStamp: ::LPCWSTR) -> ::HRESULT,
    fn GetBackupStamp(&mut self, pbstrBackupStamp: *mut ::BSTR) -> ::HRESULT,
    fn GetPreviousBackupStamp(&mut self, pbstrBackupStamp: *mut ::BSTR) -> ::HRESULT,
    fn GetBackupOptions(&mut self, pbstrBackupOptions: *mut ::BSTR) -> ::HRESULT,
    fn GetRestoreOptions(&mut self, pbstrRestoreOptions: *mut ::BSTR) -> ::HRESULT,
    fn GetRestoreSubcomponentCount(&mut self, pcRestoreSubcomponent: *mut ::UINT) -> ::HRESULT,
    fn GetRestoreSubcomponent(
        &mut self, iComponent: ::UINT, pbstrLogicalPath: *mut ::BSTR,
        pbstrComponentName: *mut ::BSTR, pbRepair: *mut bool
    ) -> ::HRESULT,
    fn GetFileRestoreStatus(&mut self, pStatus: *mut VSS_FILE_RESTORE_STATUS) -> ::HRESULT,
    fn AddDifferencedFilesByLastModifyTime(
        &mut self, wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: ::BOOL,
        ftLastModifyTime: ::FILETIME
    ) -> ::HRESULT,
    fn AddDifferencedFilesByLastModifyLSN(
        &mut self, wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: ::BOOL,
        bstrLsnString: ::BSTR
    ) -> ::HRESULT,
    fn GetDifferencedFilesCount(&mut self, pcDifferencedFiles: *mut ::UINT) -> ::HRESULT,
    fn GetDifferencedFile(
        &mut self, iDifferencedFile: ::UINT, pbstrPath: *mut ::BSTR, pbstrFilespec: *mut ::BSTR,
        pbRecursive: *mut ::BOOL, pbstrLsnString: *mut ::BSTR, pftLastModifyTime: *mut ::FILETIME
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssWriterComponents(IVssWriterComponentsVtbl) {
    fn GetComponentCount(&mut self, pcComponents: *mut ::UINT) -> ::HRESULT,
    fn GetWriterInfo(
        &mut self, pidInstance: *mut ::VSS_ID, pidWriter: *mut ::VSS_ID
    ) -> ::HRESULT,
    fn GetComponent(
        &mut self, iComponent: ::UINT, ppComponent: *mut *mut ::IVssComponent
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssComponentEx(IVssComponentExVtbl): IVssComponent(IVssComponentVtbl) {
    fn SetPrepareForBackupFailureMsg(&mut self, wszFailureMsg: ::LPCWSTR) -> ::HRESULT,
    fn SetPostSnapshotFailureMsg(&mut self, wszFailureMsg: ::LPCWSTR) -> ::HRESULT,
    fn GetPrepareForBackupFailureMsg(&mut self, pbstrFailureMsg: *mut ::BSTR) -> ::HRESULT,
    fn GetPostSnapshotFailureMsg(&mut self, pbstrFailureMsg: *mut ::BSTR) -> ::HRESULT,
    fn GetAuthoritativeRestore(&mut self, pbAuth: *mut bool) -> ::HRESULT,
    fn GetRollForward(
        &mut self, pRollType: *mut ::VSS_ROLLFORWARD_TYPE, pbstrPoint: *mut ::BSTR
    ) -> ::HRESULT,
    fn GetRestoreName(&mut self, pbstrName: *mut ::BSTR) -> ::HRESULT
}
);
RIDL!(
interface IVssComponentEx2(IVssComponentEx2Vtbl): IVssComponentEx(IVssComponentExVtbl) {
    fn SetFailure(
        &mut self, hr: ::HRESULT, hrApplication: ::HRESULT, wszApplicationMessage: ::LPCWSTR,
        dwReserved: ::DWORD
    ) -> ::HRESULT,
    fn GetFailure(
        &mut self, phr: *mut ::HRESULT, phrApplication: *mut ::HRESULT,
        pbstrApplicationMessage: *mut ::BSTR, pdwReserved: *mut ::DWORD
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssCreateWriterMetadata(IVssCreateWriterMetadataVtbl) {
    fn AddIncludeFiles(
        &mut self, wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: bool,
        wszAlternateLocation: ::LPCWSTR
    ) -> ::HRESULT,
    fn AddExcludeFiles(
        &mut self, wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: bool
    ) -> ::HRESULT,
    fn AddComponent(
        &mut self, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszCaption: ::LPCWSTR, pbIcon: *const ::BYTE, cbIcon: ::UINT,
        bRestoreMetadata: bool, bNotifyOnBackupComplete: bool, bSelectableForRestore: bool,
        dwComponentFlags: ::DWORD
    ) -> ::HRESULT,
    fn AddDatabaseFiles(
        &mut self, wszLogicalPath: ::LPCWSTR, wszDatabaseName: ::LPCWSTR, wszPath: ::LPCWSTR,
        wszFilespec: ::LPCWSTR, dwBackupTypeMask: ::DWORD
    ) -> ::HRESULT,
    fn AddDatabaseLogFiles(&mut self, wszLogicalPath: ::LPCWSTR, wszDatabaseName: ::LPCWSTR,
        wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, dwBackupTypeMask: ::DWORD
    ) -> ::HRESULT,
    fn AddFilesToFileGroup(&mut self, wszLogicalPath: ::LPCWSTR, wszGroupName: ::LPCWSTR,
        wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: bool,
        wszAlternateLocation: ::LPCWSTR, dwBackupTypeMask: ::DWORD
    ) -> ::HRESULT,
    fn SetRestoreMethod(&mut self, method: ::VSS_RESTOREMETHOD_ENUM, wszService: ::LPCWSTR,
        wszUserProcedure: ::LPCWSTR, writerRestore: ::VSS_WRITERRESTORE_ENUM,
        bRebootRequired: bool
    ) -> ::HRESULT,
    fn AddAlternateLocationMapping(&mut self, wszSourcePath: ::LPCWSTR,
        wszSourceFilespec: ::LPCWSTR, bRecursive: bool, wszDestination: ::LPCWSTR
    ) -> ::HRESULT,
    fn AddComponentDependency(&mut self, wszForLogicalPath: ::LPCWSTR,
        wszForComponentName: ::LPCWSTR, onWriterId: ::VSS_ID, wszOnLogicalPath: ::LPCWSTR,
        wszOnComponentName: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetBackupSchema(&mut self, dwSchemaMask: ::DWORD) -> ::HRESULT,
    fn GetDocument(&mut self, pDoc: *mut *mut ::VOID) -> ::HRESULT, //TODO IXMLDOMDocument
    fn SaveAsXML(&mut self, pbstrXML: *mut ::BSTR) -> ::HRESULT
}
);
//IVssCreateWriterMetadataEx
//IVssWriterImpl
//IVssCreateExpressWriterMetadata
//IVssExpressWriter
//CVssWriter
//CVssWriterEx
//CVssWriterEx2
