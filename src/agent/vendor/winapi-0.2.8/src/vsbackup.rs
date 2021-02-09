// Copyright © 2015, Brian Vincent
// Licensed under the MIT License <LICENSE.md>
//! VSS backup interfaces
DEFINE_GUID!(IID_IVssExamineWriterMetadata, 0x902fcf7f, 0xb7fd, 0x42f8,
    0x81, 0xf1, 0xb2, 0xe4, 0x00, 0xb1, 0xe5, 0xbd);
DEFINE_GUID!(IID_IVssExamineWriterMetadataEx, 0x0c0e5ec0, 0xca44, 0x472b,
    0xb7, 0x02, 0xe6, 0x52, 0xdb, 0x1c, 0x04, 0x51);
DEFINE_GUID!(IID_IVssBackupComponents, 0x665c1d5f, 0xc218, 0x414d,
    0xa0, 0x5d, 0x7f, 0xef, 0x5f, 0x9d, 0x5c, 0x86);
DEFINE_GUID!(IID_IVssBackupComponentsEx, 0x963f03ad, 0x9e4c, 0x4a34,
    0xac, 0x15, 0xe4, 0xb6, 0x17, 0x4e, 0x50, 0x36);
STRUCT!{struct VSS_COMPONENTINFO {
    type_: ::VSS_COMPONENT_TYPE, // type is a keyword in rust
    bstrLogicalPath: ::BSTR,
    bstrComponentName: ::BSTR,
    bstrCaption: ::BSTR,
    pbIcon: *mut ::BYTE,
    cbIcon: ::UINT,
    bRestoreMetadata: bool,
    bNotifyOnBackupComplete: bool,
    bSelectable: bool,
    bSelectableForRestore: bool,
    dwComponentFlags: ::DWORD,
    cFileCount: ::UINT,
    cDatabases: ::UINT,
    cLogFiles: ::UINT,
    cDependencies: ::UINT,
}}
pub type PVSSCOMPONENTINFO = *const ::VSS_COMPONENTINFO;
RIDL!(
interface IVssWMComponent(IVssWMComponentVtbl): IUnknown(IUnknownVtbl) {
    fn GetComponentInfo(&mut self, ppInfo: *mut ::PVSSCOMPONENTINFO) -> ::HRESULT,
    fn FreeComponentInfo(&mut self, pInfo: ::PVSSCOMPONENTINFO) -> ::HRESULT,
    fn GetFile(&mut self, iFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc) -> ::HRESULT,
    fn GetDatabaseFile(
        &mut self, iDBFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn GetDatabaseLogFile(
        &mut self, iDbLogFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn GetDependency(
        &mut self, iDependency: ::UINT, ppDependency: *mut *mut ::IVssWMDependency
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssExamineWriterMetadata(IVssExamineWriterMetadataVtbl): IUnknown(IUnknownVtbl) {
    fn GetIdentity(
        &mut self, pidInstance: *mut ::VSS_ID, pidWriter: *mut ::VSS_ID,
        pbstrWriterName: *mut ::BSTR, pUsage: *mut ::VSS_USAGE_TYPE,
        pSource: *mut ::VSS_SOURCE_TYPE
    ) -> ::HRESULT,
    fn GetFileCounts(&mut self, pcIncludeFiles: *mut ::UINT, pcExcludeFiles: *mut ::UINT,
        pcComponents: *mut ::UINT
    ) -> ::HRESULT,
    fn GetIncludeFile(
        &mut self, iFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn GetExcludeFile(
        &mut self, iFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn GetComponent(
        &mut self, iComponent: ::UINT, ppComponent: *mut *mut ::IVssWMComponent
    ) -> ::HRESULT,
    fn GetRestoreMethod(
        &mut self, pMethod: *mut ::VSS_RESTOREMETHOD_ENUM, pbstrService: *mut ::BSTR,
        pbstrUserProcedure: *mut ::BSTR, pwriterRestore: *mut ::VSS_WRITERRESTORE_ENUM,
        pbRebootRequired: *mut bool, pcMappings: *mut ::UINT
    ) -> ::HRESULT,
    fn GetAlternateLocationMapping(
        &mut self, iMapping: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT,
    fn GetBackupSchema(&mut self, pdwSchemaMask: *mut ::DWORD) -> ::HRESULT,
    fn GetDocument(&mut self, pDoc: *mut ::c_void) -> ::HRESULT, //TODO IXMLDOMDocument
    fn SaveAsXML(&mut self, pbstrXML: *mut ::BSTR) -> ::HRESULT,
    fn LoadFromXML(&mut self, pbstrXML: *mut ::BSTR) -> ::HRESULT
}
);
RIDL!(
interface IVssExamineWriterMetadataEx(IVssExamineWriterMetadataExVtbl):
    IVssExamineWriterMetadata(IVssExamineWriterMetadataVtbl) {
    fn GetIdentityEx(
        &mut self, pidInstance: *mut ::VSS_ID, pidWriter: *mut ::VSS_ID,
        pbstrWriterName: *mut ::BSTR, pbstrInstanceName: *mut ::BSTR,
        pUsage: *mut ::VSS_USAGE_TYPE, pSource: *mut ::VSS_SOURCE_TYPE
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssExamineWriterMetadataEx2(IVssExamineWriterMetadataEx2Vtbl):
    IVssExamineWriterMetadataEx(IVssExamineWriterMetadataExVtbl) {
    fn GetVersion(
        &mut self, pdwMajorVersion: *mut ::DWORD, pdwMinorVersion: *mut ::DWORD
    ) -> ::HRESULT,
    fn GetExcludeFromSnapshotCount(&mut self, pcExcludedFromSnapshot: *mut ::UINT) -> ::HRESULT,
    fn GetExcludeFromSnapshotFile(
        &mut self, iFile: ::UINT, ppFiledesc: *mut *mut ::IVssWMFiledesc
    ) -> ::HRESULT
}
);
#[repr(C)]
pub struct IVssWriterComponentsExt {
    pub lpVtbl: *const IVssWriterComponentsExtVtbl,
}
#[repr(C)]
pub struct IVssWriterComponentsExtVtbl {
    pub parent1: ::IVssWriterComponentsVtbl,
    pub parent2: ::IUnknownVtbl,
}
RIDL!(
interface IVssBackupComponents(IVssBackupComponentsVtbl): IUnknown(IUnknownVtbl) {
    fn GetWriterComponentsCount(&mut self, pcComponents: *mut ::UINT) -> ::HRESULT,
    fn GetWriterComponents(
        &mut self, iWriter: ::UINT, ppWriter: *mut *mut IVssWriterComponentsExt
    ) -> ::HRESULT,
    fn InitializeForBackup(&mut self, bstrXML: ::BSTR) -> ::HRESULT,
    fn SetBackupState(
        &mut self, bSelectComponents: bool, bBackupBootableSystemState: bool,
        backupType: ::VSS_BACKUP_TYPE, bPartialFileSupport: bool
    ) -> ::HRESULT,
    fn InitializeForRestore(&mut self, bstrXML: ::BSTR) -> ::HRESULT,
    fn SetRestoreState(&mut self, restoreType: ::VSS_RESTORE_TYPE) -> ::HRESULT,
    fn GatherWriterMetadata(&mut self, pAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn GetWriterMetadataCount(&mut self, pcWriters: *mut ::UINT) -> ::HRESULT,
    fn GetWriterMetadata(
        &mut self, iWriter: ::UINT, pidInstance: *mut ::VSS_ID,
        ppMetadata: *mut *mut IVssExamineWriterMetadata
    ) -> ::HRESULT,
    fn FreeWriterMetadata(&mut self) -> ::HRESULT,
    fn AddComponent(
        &mut self, instanceId: ::VSS_ID, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE,
        wszLogicalPath: ::LPCWSTR, wszComponentName: ::LPCWSTR
    ) -> ::HRESULT,
    fn PrepareForBackup(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn AbortBackup(&mut self) -> ::HRESULT,
    fn GatherWriterStatus(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn GetWriterStatusCount(&mut self, pcWriters: *mut ::UINT) -> ::HRESULT,
    fn FreeWriterStatus(&mut self) -> ::HRESULT,
    fn GetWriterStatus(
        &mut self, iWriter: ::UINT, pidInstance: *mut ::VSS_ID, pidWriter: *mut ::VSS_ID,
        pbstrWriter: *mut ::BSTR, pnStatus: *mut ::VSS_WRITER_STATE,
        phResultFailure: *mut ::HRESULT
    ) -> ::HRESULT,
    fn SetBackupSucceeded(
        &mut self, instanceId: ::VSS_ID, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE,
        wszLogicalPath: ::LPCWSTR, wszComponentName: ::LPCWSTR, bSucceded: bool
    ) -> ::HRESULT,
    fn SetBackupOptions(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszBackupOptions: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetSelectedForRestore(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, bSelectedForRestore: bool
    ) -> ::HRESULT,
    fn SetRestoreOptions(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszRestoreOptions: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetAdditionalRestores(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, bAdditionalRestores: bool
    ) -> ::HRESULT,
    fn SetPreviousBackupStamp(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszPreviousBackupStamp: ::LPCWSTR
    ) -> ::HRESULT,
    fn SaveAsXML(&mut self, pbstrXML: *mut ::BSTR) -> ::HRESULT,
    fn BackupComplete(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn AddAlternativeLocationMapping(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszPath: ::LPCWSTR, wszFilespec: ::LPCWSTR, bRecursive: bool,
        wszDestination: ::LPCWSTR
    ) -> ::HRESULT,
    fn AddRestoreSubcomponent(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszSubComponentLogicalPath: ::LPCWSTR,
        wszSubComponentName: ::LPCWSTR, bRepair: bool
    ) -> ::HRESULT,
    fn SetFileRestoreStatus(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, status: ::VSS_FILE_RESTORE_STATUS
    ) -> ::HRESULT,
    fn AddNewTarget(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszPath: ::LPCWSTR, wszFileName: ::LPCWSTR, bRecursive: bool,
        wszAlternatePath: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetRangesFilePath(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, iPartialFile: ::UINT, wszRangesFile: ::LPCWSTR
    ) -> ::HRESULT,
    fn PreRestore(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn PostRestore(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn SetContext(&mut self, lContext: ::LONG) -> ::HRESULT,
    fn StartSnapshotSet(&mut self, pSnapshotSetId: *mut ::VSS_ID) -> ::HRESULT,
    fn AddToSnapshotSet(
        &mut self, pwszVolumeName: ::VSS_PWSZ, ProviderId: ::VSS_ID, pidSnapshot: *mut ::VSS_ID
    ) -> ::HRESULT,
    fn DoSnapshotSet(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn DeleteSnapshots(
        &mut self, SourceObjectId: ::VSS_ID, eSourceObjectType: ::VSS_OBJECT_TYPE,
        bForceDelete: ::BOOL, plDeletedSnapshots: *mut ::LONG, pNondeletedSnapshotID: *mut ::VSS_ID
    ) -> ::HRESULT,
    fn ImportSnapshots(&mut self, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn BreakSnapshotSet(&mut self, SnapshotSetId: ::VSS_ID) -> ::HRESULT,
    fn GetSnapshotProperties(
        &mut self, SnapshotId: ::VSS_ID,
        pProp: *mut ::VSS_SNAPSHOT_PROP
    ) -> ::HRESULT,
    fn Query(&mut self, QueriedObjectId: ::VSS_ID, eQueriedObjectType: ::VSS_OBJECT_TYPE,
        eReturnedObjectsType: ::VSS_OBJECT_TYPE, ppEnum: *mut *mut ::IVssEnumObject) -> ::HRESULT,
    fn IsVolumeSupported(
        &mut self, ProviderId: ::VSS_ID, pwszVolumeName: ::VSS_PWSZ,
        pbSupportedByThisProvider: *mut ::BOOL
    ) -> ::HRESULT,
    fn DisableWriterClasses(
        &mut self, rgWriterClassId: *const ::VSS_ID, cClassId: ::UINT
    ) -> ::HRESULT,
    fn EnableWriterClasses(
        &mut self, rgWriterClassId: *const ::VSS_ID, cClassId: ::UINT
    ) -> ::HRESULT,
    fn DisableWriterInstances(
        &mut self, rgWriterInstanceId: *const ::VSS_ID, cInstanceId: ::UINT
    ) -> ::HRESULT,
    fn ExposeSnapshot(&mut self, SnapshotId: ::VSS_ID, wszPathFromRoot: ::VSS_PWSZ,
        lAttributes: ::LONG, wszExpose: ::VSS_PWSZ, pwszExposed: ::VSS_PWSZ
    ) -> ::HRESULT,
    fn RevertToSnapshot(&mut self, SnapshotId: ::VSS_ID, bForceDismount: ::BOOL) -> ::HRESULT,
    fn QueryRevertStatus(
        &mut self, pwszVolume: ::VSS_PWSZ, ppAsync: *mut *mut ::IVssAsync
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssBackupComponentsEx(IVssBackupComponentsExVtbl):
    IVssBackupComponents(IVssBackupComponentsVtbl) {
    fn GetWriterMetadataEx(
        &mut self, iWriter: ::UINT, pidInstance: *mut ::VSS_ID,
        ppMetadata: *mut *mut ::IVssExamineWriterMetadataEx
    ) -> ::HRESULT,
    fn SetSelectedForRestoreEx(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, bSelectedForRestore: bool, instanceId: ::VSS_ID
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssBackupComponentsEx2(IVssBackupComponentsEx2Vtbl):
    IVssBackupComponentsEx(IVssBackupComponentsExVtbl) {
    fn UnexposeSnapshot(&mut self, snapshotId: ::VSS_ID) -> ::HRESULT,
    fn SetAuthoritativeRestore(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, bAuth: bool
    ) -> ::HRESULT,
    fn SetRollForward(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, rollType: ::VSS_ROLLFORWARD_TYPE,
        wszRollForwardPoint: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetRestoreName(
        &mut self, writerId: ::VSS_ID, ct: ::VSS_COMPONENT_TYPE, wszLogicalPath: ::LPCWSTR,
        wszComponentName: ::LPCWSTR, wszRestoreName: ::LPCWSTR
    ) -> ::HRESULT,
    fn BreakSnapshotSetEx(
        &mut self, SnapshotSetID: ::VSS_ID, dwBreakFlags: ::DWORD, ppAsync: *mut *mut ::IVssAsync
    ) -> ::HRESULT,
    fn PreFastRecovery(
        &mut self, SnapshotSetID: ::VSS_ID, dwPreFastRecoveryFlags: ::DWORD,
        ppAsync: *mut *mut ::IVssAsync
    ) -> ::HRESULT,
    fn FastRecovery(
        &mut self, SnapshotSetID: ::VSS_ID, dwFastRecoveryFlags: ::DWORD,
        ppAsync: *mut *mut ::IVssAsync
    ) -> ::HRESULT
}
);
RIDL!(
interface IVssBackupComponentsEx3(IVssBackupComponentsEx3Vtbl):
    IVssBackupComponentsEx2(IVssBackupComponentsEx2Vtbl) {
    fn GetWriterStatusEx(
        &mut self, iWriter: ::UINT, pidInstance: *mut ::VSS_ID, pidWriter: *mut ::VSS_ID,
        pbstrWriter: *mut ::BSTR, pnStatus: *mut ::VSS_WRITER_STATE,
        phrFailureWriter: *mut ::HRESULT, phrApplication: *mut ::HRESULT,
        pbstrApplicationMessage: *mut ::BSTR
    ) -> ::HRESULT,
    fn AddSnapshotToRecoverySet(
        &mut self, snapshotId: ::VSS_ID, dwFlags: ::DWORD, pwszDestinationVolume: ::VSS_PWSZ
    ) -> ::HRESULT,
    fn RecoverSet(&mut self, dwFlags: ::DWORD, ppAsync: *mut *mut ::IVssAsync) -> ::HRESULT,
    fn GetSessionId(&mut self, idSession: *mut ::VSS_ID) -> ::HRESULT
}
);
RIDL!(
interface IVssBackupComponentsEx4(IVssBackupComponentsEx4Vtbl):
    IVssBackupComponentsEx3(IVssBackupComponentsEx3Vtbl) {
    fn GetRootAndLogicalPrefixPaths(
        &mut self, pwszFilePath: ::VSS_PWSZ, ppwszRootPath: *mut ::VSS_PWSZ,
        ppwszLogicalPrefix: *mut ::VSS_PWSZ, bNormalizeFQDNforRootPath: ::BOOL
    ) -> ::HRESULT
}
);
pub const VSS_SW_BOOTABLE_STATE: ::DWORD = 1;
