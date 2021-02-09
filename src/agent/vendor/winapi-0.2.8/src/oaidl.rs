// Copyright © 2015, Connor Hilarides
// Licensed under the MIT License <LICENSE.md>
//! Mappings for the contents of OAIdl.h
pub type wireBRECORD = *mut _wireBRECORD;
pub type wireVARIANT = *mut _wireVARIANT;
STRUCT!{struct SAFEARRAYBOUND {
    cElements: ::ULONG,
    lLbound: ::LONG,
}}
STRUCT!{struct SAFEARR_BSTR {
    Size: ::ULONG,
    aBstr: *mut ::wireBSTR,
}}
STRUCT!{struct SAFEARR_UNKNOWN {
    Size: ::ULONG,
    apUnknown: *mut *mut ::IUnknown,
}}
STRUCT!{struct SAFEARR_DISPATCH {
    Size: ::ULONG,
    apDispatch: *mut *mut IDispatch,
}}
STRUCT!{struct SAFEARR_VARIANT {
    Size: ::ULONG,
    aVariant: *mut wireVARIANT,
}}
STRUCT!{struct SAFEARR_BRECORD {
    Size: ::ULONG,
    aRecord: *mut wireBRECORD,
}}
STRUCT!{struct SAFEARR_HAVEIID {
    Size: ::ULONG,
    apUnknown: *mut *mut ::IUnknown,
    iid: ::IID,
}}
ENUM!{enum SF_TYPE {
    SF_ERROR = ::VT_ERROR.0,
    SF_I1 = ::VT_I1.0,
    SF_I2 = ::VT_I2.0,
    SF_I4 = ::VT_I4.0,
    SF_I8 = ::VT_I8.0,
    SF_BSTR = ::VT_BSTR.0,
    SF_UNKNOWN = ::VT_UNKNOWN.0,
    SF_DISPATCH = ::VT_DISPATCH.0,
    SF_VARIANT = ::VT_VARIANT.0,
    SF_RECORD = ::VT_RECORD.0,
    SF_HAVEIID = ::VT_UNKNOWN.0 | ::VT_RESERVED.0,
}}
STRUCT!{struct SAFEARRAYUNION {
    sfType: ::ULONG,
    u: __MIDL_IOleAutomationTypes_0001,
}}
#[cfg(target_arch = "x86_64")]
STRUCT!{struct __MIDL_IOleAutomationTypes_0001 {
    data0: u32,
    data1: [u32; 6],
}}
#[cfg(target_arch = "x86")]
STRUCT!{struct __MIDL_IOleAutomationTypes_0001 {
    data0: u32,
    data1: [u32; 5],
}}
UNION!(__MIDL_IOleAutomationTypes_0001, data0, BstrStr, BstrStr_mut, SAFEARR_BSTR);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, UnknownStr, UnknownStr_mut, SAFEARR_UNKNOWN);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, DispatchStr, DispatchStr_mut, SAFEARR_DISPATCH);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, VariantStr, VariantStr_mut, SAFEARR_VARIANT);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, RecordStr, RecordStr_mut, SAFEARR_BRECORD);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, HaveIidStr, HaveIidStr_mut, SAFEARR_HAVEIID);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, ByteStr, ByteStr_mut, ::BYTE_SIZEDARR);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, WordStr, WordStr_mut, ::WORD_SIZEDARR);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, LongStr, LongStr_mut, ::DWORD_SIZEDARR);
UNION!(__MIDL_IOleAutomationTypes_0001, data0, HyperStr, HyperStr_mut, ::HYPER_SIZEDARR);
STRUCT!{struct _wireSAFEARRAY {
    cDims: ::USHORT,
    fFeatures: ::USHORT,
    cbElements: ::ULONG,
    cLocks: ::ULONG,
    uArrayStructs: SAFEARRAYUNION,
    rgsaBound: [SAFEARRAYBOUND; 1],
}}
pub type wireSAFEARRAY = *mut _wireSAFEARRAY;
pub type wirePSAFEARRAY = *mut wireSAFEARRAY;
STRUCT!{struct SAFEARRAY {
    cDims: ::USHORT,
    fFeatures: ::USHORT,
    cbElements: ::ULONG,
    cLocks: ::ULONG,
    pvData: ::PVOID,
    rgsabound: [SAFEARRAYBOUND; 1],
}}
pub type LPSAFEARRAY = *mut SAFEARRAY;
pub const FADF_AUTO: ::DWORD = 0x1;
pub const FADF_STATIC: ::DWORD = 0x2;
pub const FADF_EMBEDDED: ::DWORD = 0x4;
pub const FADF_FIXEDSIZE: ::DWORD = 0x10;
pub const FADF_RECORD: ::DWORD = 0x20;
pub const FADF_HAVEIID: ::DWORD = 0x40;
pub const FADF_HAVEVARTYPE: ::DWORD = 0x80;
pub const FADF_BSTR: ::DWORD = 0x100;
pub const FADF_UNKNOWN: ::DWORD = 0x200;
pub const FADF_DISPATCH: ::DWORD = 0x400;
pub const FADF_VARIANT: ::DWORD = 0x800;
pub const FADF_RESERVED: ::DWORD = 0xf008;
#[cfg(target_arch = "x86_64")]
STRUCT!{struct VARIANT {
    data0: u64,
    data1: u64,
    data2: u64,
}}
#[cfg(target_arch = "x86")]
STRUCT!{struct VARIANT {
    data0: u64,
    data1: u32,
    data2: u32,
}}
UNION!(VARIANT, data0, vt, vt_mut, ::VARTYPE);
UNION!(VARIANT, data1, llVal, llVal_mut, ::LONGLONG);
UNION!(VARIANT, data1, lVal, lVal_mut, ::LONG);
UNION!(VARIANT, data1, bVal, bVal_mut, ::BYTE);
UNION!(VARIANT, data1, iVal, iVal_mut, ::SHORT);
UNION!(VARIANT, data1, fltVal, fltVal_mut, ::FLOAT);
UNION!(VARIANT, data1, dblVal, dblVal_mut, ::DOUBLE);
UNION!(VARIANT, data1, boolVal, boolVal_mut, ::VARIANT_BOOL);
UNION!(VARIANT, data1, scode, scode_mut, ::SCODE);
UNION!(VARIANT, data1, cyVal, cyVal_mut, ::CY);
UNION!(VARIANT, data1, date, date_mut, ::DATE);
UNION!(VARIANT, data1, bstrVal, bstrVal_mut, ::BSTR);
UNION!(VARIANT, data1, punkVal, punkVal_mut, *mut ::IUnknown);
UNION!(VARIANT, data1, pdispVal, pdispVal_mut, *mut IDispatch);
UNION!(VARIANT, data1, parray, parray_mut, *mut SAFEARRAY);
UNION!(VARIANT, data1, pllVal, pllVal_mut, *mut ::LONGLONG);
UNION!(VARIANT, data1, plVal, plVal_mut, *mut ::LONG);
UNION!(VARIANT, data1, pbVal, pbVal_mut, *mut ::BYTE);
UNION!(VARIANT, data1, piVal, piVal_mut, *mut ::SHORT);
UNION!(VARIANT, data1, pfltVal, pfltVal_mut, *mut ::FLOAT);
UNION!(VARIANT, data1, pdblVal, pdblVal_mut, *mut ::DOUBLE);
UNION!(VARIANT, data1, pboolVal, pboolVal_mut, *mut ::VARIANT_BOOL);
UNION!(VARIANT, data1, pscode, pscode_mut, *mut ::SCODE);
UNION!(VARIANT, data1, pcyVal, pcyVal_mut, *mut ::CY);
UNION!(VARIANT, data1, pdate, pdate_mut, *mut ::DATE);
UNION!(VARIANT, data1, pbstrVal, pbstrVal_mut, *mut ::BSTR);
UNION!(VARIANT, data1, ppunkVal, ppunkVal_mut, *mut *mut ::IUnknown);
UNION!(VARIANT, data1, ppdispVal, ppdispVal_mut, *mut *mut IDispatch);
UNION!(VARIANT, data1, pparray, pparray_mut, *mut *mut SAFEARRAY);
UNION!(VARIANT, data1, pvarVal, pvarVal_mut, *mut VARIANT);
UNION!(VARIANT, data1, byref, byref_mut, ::PVOID);
UNION!(VARIANT, data1, cVal, cVal_mut, ::CHAR);
UNION!(VARIANT, data1, uiVal, uiVal_mut, ::USHORT);
UNION!(VARIANT, data1, ulVal, ulVal_mut, ::ULONG);
UNION!(VARIANT, data1, ullVal, ullVal_mut, ::ULONGLONG);
UNION!(VARIANT, data1, intVal, intVal_mut, ::INT);
UNION!(VARIANT, data1, uintVal, uintVal_mut, ::UINT);
UNION!(VARIANT, data1, pdecVal, pdecVal_mut, *mut ::DECIMAL);
UNION!(VARIANT, data1, pcVal, pcVal_mut, *mut ::CHAR);
UNION!(VARIANT, data1, puiVal, puiVal_mut, *mut ::USHORT);
UNION!(VARIANT, data1, pulVal, pulVal_mut, *mut ::ULONG);
UNION!(VARIANT, data1, pullVal, pullVal_mut, *mut ::ULONGLONG);
UNION!(VARIANT, data1, pintVal, pintVal_mut, *mut ::INT);
UNION!(VARIANT, data1, puintVal, puintVal_mut, *mut ::UINT);
UNION!(VARIANT, data1, pvRecord, pvRecord_mut, ::PVOID);
UNION!(VARIANT, data2, pRecInfo, pRecInfo_mut, *mut IRecordInfo);
UNION!(VARIANT, data0, decVal, decVal_mut, ::DECIMAL);
pub type LPVARIANT = *mut VARIANT;
pub type VARIANTARG = VARIANT;
pub type LPVARIANTARG = *mut VARIANT;
pub type REFVARIANT = *const VARIANT;
STRUCT!{struct _wireBRECORD {
    fFlags: ::ULONG,
    clSize: ::ULONG,
    pRecInfo: *mut IRecordInfo,
    pRecord: *mut ::BYTE,
}}
STRUCT!{struct _wireVARIANT {
    clSize: ::DWORD,
    rpcReserved: ::DWORD,
    vt: ::USHORT,
    wReserved1: ::USHORT,
    wReserved2: ::USHORT,
    wReserved3: ::USHORT,
    data0: u64,
    data1: u64,
}}
UNION!(_wireVARIANT, data0, llVal, llVal_mut, ::LONGLONG);
UNION!(_wireVARIANT, data0, lVal, lVal_mut, ::LONG);
UNION!(_wireVARIANT, data0, bVal, bVal_mut, ::BYTE);
UNION!(_wireVARIANT, data0, iVal, iVal_mut, ::SHORT);
UNION!(_wireVARIANT, data0, fltVal, fltVal_mut, ::FLOAT);
UNION!(_wireVARIANT, data0, dblVal, dblVal_mut, ::DOUBLE);
UNION!(_wireVARIANT, data0, boolVal, boolVal_mut, ::VARIANT_BOOL);
UNION!(_wireVARIANT, data0, scode, scode_mut, ::SCODE);
UNION!(_wireVARIANT, data0, cyVal, cyVal_mut, ::CY);
UNION!(_wireVARIANT, data0, date, date_mut, ::DATE);
UNION!(_wireVARIANT, data0, bstrVal, bstrVal_mut, ::wireBSTR);
UNION!(_wireVARIANT, data0, punkVal, punkVal_mut, *mut ::IUnknown);
UNION!(_wireVARIANT, data0, pdispVal, pdispVal_mut, *mut IDispatch);
UNION!(_wireVARIANT, data0, parray, parray_mut, wirePSAFEARRAY);
UNION!(_wireVARIANT, data0, brecVal, brecVal_mut, wireBRECORD);
UNION!(_wireVARIANT, data0, pllVal, pllVal_mut, *mut ::LONGLONG);
UNION!(_wireVARIANT, data0, plVal, plVal_mut, *mut ::LONG);
UNION!(_wireVARIANT, data0, pbVal, pbVal_mut, *mut ::BYTE);
UNION!(_wireVARIANT, data0, piVal, piVal_mut, *mut ::SHORT);
UNION!(_wireVARIANT, data0, pfltVal, pfltVal_mut, *mut ::FLOAT);
UNION!(_wireVARIANT, data0, pdblVal, pdblVal_mut, *mut ::DOUBLE);
UNION!(_wireVARIANT, data0, pboolVal, pboolVal_mut, *mut ::VARIANT_BOOL);
UNION!(_wireVARIANT, data0, pscode, pscode_mut, *mut ::SCODE);
UNION!(_wireVARIANT, data0, pcyVal, pcyVal_mut, *mut ::CY);
UNION!(_wireVARIANT, data0, pdate, pdate_mut, *mut ::DATE);
UNION!(_wireVARIANT, data0, pbstrVal, pbstrVal_mut, *mut ::wireBSTR);
UNION!(_wireVARIANT, data0, ppunkVal, ppunkVal_mut, *mut *mut ::IUnknown);
UNION!(_wireVARIANT, data0, ppdispVal, ppdispVal_mut, *mut *mut IDispatch);
UNION!(_wireVARIANT, data0, pparray, pparray_mut, *mut wirePSAFEARRAY);
UNION!(_wireVARIANT, data0, pvarVal, pvarVal_mut, *mut wireVARIANT);
UNION!(_wireVARIANT, data0, cVal, cVal_mut, ::CHAR);
UNION!(_wireVARIANT, data0, uiVal, uiVal_mut, ::USHORT);
UNION!(_wireVARIANT, data0, ulVal, ulVal_mut, ::ULONG);
UNION!(_wireVARIANT, data0, ullVal, ullVal_mut, ::ULONGLONG);
UNION!(_wireVARIANT, data0, intVal, intVal_mut, ::INT);
UNION!(_wireVARIANT, data0, uintVal, uintVal_mut, ::UINT);
UNION!(_wireVARIANT, data0, decVal, decVal_mut, ::DECIMAL);
UNION!(_wireVARIANT, data0, pcVal, pcVal_mut, *mut ::CHAR);
UNION!(_wireVARIANT, data0, puiVal, puiVal_mut, *mut ::USHORT);
UNION!(_wireVARIANT, data0, pulVal, pulVal_mut, *mut ::ULONG);
UNION!(_wireVARIANT, data0, pullVal, pullVal_mut, *mut ::ULONGLONG);
UNION!(_wireVARIANT, data0, pintVal, pintVal_mut, *mut ::INT);
UNION!(_wireVARIANT, data0, puintVal, puintVal_mut, *mut ::UINT);
UNION!(_wireVARIANT, data0, pdecVal, pdecVal_mut, *mut ::DECIMAL);
pub type DISPID = ::LONG;
pub type MEMBERID = DISPID;
pub type HREFTYPE = ::DWORD;
ENUM!{enum TYPEKIND {
    TKIND_ENUM = 0,
    TKIND_RECORD,
    TKIND_MODULE,
    TKIND_INTERFACE,
    TKIND_DISPATCH,
    TKIND_COCLASS,
    TKIND_ALIAS,
    TKIND_UNION,
    TKIND_MAX,
}}
#[cfg(target_arch = "x86_64")]
STRUCT!{struct TYPEDESC {
    data: u64,
    vt: ::VARTYPE,
}}
#[cfg(target_arch = "x86")]
STRUCT!{struct TYPEDESC {
    data: u32,
    vt: ::VARTYPE,
}}
UNION!(TYPEDESC, data, lptdesc, lptdesc_mut, *mut TYPEDESC);
UNION!(TYPEDESC, data, lpadesc, lpadesc_mut, *mut ARRAYDESC);
UNION!(TYPEDESC, data, hreftype, hreftype_mut, HREFTYPE);
STRUCT!{struct ARRAYDESC {
    tdescElem: TYPEDESC,
    cDims: ::USHORT,
    rgbounds: [SAFEARRAYBOUND; 1],
}}
STRUCT!{struct PARAMDESCEX {
    cBytes: ::ULONG,
    varDefaultValue: VARIANTARG,
}}
pub type LPPARAMDESCEX = *mut PARAMDESCEX;
STRUCT!{struct PARAMDESC {
    pparamdescex: LPPARAMDESCEX,
    wParamFlags: ::USHORT,
}}
pub type LPPARAMDESC = *mut PARAMDESC;
pub const PARAMFLAG_NONE: ::DWORD = 0;
pub const PARAMFLAG_FIN: ::DWORD = 0x1;
pub const PARAMFLAG_FOUT: ::DWORD = 0x2;
pub const PARAMFLAG_FLCID: ::DWORD = 0x4;
pub const PARAMFLAG_FRETVAL: ::DWORD = 0x8;
pub const PARAMFLAG_FOPT: ::DWORD = 0x10;
pub const PARAMFLAG_FHASDEFAULT: ::DWORD = 0x20;
pub const PARAMFLAG_FHASCUSTDATA: ::DWORD = 0x40;
STRUCT!{struct IDLDESC {
    dwReserved: ::ULONG_PTR,
    wIDLFlags: ::USHORT,
}}
pub type LPIDLDESC = *mut IDLDESC;
pub const IDLFLAG_NONE: ::DWORD = PARAMFLAG_NONE;
pub const IDLFLAG_FIN: ::DWORD = PARAMFLAG_FIN;
pub const IDLFLAG_FOUT: ::DWORD = PARAMFLAG_FOUT;
pub const IDLFLAG_FLCID: ::DWORD = PARAMFLAG_FLCID;
pub const IDLFLAG_FRETVAL: ::DWORD = PARAMFLAG_FRETVAL;
STRUCT!{struct ELEMDESC {
    tdesc: TYPEDESC,
    idldesc: IDLDESC,
}}
UNION!(ELEMDESC, idldesc, paramdesc, paramdesc_mut, PARAMDESC);
pub type LPELEMDESC = *mut ELEMDESC;
STRUCT!{struct TYPEATTR {
    guid: ::GUID,
    lcid: ::LCID,
    dwReserved: ::DWORD,
    memidConstructor: ::MEMBERID,
    memidDestructor: ::MEMBERID,
    lpstrSchema: ::LPOLESTR,
    cbSizeInstance: ::ULONG,
    typekind: ::TYPEKIND,
    cFuncs: ::WORD,
    cVars: ::WORD,
    cImplTypes: ::WORD,
    cbSizeVft: ::WORD,
    cbAlignment: ::WORD,
    wTypeFlags: ::WORD,
    wMajorVerNum: ::WORD,
    wMinorVerNum: ::WORD,
    tdescAlias: ::TYPEDESC,
    idldescType: ::IDLDESC,
}}
pub type LPTYPEATTR = *mut TYPEATTR;
STRUCT!{struct DISPPARAMS {
    rgvarg: *mut VARIANTARG,
    rgdispidNamedArgs: *mut DISPID,
    cArgs: ::UINT,
    cNamedArgs: ::UINT,
}}
STRUCT!{nodebug struct EXCEPINFO {
    wCode: ::WORD,
    wReserved: ::WORD,
    bstrSource: ::BSTR,
    bstrDescription: ::BSTR,
    bstrHelpFile: ::BSTR,
    dwHelpContext: ::DWORD,
    pvReserved: ::PVOID,
    pfnDeferredFillIn: Option<unsafe extern "system" fn(einfo: *mut EXCEPINFO) -> ::HRESULT>,
    scode: ::SCODE,
}}
ENUM!{enum CALLCONV {
    CC_FASTCALL = 0,
    CC_CDECL = 1,
    CC_MSCPASCAL,
    CC_PASCAL,
    CC_MACPASCAL,
    CC_STDCALL,
    CC_FPFASTCALL,
    CC_SYSCALL,
    CC_MPWCDECL,
    CC_MPWPASCAL,
    CC_MAX,
}}
ENUM!{enum FUNCKIND {
    FUNC_VIRTUAL = 0,
    FUNC_PUREVIRTUAL,
    FUNC_NONVIRTUAL,
    FUNC_STATIC,
    FUNC_DISPATCH,
}}
FLAGS!{enum INVOKEKIND {
    INVOKE_FUNC = 1,
    INVOKE_PROPERTYGET = 2,
    INVOKE_PROPERTYPUT = 4,
    INVOKE_PROPERTYPUTREF = 8,
}}
STRUCT!{struct FUNCDESC {
    memid: ::MEMBERID,
    lprgscode: *mut ::SCODE,
    lprgelemdescParam: *mut ::ELEMDESC,
    funckind: ::FUNCKIND,
    invkind: ::INVOKEKIND,
    callconv: ::CALLCONV,
    cParams: ::SHORT,
    cParamsOpt: ::SHORT,
    oVft: ::SHORT,
    cScodes: ::SHORT,
    elemdescFunc: ::ELEMDESC,
    wFuncFlags: ::WORD,
}}
pub type LPFUNCDESC = *mut FUNCDESC;
ENUM!{enum VARKIND {
    VAR_PERINSTANCE = 0,
    VAR_STATIC,
    VAR_CONST,
    VAR_DISPATCH,
}}
pub const IMPLTYPEFLAG_FDEFAULT: ::DWORD = 0x1;
pub const IMPLTYPEFLAG_FSOURCE: ::DWORD = 0x2;
pub const IMPLTYPEFLAG_FRESTRICTED: ::DWORD = 0x4;
pub const IMPLTYPEFLAG_FDEFAULTVTABLE: ::DWORD = 0x8;
STRUCT!{struct VARDESC {
    memid: MEMBERID,
    lpstrSchema: ::LPOLESTR,
    lpvarValue: *mut VARIANT,
    elemdescVar: ::ELEMDESC,
    wVarFlags: ::WORD,
    varkind: VARKIND,
}}
UNION!(VARDESC, lpvarValue, oInst, oInst_mut, ::ULONG);
pub type LPVARDESC = *mut VARDESC;
FLAGS!{enum TYPEFLAGS {
    TYPEFLAG_FAPPOBJECT = 0x1,
    TYPEFLAG_FCANCREATE = 0x2,
    TYPEFLAG_FLICENSED  = 0x4,
    TYPEFLAG_FPREDECLID = 0x8,
    TYPEFLAG_FHIDDEN = 0x10,
    TYPEFLAG_FCONTROL = 0x20,
    TYPEFLAG_FDUAL = 0x40,
    TYPEFLAG_FNONEXTENSIBLE = 0x80,
    TYPEFLAG_FOLEAUTOMATION = 0x100,
    TYPEFLAG_FRESTRICTED = 0x200,
    TYPEFLAG_FAGGREGATABLE = 0x400,
    TYPEFLAG_FREPLACEABLE = 0x800,
    TYPEFLAG_FDISPATCHABLE = 0x1000,
    TYPEFLAG_FREVERSEBIND = 0x2000,
    TYPEFLAG_FPROXY = 0x4000,
}}
FLAGS!{enum FUNCFLAGS {
    FUNCFLAG_FRESTRICTED = 0x1,
    FUNCFLAG_FSOURCE = 0x2,
    FUNCFLAG_FBINDABLE = 0x4,
    FUNCFLAG_FREQUESTEDIT = 0x8,
    FUNCFLAG_FDISPLAYBIND = 0x10,
    FUNCFLAG_FDEFAULTBIND = 0x20,
    FUNCFLAG_FHIDDEN = 0x40,
    FUNCFLAG_FUSESGETLASTERROR = 0x80,
    FUNCFLAG_FDEFAULTCOLLELEM = 0x100,
    FUNCFLAG_FUIDEFAULT = 0x200,
    FUNCFLAG_FNONBROWSABLE = 0x400,
    FUNCFLAG_FREPLACEABLE = 0x800,
    FUNCFLAG_FIMMEDIATEBIND = 0x1000,
}}
FLAGS!{enum VARFLAGS {
    VARFLAG_FREADONLY = 0x1,
    VARFLAG_FSOURCE = 0x2,
    VARFLAG_FBINDABLE = 0x4,
    VARFLAG_FREQUESTEDIT = 0x8,
    VARFLAG_FDISPLAYBIND = 0x10,
    VARFLAG_FDEFAULTBIND = 0x20,
    VARFLAG_FHIDDEN = 0x40,
    VARFLAG_FRESTRICTED = 0x80,
    VARFLAG_FDEFAULTCOLLELEM = 0x100,
    VARFLAG_FUIDEFAULT = 0x200,
    VARFLAG_FNONBROWSABLE = 0x400,
    VARFLAG_FREPLACEABLE = 0x800,
    VARFLAG_FIMMEDIATEBIND = 0x1000,
}}
STRUCT!{struct CLEANLOCALSTORAGE {
    pInterface: *mut ::IUnknown,
    pStorage: ::PVOID,
    flags: ::DWORD,
}}
STRUCT!{struct CUSTDATAITEM {
    guid: ::GUID,
    varValue: VARIANTARG,
}}
pub type LPCUSTDATAITEM = *mut CUSTDATAITEM;
STRUCT!{struct CUSTDATA {
    cCustData: ::DWORD,
    prgCustData: LPCUSTDATAITEM,
}}
pub type LPCUSTDATA = *mut CUSTDATA;
pub type LPCREATETYPEINFO = *mut ICreateTypeInfo;
RIDL!(
interface ICreateTypeInfo(ICreateTypeInfoVtbl): IUnknown(IUnknownVtbl) {
    fn SetGuid(&mut self, guid: ::REFGUID) -> ::HRESULT,
    fn SetTypeFlags(&mut self, uTypeFlags: ::UINT) -> ::HRESULT,
    fn SetDocString(&mut self, pStrDoc: ::LPOLESTR) -> ::HRESULT,
    fn SetHelpContext(&mut self, dwHelpContext: ::DWORD) -> ::HRESULT,
    fn SetVersion(&mut self, wMajorVerNum: ::WORD, wMinorVerNum: ::WORD) -> ::HRESULT,
    fn AddRefTypeInfo(&mut self, pTInfo: *mut ITypeInfo) -> ::HRESULT,
    fn AddFuncDesc(&mut self, index: ::UINT, pFuncDesc: *mut FUNCDESC) -> ::HRESULT,
    fn SetImplTypeFlags(&mut self, index: ::UINT, implTypeFlags: ::INT) -> ::HRESULT,
    fn SetAlignment(&mut self, cbAlignment: ::WORD) -> ::HRESULT,
    fn SetSchema(&mut self, pStrSchema: ::LPOLESTR) -> ::HRESULT,
    fn AddVarDesc(&mut self, index: ::UINT, pVarDesc: *mut VARDESC) -> ::HRESULT,
    fn SetFuncAndParamNames(
        &mut self, index: ::UINT, rgszNames: *mut ::LPOLESTR, cNames: ::UINT
    ) -> ::HRESULT,
    fn SetVarName(&mut self, index: ::UINT, szName: ::LPOLESTR) -> ::HRESULT,
    fn SetTypeDescAlias(&mut self, pTDescAlias: *mut TYPEDESC) -> ::HRESULT,
    fn DefineFuncAsDllEntry(
        &mut self, index: ::UINT, szDllName: ::LPOLESTR, szProcName: ::LPOLESTR
    ) -> ::HRESULT,
    fn SetFuncDocString(&mut self, index: ::UINT, szDocString: ::LPOLESTR) -> ::HRESULT,
    fn SetVarDocString(&mut self, index: ::UINT, szDocString: ::LPOLESTR) -> ::HRESULT,
    fn SetFuncHelpContext(&mut self, index: ::UINT, dwHelpContext: ::DWORD) -> ::HRESULT,
    fn SetVarHelpContext(&mut self, index: ::UINT, dwHelpContext: ::DWORD) -> ::HRESULT,
    fn SetMops(&mut self, index: ::UINT, bstrMops: ::BSTR) -> ::HRESULT,
    fn SetTypeIdldesc(&mut self, pIdlDesc: *mut IDLDESC) -> ::HRESULT,
    fn LayOut(&mut self) -> ::HRESULT
}
);
// FIXME: Implement these interfaces
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ICreateTypeInfo2;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ICreateTypeLib;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ICreateTypeLib2;
pub type LPDISPATCH = *mut IDispatch;
pub const DISPID_UNKNOWN: ::INT = -1;
pub const DISPID_VALUE: ::INT = 0;
pub const DISPID_PROPERTYPUT: ::INT = -3;
pub const DISPID_NEWENUM: ::INT = -4;
pub const DISPID_EVALUATE: ::INT = -5;
pub const DISPID_CONSTRUCTOR: ::INT = -6;
pub const DISPID_DESTRUCTOR: ::INT = -7;
pub const DISPID_COLLECT: ::INT = -8;
RIDL!(
interface IDispatch(IDispatchVtbl): IUnknown(IUnknownVtbl) {
    fn GetTypeInfoCount(&mut self, pctinfo: *mut ::UINT) -> ::HRESULT,
    fn GetTypeInfo(
        &mut self, iTInfo: ::UINT, lcid: ::LCID, ppTInfo: *mut *mut ITypeInfo
    ) -> ::HRESULT,
    fn GetIDsOfNames(
        &mut self, riid: ::REFIID, rgszNames: *mut ::LPOLESTR, cNames: ::UINT, lcid: ::LCID,
        rgDispId: *mut ::DISPID
    ) -> ::HRESULT,
    fn Invoke(
        &mut self, dispIdMember: ::DISPID, riid: ::REFIID, lcid: ::LCID, wFlags: ::WORD,
        pDispParams: *mut ::DISPPARAMS, pVarResult: *mut VARIANT, pExcepInfo: *mut ::EXCEPINFO,
        puArgErr: *mut ::UINT
    ) -> ::HRESULT
}
);
// FIXME: Implement these interfaces
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct IEnumVARIANT;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeComp;
RIDL!(
interface ITypeInfo(ITypeInfoVtbl): IUnknown(IUnknownVtbl) {
    fn GetTypeAttr(&mut self, ppTypeAttr: *mut *mut TYPEATTR) -> ::HRESULT,
    fn GetTypeComp(&mut self, ppTComp: *mut *mut ITypeComp) -> ::HRESULT,
    fn GetFuncDesc(&mut self, index: ::UINT, ppFunDesc: *mut *mut FUNCDESC) -> ::HRESULT,
    fn GetVarDesc(&mut self, index: ::UINT, pPVarDesc: *mut *mut VARDESC) -> ::HRESULT,
    fn GetNames(
        &mut self, memid: MEMBERID, rgBstrNames: *mut ::BSTR, cMaxNames: ::UINT,
        pcNames: *mut ::UINT
    ) -> ::HRESULT,
    fn GetRefTypeOfImplType(&mut self, index: ::UINT, pRefType: *mut HREFTYPE) -> ::HRESULT,
    fn GetImplTypeFlags(&mut self, index: ::UINT, pImplTypeFlags: *mut ::INT) -> ::HRESULT,
    fn GetIDsOfNames(
        &mut self, rgszNames: *mut ::LPOLESTR, cNames: ::UINT, pMemId: *mut MEMBERID
    ) -> ::HRESULT,
    fn Invoke(
        &mut self, pvInstance: ::PVOID, memid: MEMBERID, wFlags: ::WORD,
        pDispParams: *mut DISPPARAMS, pVarResult: *mut VARIANT, pExcepInfo: *mut EXCEPINFO,
        puArgErr: *mut ::UINT
    ) -> ::HRESULT,
    fn GetDocumentation(
        &mut self, memid: MEMBERID, pBstrName: *mut ::BSTR, pBstrDocString: *mut ::BSTR,
        pdwHelpContext: *mut ::DWORD, pBstrHelpFile: *mut ::BSTR
    ) -> ::HRESULT,
    fn GetDllEntry(
        &mut self, memid: MEMBERID, invKind: ::INVOKEKIND, pBstrDllName: *mut ::BSTR,
        pBstrName: *mut ::BSTR, pwOrdinal: *mut ::WORD
    ) -> ::HRESULT,
    fn GetRefTypeInfo(&mut self, hRefType: HREFTYPE, ppTInfo: *mut *mut ITypeInfo) -> ::HRESULT,
    fn AddressOfMember(
        &mut self, memid: MEMBERID, invKind: ::INVOKEKIND, ppv: *mut ::PVOID
    ) -> ::HRESULT,
    fn CreateInstance(
        &mut self, pUnkOuter: *mut ::IUnknown, riid: ::REFIID, ppvObj: *mut ::PVOID
    ) -> ::HRESULT,
    fn GetMops(&mut self, memid: MEMBERID, pBstrMops: *mut ::BSTR) -> ::HRESULT,
    fn GetContainingTypeLib(
        &mut self, ppTLib: *mut *mut ITypeLib, pIndex: *mut ::UINT
    ) -> ::HRESULT,
    fn ReleaseTypeAttr(&mut self, pTypeAttr: *mut TYPEATTR) -> (),
    fn ReleaseFuncDesc(&mut self, pFuncDesc: *mut FUNCDESC) -> (),
    fn ReleaseVarDesc(&mut self, pVarDesc: *mut VARDESC) -> ()
}
);
// FIXME: Implement these interfaces
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeInfo2;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeLib;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeLib2;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeChangeEvents;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct IErrorInfo;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ICreateErrorInfo;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ISupportErrorInfo;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeFactory;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct ITypeMarshal;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct IRecordInfo;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct IErrorLog;
#[repr(C)] #[derive(Clone, Copy, Debug)]
pub struct IPropertyBag;
